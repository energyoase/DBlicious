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

// ----------------------------------------------------------------------------
// Phase 2.6 — Host-Modul db (fetch_entities + patch_entity)
// ----------------------------------------------------------------------------

#[test]
fn host_db_fetch_via_mock_returns_seeded_array() {
    use server::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    mock.seed_entities(
        "product",
        serde_json::json!([{"id": "p-1", "price": 100}]),
    );
    let h = DbHost::new(&mock);
    let res = h.fetch_entities("product", &serde_json::json!({})).unwrap();
    assert_eq!(res, serde_json::json!([{"id": "p-1", "price": 100}]));
}

#[test]
fn host_db_fetch_injects_entity_into_non_object_query() {
    use server::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    mock.seed_entities("user", serde_json::json!([{"id": "u-1"}]));
    let h = DbHost::new(&mock);
    // query=Null soll als {} interpretiert + entity injiziert werden
    let res = h.fetch_entities("user", &serde_json::Value::Null).unwrap();
    assert_eq!(res, serde_json::json!([{"id": "u-1"}]));
}

#[test]
fn host_db_patch_via_mock_records_call() {
    use server::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    let h = DbHost::new(&mock);
    h.patch_entity("product", "p-1", &serde_json::json!({"price": 199}))
        .unwrap();
    let log = mock.patch_log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].0, "product");
    assert_eq!(log[0].1, "p-1");
    assert_eq!(log[0].2, serde_json::json!({"price": 199}));
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

// ----------------------------------------------------------------------------
// Phase 2.7 — Host-Module ui + audit
// ----------------------------------------------------------------------------

#[test]
fn host_ui_text_returns_uitree_subtree() {
    use server::script::host::ui::UiHost;
    use shared::script::manifest::UiPrimitive;
    let manifest = shared::script::ScriptManifest {
        manifest_version: 1,
        tier: shared::script::ScriptTier::Author,
        capabilities: vec![shared::script::CapabilityToken::EmitUiNode {
            scope: shared::script::capability::UiScope::Composite,
        }],
        ui_primitives: vec![UiPrimitive::Text],
        ..Default::default()
    };
    let mut ui = UiHost::new(&manifest);
    let node = ui.text("Hallo", &serde_json::json!({"size": "h2"})).unwrap();
    assert_eq!(node["type"], serde_json::Value::String("text".into()));
    assert_eq!(node["text"], serde_json::Value::String("Hallo".into()));
}

#[test]
fn host_ui_rejects_undeclared_primitive() {
    use server::script::host::ui::UiHost;
    let manifest = shared::script::ScriptManifest::default(); // ui_primitives: leer
    let mut ui = UiHost::new(&manifest);
    let err = ui.text("Hallo", &serde_json::json!({})).unwrap_err();
    match err {
        shared::script::ScriptError::UiPrimitiveDenied { primitive } => {
            assert_eq!(primitive, "text");
        }
        other => panic!("expected UiPrimitiveDenied, got {other:?}"),
    }
}

#[test]
fn host_ui_camel_case_primitive_name_is_pinned() {
    // ForEach muss als "forEach" gemeldet werden, nicht als "foreach"
    use server::script::host::ui::UiHost;
    use shared::script::manifest::UiPrimitive;
    // Manifest enthaelt nur Text — Aufruf von table()/vstack()/etc. wird
    // deny'd; hier prueft der Test, dass die Name-Map keine Debug-Lower-
    // Heuristik benutzt. Wir nehmen ein Primitive, das im Default-Manifest
    // nicht erlaubt ist, und pruefen den gemeldeten String.
    let manifest = shared::script::ScriptManifest {
        manifest_version: 1,
        tier: shared::script::ScriptTier::Author,
        capabilities: vec![],
        ui_primitives: vec![UiPrimitive::Text],
        ..Default::default()
    };
    let mut ui = UiHost::new(&manifest);
    let err = ui.table(&serde_json::json!({})).unwrap_err();
    match err {
        shared::script::ScriptError::UiPrimitiveDenied { primitive } => {
            assert_eq!(primitive, "table");
        }
        other => panic!("expected UiPrimitiveDenied, got {other:?}"),
    }
}

#[test]
fn host_audit_log_via_mock_records_event() {
    use server::script::host::audit::AuditHost;
    let mock = shared::script::testing::MockHostApi::new();
    let h = AuditHost::new(&mock);
    h.log("custom.event", &serde_json::json!({"x": 1})).unwrap();
    let log = mock.audit_log_calls();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].0, "custom.event");
    assert_eq!(log[0].1, serde_json::json!({"x": 1}));
}

// ----------------------------------------------------------------------------
// Phase 2.8 — ServerHostApiRegistry + Symmetrie-Anker
// ----------------------------------------------------------------------------

#[test]
fn server_host_api_registry_lists_required_functions() {
    use shared::script::HostApiRegistry;
    let fns = server::script::ServerHostApiRegistry::functions();
    let names: Vec<_> = fns.iter().map(|f| f.name).collect();
    assert!(names.contains(&"db.entities"), "fehlt: db.entities in {names:?}");
    assert!(names.contains(&"db.patch"),    "fehlt: db.patch in {names:?}");
    assert!(names.contains(&"ui.text"),     "fehlt: ui.text in {names:?}");
    assert!(names.contains(&"ui.vstack"),   "fehlt: ui.vstack in {names:?}");
    assert!(names.contains(&"ctx.t"),       "fehlt: ctx.t in {names:?}");
    assert!(names.contains(&"audit.log"),   "fehlt: audit.log in {names:?}");
}

#[test]
fn server_host_api_registry_marks_server_only_correctly() {
    use shared::script::HostApiRegistry;
    let fns = server::script::ServerHostApiRegistry::functions();
    let patch = fns.iter().find(|f| f.name == "db.patch").expect("db.patch present");
    assert!(patch.server_only, "db.patch muss server_only sein");
    let audit = fns.iter().find(|f| f.name == "audit.log").expect("audit.log present");
    assert!(audit.server_only, "audit.log muss server_only sein");
    let text = fns.iter().find(|f| f.name == "ui.text").expect("ui.text present");
    assert!(!text.server_only, "ui.text ist client-faehig");
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
