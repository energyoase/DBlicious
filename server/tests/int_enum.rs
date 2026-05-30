//! G7 End-to-End: eine als `FieldType::IntEnum` deklarierte Spalte
//! round-trippt durch create -> fetch als wire_name-String, waehrend die DB
//! den Integer speichert. Unbekannte wire_names werden vom Validierungs-Gate
//! abgelehnt.

use serial_test::serial;
use server::{data, example, fresh_test_setup};

const ENTITY: &str = "journal_entry";

/// Registriert `journal_entry` mit einer einzigen `value_type`-Spalte vom Typ
/// `IntEnum {0 -> SOLL, 1 -> HABEN}` im installierten Set.
fn install_int_enum_column() {
    let column = shared::ColumnMeta {
        key: "value_type".into(),
        label_key: "journal.value-type".into(),
        field_type: shared::FieldType::IntEnum {
            values: vec![
                shared::IntEnumValue {
                    value: 0,
                    label_key: "journal.soll".into(),
                    wire_name: "SOLL".into(),
                },
                shared::IntEnumValue {
                    value: 1,
                    label_key: "journal.haben".into(),
                    wire_name: "HABEN".into(),
                },
            ],
        },
        sortable: true,
        filterable: false,
        comparator_id: None,
        filter_id: None,
        editor_id: None,
        formatter_id: None,
        validator_id: None,
        action_ids: vec![],
    };
    example::mutate(|set| {
        set.entities.insert(
            ENTITY.to_string(),
            example::EntityTypeSet {
                columns: vec![column],
                ..Default::default()
            },
        );
    });
}

#[tokio::test]
#[serial]
async fn create_with_wire_name_persists_int_and_reads_back_as_name() {
    fresh_test_setup().await;
    install_int_enum_column();

    let created = data::create_entity(
        ENTITY,
        None,
        serde_json::json!({ "value_type": "SOLL" }),
        None,
    )
    .await;

    // Read-back als "SOLL" beweist beide Konvertierungen: haette encode den
    // String nicht zu 0 gemacht, scheiterte decode (as_i64) und liefere Null.
    let fetched = data::entity_by_id(ENTITY, &created.id)
        .await
        .expect("entity_by_id");
    assert_eq!(
        fetched.fields.0.get("value_type"),
        Some(&serde_json::Value::String("SOLL".into()))
    );
}

#[tokio::test]
#[serial]
async fn unknown_wire_name_is_a_blocking_validation_error() {
    fresh_test_setup().await;
    install_int_enum_column();

    let fields = serde_json::json!({ "value_type": "MAYBE" })
        .as_object()
        .unwrap()
        .clone();
    let result = data::validate_against_editor(ENTITY, &fields);
    assert!(
        result.has_blocking(),
        "unbekannter wire_name muss die Validierung blockieren"
    );
}
