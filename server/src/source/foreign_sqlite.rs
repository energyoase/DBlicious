//! `ForeignSqliteSource` — liest/schreibt eine fremde SQLite-DB. Legt
//! KEIN eigenes Schema an; introspiziert beim `init`.

use async_trait::async_trait;

use shared::source::{EntityBinding, EntityId};
use shared::{Entity, EntityPage};

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
    fn name(&self) -> &str { &self.name }
    fn kind(&self) -> &'static str { "foreign-sqlite" }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: true,
            supports_composite_pk: true,
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
        let conn = self.conn.as_ref().ok_or_else(|| SourceError::Other("init not called".into()))?;
        let table_meta = self.introspected.tables.get(&table)
            .ok_or_else(|| SourceError::Other(format!("table not found: {table}")))?;

        // SELECT columns: when column_map is empty -> all table columns;
        // otherwise the mapped DB columns plus the primary-key columns.
        let select_cols: Vec<String> = if binding.column_map.is_empty() {
            table_meta.columns.iter().map(|c| c.name.clone()).collect()
        } else {
            let mut s: std::collections::BTreeSet<String> = binding.column_map.values().cloned().collect();
            for pk in &table_meta.primary_key { s.insert(pk.clone()); }
            s.into_iter().collect()
        };

        let cols_sql = select_cols.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
        let table_sql = quote_ident(&table);

        let page = query.page.max(1);
        let page_size = query.page_size.max(1);
        let offset = (page - 1) as i64 * page_size as i64;

        // ORDER BY: only single-column sort, with logical-name -> DB-name mapping.
        let order_sql = match &query.sort {
            Some(s) => {
                let db_col = binding.column_map.get(&s.field).cloned().unwrap_or_else(|| s.field.clone());
                let dir = match s.direction {
                    shared::SortDirection::Asc => "ASC",
                    shared::SortDirection::Desc => "DESC",
                };
                format!(" ORDER BY {} {}", quote_ident(&db_col), dir)
            }
            None => String::new(),
        };

        // Filter-Pushdown comes in Task 19 — for now: empty WHERE.
        let where_sql = String::new();

        let sql_list = format!(
            "SELECT {cols_sql} FROM {table_sql}{where_sql}{order_sql} LIMIT {page_size} OFFSET {offset}"
        );
        let sql_count = format!("SELECT COUNT(*) AS c FROM {table_sql}{where_sql}");

        use sea_orm::{DbBackend, FromQueryResult, Statement, JsonValue};

        let rows_json: Vec<serde_json::Value> = JsonValue::find_by_statement(
            Statement::from_string(DbBackend::Sqlite, sql_list)
        ).all(conn).await?;

        #[derive(FromQueryResult)]
        struct CountRow { c: i64 }
        let count_row: CountRow = CountRow::find_by_statement(
            Statement::from_string(DbBackend::Sqlite, sql_count)
        ).one(conn).await?.ok_or_else(|| SourceError::Other("count returned no row".into()))?;

        // Map DB column names back to logical keys (reverse of column_map).
        let reverse_map: std::collections::BTreeMap<String, String> =
            binding.column_map.iter().map(|(k, v)| (v.clone(), k.clone())).collect();

        let items: Vec<shared::Entity> = rows_json.into_iter().map(|row| {
            let obj = row.as_object().cloned().unwrap_or_default();
            let mut fields = serde_json::Map::new();
            for (db_col, val) in obj {
                let logical = reverse_map.get(&db_col).cloned().unwrap_or(db_col);
                fields.insert(logical, val);
            }
            let id = encode_pk_from_row(&fields, &binding.primary_key);
            shared::Entity { id, fields }
        }).collect();

        Ok(EntityPage {
            items,
            total_count: count_row.c as u64,
            page: page as u32,
            page_size: page_size as u32,
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
        let conn = self.conn.as_ref().ok_or_else(|| SourceError::Other("init not called".into()))?;
        let table_meta = self.introspected.tables.get(&table)
            .ok_or_else(|| SourceError::Other(format!("table not found: {table}")))?;

        // PK column values, matching the binding's primary_key column order.
        let pk_logical = &binding.primary_key;
        let pk_values: Vec<String> = match id {
            EntityId::Single(s) => {
                if pk_logical.len() != 1 {
                    return Err(SourceError::Other(format!(
                        "single-PK id but binding has {} PK columns", pk_logical.len()
                    )));
                }
                vec![s.clone()]
            }
            EntityId::Composite(parts) => {
                if parts.len() != pk_logical.len() {
                    return Err(SourceError::Other(format!(
                        "composite-PK id has {} parts, binding has {} columns",
                        parts.len(), pk_logical.len()
                    )));
                }
                parts.clone()
            }
        };

        // WHERE: AND of (pk_db_col = ?) — values bound, not interpolated.
        let where_clauses: Vec<String> = pk_logical.iter().map(|logical| {
            let db_col = binding.column_map.get(logical).cloned().unwrap_or_else(|| logical.clone());
            format!("{} = ?", quote_ident(&db_col))
        }).collect();
        let where_sql = format!(" WHERE {}", where_clauses.join(" AND "));

        let select_cols: Vec<String> = if binding.column_map.is_empty() {
            table_meta.columns.iter().map(|c| c.name.clone()).collect()
        } else {
            let mut s: std::collections::BTreeSet<String> = binding.column_map.values().cloned().collect();
            for pk in &table_meta.primary_key { s.insert(pk.clone()); }
            s.into_iter().collect()
        };

        let cols_sql = select_cols.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
        let sql = format!("SELECT {cols_sql} FROM {} {} LIMIT 1", quote_ident(&table), where_sql);

        use sea_orm::{DbBackend, FromQueryResult, Statement, Value, JsonValue};
        let values: Vec<Value> = pk_values.into_iter().map(Value::from).collect();
        let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, sql, values);

        let row_opt: Option<serde_json::Value> = JsonValue::find_by_statement(stmt).one(conn).await?;
        let Some(row) = row_opt else { return Ok(None) };

        let reverse_map: std::collections::BTreeMap<String, String> =
            binding.column_map.iter().map(|(k, v)| (v.clone(), k.clone())).collect();
        let obj = row.as_object().cloned().unwrap_or_default();
        let mut fields = serde_json::Map::new();
        for (db_col, val) in obj {
            let logical = reverse_map.get(&db_col).cloned().unwrap_or(db_col);
            fields.insert(logical, val);
        }
        let id_string = encode_pk_from_row(&fields, &binding.primary_key);
        Ok(Some(shared::Entity { id: id_string, fields }))
    }
    async fn create(&self, _b: &EntityBinding, _id: Option<String>,
        _f: serde_json::Map<String, serde_json::Value>, _a: Option<&str>)
        -> Result<Entity, SourceError> { unimplemented!("Task 18") }
    async fn update(&self, _b: &EntityBinding, _id: &EntityId,
        _f: serde_json::Map<String, serde_json::Value>, _a: Option<&str>)
        -> Result<Option<Entity>, SourceError> { unimplemented!("Task 18") }
    async fn delete(&self, _b: &EntityBinding, _id: &EntityId)
        -> Result<bool, SourceError> { unimplemented!("Task 18") }
}

pub(super) fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub(super) fn encode_pk_from_row(
    fields: &serde_json::Map<String, serde_json::Value>,
    pk_cols: &[String],
) -> String {
    let parts: Vec<String> = pk_cols.iter().map(|c| {
        fields.get(c).map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        }).unwrap_or_default()
    }).collect();
    if parts.len() == 1 {
        shared::source::EntityId::Single(parts.into_iter().next().unwrap()).encode()
    } else {
        shared::source::EntityId::Composite(parts).encode()
    }
}
