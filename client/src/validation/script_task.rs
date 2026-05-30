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
use crate::validation::TaskFn;

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
