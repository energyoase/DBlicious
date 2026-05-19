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

#[test]
fn single_containing_double_colon_violates_contract() {
    // Dokumentiert: ein Single mit "::" ist eine Vertragsverletzung des
    // Aufrufers (siehe EntityId-Doc-Comment). Wir pinnen das aktuelle
    // (unsichere) Verhalten als Regression-Schutz — Aenderung der
    // Dekodierungsregel ist ein Wire-Break.
    let s = EntityId::Single("a::b".into());
    let encoded = s.encode();
    let decoded = EntityId::decode(&encoded);
    assert_eq!(decoded, EntityId::Composite(vec!["a".into(), "b".into()]),
        "documented contract violation");
}

#[test]
fn single_looking_like_json_array_violates_contract() {
    // Analog zu oben: Single mit valider JSON-Array-of-Strings-Form
    // wird als Composite dekodiert.
    let s = EntityId::Single(r#"["a","b"]"#.into());
    let encoded = s.encode();
    let decoded = EntityId::decode(&encoded);
    assert_eq!(decoded, EntityId::Composite(vec!["a".into(), "b".into()]),
        "documented contract violation");
}

use shared::source::{BindingLocator, EntityBinding};
use std::collections::BTreeMap;

#[test]
fn binding_locator_table_roundtrips_as_tagged_json() {
    let loc = BindingLocator::Table { table: "DatevAccounts".into() };
    let json = serde_json::to_string(&loc).unwrap();
    assert!(json.contains(r#""kind":"table""#));
    assert!(json.contains(r#""table":"DatevAccounts""#));
    let back: BindingLocator = serde_json::from_str(&json).unwrap();
    assert_eq!(back, loc);
}

#[test]
fn binding_locator_generic_entity_row_uses_camel_case_inner_field() {
    let loc = BindingLocator::GenericEntityRow { entity_type: "product".into() };
    let json = serde_json::to_string(&loc).unwrap();
    assert!(json.contains(r#""kind":"genericEntityRow""#));
    assert!(json.contains(r#""entityType":"product""#),
        "expected camelCase inner field, got {json}");
    let back: BindingLocator = serde_json::from_str(&json).unwrap();
    assert_eq!(back, loc);
}

#[test]
fn entity_binding_omits_empty_column_map_and_default_read_only() {
    let b = EntityBinding {
        source: "local".into(),
        locator: BindingLocator::GenericEntityRow { entity_type: "product".into() },
        primary_key: vec!["id".into()],
        read_only: false,
        column_map: BTreeMap::new(),
    };
    let json = serde_json::to_string(&b).unwrap();
    assert!(!json.contains("columnMap"), "empty columnMap should be omitted, got {json}");
    // read_only=false is the default and may or may not be present;
    // we only require it round-trips.
    let back: EntityBinding = serde_json::from_str(&json).unwrap();
    assert_eq!(back, b);
}

#[test]
fn entity_binding_preserves_column_map_and_camel_case_primary_key() {
    let mut map = BTreeMap::new();
    map.insert("number".into(), "Number".into());
    map.insert("name".into(), "Name".into());
    let b = EntityBinding {
        source: "d2v_legacy".into(),
        locator: BindingLocator::Table { table: "DatevAccounts".into() },
        primary_key: vec!["number".into()],
        read_only: true,
        column_map: map,
    };
    let json = serde_json::to_string(&b).unwrap();
    assert!(json.contains(r#""primaryKey":["number"]"#),
        "expected camelCase primaryKey, got {json}");
    assert!(json.contains(r#""columnMap":{"name":"Name","number":"Number"}"#),
        "expected sorted BTreeMap serialization, got {json}");
    let back: EntityBinding = serde_json::from_str(&json).unwrap();
    assert_eq!(back, b);
}
