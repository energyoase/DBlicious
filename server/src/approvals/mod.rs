//! Approval-Workflow-Service (Phase 1.7.6 Single-Stage MVP).
//!
//! API:
//! - [`request`]: legt einen pending approval an
//! - [`decide`]: setzt approve|reject; bei approve triggert die
//!   gebundene State-Machine-Transition auf der Entity
//! - [`list_pending_for_entity`]: lookup
//!
//! Multi-Stage + Delegation: Folge-Item. Schema hat `current_stage_idx`
//! als Vorbereitung.

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use thiserror::Error;

use crate::audit;
use crate::entity::approvals;

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_APPROVED: &str = "approved";
pub const STATUS_REJECTED: &str = "rejected";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Reject,
}

impl Decision {
    pub fn as_status(self) -> &'static str {
        match self {
            Decision::Approve => STATUS_APPROVED,
            Decision::Reject => STATUS_REJECTED,
        }
    }
}

#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("approval_not_found: {0}")]
    NotFound(String),
    #[error("approval_already_decided: id={0}")]
    AlreadyDecided(String),
    #[error("transition_failed: {0}")]
    TransitionFailed(String),
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
}

/// Legt eine neue Approval-Anforderung an. Liefert die generierte ID.
pub async fn request(
    conn: &DatabaseConnection,
    entity_type: &str,
    entity_id: &str,
    target_event: &str,
    requested_by: Option<&str>,
) -> Result<String, ApprovalError> {
    let id = ulid::Ulid::new().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    approvals::ActiveModel {
        id: ActiveValue::Set(id.clone()),
        entity_type: ActiveValue::Set(entity_type.to_string()),
        entity_id: ActiveValue::Set(entity_id.to_string()),
        target_event: ActiveValue::Set(target_event.to_string()),
        status: ActiveValue::Set(STATUS_PENDING.to_string()),
        current_stage_idx: ActiveValue::Set(0),
        requested_by: ActiveValue::Set(requested_by.map(String::from)),
        requested_at: ActiveValue::Set(now),
        last_decision_at: ActiveValue::Set(None),
        last_decided_by: ActiveValue::Set(None),
        comment: ActiveValue::Set(None),
        tenant_id: ActiveValue::Set(None),
    }
    .insert(conn)
    .await?;
    Ok(id)
}

pub async fn get(
    conn: &DatabaseConnection,
    id: &str,
) -> Result<Option<approvals::Model>, ApprovalError> {
    Ok(approvals::Entity::find_by_id(id.to_string())
        .one(conn)
        .await?)
}

/// Entscheidet ueber eine pending approval. Bei Approve wird die
/// State-Machine-Transition auf der Entity getriggert; ein Fehler dort
/// laesst die Approval pending (kein partielles Schreiben).
pub async fn decide(
    conn: &DatabaseConnection,
    id: &str,
    decision: Decision,
    user_id: &str,
    comment: Option<&str>,
) -> Result<approvals::Model, ApprovalError> {
    let m = get(conn, id)
        .await?
        .ok_or_else(|| ApprovalError::NotFound(id.to_string()))?;
    if m.status != STATUS_PENDING {
        return Err(ApprovalError::AlreadyDecided(id.to_string()));
    }

    // Bei Approve: zuerst Transition triggern. Erst wenn erfolgreich,
    // approval-Status setzen — sonst bleibt approval pending und ein
    // Retry ist moeglich.
    if matches!(decision, Decision::Approve) {
        crate::state_machine::apply_transition(
            &m.entity_type,
            &m.entity_id,
            &m.target_event,
            Some(user_id),
        )
        .await
        .map_err(|e| ApprovalError::TransitionFailed(format!("{e}")))?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut am: approvals::ActiveModel = m.into();
    am.status = ActiveValue::Set(decision.as_status().to_string());
    am.last_decision_at = ActiveValue::Set(Some(now));
    am.last_decided_by = ActiveValue::Set(Some(user_id.to_string()));
    am.comment = ActiveValue::Set(comment.map(String::from));
    let updated = am.update(conn).await?;

    // Audit-Eintrag.
    audit::record_approval_decision(
        Some(user_id),
        &updated.entity_type,
        &updated.entity_id,
        &updated.target_event,
        decision,
        comment,
    )
    .await;

    Ok(updated)
}

/// Liefert alle pending approvals fuer ein konkretes Entity-Instance.
/// Hilft bei UI-Listen und beim Idempotenz-Check (kein doppelter
/// request).
pub async fn list_pending_for_entity(
    conn: &DatabaseConnection,
    entity_type: &str,
    entity_id: &str,
) -> Result<Vec<approvals::Model>, ApprovalError> {
    Ok(approvals::Entity::find()
        .filter(approvals::Column::EntityType.eq(entity_type))
        .filter(approvals::Column::EntityId.eq(entity_id))
        .filter(approvals::Column::Status.eq(STATUS_PENDING))
        .all(conn)
        .await?)
}
