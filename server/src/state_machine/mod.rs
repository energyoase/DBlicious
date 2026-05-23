//! State-Machine-Service (Phase 1.7.5).
//!
//! Server-seitige Logik fuer `apply_transition(entity_type, id, event)`:
//!
//! 1. Lese `EntitySettings.state_machine` — wenn `None`, Error
//!    `no_state_machine`.
//! 2. Lese Entity, extrahiere `current_state` aus `state_field`.
//! 3. Finde Transition `(from == current_state, event)`. Wildcard `"*"`
//!    matcht jeden State.
//! 4. Werte Guard aus (falls vorhanden). Liefert `false` ⇒ Error
//!    `guard_failed`.
//! 5. Pruefe Permission (falls Transition `permission` setzt oder
//!    Default `"<entity_type>.<event>"`). Liefert `Deny` ⇒ Error
//!    `forbidden`.
//! 6. Update Entity-Felder: `state_field = to`. Schreibe Audit-Eintrag
//!    `kind = "state_transition"` in `audit_log`.

use thiserror::Error;

use shared::auth::{Effect, Op, Resource};
use shared::state_machine::Transition;

use crate::audit;
use crate::data;

#[derive(Debug, Error)]
pub enum TransitionError {
    #[error("no_state_machine: entity_type '{0}' has no state_machine configured")]
    NoStateMachine(String),
    #[error("entity_not_found: '{entity_type}' id='{id}'")]
    NotFound { entity_type: String, id: String },
    #[error("no_matching_transition: from='{from}' event='{event}'")]
    NoMatchingTransition { from: String, event: String },
    #[error("guard_failed: transition from='{from}' to='{to}' on event='{event}'")]
    GuardFailed { from: String, to: String, event: String },
    #[error("forbidden: user lacks permission '{permission}'")]
    Forbidden { permission: String },
    #[error("invalid_target_state: '{to}' is not in state_machine.states")]
    InvalidTargetState { to: String },
    #[error("database error: {0}")]
    Db(String),
}

/// Ergebnis einer erfolgreichen Transition.
#[derive(Debug, Clone)]
pub struct TransitionOutcome {
    pub from:  String,
    pub to:    String,
    pub event: String,
}

/// Wendet eine State-Transition an. Aktualisiert das Entity-Feld + Audit.
///
/// `actor_user_id` ist optional — wenn None, ist keine Permission-
/// Pruefung moeglich; der Aufrufer entscheidet, ob das ein Fehler ist.
/// (System-Triggers laufen z.B. ohne User.)
pub async fn apply_transition(
    entity_type:   &str,
    id:            &str,
    event:         &str,
    actor_user_id: Option<&str>,
) -> Result<TransitionOutcome, TransitionError> {
    let sm = data::settings_for_async(entity_type)
        .await
        .and_then(|s| s.state_machine.clone())
        .ok_or_else(|| TransitionError::NoStateMachine(entity_type.to_string()))?;

    // Direkt ueber die Source, damit wir das shared::Entity mit Map-fields
    // bekommen statt das schema::Entity mit Json<Value>.
    let binding = shared::source::default_binding_for(entity_type);
    let actual_binding = data::settings_for_async(entity_type)
        .await
        .and_then(|s| s.binding)
        .unwrap_or(binding);
    let source = crate::source::registry()
        .route(&actual_binding)
        .map_err(|e| TransitionError::Db(format!("{e}")))?;
    let entity_id = shared::source::EntityId::decode(id);
    let entity = source
        .get(&actual_binding, &entity_id)
        .await
        .map_err(|e| TransitionError::Db(format!("{e}")))?
        .ok_or_else(|| TransitionError::NotFound {
            entity_type: entity_type.to_string(),
            id:          id.to_string(),
        })?;

    let fields = entity.fields.clone();
    let current_state = fields
        .get(sm.state_field_name())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let transition = sm
        .find_transition(&current_state, event)
        .cloned()
        .ok_or_else(|| TransitionError::NoMatchingTransition {
            from:  current_state.clone(),
            event: event.to_string(),
        })?;

    // Validierung: Ziel-State muss in der Liste sein (falls Liste nicht leer).
    if !sm.is_known_state(&transition.to) {
        return Err(TransitionError::InvalidTargetState { to: transition.to });
    }

    // Guard.
    if let Some(guard) = &transition.guard {
        let ast = guard
            .parse()
            .map_err(|e| TransitionError::Db(format!("guard parse: {e:?}")))?;
        if !ast.evaluate(&fields) {
            return Err(TransitionError::GuardFailed {
                from:  current_state,
                to:    transition.to,
                event: event.to_string(),
            });
        }
    }

    // Permission.
    if let Some(user_id) = actor_user_id {
        let perm_name = transition_permission_name(entity_type, &transition);
        let resource = Resource::Action { name: perm_name.clone() };
        match crate::auth::resolver::effective(user_id, &resource, Op::Execute).await {
            Ok(Effect::Allow) => {}
            _ => return Err(TransitionError::Forbidden { permission: perm_name }),
        }
    }

    // Update Entity-Feld.
    let mut patch = serde_json::Map::new();
    patch.insert(sm.state_field_name().to_string(), serde_json::Value::String(transition.to.clone()));
    data::update_entity(
        entity_type,
        id,
        serde_json::Value::Object(patch),
        actor_user_id,
    )
    .await
    .ok_or_else(|| TransitionError::Db("update_entity returned None".into()))?;

    // Audit-Eintrag.
    audit::record_state_transition(
        actor_user_id,
        entity_type,
        id,
        &current_state,
        &transition.to,
        event,
    )
    .await;

    Ok(TransitionOutcome {
        from:  current_state,
        to:    transition.to,
        event: event.to_string(),
    })
}

/// Default-Konvention: `"<entity_type>.<event>"`, sofern die Transition
/// keine eigene Permission angibt.
fn transition_permission_name(entity_type: &str, t: &Transition) -> String {
    t.permission
        .clone()
        .unwrap_or_else(|| format!("{entity_type}.{}", t.event))
}

