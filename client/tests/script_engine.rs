//! Engine + Sandbox + Host-Tests (Client-Seite). Pendant zu
//! `server/tests/script_engine.rs` (Phase 2 / 3).
//!
//! Diese Tests laufen unter `cargo test --workspace` — sie sind native
//! Rust-Tests, weil die Rhai-Engine synchron ist und keine Browser-APIs
//! braucht. Der WASM-spezifische Pfad in `sandbox.rs` (Performance-now-
//! basierte Deadline) wird hier nicht direkt geuebt; das Operation-Limit
//! der Engine schliesst die wichtigste Lücke (vgl. den Runaway-Loop-Test
//! ganz unten).
//!
//! Bewusst kein `#[cfg(target_arch = "wasm32")]`-Block: diese Tests sollen
//! im standard Cargo-Lauf greifen, nicht hinter `wasm-pack` versteckt sein
//! (sonst wuerden sie in CI nicht ohne Browser laufen).

use shared::script::engine::ScriptEngine;
use shared::script::{CapabilityToken, ScriptManifest, ScriptTier};

// ----------------------------------------------------------------------------
// Phase 4.1 — Rhai-Engine-Wrapper hinter dem ScriptEngine-Trait (Client)
// ----------------------------------------------------------------------------

#[test]
fn rhai_engine_compiles_trivial_script() {
    let engine = client::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile("40 + 2", &manifest).expect("compile");
    let _ = ast;
}

#[test]
fn rhai_engine_actually_evaluates_arithmetic_and_array_ops() {
    // B2-Regression (Client-Pendant): die fuenf Sprach-Packages muessen
    // geladen sein, sonst fehlt selbst `.len()` auf Arrays.
    use shared::script::engine::{ScriptCtx, ScriptValue};
    let engine = client::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let host = std::sync::Arc::new(shared::script::testing::MockHostApi::new());

    let ast = engine.compile("40 + 2", &manifest).expect("compile");
    assert_eq!(
        engine.run(&ast, host.clone(), ScriptCtx::default()).expect("eval"),
        ScriptValue::Number(42.0)
    );

    let ast2 = engine
        .compile("let xs = [1, 2, 3]; if xs.len() == 3 { \"ok\" } else { \"no\" }", &manifest)
        .expect("compile");
    assert_eq!(
        engine.run(&ast2, host.clone(), ScriptCtx::default()).expect("eval array"),
        ScriptValue::String("ok".into())
    );
}

#[test]
fn rhai_engine_rejects_eval_symbol() {
    let engine = client::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let res = engine.compile("eval(\"40+2\")", &manifest);
    assert!(res.is_err(), "eval() darf NICHT kompilieren");
}

#[test]
fn rhai_engine_rejects_print_and_debug_and_import() {
    let engine = client::script::engine::RhaiEngine::new();
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

#[test]
fn rhai_engine_rejects_wasm_kind_with_dedicated_error() {
    use shared::script::ScriptKind;
    let err = client::script::engine::rhai::compile_wasm(&ScriptKind::Wasm {
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
// Phase 4.2 — Sandbox gate() + Token-Audit-Buffer (Client-Pendant)
// ----------------------------------------------------------------------------

#[test]
fn sandbox_denies_call_when_token_not_in_manifest() {
    use client::script::sandbox::Sandbox;
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ReadOwnEntities],
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    let res = sb.gate(&CapabilityToken::WriteEntity { validated: true }, || {
        Ok::<_, shared::script::ScriptError>(42)
    });
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
    use client::script::sandbox::Sandbox;
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
    use client::script::sandbox::Sandbox;
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

#[test]
fn sandbox_timeout_constraint_fires_when_deadline_exceeded() {
    use client::script::sandbox::Sandbox;
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

// ----------------------------------------------------------------------------
// Phase 4.3 — Host-Module gegen MockHostApi (Symmetrie zu server/tests/...)
// ----------------------------------------------------------------------------

#[test]
fn host_i18n_t_uses_host_api_translation() {
    use client::script::host::i18n::I18nHost;
    let mock = shared::script::testing::MockHostApi::new();
    let h = I18nHost::new(&mock);
    let s = h.t("dashboard.title", &serde_json::json!({})).unwrap();
    assert_eq!(s, "[t:dashboard.title]"); // MockHostApi-Konvention
}

#[test]
fn host_db_fetch_via_mock_returns_seeded_array() {
    use client::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    mock.seed_entities("product", serde_json::json!([{"id": "p-1", "price": 100}]));
    let h = DbHost::new(&mock);
    let res = h.fetch_entities("product", &serde_json::json!({})).unwrap();
    assert_eq!(res, serde_json::json!([{"id": "p-1", "price": 100}]));
}

#[test]
fn host_db_fetch_injects_entity_into_non_object_query() {
    use client::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    mock.seed_entities("user", serde_json::json!([{"id": "u-1"}]));
    let h = DbHost::new(&mock);
    let res = h.fetch_entities("user", &serde_json::Value::Null).unwrap();
    assert_eq!(res, serde_json::json!([{"id": "u-1"}]));
}

#[test]
fn host_db_patch_on_client_is_server_only_function_error() {
    // Spec §5.3: `WriteEntity` ist `server_only=true`. Auf dem Client gibt
    // es keinen Patch-Pfad — DbHost::patch_entity rejected sofort, bevor
    // ein HostApi-Call ueberhaupt stattfindet.
    use client::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    let h = DbHost::new(&mock);
    let err = h
        .patch_entity("product", "p-1", &serde_json::json!({"price": 199}))
        .unwrap_err();
    match err {
        shared::script::ScriptError::ServerOnlyFunction { name } => {
            assert_eq!(name, "db.patch");
        }
        other => panic!("expected ServerOnlyFunction, got {other:?}"),
    }
    // Und der Mock soll keinen Patch-Call gesehen haben:
    assert!(
        mock.patch_log().is_empty(),
        "client darf nicht in patch_log() landen"
    );
}

#[test]
fn host_ctx_returns_read_only_fields_from_script_ctx() {
    use client::script::host::ctx::CtxHost;
    use shared::script::engine::ScriptCtx;
    let h = CtxHost::new(ScriptCtx {
        user_id: Some("u-1".into()),
        tenant_id: Some("t-1".into()),
        locale: "de".into(),
    });
    assert_eq!(h.user_id(), Some("u-1"));
    assert_eq!(h.tenant_id(), Some("t-1"));
    assert_eq!(h.locale(), "de");
    // now() ist nicht-deterministisch; im native-Test reicht das `T`-Zeichen
    // als Form-Check (siehe Implementierung in host/ctx.rs).
    let now = h.now();
    assert!(now.contains('T'), "ISO-8601-aehnliche Form erwartet: {now}");
}

#[test]
fn host_ui_text_returns_uitree_subtree() {
    use client::script::host::ui::UiHost;
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
    let node = ui
        .text("Hallo", &serde_json::json!({"size": "h2"}))
        .unwrap();
    assert_eq!(node["type"], serde_json::Value::String("text".into()));
    assert_eq!(node["text"], serde_json::Value::String("Hallo".into()));
}

#[test]
fn host_ui_rejects_undeclared_primitive() {
    use client::script::host::ui::UiHost;
    let manifest = shared::script::ScriptManifest::default();
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
    use client::script::host::ui::UiHost;
    use shared::script::manifest::UiPrimitive;
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
fn host_audit_log_on_client_is_server_only_function_error() {
    // S8 (Q0009-Review): `audit.log` ist `server_only=true`. Der Client-Host
    // lehnt einen Skript-Aufruf hart mit `ServerOnlyFunction` ab (analog
    // `db.patch`), statt zu delegieren — Defence-in-depth. Interne Renderer-
    // Run-Events laufen ueber `audit_queue::push`, nicht ueber diese Schicht.
    use client::script::host::audit::AuditHost;
    let h = AuditHost::new();
    let err = h
        .log("custom.event", &serde_json::json!({"x": 1}))
        .unwrap_err();
    match err {
        shared::script::ScriptError::ServerOnlyFunction { name } => {
            assert_eq!(name, "audit.log");
        }
        other => panic!("expected ServerOnlyFunction, got {other:?}"),
    }
}

// ----------------------------------------------------------------------------
// Phase 4.4 — ClientHostApiRegistry + Symmetry-Check vs Server
// ----------------------------------------------------------------------------

#[test]
fn client_host_api_registry_lists_required_functions() {
    use shared::script::HostApiRegistry;
    let fns = client::script::ClientHostApiRegistry::functions();
    let names: Vec<_> = fns.iter().map(|f| f.name).collect();
    assert!(names.contains(&"db.entities"));
    assert!(names.contains(&"ui.text"));
    assert!(names.contains(&"ui.vstack"));
    assert!(names.contains(&"ctx.t"));
    // server_only-Eintraege existieren auf der Liste, aber sind als solche
    // markiert:
    let patch = fns.iter().find(|f| f.name == "db.patch").expect("db.patch");
    assert!(patch.server_only, "db.patch muss server_only sein");
    let audit = fns
        .iter()
        .find(|f| f.name == "audit.log")
        .expect("audit.log");
    assert!(audit.server_only, "audit.log muss server_only sein");
}

#[test]
fn engine_max_operations_kicks_in_on_runaway_loop() {
    let engine = client::script::engine::RhaiEngine::new();
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
    let host = std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    let res = engine.run(&ast, host.clone(), shared::script::engine::ScriptCtx::default());
    assert!(res.is_err(), "Endlosschleife muss abbrechen");
}
