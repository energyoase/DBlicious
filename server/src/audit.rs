//! Audit-Log-Schicht (Phase 0.7.6).
//!
//! Schreibt Eintraege in die `audit_log`-Tabelle. Failt **nicht** den Aufrufer,
//! wenn das Schreiben fehlschlaegt (z.B. wenn der DB-Pool noch nicht
//! initialisiert ist) — das Audit ist eine Beobachtungs-Schicht, kein
//! Korrektheits-Kern, und darf den Hauptpfad nicht stoeren.

use chrono::Utc;
use sea_orm::ActiveValue;

use crate::db::conn;
use crate::entity;

/// Klassifikation eines Audit-Eintrags.
#[derive(Debug, Clone, Copy)]
pub enum AuditKind {
    /// Zugriff wurde vom Resolver verweigert.
    Deny,
    /// Eine Permission wurde hinzugefuegt (kuenftiger Mutation-Endpoint).
    PermissionAdded,
    /// Eine Permission wurde entfernt.
    PermissionRemoved,
    /// Phase 1.7.5: State-Machine-Transition erfolgt.
    StateTransition,
}

impl AuditKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditKind::Deny => "deny",
            AuditKind::PermissionAdded => "permission_added",
            AuditKind::PermissionRemoved => "permission_removed",
            AuditKind::StateTransition => "state_transition",
        }
    }
}

/// Schreibt einen Eintrag in das Audit-Log. Best-effort: Fehler werden
/// als `tracing::warn` geloggt, nicht propagiert.
pub async fn record(
    kind: AuditKind,
    actor_user_id: Option<&str>,
    resource_kind: Option<&str>,
    resource_id: Option<&str>,
    op: Option<&str>,
    payload: Option<serde_json::Value>,
) {
    use sea_orm::ActiveModelTrait;
    let db = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(conn)) {
        Ok(db) => db,
        Err(_) => {
            // Pool nicht da (z.B. waehrend Test-Teardown). Stillschweigend
            // weglaufen — kein Audit ist besser als ein Crash.
            tracing::debug!(target: "server::audit", "no db pool — audit suppressed");
            return;
        }
    };
    let am = entity::audit_log::ActiveModel {
        id: ActiveValue::NotSet,
        occurred_at: ActiveValue::Set(Utc::now().to_rfc3339()),
        kind: ActiveValue::Set(kind.as_str().to_string()),
        actor_user_id: ActiveValue::Set(actor_user_id.map(str::to_string)),
        resource_kind: ActiveValue::Set(resource_kind.map(str::to_string)),
        resource_id: ActiveValue::Set(resource_id.map(str::to_string)),
        op: ActiveValue::Set(op.map(str::to_string)),
        payload_json: ActiveValue::Set(payload.map(|v| v.to_string())),
    };
    if let Err(e) = am.insert(&db).await {
        tracing::warn!(target: "server::audit", "audit insert failed: {e}");
    }
}

/// Bequemer Helfer fuer den haeufigsten Fall: ein abgelehnter
/// CRUD-/Action-Aufruf.
pub async fn record_deny(actor_user_id: &str, entity_type: &str, op: &str) {
    record(
        AuditKind::Deny,
        Some(actor_user_id),
        Some("entityType"),
        Some(entity_type),
        Some(op),
        None,
    )
    .await;
}

/// Phase 1.7.5: Audit-Eintrag fuer eine State-Machine-Transition.
/// `actor_user_id = None` ⇒ System-Triggers (z.B. automatischer
/// Approval-Folge-Schritt).
pub async fn record_state_transition(
    actor_user_id: Option<&str>,
    entity_type:   &str,
    entity_id:     &str,
    from_state:    &str,
    to_state:      &str,
    event:         &str,
) {
    record(
        AuditKind::StateTransition,
        actor_user_id,
        Some("entityInstance"),
        Some(&format!("{entity_type}/{entity_id}")),
        Some(event),
        Some(serde_json::json!({
            "from":  from_state,
            "to":    to_state,
            "event": event,
        })),
    )
    .await;
}
