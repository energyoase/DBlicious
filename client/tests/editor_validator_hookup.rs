//! Q0017 (Stage-3 von Q0014): beweist, dass `register_script_validators` —
//! die Funktion, die der Editor-Load benutzt — einen `script:`-Validator aus
//! einer `ColumnMeta` in das echte `ValidationSystem` registriert, sodass ein
//! verletzender Datensatz blockiert und ein valider durchlaeuft. Reale
//! Rhai-Engine, kein Mock-Praedikat; kein Leptos-Mount noetig.

use std::sync::Arc;

use client::script::registry::ScriptRegistry;
use client::validation::script_task::register_script_validators;
use client::validation::ValidationSystem;
use shared::script::engine::ScriptCtx;
use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
use shared::script::testing::MockHostApi;
use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptTier};

const BALANCE_SRC: &str = include_str!("../../examples/d2v/scripts/d2v_balance_validator.rhai");

fn validator_script(id: &str, source: &str) -> Script {
    Script {
        id: ScriptId(id.into()),
        kind: ScriptKind::Provider {
            slot: ProviderSlot::Validator,
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly, CapabilityToken::ReadI18n],
            ..Default::default()
        },
        source: source.into(),
        version: 1,
        state: ScriptState::Active,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    }
}

fn col(key: &str, validator_id: Option<&str>) -> shared::ColumnMeta {
    shared::ColumnMeta {
        key: key.into(),
        label_key: format!("field.{key}"),
        field_type: shared::FieldType::Text,
        sortable: false,
        filterable: false,
        comparator_id: None,
        filter_id: None,
        editor_id: None,
        formatter_id: None,
        validator_id: validator_id.map(str::to_string),
        action_ids: Vec::new(),
    }
}

fn fields(v: f64, vt: &str, pv: f64, pt: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    m.insert("value".into(), serde_json::json!(v));
    m.insert("valueType".into(), serde_json::json!(vt));
    m.insert("partnerValue".into(), serde_json::json!(pv));
    m.insert("partnerValueType".into(), serde_json::json!(pt));
    m
}

/// Baut ein `ValidationSystem` ueber den Editor-Pfad: Columns -> register.
fn build_system() -> ValidationSystem {
    let reg = ScriptRegistry::new();
    reg.insert(validator_script("d2v_balance_validator", BALANCE_SRC));
    let host = Arc::new(MockHostApi::new());
    let columns = vec![
        col("value", Some("script:d2v_balance_validator")),
        // Negativ-Kontrolle: kein Validator -> kein Task.
        col("description", None),
        // Negativ-Kontrolle: nicht-script-Prefix -> kein Task.
        col("key", Some("static-x")),
    ];
    let mut sys = ValidationSystem::new();
    register_script_validators(
        &mut sys,
        "datev_entry",
        &columns,
        Arc::new(reg),
        host,
        ScriptCtx::default(),
    );
    sys
}

#[test]
fn unbalanced_record_is_blocked() {
    let sys = build_system();
    // SOLL 100 vs HABEN 90 -> unbalanciert -> false -> blockierender Fehler.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 90.0, "HABEN"));
    assert!(res.has_blocking(), "unbalanced record must block save");
    let msg = res
        .messages
        .iter()
        .find(|m| m.target.as_deref() == Some("value"))
        .expect("there must be a message targeting `value`");
    assert_eq!(
        msg.message_key, "validation.script",
        "script validator uses SCRIPT_VALIDATION_KEY"
    );
}

#[test]
fn balanced_record_passes() {
    let sys = build_system();
    // SOLL 100 vs HABEN 100 -> balanciert -> true -> kein Fehler.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 100.0, "HABEN"));
    assert!(res.is_empty(), "balanced record must pass, got: {res:?}");
}

#[test]
fn non_script_and_missing_validator_register_no_task() {
    // Nur die `value`-Column erzeugt einen Task; description/key nicht.
    let sys = build_system();
    // Balanciert -> wenn nur der eine value-Task laeuft, ist das Ergebnis leer.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 100.0, "HABEN"));
    assert!(
        res.is_empty(),
        "non-script and missing validators must not add tasks"
    );
}
