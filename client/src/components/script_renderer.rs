//! Leptos-Renderer fuer `UiNode { kind: NodeKind::Script(ScriptNodeRef) }`
//! (Q0009 Phase 5.3).
//!
//! Aufgabe:
//!   1. Skript aus dem `ScriptRegistry` holen (Lookup ueber `ScriptId`).
//!   2. Wenn `state == Active`: durch die Phase-4-Engine + Sandbox laufen
//!      lassen. Der Rueckgabewert ist ein JSON-`UiNode`-Subtree (Spec §7.2),
//!      den wir rekursiv rendern.
//!   3. Bei `state != Active`: Inline-Placeholder ("Skript ist im Draft").
//!   4. Bei Run-Fehlern: gestylte Error-Box mit i18n-Key + Details-Disclosure.
//!
//! Bewusst zweigeteilt:
//!   - `render_decision(...)` — pure Funktion, vollstaendig testbar ohne
//!     Leptos-Mount.
//!   - `ScriptRenderer` — der Leptos-Component-Wrapper, der `render_decision`
//!     in eine `view!`-Ausgabe ueberfuehrt.
//!
//! State-Persistenz: ein per-Component `RwSignal<HashMap<String, ScriptValue>>`
//! lebt im Component-Scope. Bei Re-Render (Reaktivitaet) wird der State
//! durchgereicht, bei Unmount geht er weg. Phase 5 nutzt das noch nicht aktiv
//! — der Skript-Run hat keinen `ctx.state`-Zugriff im heutigen Trait — der
//! Hook bleibt als Marker fuer Phase 6.

use leptos::prelude::*;
use serde_json::Value;

use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::error::ScriptError;
use shared::script::model::{Script, ScriptKind, ScriptState};
use shared::script::ScriptNodeRef;

use crate::script::engine::RhaiEngine;
use crate::script::registry::ScriptRegistry;
use crate::script::sandbox::Sandbox;

/// Was wuerde dieser Renderer fuer den uebergebenen `ScriptNodeRef`
/// anzeigen? `RenderDecision` ist die pure Form — `ScriptRenderer` ist
/// nur der duenne Leptos-Wrapper darum.
#[derive(Debug, Clone, PartialEq)]
pub enum RenderDecision {
    /// Erfolgreicher Run — `node` ist der JSON-Subtree, den der Renderer in
    /// einen Leptos-View ueberfuehrt.
    Ok { node: Value },
    /// Skript ist im Registry nicht bekannt. `script_id` ist der Schluessel.
    Missing { script_id: String },
    /// Skript existiert, ist aber im Draft/Locked. `message` ist der i18n-
    /// rohe Hinweis (heute: das `last_error.msg`, falls vorhanden, sonst der
    /// State-Name).
    Placeholder {
        script_id: String,
        state: String,
        message: Option<String>,
    },
    /// Lauf hat einen Fehler ergeben — `error_key` ist der i18n-Schluessel
    /// (`script.error.<variantTag>`), `details` der entwickler-lesbare
    /// String.
    Error { error_key: String, details: String },
}

/// Bestimmt die Render-Entscheidung fuer einen `ScriptNodeRef`. Pure
/// Funktion, vollstaendig testbar ohne Leptos.
pub fn render_decision(
    script_ref: &ScriptNodeRef,
    registry: &ScriptRegistry,
    host: std::sync::Arc<dyn HostApi>,
    ctx: ScriptCtx,
) -> RenderDecision {
    let Some(script) = registry.get(&script_ref.script_id) else {
        return RenderDecision::Missing {
            script_id: script_ref.script_id.0.clone(),
        };
    };

    if script.state != ScriptState::Active {
        let msg = script.last_error.as_ref().map(error_message);
        return RenderDecision::Placeholder {
            script_id: script_ref.script_id.0.clone(),
            state: format!("{:?}", script.state).to_lowercase(),
            message: msg,
        };
    }

    // Optionaler Version-Pin: Phase 5 ignoriert ihn, weil die Registry
    // nur die letzte Active-Version fuehrt. Wenn der Pin in einer
    // spaeteren Phase relevant wird, wird er hier abgeglichen.
    let _ = script_ref.version_pin;

    // Component-Skripte sind die einzige `kind`, die `UiNode`-Subtrees
    // zurueckgeben. Provider-Skripte sind hier ein Fehler.
    if !matches!(script.kind, ScriptKind::Component { .. }) {
        return RenderDecision::Error {
            error_key: "script.error.notAComponent".into(),
            details: format!(
                "Script {} is not a Component (kind={:?})",
                script.id.0, script.kind
            ),
        };
    }

    run_script(&script, host, ctx).map_or_else(
        |e| RenderDecision::Error {
            error_key: format!("script.error.{}", error_tag(&e)),
            details: format!("{e:?}"),
        },
        |value| RenderDecision::Ok {
            node: script_value_to_json(value),
        },
    )
}

/// Engine-Run hinter Sandbox-Deadline-Check.
fn run_script(
    script: &Script,
    host: std::sync::Arc<dyn HostApi>,
    ctx: ScriptCtx,
) -> Result<ScriptValue, ScriptError> {
    let engine = RhaiEngine::new();
    let ast = engine.compile(&script.source, &script.manifest)?;
    let sb = Sandbox::new(&script.manifest);
    sb.check_deadline()?;
    engine.run(&ast, host, ctx)
}

fn script_value_to_json(v: ScriptValue) -> Value {
    match v {
        ScriptValue::String(s) => Value::String(s),
        ScriptValue::Number(n) => serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ScriptValue::Bool(b) => Value::Bool(b),
        ScriptValue::Json(j) => j,
        ScriptValue::Unit => Value::Null,
    }
}

fn error_message(err: &ScriptError) -> String {
    match err {
        ScriptError::ParseFailed { line, col, msg } => format!("Parse {line}:{col}: {msg}"),
        ScriptError::ManifestInvalid { reason } => format!("Manifest: {}", reason.reason),
        ScriptError::HostError { source } => source.clone(),
        ScriptError::Timeout { limit_ms } => format!("Timeout {limit_ms}ms"),
        ScriptError::ValidationFailed { msg_key, .. } => msg_key.clone(),
        ScriptError::ServerOnlyFunction { name } => format!("server-only: {name}"),
        ScriptError::CapabilityDenied { token } => format!("denied: {token:?}"),
        ScriptError::UiPrimitiveDenied { primitive } => format!("ui denied: {primitive}"),
        ScriptError::MemoryExceeded { limit_kb } => format!("memory {limit_kb}KB"),
        ScriptError::InternalPanic { backtrace } => backtrace.clone(),
        ScriptError::TierExceeded { declared, user } => format!("tier {declared:?} > {user:?}"),
        ScriptError::WasmEngineNotAvailable => "wasm not available".into(),
    }
}

/// Mapped einen `ScriptError` auf den camelCase-Tag (siehe `outcome_tag` in
/// `server::script::run`). Wird in den i18n-Key gehaengt, damit das UI pro
/// Fehlerklasse ein eigenes Markup zeigen kann (`script.error.timeout`,
/// `script.error.capabilityDenied`, …).
fn error_tag(err: &ScriptError) -> &'static str {
    match err {
        ScriptError::ParseFailed { .. } => "parseFailed",
        ScriptError::ManifestInvalid { .. } => "manifestInvalid",
        ScriptError::TierExceeded { .. } => "tierExceeded",
        ScriptError::WasmEngineNotAvailable => "wasmEngineNotAvailable",
        ScriptError::CapabilityDenied { .. } => "capabilityDenied",
        ScriptError::UiPrimitiveDenied { .. } => "uiPrimitiveDenied",
        ScriptError::ServerOnlyFunction { .. } => "serverOnlyFunction",
        ScriptError::Timeout { .. } => "timeout",
        ScriptError::MemoryExceeded { .. } => "memoryExceeded",
        ScriptError::InternalPanic { .. } => "internalPanic",
        ScriptError::HostError { .. } => "hostError",
        ScriptError::ValidationFailed { .. } => "validationFailed",
    }
}

// ---------------------------------------------------------------------------
// Leptos-Wrapper
// ---------------------------------------------------------------------------

/// Component-Wrapper. Aufrufer reicht eine `ScriptNodeRef` durch — der
/// Lookup gegen Registry und Host findet zur Renderzeit statt. Heute zieht
/// der Component Registry+Host aus dem Leptos-Context (sobald Phase 6 den
/// GraphQL-Adapter fuer beide bereitstellt). Phase 5 stellt einen
/// `provide_script_render_env`-Helper zur Verfuegung, damit Tests und
/// Builder-Integration ihn selbst befuellen koennen.
#[component]
pub fn ScriptRenderer(script_ref: ScriptNodeRef) -> impl IntoView {
    use leptos::prelude::*;

    let env = use_context::<ScriptRenderEnv>();

    let decision = move || {
        env.as_ref().map(|e| {
            let registry = e.registry.clone();
            let host_clone = e.host.clone();
            let ctx = e.make_ctx();
            // `run()` nimmt den Host als `Arc<dyn HostApi>` (B3) — direkt durchreichen.
            render_decision(&script_ref, &registry, host_clone, ctx)
        })
    };

    view! {
        {move || match decision() {
            None => view! {
                <div class="script-renderer script-renderer--no-env">
                    "[no script render env]"
                </div>
            }
            .into_any(),
            Some(RenderDecision::Ok { node }) => render_ui_subtree(&node).into_any(),
            Some(RenderDecision::Missing { script_id }) => view! {
                <div class="script-renderer script-renderer--missing">
                    {format!("script not found: {script_id}")}
                </div>
            }
            .into_any(),
            Some(RenderDecision::Placeholder { script_id, state, message }) => view! {
                <div class="script-renderer script-renderer--placeholder">
                    <span>{format!("script {script_id}: {state}")}</span>
                    {message.map(|m| view! { <span>{format!(" - {m}")}</span> })}
                </div>
            }
            .into_any(),
            Some(RenderDecision::Error { error_key, details }) => view! {
                <div class="script-renderer script-renderer--error">
                    <span>{crate::i18n::t(&error_key)}</span>
                    <details>
                        <summary>{"details"}</summary>
                        <pre>{details}</pre>
                    </details>
                </div>
            }
            .into_any(),
        }}
    }
}

/// Rendert einen JSON-UiNode-Subtree (`{"type": "vstack", "children": [...]}`,
/// `{"type": "text", "text": "...", "props": ...}`, …) als Leptos-View.
///
/// Heutiger Scope (Phase 5): die fuenf Whitelist-Primitives `vstack`, `hstack`,
/// `text`, `table`, `chart`. Unbekannte Subtree-Typen rendern als plain
/// `<pre>`-Fallback (Spec §7.2.3 Defensive Fallback). Tabelle und Chart
/// rendern nur ihre Props als JSON-pre — die echte Tabellen-/Chart-
/// Integration ist nicht Teil von Phase 5.
fn render_ui_subtree(node: &Value) -> AnyView {
    let type_str = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match type_str {
        "text" => {
            let text = node
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            view! { <span>{text}</span> }.into_any()
        }
        "vstack" => {
            let children: Vec<AnyView> = node
                .get("children")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().map(render_ui_subtree).collect())
                .unwrap_or_default();
            view! {
                <div style="display: flex; flex-direction: column; gap: 0.5rem;">
                    {children.into_iter().collect_view()}
                </div>
            }
            .into_any()
        }
        "hstack" => {
            let children: Vec<AnyView> = node
                .get("children")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().map(render_ui_subtree).collect())
                .unwrap_or_default();
            view! {
                <div style="display: flex; flex-direction: row; gap: 0.5rem;">
                    {children.into_iter().collect_view()}
                </div>
            }
            .into_any()
        }
        "table" | "chart" => {
            // Heutiger Phase-5-Scope: Props nur diagnostisch anzeigen.
            // Echte Verdrahtung (Table → EntityTable, Chart → Chart-Lib)
            // ist Folgeschritt.
            let label = type_str.to_string();
            let props = node.get("props").cloned().unwrap_or(Value::Null);
            view! {
                <div class=format!("script-subtree script-subtree--{label}")>
                    <code>{format!("[{label}] {props}")}</code>
                </div>
            }
            .into_any()
        }
        _ => view! {
            <pre class="script-subtree-unknown">{node.to_string()}</pre>
        }
        .into_any(),
    }
}

// ---------------------------------------------------------------------------
// Render-Environment (Leptos-Context)
// ---------------------------------------------------------------------------

/// Bundle, das der Aufrufer dem Renderer per Leptos-Context zur Verfuegung
/// stellt. Registry + Host + Locale/User-Snapshot — alles, was die
/// Engine fuer einen Run braucht.
#[derive(Clone)]
pub struct ScriptRenderEnv {
    pub registry: std::sync::Arc<ScriptRegistry>,
    // `HostApi: Send + Sync` (Supertrait) — `dyn HostApi` ist damit bereits
    // Send+Sync; kein explizites `+ Send + Sync` noetig (matcht die
    // `render_decision`/`run`-Signatur `Arc<dyn HostApi>`).
    pub host: std::sync::Arc<dyn HostApi>,
    pub locale: String,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
}

impl ScriptRenderEnv {
    pub fn make_ctx(&self) -> ScriptCtx {
        ScriptCtx {
            user_id: self.user_id.clone(),
            tenant_id: self.tenant_id.clone(),
            locale: self.locale.clone(),
        }
    }
}

/// Installiert das Render-Environment im aktuellen Leptos-Scope. Aufrufer:
/// die Builder-Route bei Mount (Phase 5), spaeter die App-Wurzel (Phase 6).
pub fn provide_script_render_env(env: ScriptRenderEnv) {
    provide_context(env);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::script::manifest::UiPrimitive;
    use shared::script::model::{ProviderSlot, ScriptKind};
    use shared::script::testing::MockHostApi;
    use shared::script::{
        CapabilityToken, Script, ScriptId, ScriptManifest, ScriptState, ScriptTier,
    };

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
    fn missing_script_returns_missing_decision() {
        let reg = ScriptRegistry::new();
        let host = std::sync::Arc::new(MockHostApi::new());
        let dec = render_decision(&make_ref("nope"), &reg, host.clone(), ScriptCtx::default());
        match dec {
            RenderDecision::Missing { script_id } => assert_eq!(script_id, "nope"),
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn draft_script_returns_placeholder() {
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
    fn locked_script_returns_placeholder() {
        let reg = ScriptRegistry::new();
        reg.insert(component_script("c1", r#""hi""#, ScriptState::Locked));
        let host = std::sync::Arc::new(MockHostApi::new());
        let dec = render_decision(&make_ref("c1"), &reg, host.clone(), ScriptCtx::default());
        assert!(matches!(dec, RenderDecision::Placeholder { .. }));
    }

    #[test]
    fn active_component_returns_ok_with_value() {
        let reg = ScriptRegistry::new();
        reg.insert(component_script("c1", r#""hello""#, ScriptState::Active));
        let host = std::sync::Arc::new(MockHostApi::new());
        let dec = render_decision(&make_ref("c1"), &reg, host.clone(), ScriptCtx::default());
        match dec {
            RenderDecision::Ok { node } => {
                assert_eq!(node, serde_json::json!("hello"));
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn provider_kind_returns_error() {
        // Ein Provider-Skript darf nicht als UiNode::Script gerendert werden —
        // das ist eine i18n-keyed Fehlermeldung.
        let reg = ScriptRegistry::new();
        let mut s = component_script("p1", "42", ScriptState::Active);
        s.kind = ScriptKind::Provider {
            slot: ProviderSlot::Formatter,
        };
        reg.insert(s);
        let host = std::sync::Arc::new(MockHostApi::new());
        let dec = render_decision(&make_ref("p1"), &reg, host.clone(), ScriptCtx::default());
        match dec {
            RenderDecision::Error { error_key, .. } => {
                assert_eq!(error_key, "script.error.notAComponent");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_renders_as_error_box_with_i18n_key() {
        let reg = ScriptRegistry::new();
        reg.insert(component_script(
            "broken",
            "!!! this is not rhai @@@",
            ScriptState::Active,
        ));
        let host = std::sync::Arc::new(MockHostApi::new());
        let dec = render_decision(&make_ref("broken"), &reg, host.clone(), ScriptCtx::default());
        match dec {
            RenderDecision::Error { error_key, .. } => {
                assert_eq!(error_key, "script.error.parseFailed");
            }
            other => panic!("expected Error::parseFailed, got {other:?}"),
        }
    }

    #[test]
    fn error_tag_is_camelcase_per_outcome_tag_contract() {
        // Pin: dieselben Tags wie `server::script::run::outcome_tag`. Spec §10.
        assert_eq!(
            error_tag(&ScriptError::Timeout { limit_ms: 100 }),
            "timeout"
        );
        assert_eq!(
            error_tag(&ScriptError::CapabilityDenied {
                token: CapabilityToken::ComputeOnly
            }),
            "capabilityDenied"
        );
        assert_eq!(
            error_tag(&ScriptError::ServerOnlyFunction {
                name: "db.patch".into()
            }),
            "serverOnlyFunction"
        );
        assert_eq!(
            error_tag(&ScriptError::WasmEngineNotAvailable),
            "wasmEngineNotAvailable"
        );
    }

    #[test]
    fn render_env_make_ctx_passes_through_user_and_locale() {
        let env = ScriptRenderEnv {
            registry: std::sync::Arc::new(ScriptRegistry::new()),
            host: std::sync::Arc::new(MockHostApi::new()),
            locale: "de".into(),
            user_id: Some("u-1".into()),
            tenant_id: Some("t-1".into()),
        };
        let ctx = env.make_ctx();
        assert_eq!(ctx.user_id.as_deref(), Some("u-1"));
        assert_eq!(ctx.tenant_id.as_deref(), Some("t-1"));
        assert_eq!(ctx.locale, "de");
    }
}
