//! Provider-Registry-Lookup mit `script:`-Prefix (Q0009 Phase 5.2).
//!
//! Pendant zur statischen Formatter/Filter/Computed/Validator/RowAction-
//! Registry. Erkennt `formatter_id == "script:<script-id>"` o.ae. und
//! ruft den passenden Provider-Skript-Run an Stelle des hartcodierten
//! Default-Pfads.
//!
//! Resolution-Reihenfolge (Spec §6.1):
//!   1. ID hat `script:`-Prefix → Skript-Lookup in `ScriptRegistry`.
//!      - Skript fehlt / Draft / Locked / Slot-Mismatch → Fallback.
//!      - Skript Active + passender Slot → Engine-Run.
//!      - Engine-Run-Fehler → Fallback + Audit-Log.
//!   2. Sonst: Aufrufer macht statischen Lookup wie bisher.
//!
//! Symmetrie: das Server-Pendant (Spec §8) lebt in
//! `server::script::provider_lookup` — gleiche Slot-Pruefung, gleiche
//! Fallback-Semantik. Phase 5 verdrahtet nur den Client-Pfad; der
//! Server-Pfad ist in Phase 6 (GraphQL/SSR-Surface) konstanter Stuetz-
//! punkt.

use serde_json::Value;

use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::model::{ProviderSlot, ScriptKind, ScriptState};
use shared::script::ScriptId;

use crate::script::audit_queue::{push_fallback, FallbackReason};
use crate::script::engine::RhaiEngine;
use crate::script::registry::ScriptRegistry;
use crate::script::sandbox::Sandbox;

/// Prefix, mit dem ein Formatter/Filter/etc. statt einer statischen ID
/// einen Skript-Pfad referenziert.
pub const SCRIPT_PREFIX: &str = "script:";

/// Ergebnis eines Provider-Lookups.
#[derive(Debug, Clone)]
pub enum LookupResult {
    /// Skript-Lauf erfolgreich — `value` ist der Rueckgabewert.
    Ok { value: ScriptValue },
    /// Skript-Lauf fehlgeschlagen — Aufrufer soll auf den statischen Default
    /// zurueckfallen. `reason` ist bereits in die Audit-Queue gelegt.
    Fallback { reason: FallbackReason },
    /// Aufrufer hat keinen `script:`-Prefix uebergeben — Provider-Lookup
    /// trifft hier keine Aussage und der statische Pfad gilt.
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

/// Fuehrt einen Provider-Skript-Lauf aus, falls die ID einen `script:`-Prefix
/// hat. Aufrufer reicht den erwarteten Slot durch (Formatter/Filter/...) —
/// laeuft das Skript mit einem anderen Slot, geht es als Fallback durch und
/// wird audit-geloggt.
///
/// `host` ist die `HostApi`-Implementierung (im Renderer: GraphQL-backed).
/// `ctx` die Leptos-/Auth-/i18n-zusammengezogenen Werte.
pub fn lookup_provider(
    raw_id: &str,
    expected_slot: ProviderSlot,
    registry: &ScriptRegistry,
    host: &dyn HostApi,
    ctx: ScriptCtx,
) -> LookupResult {
    let Some(script_id) = parse_script_id(raw_id) else {
        return LookupResult::NotAScriptId;
    };

    let Some(script) = registry.get(&script_id) else {
        let reason = FallbackReason::Missing {
            script_id: script_id.0.clone(),
        };
        push_fallback(reason.clone());
        return LookupResult::Fallback { reason };
    };

    // State muss Active sein. Draft = "noch nicht freigeschaltet" → Fallback,
    // Locked = "Skript ist eingefroren, aber nutzbar nur via Codegen-Pfad" →
    // Fallback im Renderer (Locked wird in einer spaeteren Phase mit dem
    // Codegen-Lookup verdrahtet).
    if script.state != ScriptState::Active {
        let reason = FallbackReason::NotActive {
            script_id: script_id.0.clone(),
            state: format!("{:?}", script.state).to_lowercase(),
        };
        push_fallback(reason.clone());
        return LookupResult::Fallback { reason };
    }

    // Slot muss passen — sonst rufen wir z.B. einen Validator-Slot in einer
    // Formatter-Position. Component-Kind kann hier nicht als Formatter
    // dienen.
    match &script.kind {
        ScriptKind::Provider { slot } if *slot == expected_slot => {}
        _ => {
            let reason = FallbackReason::SlotMismatch {
                script_id: script_id.0.clone(),
                expected: provider_slot_wire(expected_slot).into(),
            };
            push_fallback(reason.clone());
            return LookupResult::Fallback { reason };
        }
    }

    // ---- Engine-Run hinter Sandbox + Audit-Buffer ----
    let engine = RhaiEngine::new();
    let ast = match engine.compile(&script.source, &script.manifest) {
        Ok(a) => a,
        Err(e) => {
            let reason = FallbackReason::CompileFailed {
                script_id: script_id.0.clone(),
                error: format!("{e:?}"),
            };
            push_fallback(reason.clone());
            return LookupResult::Fallback { reason };
        }
    };

    let sb = Sandbox::new(&script.manifest);
    if let Err(e) = sb.check_deadline() {
        let reason = FallbackReason::RuntimeError {
            script_id: script_id.0.clone(),
            error: format!("{e:?}"),
        };
        push_fallback(reason.clone());
        return LookupResult::Fallback { reason };
    }
    match engine.run(&ast, host, ctx) {
        Ok(value) => LookupResult::Ok { value },
        Err(e) => {
            let reason = FallbackReason::RuntimeError {
                script_id: script_id.0.clone(),
                error: format!("{e:?}"),
            };
            push_fallback(reason.clone());
            LookupResult::Fallback { reason }
        }
    }
}

/// Wire-Name des Provider-Slots (camelCase). Identisch zum `ProviderSlot`-
/// Serde-Tag, ohne hier auf `serde_json::to_value` zurueckzugreifen.
fn provider_slot_wire(slot: ProviderSlot) -> &'static str {
    match slot {
        ProviderSlot::Formatter => "formatter",
        ProviderSlot::Filter => "filter",
        ProviderSlot::Computed => "computed",
        ProviderSlot::Validator => "validator",
        ProviderSlot::RowAction => "rowAction",
    }
}

/// Konvertiert einen Skript-Rueckgabewert in das, was ein Formatter im
/// Tabellen-Render erwartet: einen `String`. Andere `ScriptValue`-
/// Varianten werden via `to_string`-Form serialisiert (Spec §6.2).
pub fn script_value_to_display(value: &ScriptValue) -> String {
    match value {
        ScriptValue::String(s) => s.clone(),
        ScriptValue::Number(n) => n.to_string(),
        ScriptValue::Bool(b) => b.to_string(),
        ScriptValue::Json(j) => j.to_string(),
        ScriptValue::Unit => String::new(),
    }
}

/// Konvertiert einen Skript-Rueckgabewert in `serde_json::Value` fuer
/// Provider-Slots, die JSON-Subtrees produzieren (Validator-Errors, UiNode-
/// Snippets, RowAction-Payloads).
pub fn script_value_to_json(value: &ScriptValue) -> Value {
    match value {
        ScriptValue::String(s) => Value::String(s.clone()),
        ScriptValue::Number(n) => serde_json::Number::from_f64(*n)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ScriptValue::Bool(b) => Value::Bool(*b),
        ScriptValue::Json(j) => j.clone(),
        ScriptValue::Unit => Value::Null,
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
        assert_eq!(
            parse_script_id("script:foo"),
            Some(ScriptId("foo".into()))
        );
        assert!(parse_script_id("text-plain").is_none());
    }

    #[test]
    fn not_a_script_id_signals_static_path() {
        let reg = ScriptRegistry::new();
        let mock = MockHostApi::new();
        let res = lookup_provider(
            "text-plain",
            ProviderSlot::Formatter,
            &reg,
            &mock,
            ScriptCtx::default(),
        );
        assert!(matches!(res, LookupResult::NotAScriptId));
    }

    #[test]
    fn missing_script_falls_back() {
        let reg = ScriptRegistry::new();
        let mock = MockHostApi::new();
        let res = lookup_provider(
            "script:does-not-exist",
            ProviderSlot::Formatter,
            &reg,
            &mock,
            ScriptCtx::default(),
        );
        match res {
            LookupResult::Fallback {
                reason: FallbackReason::Missing { script_id },
            } => assert_eq!(script_id, "does-not-exist"),
            other => panic!("expected Fallback::Missing, got {other:?}"),
        }
    }

    #[test]
    fn draft_state_falls_back() {
        let reg = ScriptRegistry::new();
        reg.insert(formatter_script("fmt-1", "42", ScriptState::Draft));
        let mock = MockHostApi::new();
        let res = lookup_provider(
            "script:fmt-1",
            ProviderSlot::Formatter,
            &reg,
            &mock,
            ScriptCtx::default(),
        );
        match res {
            LookupResult::Fallback {
                reason: FallbackReason::NotActive { state, .. },
            } => assert_eq!(state, "draft"),
            other => panic!("expected Fallback::NotActive draft, got {other:?}"),
        }
    }

    #[test]
    fn slot_mismatch_falls_back() {
        let reg = ScriptRegistry::new();
        reg.insert(formatter_script("fmt-1", "42", ScriptState::Active));
        let mock = MockHostApi::new();
        let res = lookup_provider(
            "script:fmt-1",
            ProviderSlot::Validator, // mismatching slot
            &reg,
            &mock,
            ScriptCtx::default(),
        );
        match res {
            LookupResult::Fallback {
                reason: FallbackReason::SlotMismatch { expected, .. },
            } => assert_eq!(expected, "validator"),
            other => panic!("expected Fallback::SlotMismatch, got {other:?}"),
        }
    }

    #[test]
    fn active_formatter_runs_engine_and_returns_value() {
        let reg = ScriptRegistry::new();
        reg.insert(formatter_script("fmt-1", "40 + 2", ScriptState::Active));
        let mock = MockHostApi::new();
        let res = lookup_provider(
            "script:fmt-1",
            ProviderSlot::Formatter,
            &reg,
            &mock,
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
    fn compile_error_falls_back() {
        let reg = ScriptRegistry::new();
        reg.insert(formatter_script(
            "fmt-broken",
            "this is not rhai !!! @@@",
            ScriptState::Active,
        ));
        let mock = MockHostApi::new();
        let res = lookup_provider(
            "script:fmt-broken",
            ProviderSlot::Formatter,
            &reg,
            &mock,
            ScriptCtx::default(),
        );
        assert!(matches!(
            res,
            LookupResult::Fallback {
                reason: FallbackReason::CompileFailed { .. }
            }
        ));
    }

    #[test]
    fn script_value_to_display_serializes_each_variant() {
        assert_eq!(
            script_value_to_display(&ScriptValue::String("x".into())),
            "x"
        );
        assert_eq!(script_value_to_display(&ScriptValue::Number(3.5)), "3.5");
        assert_eq!(script_value_to_display(&ScriptValue::Bool(true)), "true");
        assert_eq!(script_value_to_display(&ScriptValue::Unit), "");
    }
}
