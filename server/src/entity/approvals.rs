//! Approval-Workflow (Phase 1.7.6, Single-Stage MVP).
//!
//! Eine `approvals`-Row repraesentiert eine offene oder abgeschlossene
//! Freigabe-Anforderung fuer ein konkretes Entity-Instance + Event.
//! Beim `approve` auf der letzten Stage triggert der Service die
//! gebundene State-Machine-Transition.
//!
//! Multi-Stage + Delegation: kommen als Folge-Item; das Schema laesst
//! beides offen (`current_stage_idx`, `last_decision_at`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "approvals")]
pub struct Model {
    /// ULID/UUID — vom Service vergeben.
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    /// Entity, das freigegeben werden soll.
    #[sea_orm(column_type = "Text")]
    pub entity_type: String,
    #[sea_orm(column_type = "Text")]
    pub entity_id: String,
    /// State-Machine-Event, das beim finalen `approve` getriggert wird
    /// (z.B. `"post"` fuer Invoice → posted). Service ruft danach
    /// `state_machine::apply_transition`.
    #[sea_orm(column_type = "Text")]
    pub target_event: String,
    /// `"pending"` | `"approved"` | `"rejected"`.
    #[sea_orm(column_type = "Text")]
    pub status: String,
    /// Multi-Stage-Vorbereitung. Heute immer `0`.
    pub current_stage_idx: i32,
    /// User, der angefordert hat.
    #[sea_orm(column_type = "Text", nullable)]
    pub requested_by: Option<String>,
    /// ISO-8601 — wann angefordert.
    #[sea_orm(column_type = "Text")]
    pub requested_at: String,
    /// ISO-8601 — wann zuletzt entschieden (approve/reject).
    #[sea_orm(column_type = "Text", nullable)]
    pub last_decision_at: Option<String>,
    /// User, der entschieden hat (im Single-Stage = approver).
    #[sea_orm(column_type = "Text", nullable)]
    pub last_decided_by: Option<String>,
    /// Freitext-Kommentar zur Entscheidung.
    #[sea_orm(column_type = "Text", nullable)]
    pub comment: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
