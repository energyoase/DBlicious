//! Bulk-Export (Phase 1.7.14).
//!
//! Heute: CSV (RFC-4180-konform: `,`-Trenner, `"`-Quoting bei
//! Sonderzeichen, CRLF-Line-Endings) und XLSX (typisierte Zellen ueber
//! `rust_xlsxwriter`).
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
    #[error("xlsx: {0}")]
    Xlsx(String),
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

// =============================================================================
// XLSX (Phase 1.7.14 Folge-Item)
// =============================================================================

/// Pendant zu [`export_csv`] fuer XLSX. Liefert ein vollstaendiges
/// Workbook als Byte-Buffer; der Aufrufer (GraphQL-Resolver, CLI, ...)
/// kann das direkt an einen `Content-Type: application/vnd.openxml...`-
/// Response anhaengen.
pub async fn export_xlsx(
    conn:        &DatabaseConnection,
    entity_type: &str,
    columns:     &[&str],
) -> Result<Vec<u8>, ExportError> {
    let rows = entities::Entity::find()
        .filter(entities::Column::EntityType.eq(entity_type))
        .all(conn)
        .await?;
    let mut entries: Vec<serde_json::Map<String, serde_json::Value>> =
        Vec::with_capacity(rows.len());
    for r in rows {
        let v: serde_json::Value =
            serde_json::from_str(&r.fields_json).map_err(|e| ExportError::Json(format!("{e}")))?;
        if let serde_json::Value::Object(o) = v {
            entries.push(o);
        } else {
            entries.push(serde_json::Map::new());
        }
    }
    entries_to_xlsx(&entries, columns)
}

/// Reine Format-Funktion. Eine Worksheet "Export"; Spalten-Reihenfolge
/// = `columns`-Reihenfolge; Header in Zeile 0; Zellen in Zeile 1..n.
///
/// Typed-Mapping je `serde_json::Value`:
///   - `String` → `write_string`
///   - `Number` (f64-convertible) → `write_number`
///   - `Bool` → `write_boolean`
///   - `Null` / fehlend → keine Zelle (bleibt leer)
///   - `Array`/`Object` → kompakte JSON-Serialisierung als String
pub fn entries_to_xlsx(
    entries: &[serde_json::Map<String, serde_json::Value>],
    columns: &[&str],
) -> Result<Vec<u8>, ExportError> {
    use rust_xlsxwriter::{Workbook, Worksheet};

    let mut wb = Workbook::new();
    let ws: &mut Worksheet = wb
        .add_worksheet()
        .set_name("Export")
        .map_err(|e| ExportError::Xlsx(format!("set_name: {e}")))?;

    // Header.
    for (col_idx, c) in columns.iter().enumerate() {
        ws.write_string(0, col_idx as u16, *c)
            .map_err(|e| ExportError::Xlsx(format!("header: {e}")))?;
    }

    // Rows.
    for (row_off, entry) in entries.iter().enumerate() {
        let row_idx = (row_off + 1) as u32;
        for (col_idx, c) in columns.iter().enumerate() {
            let col = col_idx as u16;
            let Some(val) = entry.get(*c) else { continue };
            match val {
                serde_json::Value::Null => {} // leere Zelle
                serde_json::Value::String(s) => {
                    ws.write_string(row_idx, col, s.as_str())
                        .map_err(|e| ExportError::Xlsx(format!("write_string: {e}")))?;
                }
                serde_json::Value::Bool(b) => {
                    ws.write_boolean(row_idx, col, *b)
                        .map_err(|e| ExportError::Xlsx(format!("write_boolean: {e}")))?;
                }
                serde_json::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        ws.write_number(row_idx, col, f)
                            .map_err(|e| ExportError::Xlsx(format!("write_number: {e}")))?;
                    } else {
                        // f64 ist die einzige Schnittstelle; bei
                        // out-of-range serialisieren wir als Text.
                        ws.write_string(row_idx, col, &n.to_string())
                            .map_err(|e| ExportError::Xlsx(format!("write_number_as_string: {e}")))?;
                    }
                }
                other => {
                    let s = serde_json::to_string(other)
                        .map_err(|e| ExportError::Xlsx(format!("compact_json: {e}")))?;
                    ws.write_string(row_idx, col, &s)
                        .map_err(|e| ExportError::Xlsx(format!("write_compact: {e}")))?;
                }
            }
        }
    }

    wb.save_to_buffer()
        .map_err(|e| ExportError::Xlsx(format!("save_to_buffer: {e}")))
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

    // ----- XLSX -----

    fn read_xlsx_back(bytes: &[u8]) -> Vec<Vec<calamine::Data>> {
        use calamine::{Reader, Xlsx};
        use std::io::Cursor;
        let cursor = Cursor::new(bytes.to_vec());
        let mut wb: Xlsx<_> = Xlsx::new(cursor).expect("Workbook lesbar");
        // Erste (und einzige) Sheet:
        let sheets = wb.sheet_names();
        let first = sheets.first().expect("mindestens ein Sheet").clone();
        let range = wb.worksheet_range(&first).expect("Sheet lesbar");
        range.rows().map(|r| r.to_vec()).collect()
    }

    #[test]
    fn xlsx_buffer_starts_with_zip_magic_bytes() {
        let buf = entries_to_xlsx(&[], &["a"]).unwrap();
        assert!(buf.len() > 4, "XLSX-Buffer muss > 4 Bytes sein");
        assert_eq!(&buf[..4], b"PK\x03\x04", "XLSX ist ein ZIP");
    }

    #[test]
    fn xlsx_header_only_for_empty_entries() {
        let buf = entries_to_xlsx(&[], &["a", "b"]).unwrap();
        let rows = read_xlsx_back(&buf);
        assert_eq!(rows.len(), 1, "nur Header-Zeile");
        assert_eq!(rows[0].len(), 2);
        assert_eq!(rows[0][0], calamine::Data::String("a".into()));
        assert_eq!(rows[0][1], calamine::Data::String("b".into()));
    }

    #[test]
    fn xlsx_round_trip_string_number_bool() {
        let e = vec![entry(&[
            ("name", serde_json::json!("Hammer")),
            ("price", serde_json::json!(9.99)),
            ("active", serde_json::json!(true)),
        ])];
        let buf = entries_to_xlsx(&e, &["name", "price", "active"]).unwrap();
        let rows = read_xlsx_back(&buf);
        assert_eq!(rows.len(), 2, "Header + 1 Datenzeile");
        let data = &rows[1];
        assert_eq!(data[0], calamine::Data::String("Hammer".into()));
        // calamine liest Floats als Data::Float
        assert_eq!(data[1], calamine::Data::Float(9.99));
        assert_eq!(data[2], calamine::Data::Bool(true));
    }

    #[test]
    fn xlsx_missing_or_null_value_stays_empty_cell() {
        let e = vec![entry(&[("a", serde_json::json!(1))])];
        let buf = entries_to_xlsx(&e, &["a", "b"]).unwrap();
        let rows = read_xlsx_back(&buf);
        // calamine kann die Trailing-Empty-Zelle weglassen — wir
        // pruefen pragmatisch: erste Zelle ist 1, zweite ist entweder
        // gar nicht da oder Data::Empty.
        let data = &rows[1];
        assert_eq!(data[0], calamine::Data::Float(1.0));
        if let Some(second) = data.get(1) {
            assert!(matches!(second, calamine::Data::Empty), "leere Zelle erwartet, war {:?}", second);
        }
    }

    #[test]
    fn xlsx_array_and_object_are_serialized_as_compact_json() {
        let e = vec![entry(&[
            ("tags", serde_json::json!(["x", "y"])),
            ("meta", serde_json::json!({"k": 1})),
        ])];
        let buf = entries_to_xlsx(&e, &["tags", "meta"]).unwrap();
        let rows = read_xlsx_back(&buf);
        let data = &rows[1];
        assert_eq!(data[0], calamine::Data::String(r#"["x","y"]"#.into()));
        assert_eq!(data[1], calamine::Data::String(r#"{"k":1}"#.into()));
    }
}
