//! Integrationstest fuer den Provider-Registry-Lookup (Q0009 Phase 5.2).
//!
//! Symmetrie-Vertrag: dieselbe Test-Fixture (ein Formatter-Skript, das `40+2`
//! liefert) muss auch auf der Server-Seite (`server/tests/script_provider_lookup.rs`)
//! das gleiche Ergebnis produzieren. Damit ist der Vertrag "ein Provider-
//! Skript liefert in beiden Welten identisch" reproduzierbar.

use client::script::audit_queue::{drain, FallbackReason};
use client::script::provider_lookup::{
    lookup_provider, parse_script_id, script_value_to_display, LookupResult, SCRIPT_PREFIX,
};
use client::script::registry::ScriptRegistry;
use shared::script::engine::{ScriptCtx, ScriptInputs, ScriptValue};
use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
use shared::script::testing::MockHostApi;
use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptTier};

/// Baut ein Provider-Formatter-Skript mit Reader-Tier + ComputeOnly-
/// Capability. Reicht fuer Tests, die `40 + 2`-Style-Ausdruecke evaluieren.
fn formatter_script(id: &str, source: &str, state: ScriptState) -> Script {
    Script {
        id: ScriptId(id.into()),
        kind: ScriptKind::Provider {
            slot: ProviderSlot::Formatter,
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly],
            ..Default::default()
        },
        source: source.into(),
        version: 1,
        state,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    }
}

#[test]
fn script_prefix_constant_is_lowercase_and_terminated_with_colon() {
    // Wire-Form-Pin: andere Crates verlassen sich auf die Form.
    assert_eq!(SCRIPT_PREFIX, "script:");
}

#[test]
fn parse_script_id_strips_known_prefix() {
    assert_eq!(
        parse_script_id("script:disc"),
        Some(ScriptId("disc".into()))
    );
    assert_eq!(parse_script_id("text"), None);
    assert_eq!(parse_script_id("scripts:wrong"), None);
}

#[test]
fn provider_lookup_returns_value_for_active_formatter() {
    let reg = ScriptRegistry::new();
    reg.insert(formatter_script(
        "discount-tier",
        "40 + 2",
        ScriptState::Active,
    ));
    let host = std::sync::Arc::new(MockHostApi::new());
    let res = lookup_provider(
        "script:discount-tier",
        ProviderSlot::Formatter,
        &reg,
        host.clone(),
        ScriptCtx::default(),
        ScriptInputs::default(),
    );
    match res {
        LookupResult::Ok { value } => {
            // Symmetrie-Pin: gleicher Display-String wie auf der Server-Seite.
            assert_eq!(script_value_to_display(&value), "42");
            // ScriptValue-Variante: i64 wird als Number(f64) gefuehrt.
            match value {
                ScriptValue::Number(n) => assert!((n - 42.0).abs() < 1e-9),
                other => panic!("expected Number(42), got {other:?}"),
            }
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[test]
fn provider_lookup_falls_back_when_state_locked() {
    let reg = ScriptRegistry::new();
    reg.insert(formatter_script("loc", "42", ScriptState::Locked));
    let host = std::sync::Arc::new(MockHostApi::new());
    let res = lookup_provider(
        "script:loc",
        ProviderSlot::Formatter,
        &reg,
        host.clone(),
        ScriptCtx::default(),
        ScriptInputs::default(),
    );
    assert!(matches!(
        res,
        LookupResult::Fallback {
            reason: FallbackReason::NotActive { .. }
        }
    ));
}

#[test]
fn provider_lookup_static_id_signals_not_a_script() {
    let reg = ScriptRegistry::new();
    let host = std::sync::Arc::new(MockHostApi::new());
    let res = lookup_provider(
        "text-plain",
        ProviderSlot::Formatter,
        &reg,
        host.clone(),
        ScriptCtx::default(),
        ScriptInputs::default(),
    );
    assert!(matches!(res, LookupResult::NotAScriptId));
}

#[test]
fn provider_lookup_logs_fallback_into_audit_queue() {
    // Vor dem Test drainen, damit fremde Tests die Queue nicht stoeren.
    let _ = drain();

    let reg = ScriptRegistry::new();
    let host = std::sync::Arc::new(MockHostApi::new());
    let _ = lookup_provider(
        "script:does-not-exist-12345",
        ProviderSlot::Formatter,
        &reg,
        host.clone(),
        ScriptCtx::default(),
        ScriptInputs::default(),
    );

    let drained = drain();
    let mine: Vec<_> = drained
        .iter()
        .filter(|e| {
            e.payload
                .get("scriptId")
                .and_then(|v| v.as_str())
                .map(|s| s == "does-not-exist-12345")
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(mine.len(), 1, "Fallback muss eine Audit-Zeile schreiben");
    assert_eq!(mine[0].event, "script.provider.fallback.missing");
}
