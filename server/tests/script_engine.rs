//! Engine + Sandbox-Tests (Server-Seite). Pendant in
//! `client/tests/script_engine.rs` (Phase 4).
//!
//! Tests sind synchron (kein `tokio::test`): die Rhai-Engine ist
//! synchron, und die `MockHostApi` aus `shared::script::testing` ebenfalls.
//! Damit braucht diese Datei kein `serial_test`-Setup und beruehrt den
//! prozessweiten DB-Pool nicht.

use shared::script::engine::ScriptEngine;
use shared::script::{CapabilityToken, ScriptManifest, ScriptTier};

// ----------------------------------------------------------------------------
// Phase 2.1 — Rhai-Engine-Wrapper hinter dem ScriptEngine-Trait
// ----------------------------------------------------------------------------

#[test]
fn rhai_engine_compiles_trivial_script() {
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile("40 + 2", &manifest).expect("compile");
    // AST darf nicht-leer sein — keine API zum Inspect, also Existenz reicht:
    let _ = ast;
}

#[test]
fn rhai_engine_rejects_eval_symbol() {
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let res = engine.compile("eval(\"40+2\")", &manifest);
    assert!(res.is_err(), "eval() darf NICHT kompilieren");
}

// ----------------------------------------------------------------------------
// Phase 2.2 — WasmEngineNotAvailable-Pin im Compile-Pfad
// ----------------------------------------------------------------------------

#[test]
fn rhai_engine_rejects_wasm_kind_with_dedicated_error() {
    use shared::script::ScriptKind;
    let err = server::script::engine::rhai::compile_wasm(&ScriptKind::Wasm {
        wasm_bytes: vec![0, 1, 2],
        entry: "main".into(),
    })
    .unwrap_err();
    assert!(matches!(
        err,
        shared::script::ScriptError::WasmEngineNotAvailable
    ));
}
