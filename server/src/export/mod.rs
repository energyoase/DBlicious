//! Bulk-Export (Phase 1.7.14).
//!
//! Heute: CSV (RFC-4180-konform: `,`-Trenner, `"`-Quoting bei
//! Sonderzeichen, CRLF-Line-Endings). Excel-Export (XLSX) als Folge-
//! Item — bewusst klein gehalten, weil rust_xlsxwriter adds ~5MB.
//!
//! Permission-Gating macht der GraphQL-Resolver (Read auf
//! `entity_type`); diese Funktion ist pure Service-Layer.

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entity::entities;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("database: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("json: {0}")]
    Json(String),
}

/// Exportiert alle Zeilen eines `entity_type` als CSV.
///
/// `columns` ist die geordnete Liste der Feld-Keys, die als Spalten
/// rauskommen. Reihen werden anhand der `fields_json`-Spalte gelesen;
/// fehlende Felder werden als leere Zellen ausgegeben.
///
/// Anwendung: GraphQL-Resolver liest dieselben Filter wie die UI,
/// uebergibt sie an `entities_page_raw`, sammelt **alle** Pages, und
/// uebergibt die `Entity`-Liste an `entities_to_csv`. Diese Funktion
/// hier ist die "direkte" Variante ohne Filter (default = alle Rows).
pub async fn export_csv(
    conn:        &DatabaseConnection,
    entity_type: &str,
    columns:     &[&str],
) -> Result<String, ExportError> {
    let rows = entities::Entity::find()
        .filter(entities::Column::EntityType.eq(entity_type))
        .all(conn)
        .await?;
    let mut entries: Vec<serde_json::Map<String, serde_json::Value>> =
        Vec::with_capacity(rows.len());
    for r in rows {
        let v: serde_json::Value =
            serde_json::from_str(&r.fields_json).map_err(|e| ExportError::Json(format!("{e}")))?;
        let obj = match v {
            serde_json::Value::Object(o) => o,
            _ => serde_json::Map::new(),
        };
        entries.push(obj);
    }
    Ok(entries_to_csv(&entries, columns))
}

/// Reine Format-Funktion — gut isoliert testbar.
pub fn entries_to_csv(
    entries: &[serde_json::Map<String, serde_json::Value>],
    columns: &[&str],
) -> String {
    let mut out = String::new();
    // Header
    for (i, c) in columns.iter().enumerate() {
        if i > 0 { out.push(','); }
        push_csv_field(&mut out, c);
    }
    out.push_str("\r\n");
    // Rows
    for entry in entries {
        for (i, c) in columns.iter().enumerate() {
            if i > 0 { out.push(','); }
            let cell = match entry.get(*c) {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Null) | None => String::new(),
                Some(other) => other.to_string(),
            };
            push_csv_field(&mut out, &cell);
        }
        out.push_str("\r\n");
    }
    out
}

/// RFC-4180: Quoting nur, wenn das Feld `,`, `"`, `\r` oder `\n`
/// enthaelt. Innere `"` werden verdoppelt.
fn push_csv_field(out: &mut String, value: &str) {
    let needs_quoting = value.contains([',', '"', '\r', '\n']);
    if !needs_quoting {
        out.push_str(value);
        return;
    }
    out.push('"');
    for c in value.chars() {
        if c == '"' {
            out.push('"');
            out.push('"');
        } else {
            out.push(c);
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn header_only_for_empty_entries() {
        let s = entries_to_csv(&[], &["a", "b"]);
        assert_eq!(s, "a,b\r\n");
    }

    #[test]
    fn simple_string_and_number() {
        let e = vec![entry(&[
            ("name", serde_json::json!("Hammer")),
            ("price", serde_json::json!(9.99)),
        ])];
        let s = entries_to_csv(&e, &["name", "price"]);
        assert_eq!(s, "name,price\r\nHammer,9.99\r\n");
    }

    #[test]
    fn quoting_when_value_contains_comma() {
        let e = vec![entry(&[("x", serde_json::json!("a,b"))])];
        let s = entries_to_csv(&e, &["x"]);
        assert_eq!(s, "x\r\n\"a,b\"\r\n");
    }

    #[test]
    fn quoting_when_value_contains_quote() {
        let e = vec![entry(&[("x", serde_json::json!("a\"b"))])];
        let s = entries_to_csv(&e, &["x"]);
        assert_eq!(s, "x\r\n\"a\"\"b\"\r\n");
    }

    #[test]
    fn quoting_when_value_contains_newline() {
        let e = vec![entry(&[("x", serde_json::json!("a\nb"))])];
        let s = entries_to_csv(&e, &["x"]);
        assert_eq!(s, "x\r\n\"a\nb\"\r\n");
    }

    #[test]
    fn missing_or_null_becomes_empty() {
        let e = vec![entry(&[("a", serde_json::json!(1))])];
        let s = entries_to_csv(&e, &["a", "b"]);
        assert_eq!(s, "a,b\r\n1,\r\n");
    }

    #[test]
    fn bool_renders_as_text() {
        let e = vec![entry(&[("ok", serde_json::json!(true))])];
        let s = entries_to_csv(&e, &["ok"]);
        assert_eq!(s, "ok\r\ntrue\r\n");
    }
}
