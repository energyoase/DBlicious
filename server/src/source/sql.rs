//! SQL-Helfer fuer relationale Sources (`foreign-sqlite`, `managed-sqlite`-
//! Table-Locator, spaeter postgres/mysql).
//!
//! Diese Funktionen waren urspruenglich in `foreign_sqlite.rs` versteckt;
//! seit Phase 0.6 B4 nutzen sie auch `managed_sqlite.rs`'s Table-Locator-
//! Pfad. Backend-Dispatch macht der Aufrufer (uebergibt `DbBackend`).

use sea_orm::{DbBackend, Statement, Value};

use shared::source::{EntityBinding, EntityId};

pub fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub fn json_to_sea_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null   => Value::String(None),
        serde_json::Value::Bool(b) => Value::Bool(Some(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::BigInt(Some(i))
            } else if let Some(f) = n.as_f64() {
                Value::Double(Some(f))
            } else {
                Value::String(Some(Box::new(n.to_string())))
            }
        }
        serde_json::Value::String(s) => Value::String(Some(Box::new(s.clone()))),
        other => Value::String(Some(Box::new(other.to_string()))),
    }
}

pub fn encode_pk_from_row(
    fields: &serde_json::Map<String, serde_json::Value>,
    pk_cols: &[String],
) -> String {
    let parts: Vec<String> = pk_cols
        .iter()
        .map(|c| {
            fields
                .get(c)
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default()
        })
        .collect();
    if parts.len() == 1 {
        shared::source::EntityId::Single(parts.into_iter().next().unwrap()).encode()
    } else {
        shared::source::EntityId::Composite(parts).encode()
    }
}

/// Baut die WHERE-Klausel + Parameter aus einem `FilterCriteria`.
/// Mapping logische → DB-Spalten kommt aus `binding.column_map`.
pub fn build_filter_sql(
    filter:  &shared::FilterCriteria,
    binding: &EntityBinding,
) -> (String, Vec<Value>) {
    let resolve = |logical: &str| {
        binding
            .column_map
            .get(logical)
            .cloned()
            .unwrap_or_else(|| logical.to_string())
    };

    let mut clauses: Vec<String> = Vec::new();
    let mut values:  Vec<Value>  = Vec::new();
    for cf in &filter.predicates {
        let db_col = resolve(&cf.key);
        match &cf.predicate {
            shared::FilterPredicate::TextContains { value, case_insensitive } => {
                if *case_insensitive {
                    clauses.push(format!("LOWER({}) LIKE LOWER(?)", quote_ident(&db_col)));
                } else {
                    clauses.push(format!("{} LIKE ?", quote_ident(&db_col)));
                }
                values.push(Value::from(format!("%{value}%")));
            }
            shared::FilterPredicate::TextEquals { value, case_insensitive } => {
                if *case_insensitive {
                    clauses.push(format!("LOWER({}) = LOWER(?)", quote_ident(&db_col)));
                } else {
                    clauses.push(format!("{} = ?", quote_ident(&db_col)));
                }
                values.push(Value::from(value.clone()));
            }
            shared::FilterPredicate::NumberEquals { value } => {
                clauses.push(format!("{} = ?", quote_ident(&db_col)));
                values.push(Value::from(*value));
            }
            shared::FilterPredicate::NumberRange { min, max } => {
                if let Some(min) = min {
                    clauses.push(format!("{} >= ?", quote_ident(&db_col)));
                    values.push(Value::from(*min));
                }
                if let Some(max) = max {
                    clauses.push(format!("{} <= ?", quote_ident(&db_col)));
                    values.push(Value::from(*max));
                }
            }
            shared::FilterPredicate::BoolEquals { value } => {
                clauses.push(format!("{} = ?", quote_ident(&db_col)));
                values.push(Value::from(if *value { 1i32 } else { 0i32 }));
            }
            shared::FilterPredicate::DateRange { from, to } => {
                if let Some(from) = from {
                    clauses.push(format!("{} >= ?", quote_ident(&db_col)));
                    values.push(Value::from(from.clone()));
                }
                if let Some(to) = to {
                    clauses.push(format!("{} <= ?", quote_ident(&db_col)));
                    values.push(Value::from(to.clone()));
                }
            }
            shared::FilterPredicate::EnumIn { values: vs } => {
                if !vs.is_empty() {
                    let placeholders = vs.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                    clauses.push(format!("{} IN ({})", quote_ident(&db_col), placeholders));
                    for v in vs {
                        values.push(Value::from(v.clone()));
                    }
                }
            }
            shared::FilterPredicate::IsNull => {
                clauses.push(format!("{} IS NULL", quote_ident(&db_col)));
            }
            shared::FilterPredicate::IsNotNull => {
                clauses.push(format!("{} IS NOT NULL", quote_ident(&db_col)));
            }
        }
    }
    if let Some(search) = filter.global_search.as_deref() {
        if !search.is_empty() {
            let mut search_clauses: Vec<String> = Vec::new();
            for (_, db_col) in &binding.column_map {
                search_clauses.push(format!("LOWER({}) LIKE LOWER(?)", quote_ident(db_col)));
                values.push(Value::from(format!("%{search}%")));
            }
            if !search_clauses.is_empty() {
                clauses.push(format!("({})", search_clauses.join(" OR ")));
            }
        }
    }
    let sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    (sql, values)
}

/// Baut ein vorbereitetes Statement aus SQL + Werten unter dem gewuenschten
/// Backend. Konvenienz-Wrapper, damit Aufrufer nicht ueberall `DbBackend`
/// importieren muessen.
pub fn make_statement(backend: DbBackend, sql: String, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(backend, sql, values)
}

// =============================================================================
// Generische Table-Locator-CRUD-Implementierungen.
//
// Alle relationalen Sources (foreign-sqlite, managed-sqlite Table-Locator,
// postgres, mysql) teilen sich diese Implementierung — nur Connection + Backend
// kommen vom jeweiligen Source-Wrapper.
// =============================================================================

use sea_orm::{ConnectionTrait, FromQueryResult, JsonValue};

use shared::{Entity, EntityPage};

use super::{PageQuery, SourceError};

fn resolve_col(binding: &EntityBinding, logical: &str) -> String {
    binding
        .column_map
        .get(logical)
        .cloned()
        .unwrap_or_else(|| logical.to_string())
}

fn select_columns_sql_from_binding(binding: &EntityBinding) -> Vec<String> {
    let mut s: std::collections::BTreeSet<String> = binding.column_map.values().cloned().collect();
    for pk in &binding.primary_key {
        s.insert(resolve_col(binding, pk));
    }
    s.into_iter().collect()
}

fn rows_to_entities(binding: &EntityBinding, rows_json: Vec<serde_json::Value>) -> Vec<Entity> {
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

pub async fn relational_list_page(
    conn:    &sea_orm::DatabaseConnection,
    backend: DbBackend,
    binding: &EntityBinding,
    table:   &str,
    query:   &PageQuery,
    extra_select_cols: Option<&[String]>,
) -> Result<EntityPage, SourceError> {
    // Wenn der Aufrufer Spalten aus Introspect mitbringt (foreign-sqlite),
    // benutzen wir die wenn `column_map` leer ist; sonst aus binding.
    let select_cols: Vec<String> = if binding.column_map.is_empty() {
        extra_select_cols
            .map(|v| v.to_vec())
            .unwrap_or_default()
    } else {
        select_columns_sql_from_binding(binding)
    };
    if select_cols.is_empty() {
        return Err(SourceError::Other(
            "Table-Locator braucht column_map oder Introspect".into(),
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

    let stmt_list = make_statement(backend, sql_list, where_values.clone());
    let rows_json: Vec<serde_json::Value> =
        JsonValue::find_by_statement(stmt_list).all(conn).await?;

    #[derive(FromQueryResult)]
    struct CountRow { c: i64 }
    let stmt_count = make_statement(backend, sql_count, where_values);
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

pub async fn relational_get(
    conn:    &sea_orm::DatabaseConnection,
    backend: DbBackend,
    binding: &EntityBinding,
    table:   &str,
    id:      &EntityId,
    extra_select_cols: Option<&[String]>,
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

    let select_cols: Vec<String> = if binding.column_map.is_empty() {
        extra_select_cols.map(|v| v.to_vec()).unwrap_or_default()
    } else {
        select_columns_sql_from_binding(binding)
    };
    let cols_sql = select_cols.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT {cols_sql} FROM {} {} LIMIT 1",
        quote_ident(table),
        where_sql
    );

    let values: Vec<Value> = pk_values.into_iter().map(Value::from).collect();
    let stmt = make_statement(backend, sql, values);

    let row_opt: Option<serde_json::Value> =
        JsonValue::find_by_statement(stmt).one(conn).await?;
    Ok(row_opt.map(|row| {
        rows_to_entities(binding, vec![row])
            .into_iter()
            .next()
            .expect("rows_to_entities returns one")
    }))
}

pub async fn relational_create(
    conn:    &sea_orm::DatabaseConnection,
    backend: DbBackend,
    binding: &EntityBinding,
    table:   &str,
    fields:  serde_json::Map<String, serde_json::Value>,
) -> Result<Entity, SourceError> {
    let mut cols: Vec<String> = Vec::new();
    let mut placeholders: Vec<String> = Vec::new();
    let mut values: Vec<Value> = Vec::new();
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
    let stmt = make_statement(backend, sql, values);
    conn.execute(stmt).await?;

    let id = encode_pk_from_row(&fields, &binding.primary_key);
    Ok(Entity { id, fields })
}

pub async fn relational_update(
    conn:    &sea_orm::DatabaseConnection,
    backend: DbBackend,
    binding: &EntityBinding,
    table:   &str,
    id:      &EntityId,
    patch:   serde_json::Map<String, serde_json::Value>,
    extra_select_cols: Option<&[String]>,
) -> Result<Option<Entity>, SourceError> {
    if patch.is_empty() {
        return relational_get(conn, backend, binding, table, id, extra_select_cols).await;
    }
    let mut set_parts: Vec<String> = Vec::new();
    let mut values:    Vec<Value>  = Vec::new();
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
        values.push(Value::from(pv.clone()));
    }
    let sql = format!(
        "UPDATE {} SET {} WHERE {}",
        quote_ident(table),
        set_parts.join(", "),
        where_clauses.join(" AND "),
    );
    let stmt = make_statement(backend, sql, values);
    conn.execute(stmt).await?;
    relational_get(conn, backend, binding, table, id, extra_select_cols).await
}

pub async fn relational_delete(
    conn:    &sea_orm::DatabaseConnection,
    backend: DbBackend,
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
    let mut values: Vec<Value> = Vec::new();
    for pv in &pk_values {
        values.push(Value::from(pv.clone()));
    }
    let sql = format!(
        "DELETE FROM {} WHERE {}",
        quote_ident(table),
        where_clauses.join(" AND "),
    );
    let stmt = make_statement(backend, sql, values);
    let res = conn.execute(stmt).await?;
    Ok(res.rows_affected() > 0)
}
