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

// ----------------------------------------------------------------------------
// Phase 2.3 — Sandbox gate() + Token-Audit-Buffer
// ----------------------------------------------------------------------------

#[test]
fn sandbox_denies_call_when_token_not_in_manifest() {
    use server::script::sandbox::Sandbox;
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ReadOwnEntities],
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    let res = sb.gate(
        &CapabilityToken::WriteEntity { validated: true },
        || Ok::<_, shared::script::ScriptError>(42),
    );
    assert!(matches!(
        res,
        Err(shared::script::ScriptError::CapabilityDenied { .. })
    ));
    assert_eq!(
        sb.token_uses().len(),
        1,
        "Audit-Buffer haelt auch denials fest"
    );
}

#[test]
fn sandbox_records_successful_token_use() {
    use server::script::sandbox::Sandbox;
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ReadOwnEntities],
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    let v = sb
        .gate(&CapabilityToken::ReadOwnEntities, || {
            Ok::<_, shared::script::ScriptError>(7)
        })
        .unwrap();
    assert_eq!(v, 7);
    assert_eq!(sb.token_uses().len(), 1);
}

#[test]
fn sandbox_catches_panic_in_host_body() {
    use server::script::sandbox::Sandbox;
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    let res: Result<i32, _> = sb.gate(&CapabilityToken::ComputeOnly, || {
        panic!("absichtlicher panic");
    });
    assert!(matches!(
        res,
        Err(shared::script::ScriptError::InternalPanic { .. })
    ));
}

// ----------------------------------------------------------------------------
// Phase 2.4 — Negative-Tests pro Sandbox-Constraint
// ----------------------------------------------------------------------------

#[test]
fn timeout_constraint_fires_when_deadline_exceeded() {
    use server::script::sandbox::Sandbox;
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        timeout_ms: Some(0), // sofort abgelaufen
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let res = sb.gate(&CapabilityToken::ComputeOnly, || {
        Ok::<_, shared::script::ScriptError>(1)
    });
    assert!(matches!(
        res,
        Err(shared::script::ScriptError::Timeout { .. })
    ));
}

#[test]
fn engine_rejects_print_and_debug_symbols() {
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    assert!(
        engine.compile("print(\"x\")", &manifest).is_err(),
        "print darf NICHT kompilieren"
    );
    assert!(
        engine.compile("debug(\"x\")", &manifest).is_err(),
        "debug darf NICHT kompilieren"
    );
    assert!(
        engine.compile("import \"foo\" as bar;", &manifest).is_err(),
        "import darf NICHT kompilieren"
    );
}

// ----------------------------------------------------------------------------
// Phase 2.5 — Host-Module i18n + ctx (read-only)
// ----------------------------------------------------------------------------

#[test]
fn host_i18n_t_uses_host_api_translation() {
    use server::script::host::i18n::I18nHost;
    let mock = shared::script::testing::MockHostApi::new();
    let h = I18nHost::new(&mock);
    let s = h.t("dashboard.title", &serde_json::json!({})).unwrap();
    assert_eq!(s, "[t:dashboard.title]"); // MockHostApi-Konvention
}

#[test]
fn host_ctx_returns_read_only_fields_from_script_ctx() {
    use server::script::host::ctx::CtxHost;
    use shared::script::engine::ScriptCtx;
    let h = CtxHost::new(ScriptCtx {
        user_id: Some("u-1".into()),
        tenant_id: Some("t-1".into()),
        locale: "de".into(),
    });
    assert_eq!(h.user_id(), Some("u-1"));
    assert_eq!(h.tenant_id(), Some("t-1"));
    assert_eq!(h.locale(), "de");
    // now() ist nicht-deterministisch, aber muss RFC-3339-Form haben:
    let now = h.now();
    assert!(now.contains('T'), "RFC-3339 expected: {now}");
}

#[test]
fn engine_max_operations_kicks_in_on_runaway_loop() {
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine
        .compile(
            "let i = 0; while i < 1_000_000_000 { i = i + 1; } i",
            &manifest,
        )
        .expect("compile");
    let host = shared::script::testing::MockHostApi::new();
    let res = engine.run(&ast, &host, shared::script::engine::ScriptCtx::default());
    // Wir akzeptieren JEDE Error-Variante (Operations, Timeout) — wichtig
    // ist nur, dass die Schleife nicht erfolgreich durchlaeuft.
    assert!(res.is_err(), "Endlosschleife muss abbrechen");
}
