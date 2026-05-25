//! Loader-Tests fuer `examples/d2v/` (D2V Daten-Port, Track A).
//!
//! Verifiziert, dass alle 17 Entity-Metadaten korrekt parsen und die
//! per-entity Bindings strukturell stimmen (Source, Locator-Form,
//! PK-Arity, Read-Only-Flag, columnMap-Lueckenlosigkeit). Kein DB-IO —
//! diese Tests pruefen NUR den Datei-Loader.

use std::path::{Path, PathBuf};

use server::example;
use shared::source::BindingLocator;

fn d2v_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("d2v")
}

const EXPECTED_ENTITY_TYPES: &[&str] = &[
    "company",
    "datev_account",
    "datev_account_entry",
    "datev_calculation",
    "datev_calculation_entry",
    "datev_calculation_value",
    "datev_entry",
    "datev_entry_change_tracking",
    "datev_entry_group",
    "datev_entry_stack",
    "star_money_account",
    "star_money_bank",
    "star_money_booking_text",
    "star_money_credit_card",
    "star_money_credit_card_entry",
    "star_money_entry",
    "susa_entry",
];

#[test]
fn d2v_loads_all_17_entities() {
    let set = example::load(&d2v_dir()).expect("examples/d2v muss ladbar sein");

    let types: Vec<&str> = set.entity_types().collect();
    assert_eq!(
        types.len(),
        EXPECTED_ENTITY_TYPES.len(),
        "Entity-Count weicht ab: {:?}",
        types
    );
    for required in EXPECTED_ENTITY_TYPES {
        assert!(
            types.contains(required),
            "Entity-Type '{required}' fehlt: {:?}",
            types
        );
    }
}

#[test]
fn d2v_every_entity_has_columns_editor_settings_and_binding() {
    let set = example::load(&d2v_dir()).expect("laden");
    for ty in EXPECTED_ENTITY_TYPES {
        let e = set
            .entities
            .get(*ty)
            .unwrap_or_else(|| panic!("'{ty}' fehlt"));
        assert!(!e.columns.is_empty(), "{ty}: columns leer");
        assert!(e.editor.is_some(), "{ty}: editor fehlt");
        let settings = e
            .settings
            .as_ref()
            .unwrap_or_else(|| panic!("{ty}: settings fehlt"));
        let binding = settings
            .binding
            .as_ref()
            .unwrap_or_else(|| panic!("{ty}: binding fehlt (binding.json nicht erkannt?)"));
        assert_eq!(binding.source, "d2v_legacy", "{ty}: falsche Source");
        match &binding.locator {
            BindingLocator::Table { table } => {
                assert!(!table.is_empty(), "{ty}: Table-Name leer");
            }
            other => panic!("{ty}: Locator muss Table sein, war {other:?}"),
        }
    }
}

#[test]
fn d2v_composite_pks_are_correct() {
    let set = example::load(&d2v_dir()).expect("laden");
    let cases = [
        (
            "datev_account_entry",
            &["entryId", "accountNr", "offsetAccountNr"][..],
        ),
        (
            "datev_calculation_value",
            &["calculationId", "year", "method"][..],
        ),
        ("star_money_account", &["bankCode", "code"][..]),
        ("susa_entry", &["accountNr", "year"][..]),
    ];
    for (ty, expected) in cases {
        let pk = &set.entities[ty]
            .settings
            .as_ref()
            .unwrap()
            .binding
            .as_ref()
            .unwrap()
            .primary_key;
        assert_eq!(pk.as_slice(), expected, "{ty}: PK weicht ab");
    }
}

#[test]
fn d2v_read_only_bindings_match_spec() {
    let set = example::load(&d2v_dir()).expect("laden");
    let read_only = [
        "company",                      // EXPERIMENTAL
        "datev_entry_group",            // EVAL
        "datev_entry_change_tracking",  // EVAL
        "star_money_booking_text",      // LEGACY-IMPORT
        "star_money_credit_card",       // EXPERIMENTAL
        "star_money_credit_card_entry", // EXPERIMENTAL
    ];
    for ty in read_only {
        let ro = set.entities[ty]
            .settings
            .as_ref()
            .unwrap()
            .binding
            .as_ref()
            .unwrap()
            .read_only;
        assert!(ro, "{ty}: erwartet readOnly=true");
    }
    // Stichprobe: ACTIVE-Entity ist NICHT read-only.
    let ro_active = set.entities["datev_account"]
        .settings
        .as_ref()
        .unwrap()
        .binding
        .as_ref()
        .unwrap()
        .read_only;
    assert!(!ro_active, "datev_account muss editierbar sein");
}

#[test]
fn d2v_column_map_covers_every_column() {
    let set = example::load(&d2v_dir()).expect("laden");
    for ty in EXPECTED_ENTITY_TYPES {
        let e = &set.entities[*ty];
        let map = &e
            .settings
            .as_ref()
            .unwrap()
            .binding
            .as_ref()
            .unwrap()
            .column_map;
        for col in &e.columns {
            assert!(
                map.contains_key(&col.key),
                "{ty}: columnMap-Eintrag fehlt fuer Spalte '{}'",
                col.key
            );
        }
    }
}

#[test]
fn d2v_navigation_has_six_top_level_groups() {
    let set = example::load(&d2v_dir()).expect("laden");
    assert_eq!(
        set.navigation.len(),
        6,
        "Erwarte 6 Top-Level-Nav-Gruppen, fand {}",
        set.navigation.len()
    );
}

#[test]
fn d2v_groups_grant_read_to_every_entity() {
    let set = example::load(&d2v_dir()).expect("laden");
    let bookkeepers = set
        .groups
        .iter()
        .find(|g| g.id == "g-bookkeepers")
        .expect("g-bookkeepers Gruppe fehlt");
    for ty in EXPECTED_ENTITY_TYPES {
        assert!(
            bookkeepers
                .permissions
                .iter()
                .any(|p| p.entity_type == *ty && p.can_read),
            "g-bookkeepers fehlt canRead fuer '{ty}'"
        );
    }
}

#[test]
fn d2v_datev_entry_value_type_is_directional_enum() {
    // D2V ValueType (EF-Core-Enum: DEBIT/SOLL=1, CREDIT/HABEN=2) traegt das
    // Vorzeichen, das `value` in der Saldo-Aggregation (Welle 2) gewichtet:
    // SOLL = +1, HABEN = -1 (siehe D2V `AsStarMoneyValue`).
    let set = example::load(&d2v_dir()).expect("laden");
    let col = set.entities["datev_entry"]
        .columns
        .iter()
        .find(|c| c.key == "valueType")
        .expect("datev_entry: valueType-Spalte fehlt");
    match &col.field_type {
        shared::FieldType::DirectionalEnum {
            values,
            amount_field,
        } => {
            assert_eq!(amount_field, "value", "amountField muss auf 'value' zeigen");
            let soll = values
                .iter()
                .find(|v| v.wire_name == "SOLL")
                .expect("SOLL-Wert fehlt");
            assert_eq!(soll.value, 1, "SOLL = DB-int 1");
            assert_eq!(soll.sign, 1, "SOLL = +1");
            let haben = values
                .iter()
                .find(|v| v.wire_name == "HABEN")
                .expect("HABEN-Wert fehlt");
            assert_eq!(haben.value, 2, "HABEN = DB-int 2");
            assert_eq!(haben.sign, -1, "HABEN = -1");
        }
        other => panic!("valueType muss DirectionalEnum sein, war {other:?}"),
    }
}
