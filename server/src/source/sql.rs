//! SQL-Helfer fuer relationale Sources (`foreign-sqlite`, `managed-sqlite`-
//! Table-Locator, spaeter postgres/mysql).
//!
//! Diese Funktionen waren urspruenglich in `foreign_sqlite.rs` versteckt;
//! seit Phase 0.6 B4 nutzen sie auch `managed_sqlite.rs`'s Table-Locator-
//! Pfad. Backend-Dispatch macht der Aufrufer (uebergibt `DbBackend`).

use sea_orm::{DbBackend, Statement, Value};

use shared::source::EntityBinding;

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
