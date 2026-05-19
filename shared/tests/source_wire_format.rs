//! Pin-Tests fuer das Wire-Format der Source-Konzepte.
//!
//! Aenderungen am Encoding waeren ein Wire-Break — Server und Client
//! koennen sonst nicht mehr eindeutig dieselbe ID interpretieren.

use shared::source::{EntityId, COMPOSITE_KEY_SEPARATOR};

#[test]
fn separator_is_double_colon() {
    assert_eq!(COMPOSITE_KEY_SEPARATOR, "::");
}

#[test]
fn single_id_roundtrips_as_plain_string() {
    let id = EntityId::Single("p-001".into());
    assert_eq!(id.encode(), "p-001");
    assert_eq!(EntityId::decode("p-001"), EntityId::Single("p-001".into()));
}

#[test]
fn composite_id_uses_double_colon() {
    let id = EntityId::Composite(vec!["1234".into(), "ACC".into()]);
    assert_eq!(id.encode(), "1234::ACC");
    assert_eq!(
        EntityId::decode("1234::ACC"),
        EntityId::Composite(vec!["1234".into(), "ACC".into()])
    );
}

#[test]
fn composite_with_embedded_separator_escalates_to_json_array() {
    // Eine Komponente enthaelt selbst "::", daher JSON-Array-Form.
    let id = EntityId::Composite(vec!["a::b".into(), "7".into()]);
    let encoded = id.encode();
    assert!(encoded.starts_with('['), "expected JSON-array form, got {encoded}");
    assert_eq!(EntityId::decode(&encoded), id);
}

#[test]
fn composite_with_empty_part_escalates_to_json_array() {
    let id = EntityId::Composite(vec!["".into(), "x".into()]);
    let encoded = id.encode();
    assert!(encoded.starts_with('['), "expected JSON-array form, got {encoded}");
    assert_eq!(EntityId::decode(&encoded), id);
}

#[test]
fn composite_with_whitespace_edges_escalates_to_json_array() {
    let id = EntityId::Composite(vec![" x ".into(), "y".into()]);
    let encoded = id.encode();
    assert!(encoded.starts_with('['), "expected JSON-array form, got {encoded}");
    assert_eq!(EntityId::decode(&encoded), id);
}

#[test]
fn single_id_starting_with_bracket_roundtrips_as_single() {
    // Edge case: ein Single-PK-String, der zufaellig mit '[' beginnt.
    // Wir wollen, dass decode("[abc]") => Single("[abc]") liefert, NICHT
    // JSON-Array-Versuch. Konvention: Single-Form akzeptiert beliebige
    // Strings; JSON-Array-Form wird nur erkannt, wenn der String valides
    // JSON-Array-of-Strings ist.
    let id = EntityId::Single("[abc]".into());
    assert_eq!(id.encode(), "[abc]");
    assert_eq!(EntityId::decode("[abc]"), EntityId::Single("[abc]".into()));
}
