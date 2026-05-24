//! Integrationstest fuer den `ScriptRenderer` (Q0009 Phase 5.3).
//!
//! Diese Tests heben nur das pure `render_decision`-Verhalten ab (kein
//! Leptos-Mount). Die view-Erzeugung wird separat per `trunk build`
//! geprueft. Damit decken wir alle relevanten States — Missing, Draft,
//! Active-OK, Active-Runtime-Error, Active-Slot-Mismatch — ohne Browser.

use client::components::script_renderer::{render_decision, RenderDecision, ScriptRenderEnv};
use client::script::registry::ScriptRegistry;
use shared::script::engine::ScriptCtx;
use shared::script::manifest::UiPrimitive;
use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
use shared::script::testing::MockHostApi;
use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptNodeRef, ScriptTier};

fn component_script(id: &str, source: &str, state: ScriptState) -> Script {
    Script {
        id: ScriptId(id.into()),
        kind: ScriptKind::Component {
            entry: "view".into(),
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Author,
            capabilities: vec![CapabilityToken::ComputeOnly],
            ui_primitives: vec![UiPrimitive::Text, UiPrimitive::Vstack],
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

fn make_ref(id: &str) -> ScriptNodeRef {
    ScriptNodeRef {
        script_id: ScriptId(id.into()),
        version_pin: None,
    }
}

#[test]
fn render_active_component_returns_ok_with_string_payload() {
    let reg = ScriptRegistry::new();
    reg.insert(component_script("c1", r#""hi""#, ScriptState::Active));
    let host = std::sync::Arc::new(MockHostApi::new());
    let dec = render_decision(&make_ref("c1"), &reg, host.clone(), ScriptCtx::default());
    match dec {
        RenderDecision::Ok { node } => {
            assert_eq!(node, serde_json::json!("hi"));
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[test]
fn render_draft_returns_placeholder() {
    let reg = ScriptRegistry::new();
    reg.insert(component_script("c1", r#""hi""#, ScriptState::Draft));
    let host = std::sync::Arc::new(MockHostApi::new());
    let dec = render_decision(&make_ref("c1"), &reg, host.clone(), ScriptCtx::default());
    match dec {
        RenderDecision::Placeholder { state, .. } => assert_eq!(state, "draft"),
        other => panic!("expected Placeholder, got {other:?}"),
    }
}

#[test]
fn render_unknown_script_returns_missing() {
    let reg = ScriptRegistry::new();
    let host = std::sync::Arc::new(MockHostApi::new());
    let dec = render_decision(&make_ref("absent"), &reg, host.clone(), ScriptCtx::default());
    match dec {
        RenderDecision::Missing { script_id } => assert_eq!(script_id, "absent"),
        other => panic!("expected Missing, got {other:?}"),
    }
}

#[test]
fn render_runtime_error_returns_error_with_i18n_key() {
    let reg = ScriptRegistry::new();
    // Endlosschleife → Engine bricht via `set_max_operations` ab → wir
    // erwarten einen `Timeout`-Tag (so mapped die Engine `ErrorTooManyOperations`).
    reg.insert(component_script(
        "loop",
        "let i = 0; while i < 1_000_000_000 { i = i + 1; } i",
        ScriptState::Active,
    ));
    let host = std::sync::Arc::new(MockHostApi::new());
    let dec = render_decision(&make_ref("loop"), &reg, host.clone(), ScriptCtx::default());
    match dec {
        RenderDecision::Error { error_key, .. } => {
            assert_eq!(error_key, "script.error.timeout");
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn render_provider_kind_is_an_error() {
    // Provider-Skripte sind keine Komponenten — der Renderer muss das
    // erkennen und einen i18n-keyed Fehler liefern.
    let reg = ScriptRegistry::new();
    let mut s = component_script("p", "42", ScriptState::Active);
    s.kind = ScriptKind::Provider {
        slot: ProviderSlot::Formatter,
    };
    reg.insert(s);
    let host = std::sync::Arc::new(MockHostApi::new());
    let dec = render_decision(&make_ref("p"), &reg, host.clone(), ScriptCtx::default());
    match dec {
        RenderDecision::Error { error_key, .. } => {
            assert_eq!(error_key, "script.error.notAComponent");
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn render_env_passes_locale_and_user_through_ctx() {
    let env = ScriptRenderEnv {
        registry: std::sync::Arc::new(ScriptRegistry::new()),
        host: std::sync::Arc::new(MockHostApi::new()),
        locale: "en".into(),
        user_id: Some("u-7".into()),
        tenant_id: None,
    };
    let ctx = env.make_ctx();
    assert_eq!(ctx.locale, "en");
    assert_eq!(ctx.user_id.as_deref(), Some("u-7"));
    assert!(ctx.tenant_id.is_none());
}
