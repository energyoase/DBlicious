//! `ManagedSqliteSource` — heutiges Verhalten (eigene Tabellen, JSON-Blob
//! in `entities`-Tabelle) gewrappt unter die `Source`-Trait.
//!
//! Seit Phase 0.6 B4 zusaetzlich `BindingLocator::Table`-Support: eine
//! Entitaet kann auf eine echte SQL-Tabelle innerhalb der managed-sqlite
//! DB zeigen (statt JSON-Blob in `entities`). Code-Pfad teilt die SQL-
//! Helfer mit `foreign_sqlite` (siehe `super::sql`).

use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DbBackend, FromQueryResult, JsonValue};

use shared::source::{BindingLocator, EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::sql::{build_filter_sql, encode_pk_from_row, json_to_sea_value, quote_ident};
use super::{Capabilities, PageQuery, Source, SourceError};

pub struct ManagedSqliteSource {
    name: String,
}

impl ManagedSqliteSource {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Source for ManagedSqliteSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> &'static str {
        "managed-sqlite"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,    // im Table-Locator-Pfad echt genutzt (B4)
            supports_introspection: false,  // wir kennen unser Schema durch SeaORM-Entities
            supports_composite_pk: true,    // im Table-Locator-Pfad voll unterstuetzt (B4);
                                            // GenericEntityRow bleibt Single-PK.
            supports_ddl: true,             // Designer-Schemas laufen ueber diese Source.
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        // Delegiert an den bestehenden Boot-Pfad. Idempotent durch
        // `db::init`'s OnceLock-Slot.
        crate::db::init().await.map_err(SourceError::from)?;
        Ok(())
    }

    async fn list_page(
        &self,
        binding: &EntityBinding,
        query: &PageQuery,
    ) -> Result<EntityPage, SourceError> {
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => {
                Ok(crate::data::entities_page_raw(
                    entity_type,
                    query.page,
                    query.page_size,
                    query.sort.clone(),
                    query.filter.clone(),
                )
                .await)
            }
            BindingLocator::Table { table } => {
                table_list_page(&crate::db::conn(), binding, table, query).await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => {
                let key = match id {
                    EntityId::Single(s) => s.clone(),
                    EntityId::Composite(_) => {
                        return Err(SourceError::UnsupportedLocator(
                            "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
                        ));
                    }
                };
                Ok(crate::data::entity_by_id_raw(entity_type, &key).await)
            }
            BindingLocator::Table { table } => {
                table_get(&crate::db::conn(), binding, table, id).await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    async fn create(
        &self,
        binding: &EntityBinding,
        id: Option<String>,
        fields: serde_json::Map<String, serde_json::Value>,
        actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => Ok(
                crate::data::create_entity_raw(entity_type, id, fields, actor_user_id).await,
            ),
            BindingLocator::Table { table } => {
                table_create(&crate::db::conn(), binding, table, fields).await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    async fn update(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
        patch: serde_json::Map<String, serde_json::Value>,
        actor_user_id: Option<&str>,
    ) -> Result<Option<Entity>, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => {
                let key = match id {
                    EntityId::Single(s) => s.clone(),
                    EntityId::Composite(_) => {
                        return Err(SourceError::UnsupportedLocator(
                            "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
                        ));
                    }
                };
                Ok(crate::data::update_entity_raw(entity_type, &key, patch, actor_user_id).await)
            }
            BindingLocator::Table { table } => {
                table_update(&crate::db::conn(), binding, table, id, patch).await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    async fn delete(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<bool, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => {
                let key = match id {
                    EntityId::Single(s) => s.clone(),
                    EntityId::Composite(_) => {
                        return Err(SourceError::UnsupportedLocator(
                            "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
                        ));
                    }
                };
                Ok(crate::data::delete_entity_raw(entity_type, &key).await)
            }
            BindingLocator::Table { table } => {
                table_delete(&crate::db::conn(), binding, table, id).await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    async fn apply_schema(
        &self,
        schema: &shared::DbSchema,
    ) -> Result<usize, SourceError> {
        // Delegiert an den bestehenden ddl-Pfad (CREATE TABLE IF NOT EXISTS).
        // try_apply_schema toleriert Statement-Level-Fehler intern und liefert
        // die Anzahl erfolgreich ausgefuehrter Statements.
        Ok(crate::ddl::try_apply_schema(schema).await)
    }
}

// =============================================================================
// Table-Locator-Pfad (B4): echte SQL-Spalten, identisch zur foreign_sqlite-
// SQL-Generierung, aber gegen den managed-sqlite Connection-Pool.
// =============================================================================

fn resolve_col(binding: &EntityBinding, logical: &str) -> String {
    binding
        .column_map
        .get(logical)
        .cloned()
        .unwrap_or_else(|| logical.to_string())
}

fn select_columns_sql(binding: &EntityBinding) -> Vec<String> {
    // Wenn column_map gesetzt ist: nur die gemappten Werte + PK-Spalten.
    // Wenn nicht: SELECT * waere ideal — wir kennen das Schema aber nicht
    // ohne Introspektion. Fuer den MVP verlangen wir column_map.
    let mut s: std::collections::BTreeSet<String> = binding.column_map.values().cloned().collect();
    for pk in &binding.primary_key {
        s.insert(resolve_col(binding, pk));
    }
    s.into_iter().collect()
}

fn rows_to_entities(
    binding: &EntityBinding,
    rows_json: Vec<serde_json::Value>,
) -> Vec<Entity> {
    let reverse_map: std::collections::BTreeMap<String, String> = binding
        .column_map
        .iter()
        .map(|(k, v)| (v.clone(), k.clone()))
        .collect();
    rows_json
        .into_iter()
        .map(|row| {
            let obj = row.as_object().cloned().unwrap_or_default();
            let mut fields = serde_json::Map::new();
            for (db_col, val) in obj {
                let logical = reverse_map.get(&db_col).cloned().unwrap_or(db_col);
                fields.insert(logical, val);
            }
            let id = encode_pk_from_row(&fields, &binding.primary_key);
            Entity { id, fields }
        })
        .collect()
}

async fn table_list_page(
    conn:    &sea_orm::DatabaseConnection,
    binding: &EntityBinding,
    table:   &str,
    query:   &PageQuery,
) -> Result<EntityPage, SourceError> {
    let select_cols = select_columns_sql(binding);
    if select_cols.is_empty() {
        return Err(SourceError::Other(
            "Table-Locator braucht column_map oder primary_key".into(),
        ));
    }
    let cols_sql  = select_cols.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let table_sql = quote_ident(table);

    let page      = query.page.max(1);
    let page_size = query.page_size.max(1);
    let offset    = (page - 1) as i64 * page_size as i64;

    let order_sql = match &query.sort {
        Some(s) => {
            let db_col = resolve_col(binding, &s.field);
            let dir = match s.direction {
                shared::SortDirection::Asc  => "ASC",
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

    let stmt_list = sea_orm::Statement::from_sql_and_values(
        DbBackend::Sqlite, sql_list, where_values.clone(),
    );
    let rows_json: Vec<serde_json::Value> =
        JsonValue::find_by_statement(stmt_list).all(conn).await?;

    #[derive(FromQueryResult)]
    struct CountRow { c: i64 }
    let stmt_count = sea_orm::Statement::from_sql_and_values(
        DbBackend::Sqlite, sql_count, where_values,
    );
    let count_row: CountRow = CountRow::find_by_statement(stmt_count)
        .one(conn)
        .await?
        .ok_or_else(|| SourceError::Other("count returned no row".into()))?;

    let items = rows_to_entities(binding, rows_json);

    Ok(EntityPage {
        items,
        total_count: count_row.c as u64,
        page:        page as u32,
        page_size:   page_size as u32,
    })
}

async fn table_get(
    conn:    &sea_orm::DatabaseConnection,
    binding: &EntityBinding,
    table:   &str,
    id:      &EntityId,
) -> Result<Option<Entity>, SourceError> {
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

    let where_clauses: Vec<String> = pk_logical
        .iter()
        .map(|logical| format!("{} = ?", quote_ident(&resolve_col(binding, logical))))
        .collect();
    let where_sql = format!(" WHERE {}", where_clauses.join(" AND "));

    let select_cols = select_columns_sql(binding);
    let cols_sql = select_cols.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT {cols_sql} FROM {} {} LIMIT 1",
        quote_ident(table),
        where_sql
    );

    let values: Vec<sea_orm::Value> = pk_values.into_iter().map(sea_orm::Value::from).collect();
    let stmt = sea_orm::Statement::from_sql_and_values(DbBackend::Sqlite, sql, values);

    let row_opt: Option<serde_json::Value> =
        JsonValue::find_by_statement(stmt).one(conn).await?;
    Ok(row_opt.map(|row| {
        let obj = row.as_object().cloned().unwrap_or_default();
        let entities = rows_to_entities(binding, vec![serde_json::Value::Object(obj)]);
        entities.into_iter().next().expect("rows_to_entities returns one")
    }))
}

async fn table_create(
    conn:    &sea_orm::DatabaseConnection,
    binding: &EntityBinding,
    table:   &str,
    fields:  serde_json::Map<String, serde_json::Value>,
) -> Result<Entity, SourceError> {
    let mut cols: Vec<String> = Vec::new();
    let mut placeholders: Vec<String> = Vec::new();
    let mut values: Vec<sea_orm::Value> = Vec::new();
    for (logical, val) in &fields {
        cols.push(quote_ident(&resolve_col(binding, logical)));
        placeholders.push("?".into());
        values.push(json_to_sea_value(val));
    }
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_ident(table),
        cols.join(", "),
        placeholders.join(", "),
    );
    let stmt = sea_orm::Statement::from_sql_and_values(DbBackend::Sqlite, sql, values);
    conn.execute(stmt).await?;

    let id = encode_pk_from_row(&fields, &binding.primary_key);
    Ok(Entity { id, fields })
}

async fn table_update(
    conn:    &sea_orm::DatabaseConnection,
    binding: &EntityBinding,
    table:   &str,
    id:      &EntityId,
    patch:   serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Entity>, SourceError> {
    if patch.is_empty() {
        return table_get(conn, binding, table, id).await;
    }
    let mut set_parts: Vec<String> = Vec::new();
    let mut values:    Vec<sea_orm::Value> = Vec::new();
    for (logical, val) in &patch {
        set_parts.push(format!("{} = ?", quote_ident(&resolve_col(binding, logical))));
        values.push(json_to_sea_value(val));
    }
    let pk_logical = &binding.primary_key;
    let pk_values: Vec<String> = match id {
        EntityId::Single(s) => vec![s.clone()],
        EntityId::Composite(parts) => parts.clone(),
    };
    let where_clauses: Vec<String> = pk_logical
        .iter()
        .map(|logical| format!("{} = ?", quote_ident(&resolve_col(binding, logical))))
        .collect();
    for pv in &pk_values {
        values.push(sea_orm::Value::from(pv.clone()));
    }
    let sql = format!(
        "UPDATE {} SET {} WHERE {}",
        quote_ident(table),
        set_parts.join(", "),
        where_clauses.join(" AND "),
    );
    let stmt = sea_orm::Statement::from_sql_and_values(DbBackend::Sqlite, sql, values);
    conn.execute(stmt).await?;
    table_get(conn, binding, table, id).await
}

async fn table_delete(
    conn:    &sea_orm::DatabaseConnection,
    binding: &EntityBinding,
    table:   &str,
    id:      &EntityId,
) -> Result<bool, SourceError> {
    let pk_logical = &binding.primary_key;
    let pk_values: Vec<String> = match id {
        EntityId::Single(s) => vec![s.clone()],
        EntityId::Composite(parts) => parts.clone(),
    };
    let where_clauses: Vec<String> = pk_logical
        .iter()
        .map(|logical| format!("{} = ?", quote_ident(&resolve_col(binding, logical))))
        .collect();
    let mut values: Vec<sea_orm::Value> = Vec::new();
    for pv in &pk_values {
        values.push(sea_orm::Value::from(pv.clone()));
    }
    let sql = format!(
        "DELETE FROM {} WHERE {}",
        quote_ident(table),
        where_clauses.join(" AND "),
    );
    let stmt = sea_orm::Statement::from_sql_and_values(DbBackend::Sqlite, sql, values);
    let res = conn.execute(stmt).await?;
    Ok(res.rows_affected() > 0)
}
