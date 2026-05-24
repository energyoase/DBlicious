//! Server-Pendant zu `client::script::provider_lookup` (Q0009 Phase 5.2).
//!
//! Wird ueberall dort genutzt, wo der Server selbst eine Spalte oder einen
//! Validator mit einer `script:`-prefixed ID berendert: SSR-Render-Pfade,
//! Exporte, GraphQL-Resolver, die einen `formatter_id`-Eintrag mit
//! `script:<id>` sehen.
//!
//! Symmetrie zur Client-Seite (Spec §8): identische Slot-Pruefung,
//! identische Fallback-Reasons. Audit-Log-Schreiben passiert hier
//! anders — Server-Seite hat den `script_audit_log`-Pfad in
//! `script::run::run_and_persist`. Diese Schicht emittiert deshalb keine
//! Audit-Zeile direkt; sie liefert die Fallback-Reason zurueck und der
//! Aufrufer (heute: schema.rs Render-Pfad) entscheidet, ob er fuer den
//! Fallback einen Mini-Audit-Eintrag schreibt.
//!
//! Heute synchron (vgl. die heutigen Render-Pfade in `data.rs`/`schema.rs`).
//! Wenn ein DB-fuetterndes Skript spaeter notwendig wird, gibt es eine
//! separate `run_and_persist`-Schiene.

use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::model::{ProviderSlot, ScriptKind, ScriptState};
use shared::script::{Script, ScriptId};

use crate::script::engine::rhai::RhaiEngine;
use crate::script::sandbox::Sandbox;

/// Prefix, mit dem Provider-Slots auf Skripte verweisen. Identisch zum
/// Client-Pendant (`client::script::provider_lookup::SCRIPT_PREFIX`).
pub const SCRIPT_PREFIX: &str = "script:";

/// Spiegelt `client::script::audit_queue::FallbackReason`. Wire-Form ist die
/// gleiche, damit Server-Audit-Log und Client-Audit-Queue dieselbe Schluessel-
/// menge benutzen.
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackReason {
    Missing { script_id: String },
    NotActive { script_id: String, state: String },
    SlotMismatch { script_id: String, expected: String },
    CompileFailed { script_id: String, error: String },
    RuntimeError { script_id: String, error: String },
}

impl FallbackReason {
    pub fn event_name(&self) -> &'static str {
        match self {
            FallbackReason::Missing { .. } => "script.provider.fallback.missing",
            FallbackReason::NotActive { .. } => "script.provider.fallback.notActive",
            FallbackReason::SlotMismatch { .. } => "script.provider.fallback.slotMismatch",
            FallbackReason::CompileFailed { .. } => "script.provider.fallback.compileFailed",
            FallbackReason::RuntimeError { .. } => "script.provider.fallback.runtimeError",
        }
    }
}

#[derive(Debug, Clone)]
pub enum LookupResult {
    Ok { value: ScriptValue },
    Fallback { reason: FallbackReason },
    NotAScriptId,
}

impl LookupResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, LookupResult::Ok { .. })
    }
}

/// Extrahiert die `script:<id>`-ID. Liefert `None` fuer Nicht-Skript-IDs.
pub fn parse_script_id(raw: &str) -> Option<ScriptId> {
    raw.strip_prefix(SCRIPT_PREFIX).map(|s| ScriptId(s.into()))
}

/// Server-Pendant zum Client-`lookup_provider`. Bekommt das geladene `Script`
/// vom Aufrufer (typischerweise vom SeaORM-Layer im Resolver), damit dieser
/// Pfad synchron bleiben kann.
pub fn lookup_provider_with_script(
    raw_id: &str,
    expected_slot: ProviderSlot,
    script: Option<&Script>,
    host: std::sync::Arc<dyn HostApi>,
    ctx: ScriptCtx,
) -> LookupResult {
    let Some(script_id) = parse_script_id(raw_id) else {
        return LookupResult::NotAScriptId;
    };

    let Some(script) = script else {
        return LookupResult::Fallback {
            reason: FallbackReason::Missing {
                script_id: script_id.0,
            },
        };
    };

    if script.state != ScriptState::Active {
        return LookupResult::Fallback {
            reason: FallbackReason::NotActive {
                script_id: script_id.0,
                state: format!("{:?}", script.state).to_lowercase(),
            },
        };
    }

    match &script.kind {
        ScriptKind::Provider { slot } if *slot == expected_slot => {}
        _ => {
            return LookupResult::Fallback {
                reason: FallbackReason::SlotMismatch {
                    script_id: script_id.0,
                    expected: provider_slot_wire(expected_slot).into(),
                },
            };
        }
    }

    let engine = RhaiEngine::with_manifest(&script.manifest);
    let ast = match engine.compile(&script.source, &script.manifest) {
        Ok(a) => a,
        Err(e) => {
            return LookupResult::Fallback {
                reason: FallbackReason::CompileFailed {
                    script_id: script_id.0,
                    error: format!("{e:?}"),
                },
            }
        }
    };

    let sb = Sandbox::new(&script.manifest);
    if let Err(e) = sb.check_deadline() {
        return LookupResult::Fallback {
            reason: FallbackReason::RuntimeError {
                script_id: script_id.0,
                error: format!("{e:?}"),
            },
        };
    }

    match engine.run(&ast, host, ctx) {
        Ok(value) => LookupResult::Ok { value },
        Err(e) => LookupResult::Fallback {
            reason: FallbackReason::RuntimeError {
                script_id: script_id.0,
                error: format!("{e:?}"),
            },
        },
    }
}

fn provider_slot_wire(slot: ProviderSlot) -> &'static str {
    match slot {
        ProviderSlot::Formatter => "formatter",
        ProviderSlot::Filter => "filter",
        ProviderSlot::Computed => "computed",
        ProviderSlot::Validator => "validator",
        ProviderSlot::RowAction => "rowAction",
    }
}

/// Symmetry-Helfer fuer Tests (vgl. client::script::provider_lookup::script_value_to_display).
pub fn script_value_to_display(value: &ScriptValue) -> String {
    match value {
        ScriptValue::String(s) => s.clone(),
        ScriptValue::Number(n) => n.to_string(),
        ScriptValue::Bool(b) => b.to_string(),
        ScriptValue::Json(j) => j.to_string(),
        ScriptValue::Unit => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::script::testing::MockHostApi;
    use shared::script::{
        CapabilityToken, Script, ScriptKind, ScriptManifest, ScriptState, ScriptTier,
    };

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
    fn parse_script_id_strips_prefix() {
        assert_eq!(parse_script_id("script:abc"), Some(ScriptId("abc".into())));
        assert!(parse_script_id("xyz").is_none());
    }

    #[test]
    fn missing_script_returns_fallback() {
        let host = std::sync::Arc::new(MockHostApi::new());
        let res = lookup_provider_with_script(
            "script:miss",
            ProviderSlot::Formatter,
            None,
            host.clone(),
            ScriptCtx::default(),
        );
        assert!(matches!(
            res,
            LookupResult::Fallback {
                reason: FallbackReason::Missing { .. }
            }
        ));
    }

    #[test]
    fn active_formatter_returns_value_via_engine() {
        let s = formatter_script("disc", "40 + 2", ScriptState::Active);
        let host = std::sync::Arc::new(MockHostApi::new());
        let res = lookup_provider_with_script(
            "script:disc",
            ProviderSlot::Formatter,
            Some(&s),
            host.clone(),
            ScriptCtx::default(),
        );
        match res {
            LookupResult::Ok { value } => {
                assert_eq!(script_value_to_display(&value), "42");
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn draft_state_falls_back() {
        let s = formatter_script("d", "42", ScriptState::Draft);
        let host = std::sync::Arc::new(MockHostApi::new());
        let res = lookup_provider_with_script(
            "script:d",
            ProviderSlot::Formatter,
            Some(&s),
            host.clone(),
            ScriptCtx::default(),
        );
        match res {
            LookupResult::Fallback {
                reason: FallbackReason::NotActive { state, .. },
            } => assert_eq!(state, "draft"),
            other => panic!("expected NotActive draft, got {other:?}"),
        }
    }

    #[test]
    fn slot_mismatch_falls_back() {
        let s = formatter_script("x", "42", ScriptState::Active);
        let host = std::sync::Arc::new(MockHostApi::new());
        let res = lookup_provider_with_script(
            "script:x",
            ProviderSlot::Validator,
            Some(&s),
            host.clone(),
            ScriptCtx::default(),
        );
        match res {
            LookupResult::Fallback {
                reason: FallbackReason::SlotMismatch { expected, .. },
            } => assert_eq!(expected, "validator"),
            other => panic!("expected SlotMismatch, got {other:?}"),
        }
    }

    #[test]
    fn non_script_id_signals_static_path() {
        let host = std::sync::Arc::new(MockHostApi::new());
        let res = lookup_provider_with_script(
            "text-plain",
            ProviderSlot::Formatter,
            None,
            host.clone(),
            ScriptCtx::default(),
        );
        assert!(matches!(res, LookupResult::NotAScriptId));
    }
}
