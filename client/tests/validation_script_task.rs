//! System-Test (Q0014, Lücke A): beweist, dass `validator_id =
//! "script:d2v_balance_validator"` als TaskFn durch das echte
//! `ValidationSystem::run` laeuft (reale Rhai-Engine, kein Mock-Praedikat).

use std::sync::Arc;

use client::script::registry::ScriptRegistry;
use client::validation::script_task::script_validator_task;
use client::validation::{ValidationSystem, ValidationTask};
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

fn fields(v: f64, vt: &str, pv: f64, pt: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    m.insert("value".into(), serde_json::json!(v));
    m.insert("valueType".into(), serde_json::json!(vt));
    m.insert("partnerValue".into(), serde_json::json!(pv));
    m.insert("partnerValueType".into(), serde_json::json!(pt));
    m
}

fn build_system() -> ValidationSystem {
    let reg = ScriptRegistry::new();
    reg.insert(validator_script("d2v_balance_validator", BALANCE_SRC));
    let host = Arc::new(MockHostApi::new());
    let task = script_validator_task(
        "value",
        "script:d2v_balance_validator",
        Arc::new(reg),
        host,
        ScriptCtx::default(),
    )
    .expect("script validator task should be constructible");
    let mut sys = ValidationSystem::new();
    sys.register(
        "datev_entry",
        ValidationTask {
            target: "value".into(),
            task,
        },
    );
    sys
}

#[test]
fn balanced_fields_produce_no_validation_error() {
    let sys = build_system();
    // SOLL 100 vs HABEN 100 -> balanciert -> true -> kein Fehler.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 100.0, "HABEN"));
    assert!(res.is_empty(), "balanced should be valid, got: {res:?}");
}

#[test]
fn unbalanced_fields_block_validation() {
    let sys = build_system();
    // SOLL 100 vs HABEN 90 -> unbalanciert -> false -> blockierender Fehler.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 90.0, "HABEN"));
    assert!(!res.is_empty(), "unbalanced should produce an error");
    assert!(res.has_blocking(), "validator error must block");
}
