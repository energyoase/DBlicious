//! `ForeignSqliteSource` — liest/schreibt eine fremde SQLite-DB. Legt
//! KEIN eigenes Schema an; introspiziert beim `init`.

use async_trait::async_trait;

use shared::source::{EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::sql::{build_filter_sql, encode_pk_from_row, json_to_sea_value, quote_ident};
use super::{Capabilities, PageQuery, Source, SourceError};

pub struct ForeignSqliteSource {
    name: String,
    url: String,
    conn: Option<sea_orm::DatabaseConnection>,
    introspected: super::introspect::Schema,
}

impl ForeignSqliteSource {
    pub fn new(name: String, url: String) -> Self {
        Self {
            name,
            url,
            conn: None,
            introspected: super::introspect::Schema::default(),
        }
    }
}

#[async_trait]
impl Source for ForeignSqliteSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> &'static str {
        "foreign-sqlite"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: true,
            supports_composite_pk: true,
            supports_ddl: false, // Foreign-DBs verwalten ihr Schema selbst.
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        let mut opts = sea_orm::ConnectOptions::new(self.url.clone());
        opts.sqlx_logging(false);
        let conn = sea_orm::Database::connect(opts).await?;
        self.introspected = super::introspect::read_schema(&conn).await?;
        self.conn = Some(conn);
        Ok(())
    }

    async fn list_page(
        &self,
        binding: &EntityBinding,
        query: &PageQuery,
    ) -> Result<EntityPage, SourceError> {
        let table = match &binding.locator {
            shared::source::BindingLocator::Table { table } => table.clone(),
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| SourceError::Other("init not called".into()))?;
        let table_meta = self
            .introspected
            .tables
            .get(&table)
            .ok_or_else(|| SourceError::Other(format!("table not found: {table}")))?;

        // SELECT columns: when column_map is empty -> all table columns;
        // otherwise the mapped DB columns plus the primary-key columns.
        let select_cols: Vec<String> = if binding.column_map.is_empty() {
            table_meta.columns.iter().map(|c| c.name.clone()).collect()
        } else {
            let mut s: std::collections::BTreeSet<String> =
                binding.column_map.values().cloned().collect();
            for pk in &table_meta.primary_key {
                s.insert(pk.clone());
            }
            s.into_iter().collect()
        };

        let cols_sql = select_cols
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let table_sql = quote_ident(&table);

        let page = query.page.max(1);
        let page_size = query.page_size.max(1);
        let offset = (page - 1) as i64 * page_size as i64;

        // ORDER BY: only single-column sort, with logical-name -> DB-name mapping.
        let order_sql = match &query.sort {
            Some(s) => {
                let db_col = binding
                    .column_map
                    .get(&s.field)
                    .cloned()
                    .unwrap_or_else(|| s.field.clone());
                let dir = match s.direction {
                    shared::SortDirection::Asc => "ASC",
                    shared::SortDirection::Desc => "DESC",
                };
                format!(" ORDER BY {} {}", quote_ident(&db_col), dir)
            }
            None => String::new(),
        };

        let (where_sql, where_values) = build_filter_sql(&query.filter, binding);

        let sql_list = format!(
            "SELECT {cols_sql} FROM {table_sql}{where_sql}{order_sql} LIMIT {page_size} OFFSET {offset}"
        );
        let sql_count = format!("SELECT COUNT(*) AS c FROM {table_sql}{where_sql}");

        use sea_orm::{DbBackend, FromQueryResult, JsonValue};

        let stmt_list = sea_orm::Statement::from_sql_and_values(
            DbBackend::Sqlite,
            sql_list,
            where_values.clone(),
        );
        let rows_json: Vec<serde_json::Value> =
            JsonValue::find_by_statement(stmt_list).all(conn).await?;

        #[derive(FromQueryResult)]
        struct CountRow {
            c: i64,
        }
        let stmt_count =
            sea_orm::Statement::from_sql_and_values(DbBackend::Sqlite, sql_count, where_values);
        let count_row: CountRow = CountRow::find_by_statement(stmt_count)
            .one(conn)
            .await?
            .ok_or_else(|| SourceError::Other("count returned no row".into()))?;

        // Map DB column names back to logical keys (reverse of column_map).
        let reverse_map: std::collections::BTreeMap<String, String> = binding
            .column_map
            .iter()
            .map(|(k, v)| (v.clone(), k.clone()))
            .collect();

        let items: Vec<shared::Entity> = rows_json
            .into_iter()
            .map(|row| {
                let obj = row.as_object().cloned().unwrap_or_default();
                let mut fields = serde_json::Map::new();
                for (db_col, val) in obj {
                    let logical = reverse_map.get(&db_col).cloned().unwrap_or(db_col);
                    fields.insert(logical, val);
                }
                let id = encode_pk_from_row(&fields, &binding.primary_key);
                shared::Entity { id, fields }
            })
            .collect();

        Ok(EntityPage {
            items,
            total_count: count_row.c as u64,
            page: page as u32,
            page_size: page_size as u32,
            reference_labels: Default::default(),
        })
    }
    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        let table = match &binding.locator {
            shared::source::BindingLocator::Table { table } => table.clone(),
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| SourceError::Other("init not called".into()))?;
        let table_meta = self
            .introspected
            .tables
            .get(&table)
            .ok_or_else(|| SourceError::Other(format!("table not found: {table}")))?;

        // PK column values, matching the binding's primary_key column order.
        let pk_logical = &binding.primary_key;
        let pk_values: Vec<String> = match id {
            EntityId::Single(s) => {
                if pk_logical.len() != 1 {
                    return Err(SourceError::Other(format!(
                        "single-PK id but binding has {} PK columns",
                        pk_logical.len()
                    )));
                }
                vec![s.clone()]
            }
            EntityId::Composite(parts) => {
                if parts.len() != pk_logical.len() {
                    return Err(SourceError::Other(format!(
                        "composite-PK id has {} parts, binding has {} columns",
                        parts.len(),
                        pk_logical.len()
                    )));
                }
                parts.clone()
            }
        };

        // WHERE: AND of (pk_db_col = ?) — values bound, not interpolated.
        let where_clauses: Vec<String> = pk_logical
            .iter()
            .map(|logical| {
                let db_col = binding
                    .column_map
                    .get(logical)
                    .cloned()
                    .unwrap_or_else(|| logical.clone());
                format!("{} = ?", quote_ident(&db_col))
            })
            .collect();
        let where_sql = format!(" WHERE {}", where_clauses.join(" AND "));

        let select_cols: Vec<String> = if binding.column_map.is_empty() {
            table_meta.columns.iter().map(|c| c.name.clone()).collect()
        } else {
            let mut s: std::collections::BTreeSet<String> =
                binding.column_map.values().cloned().collect();
            for pk in &table_meta.primary_key {
                s.insert(pk.clone());
            }
            s.into_iter().collect()
        };

        let cols_sql = select_cols
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {cols_sql} FROM {} {} LIMIT 1",
            quote_ident(&table),
            where_sql
        );

        use sea_orm::{DbBackend, FromQueryResult, JsonValue, Statement, Value};
        let values: Vec<Value> = pk_values.into_iter().map(Value::from).collect();
        let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, sql, values);

        let row_opt: Option<serde_json::Value> =
            JsonValue::find_by_statement(stmt).one(conn).await?;
        let Some(row) = row_opt else { return Ok(None) };

        let reverse_map: std::collections::BTreeMap<String, String> = binding
            .column_map
            .iter()
            .map(|(k, v)| (v.clone(), k.clone()))
            .collect();
        let obj = row.as_object().cloned().unwrap_or_default();
        let mut fields = serde_json::Map::new();
        for (db_col, val) in obj {
            let logical = reverse_map.get(&db_col).cloned().unwrap_or(db_col);
            fields.insert(logical, val);
        }
        let id_string = encode_pk_from_row(&fields, &binding.primary_key);
        Ok(Some(shared::Entity {
            id: id_string,
            fields,
        }))
    }
    async fn create(
        &self,
        binding: &EntityBinding,
        _id: Option<String>,
        fields: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = match &binding.locator {
            shared::source::BindingLocator::Table { table } => table.clone(),
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| SourceError::Other("init not called".into()))?;

        // logical column name -> DB name
        let resolve = |logical: &str| {
            binding
                .column_map
                .get(logical)
                .cloned()
                .unwrap_or_else(|| logical.to_string())
        };

        let mut cols: Vec<String> = Vec::new();
        let mut placeholders: Vec<String> = Vec::new();
        let mut values: Vec<sea_orm::Value> = Vec::new();
        for (logical, val) in &fields {
            cols.push(quote_ident(&resolve(logical)));
            placeholders.push("?".into());
            values.push(json_to_sea_value(val));
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_ident(&table),
            cols.join(", "),
            placeholders.join(", "),
        );
        let stmt = sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Sqlite, sql, values);
        use sea_orm::ConnectionTrait;
        conn.execute(stmt).await?;

        let id_string = encode_pk_from_row(&fields, &binding.primary_key);
        Ok(shared::Entity {
            id: id_string,
            fields,
        })
    }

    async fn update(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
        patch: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Option<Entity>, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = match &binding.locator {
            shared::source::BindingLocator::Table { table } => table.clone(),
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| SourceError::Other("init not called".into()))?;

        let resolve = |logical: &str| {
            binding
                .column_map
                .get(logical)
                .cloned()
                .unwrap_or_else(|| logical.to_string())
        };

        if patch.is_empty() {
            return self.get(binding, id).await;
        }

        let mut set_parts: Vec<String> = Vec::new();
        let mut values: Vec<sea_orm::Value> = Vec::new();
        for (logical, val) in &patch {
            set_parts.push(format!("{} = ?", quote_ident(&resolve(logical))));
            values.push(json_to_sea_value(val));
        }

        let pk_logical = &binding.primary_key;
        let pk_values: Vec<String> = match id {
            EntityId::Single(s) => vec![s.clone()],
            EntityId::Composite(parts) => parts.clone(),
        };
        let where_clauses: Vec<String> = pk_logical
            .iter()
            .map(|logical| format!("{} = ?", quote_ident(&resolve(logical))))
            .collect();
        for pv in &pk_values {
            values.push(sea_orm::Value::from(pv.clone()));
        }

        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            quote_ident(&table),
            set_parts.join(", "),
            where_clauses.join(" AND "),
        );
        let stmt = sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Sqlite, sql, values);
        use sea_orm::ConnectionTrait;
        conn.execute(stmt).await?;

        self.get(binding, id).await
    }

    async fn delete(&self, binding: &EntityBinding, id: &EntityId) -> Result<bool, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = match &binding.locator {
            shared::source::BindingLocator::Table { table } => table.clone(),
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| SourceError::Other("init not called".into()))?;
        let resolve = |logical: &str| {
            binding
                .column_map
                .get(logical)
                .cloned()
                .unwrap_or_else(|| logical.to_string())
        };

        let pk_logical = &binding.primary_key;
        let pk_values: Vec<String> = match id {
            EntityId::Single(s) => vec![s.clone()],
            EntityId::Composite(parts) => parts.clone(),
        };
        let where_clauses: Vec<String> = pk_logical
            .iter()
            .map(|logical| format!("{} = ?", quote_ident(&resolve(logical))))
            .collect();
        let mut values: Vec<sea_orm::Value> = Vec::new();
        for pv in &pk_values {
            values.push(sea_orm::Value::from(pv.clone()));
        }

        let sql = format!(
            "DELETE FROM {} WHERE {}",
            quote_ident(&table),
            where_clauses.join(" AND "),
        );
        let stmt = sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Sqlite, sql, values);
        use sea_orm::ConnectionTrait;
        let res = conn.execute(stmt).await?;
        Ok(res.rows_affected() > 0)
    }
}
