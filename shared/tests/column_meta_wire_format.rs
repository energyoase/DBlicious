//! Wire-Pin fuer `ColumnMeta` — sichert die camelCase-Form und die
//! Abwaertskompatibilitaet des additiven `validator_id`-Felds (Q0014).
//! Muster gespiegelt aus `reference_wire_format.rs`.

use shared::{ColumnMeta, FieldType};

fn base() -> ColumnMeta {
    ColumnMeta {
        key: "stackId".into(),
        label_key: "field.datev_entry.stack_id".into(),
        field_type: FieldType::Integer,
        sortable: true,
        filterable: true,
        comparator_id: None,
        filter_id: None,
        editor_id: None,
        formatter_id: None,
        validator_id: None,
        action_ids: vec![],
    }
}

#[test]
fn validator_id_roundtrips_as_camel_case() {
    let mut c = base();
    c.validator_id = Some("script:d2v_balance_validator".into());
    let json = serde_json::to_string(&c).unwrap();
    assert!(
        json.contains("\"validatorId\":\"script:d2v_balance_validator\""),
        "serialized JSON should carry validatorId, got: {json}"
    );
    let back: ColumnMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c);
}

#[test]
fn validator_id_none_is_skipped() {
    let c = base(); // validator_id: None
    let json = serde_json::to_string(&c).unwrap();
    assert!(
        !json.contains("validatorId"),
        "None validator_id must be skipped (skip_serializing_if), got: {json}"
    );
}

#[test]
fn missing_validator_id_deserializes_to_none() {
    // Alte data-dir-JSON ohne validatorId -> None (Beweis: additiv).
    let json = r#"{
        "key": "stackId",
        "labelKey": "field.datev_entry.stack_id",
        "fieldType": { "kind": "integer" },
        "sortable": true,
        "filterable": true
    }"#;
    let c: ColumnMeta = serde_json::from_str(json).unwrap();
    assert_eq!(c.validator_id, None);
}

#[test]
fn field_type_defaults_validator_id_roundtrips() {
    use shared::FieldTypeDefaults;
    let d = FieldTypeDefaults {
        validator_id: Some("script:d2v_balance_validator".into()),
        allowed_validator_ids: vec!["script:d2v_balance_validator".into()],
        ..Default::default()
    };
    let json = serde_json::to_string(&d).unwrap();
    assert!(json.contains("\"validatorId\""), "got: {json}");
    assert!(json.contains("\"allowedValidatorIds\""), "got: {json}");
    let back: FieldTypeDefaults = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
}

#[test]
fn field_type_defaults_without_validator_fields_default_empty() {
    use shared::FieldTypeDefaults;
    let d: FieldTypeDefaults = serde_json::from_str("{}").unwrap();
    assert_eq!(d.validator_id, None);
    assert!(d.allowed_validator_ids.is_empty());
}
