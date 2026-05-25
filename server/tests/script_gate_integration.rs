//! Gate-Integrations-Test (Q0009 B3/B1).
//!
//! Beweist, dass die Sandbox-Gate im ECHTEN Engine-Ausfuehrungspfad feuert —
//! nicht nur isoliert in `Sandbox::gate()` (das deckt `script_engine.rs` ab).
//! Der Q0009-Review (`docs/reviews/Q0009-review.md`, Blocker B3) vermisste
//! genau diesen Nachweis: "kein Test beweist, dass die Gate bei echter
//! Engine-Ausfuehrung feuert".
//!
//! Geprueft wird der Pfad ueber `RhaiEngine::with_manifest(...)` → `run()`,
//! der die Host-Funktionen (`db.entities`) durch `sandbox.gate(token, …)`
//! wickelt:
//!   - B3: ein `db.entities(...)`-Aufruf scheitert ohne das noetige
//!     Capability-Token (`CapabilityDenied`) bzw. liefert mit Token die
//!     gemockten Daten.
//!   - B1: `CapabilityDenied` ist unmaskable → als `ErrorTerminated`
//!     propagiert und per `try`/`catch` NICHT fangbar.
//!
//! Synchron (kein `tokio::test`): Engine + `MockHostApi` sind synchron.

use std::sync::Arc;

use shared::script::engine::{ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::testing::MockHostApi;
use shared::script::{CapabilityToken, ScriptError, ScriptManifest, ScriptTier};

/// Reader-Manifest mit genau den uebergebenen Capabilities.
fn manifest_with(caps: Vec<CapabilityToken>) -> ScriptManifest {
    ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: caps,
        ..Default::default()
    }
}

#[test]
fn db_call_without_token_is_denied_at_real_execution() {
    // B3: Manifest OHNE ReadOwnEntities — der db.entities()-Call im echten
    // eval-Pfad muss durch die Gate mit CapabilityDenied scheitern.
    let manifest = manifest_with(vec![CapabilityToken::ComputeOnly]);
    let engine = server::script::engine::RhaiEngine::with_manifest(&manifest);
    let ast = engine
        .compile(r#"db.entities("product")"#, &manifest)
        .expect("compile");

    let host = Arc::new(MockHostApi::new());
    let res = engine.run(&ast, host, ScriptCtx::default());

    match res {
        Err(ScriptError::CapabilityDenied { token }) => {
            assert_eq!(token, CapabilityToken::ReadOwnEntities);
        }
        other => panic!("erwartete CapabilityDenied(ReadOwnEntities), war: {other:?}"),
    }
}

#[test]
fn db_call_with_token_returns_seeded_data() {
    // B3-Gegenprobe: MIT ReadOwnEntities laeuft der Call durch die Gate und
    // liefert die gemockten Daten (als JSON-String, vgl. gated_db_fetch).
    let manifest = manifest_with(vec![CapabilityToken::ReadOwnEntities]);
    let engine = server::script::engine::RhaiEngine::with_manifest(&manifest);
    let ast = engine
        .compile(r#"db.entities("product")"#, &manifest)
        .expect("compile");

    let host = Arc::new(MockHostApi::new());
    host.seed_entities("product", serde_json::json!([{"id": "p-1", "price": 100}]));
    let res = engine.run(&ast, host, ScriptCtx::default());

    match res {
        Ok(ScriptValue::String(s)) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&s).expect("Ergebnis muss gueltiges JSON sein");
            assert_eq!(parsed, serde_json::json!([{"id": "p-1", "price": 100}]));
        }
        other => panic!("erwartete Ok(String(json-array)), war: {other:?}"),
    }
}

#[test]
fn capability_denied_is_not_catchable_via_try_catch() {
    // B1: CapabilityDenied ist unmaskable → wird als ErrorTerminated
    // propagiert und darf per try/catch NICHT gefangen werden. Das Skript
    // versucht, den Denial wegzufangen und 42 zurueckzugeben; der Run muss
    // trotzdem mit CapabilityDenied abbrechen (nicht Ok(42)).
    let manifest = manifest_with(vec![CapabilityToken::ComputeOnly]);
    let engine = server::script::engine::RhaiEngine::with_manifest(&manifest);
    let ast = engine
        .compile(
            r#"let x = 0; try { db.entities("product"); x = 1 } catch(e) { x = 42 } x"#,
            &manifest,
        )
        .expect("compile");

    let host = Arc::new(MockHostApi::new());
    let res = engine.run(&ast, host, ScriptCtx::default());

    assert!(
        !matches!(&res, Ok(ScriptValue::Number(n)) if *n == 42.0),
        "unmaskable CapabilityDenied wurde via catch gefangen (Ergebnis 42) — B1 verletzt: {res:?}"
    );
    assert!(
        matches!(res, Err(ScriptError::CapabilityDenied { .. })),
        "erwartete CapabilityDenied (uncatchbar)"
    );
}
