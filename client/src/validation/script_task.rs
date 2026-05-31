//! Q0014 (Lücke A): uebersetzt einen `script:`-Validator in einen
//! `ValidationTask`. Spiegelt den Formatter-Pfad aus
//! `components/table/formatters.rs` — synchron, ueber die echte Engine via
//! `lookup_provider(ProviderSlot::Validator)`.
//!
//! Konvention: der Validator-Slot gibt `true` = "valide" zurueck (so liest
//! `d2v_balance_validator.rhai`). `Bool(false)` => blockierender
//! `ValidationMessage::error`. Fallback/NotAScriptId/Nicht-Bool => kein Block
//! (Validator nicht aktiv; `lookup_provider` hat das Audit-Log bereits gefuellt).

use std::sync::Arc;

use shared::script::engine::{HostApi, ScriptCtx, ScriptInputs, ScriptValue};
use shared::script::model::ProviderSlot;
use shared::ValidationMessage;

use crate::script::provider_lookup::{lookup_provider, LookupResult, SCRIPT_PREFIX};
use crate::script::registry::ScriptRegistry;
use crate::validation::{TaskFn, ValidationSystem, ValidationTask};

/// Fluent-Schluessel-Prefix fuer Skript-Validator-Fehler.
pub const SCRIPT_VALIDATION_KEY: &str = "validation.script";

/// Baut aus einer `validator_id` mit `script:`-Prefix einen `TaskFn`.
/// `None`, wenn die ID keinen `script:`-Prefix traegt (statischer Pfad gilt
/// dann; Q0014 kennt keine statischen Validator-IDs).
pub fn script_validator_task(
    target: &str,
    validator_id: &str,
    registry: Arc<ScriptRegistry>,
    host: Arc<dyn HostApi>,
    ctx: ScriptCtx,
) -> Option<TaskFn> {
    if !validator_id.starts_with(SCRIPT_PREFIX) {
        return None;
    }
    let validator_id = validator_id.to_string();
    let target_owned = target.to_string();
    let task: TaskFn = Arc::new(move |_t, value, fields| {
        let inputs = ScriptInputs {
            value: value.clone(),
            fields: fields.clone(),
        };
        match lookup_provider(
            &validator_id,
            ProviderSlot::Validator,
            &registry,
            host.clone(),
            ctx.clone(),
            inputs,
        ) {
            // true = valide, false = blockierender Fehler.
            LookupResult::Ok {
                value: ScriptValue::Bool(false),
            } => Some(ValidationMessage::error(
                target_owned.clone(),
                SCRIPT_VALIDATION_KEY,
            )),
            // Bool(true) / andere Slot-Konventionen / Fallback / NotAScriptId
            // => nicht blockieren.
            _ => None,
        }
    });
    Some(task)
}

/// Registriert fuer jede Column mit `validator_id` (Prefix `script:`) einen
/// [`ValidationTask`] im `sys`, gebunden an den Entity-Typ. Columns ohne
/// (Script-)Validator werden ignoriert (`script_validator_task` liefert `None`).
/// Reine Funktion — kein Leptos, daher ohne Mount testbar (wie Q0014).
pub fn register_script_validators(
    sys: &mut ValidationSystem,
    entity_type: &str,
    columns: &[shared::ColumnMeta],
    registry: Arc<ScriptRegistry>,
    host: Arc<dyn HostApi>,
    ctx: ScriptCtx,
) {
    for col in columns {
        let Some(vid) = col.validator_id.as_deref() else {
            continue;
        };
        if let Some(task) =
            script_validator_task(&col.key, vid, registry.clone(), host.clone(), ctx.clone())
        {
            sys.register(
                entity_type,
                ValidationTask {
                    target: col.key.clone(),
                    task,
                },
            );
        }
    }
}

#[cfg(test)]
mod register_tests {
    use super::*;
    use crate::script::registry::ScriptRegistry;
    use crate::validation::ValidationSystem;
    use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
    use shared::script::testing::MockHostApi;
    use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptTier};

    const BALANCE_SRC: &str =
        include_str!("../../../examples/d2v/scripts/d2v_balance_validator.rhai");

    fn validator_script(id: &str) -> Script {
        Script {
            id: ScriptId(id.into()),
            kind: ScriptKind::Provider {
                slot: ProviderSlot::Validator,
            },
            manifest: ScriptManifest {
                manifest_version: 1,
                tier: ScriptTier::Reader,
                capabilities: vec![CapabilityToken::ComputeOnly, CapabilityToken::ReadI18n],
                ..Default::default()
            },
            source: BALANCE_SRC.into(),
            version: 1,
            state: ScriptState::Active,
            last_error: None,
            created_by: "u-1".into(),
            created_at: "2026-05-23T00:00:00Z".into(),
            updated_at: "2026-05-23T00:00:00Z".into(),
        }
    }

    fn col(key: &str, validator_id: Option<&str>) -> shared::ColumnMeta {
        shared::ColumnMeta {
            key: key.into(),
            label_key: format!("field.{key}"),
            field_type: shared::FieldType::Text,
            sortable: false,
            filterable: false,
            comparator_id: None,
            filter_id: None,
            editor_id: None,
            formatter_id: None,
            validator_id: validator_id.map(str::to_string),
            action_ids: Vec::new(),
        }
    }

    #[test]
    fn registers_only_script_validators() {
        let reg = ScriptRegistry::new();
        reg.insert(validator_script("d2v_balance_validator"));
        let host = std::sync::Arc::new(MockHostApi::new());
        let columns = vec![
            col("value", Some("script:d2v_balance_validator")),
            col("description", None),
            col("key", Some("static-noop")), // Kein script: -> uebersprungen.
        ];

        let mut sys = ValidationSystem::new();
        register_script_validators(
            &mut sys,
            "datev_entry",
            &columns,
            std::sync::Arc::new(reg),
            host,
            ScriptCtx::default(),
        );

        // Genau der "value"-Task wurde registriert: unbalanciert blockiert.
        let mut f = serde_json::Map::new();
        f.insert("value".into(), serde_json::json!(100.0));
        f.insert("valueType".into(), serde_json::json!("SOLL"));
        f.insert("partnerValue".into(), serde_json::json!(90.0));
        f.insert("partnerValueType".into(), serde_json::json!("HABEN"));
        let res = sys.run("datev_entry", &f);
        assert!(res.has_blocking(), "unbalanced value must block");
        assert_eq!(res.messages.len(), 1, "only the value validator registers");
        assert_eq!(res.messages[0].target.as_deref(), Some("value"));
    }
}
