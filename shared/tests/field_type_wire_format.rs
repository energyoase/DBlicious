//! Wire-Format-Tests fuer die zentralen Tagged-Enums in `shared`.
//!
//! Hintergrund (CLAUDE.md): `FieldType` ist ein `#[serde(tag = "kind", rename_all = "camelCase")]`
//! Enum — die exakte JSON-Form ist Vertragsbestandteil zwischen Server und Client.
//! `SortDirection` benutzt `#[serde(rename_all = "lowercase")]` (Sonderfall;
//! die anderen Wire-Typen sind camelCase). Diese Tests fixieren beide.
//!
//! Achtung: `#[serde(rename_all = "camelCase")]` auf der Enum-Ebene benennt
//! die **Varianten** in camelCase (`DateTime` → `"dateTime"` als `kind`-Wert),
//! aber die **Felder einer Struct-Variante** behalten ihren Rust-Namen, sofern
//! keine separate `rename_all`-Annotation auf der Variante steht.
//! Das aktuelle `FieldType` und `FilterPredicate` haben das *nicht* — die
//! inneren Felder kommen daher als `currency_code_field` / `case_insensitive`
//! ueber die Leitung. Diese Tests fixieren genau diesen Ist-Zustand.

use serde_json::{json, Value};
use shared::{
    ColumnFilter, DirectionalEnumValue, FieldType, FilterCriteria, FilterPredicate, IntEnumValue,
    Sort, SortDirection,
};

#[test]
fn field_type_text_serializes_as_kind_text() {
    let v = serde_json::to_value(FieldType::Text).unwrap();
    assert_eq!(v, json!({"kind": "text"}));
}

#[test]
fn field_type_money_uses_snake_case_inner_field() {
    // Stand: rename_all greift NICHT in die Struct-Variante hinein — die
    // CLAUDE.md-Beschreibung (`currencyCodeField`) entspricht NICHT der
    // Realitaet. Der Test fixiert den Ist-Zustand.
    let v = serde_json::to_value(FieldType::Money {
        currency_code_field: Some("currency".into()),
    })
    .unwrap();
    assert_eq!(
        v,
        json!({"kind": "money", "currency_code_field": "currency"})
    );
}

#[test]
fn field_type_money_without_currency_field_keeps_null() {
    let v = serde_json::to_value(FieldType::Money {
        currency_code_field: None,
    })
    .unwrap();
    assert_eq!(v, json!({"kind": "money", "currency_code_field": null}));
}

#[test]
fn field_type_decimal_carries_precision() {
    let v = serde_json::to_value(FieldType::Decimal { precision: 4 }).unwrap();
    assert_eq!(v, json!({"kind": "decimal", "precision": 4}));
}

#[test]
fn field_type_reference_and_collection_use_entity_field() {
    let r = serde_json::to_value(FieldType::Reference {
        entity: "category".into(),
    })
    .unwrap();
    assert_eq!(r, json!({"kind": "reference", "entity": "category"}));

    let c = serde_json::to_value(FieldType::Collection {
        entity: "tag".into(),
    })
    .unwrap();
    assert_eq!(c, json!({"kind": "collection", "entity": "tag"}));
}

#[test]
fn field_type_enum_carries_values() {
    let v = serde_json::to_value(FieldType::Enum {
        values: vec!["new".into(), "paid".into()],
    })
    .unwrap();
    assert_eq!(v, json!({"kind": "enum", "values": ["new", "paid"]}));
}

#[test]
fn field_type_roundtrips_through_json() {
    let originals = vec![
        FieldType::Text,
        FieldType::Integer,
        FieldType::Decimal { precision: 2 },
        FieldType::Boolean,
        FieldType::Date,
        FieldType::DateTime,
        FieldType::Money {
            currency_code_field: Some("currency".into()),
        },
        FieldType::Reference {
            entity: "user".into(),
        },
        FieldType::Collection {
            entity: "tag".into(),
        },
        FieldType::Enum {
            values: vec!["a".into(), "b".into()],
        },
    ];
    for ft in originals {
        let s = serde_json::to_string(&ft).unwrap();
        let back: FieldType = serde_json::from_str(&s).unwrap();
        assert_eq!(ft, back, "FieldType-Roundtrip fehlgeschlagen: {s}");
    }
}

#[test]
fn unknown_field_type_kind_fails_to_deserialize() {
    // Sicherheitsnetz: ein unbekannter `kind` darf nicht still als
    // Default-Variant durchgehen.
    let r: Result<FieldType, _> = serde_json::from_value(json!({"kind": "frobnicated"}));
    assert!(r.is_err(), "unbekannter kind muss Fehler werfen");
}

#[test]
fn int_enum_serializes_with_kind_and_values() {
    let v = serde_json::to_value(FieldType::IntEnum {
        values: vec![
            IntEnumValue {
                value: 0,
                label_key: "journal.soll".into(),
                wire_name: "SOLL".into(),
            },
            IntEnumValue {
                value: 1,
                label_key: "journal.haben".into(),
                wire_name: "HABEN".into(),
            },
        ],
    })
    .unwrap();
    assert_eq!(
        v,
        json!({
            "kind": "intEnum",
            "values": [
                {"value": 0, "labelKey": "journal.soll", "wireName": "SOLL"},
                {"value": 1, "labelKey": "journal.haben", "wireName": "HABEN"}
            ]
        })
    );
}

#[test]
fn int_enum_roundtrips() {
    let original = FieldType::IntEnum {
        values: vec![IntEnumValue {
            value: 7,
            label_key: "x".into(),
            wire_name: "X".into(),
        }],
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: FieldType = serde_json::from_str(&json).unwrap();
    assert_eq!(original, back);
}

#[test]
fn int_enum_is_scalar() {
    let ft = FieldType::IntEnum { values: vec![] };
    assert!(ft.is_scalar());
    assert_eq!(ft.kind_str(), "intEnum");
}

#[test]
fn sort_direction_serializes_as_lowercase_string() {
    assert_eq!(
        serde_json::to_value(SortDirection::Asc).unwrap(),
        Value::String("asc".into())
    );
    assert_eq!(
        serde_json::to_value(SortDirection::Desc).unwrap(),
        Value::String("desc".into())
    );
}

#[test]
fn sort_serializes_field_and_direction_camelcase() {
    let s = Sort {
        field: "createdAt".into(),
        direction: SortDirection::Desc,
    };
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v, json!({"field": "createdAt", "direction": "desc"}));
}

#[test]
fn filter_predicate_text_contains_uses_snake_case_inner_field() {
    // Wie bei `FieldType::Money`: das innere Feld bleibt snake_case.
    let p = FilterPredicate::TextContains {
        value: "abc".into(),
        case_insensitive: true,
    };
    assert_eq!(
        serde_json::to_value(&p).unwrap(),
        json!({"op": "textContains", "value": "abc", "case_insensitive": true})
    );
}

#[test]
fn filter_predicate_isnull_has_no_payload() {
    let p = FilterPredicate::IsNull;
    assert_eq!(serde_json::to_value(&p).unwrap(), json!({"op": "isNull"}));
}

#[test]
fn filter_criteria_empty_by_default() {
    let fc = FilterCriteria::default();
    assert!(fc.is_empty());
}

#[test]
fn filter_criteria_with_predicates_is_not_empty() {
    let fc = FilterCriteria {
        global_search: None,
        predicates: vec![ColumnFilter {
            key: "name".into(),
            predicate: FilterPredicate::TextEquals {
                value: "foo".into(),
                case_insensitive: false,
            },
        }],
    };
    assert!(!fc.is_empty());
}

#[test]
fn directional_enum_serializes_with_kind_amount_field_and_sign() {
    let v = serde_json::to_value(FieldType::DirectionalEnum {
        values: vec![
            DirectionalEnumValue {
                value: 0,
                label_key: "soll".into(),
                wire_name: "SOLL".into(),
                sign: 1,
            },
            DirectionalEnumValue {
                value: 1,
                label_key: "haben".into(),
                wire_name: "HABEN".into(),
                sign: -1,
            },
        ],
        amount_field: "value".into(),
    })
    .unwrap();
    assert_eq!(v["kind"], "directionalEnum");
    assert_eq!(v["amountField"], "value");
    assert_eq!(v["values"][0]["wireName"], "SOLL");
    assert_eq!(v["values"][0]["sign"], 1);
    assert_eq!(v["values"][1]["sign"], -1);
}

#[test]
fn directional_enum_roundtrips() {
    let original = FieldType::DirectionalEnum {
        values: vec![DirectionalEnumValue {
            value: 1,
            label_key: "haben".into(),
            wire_name: "HABEN".into(),
            sign: -1,
        }],
        amount_field: "value".into(),
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: FieldType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, original);
}
