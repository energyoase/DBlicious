//! Audit-Log-Tabelle (Phase 0.7.6).
//!
//! Jeder Eintrag dokumentiert ein Permission-relevantes Ereignis:
//! - `deny`: ein Zugriff wurde vom Resolver abgelehnt.
//! - `permission_added` / `permission_removed`: Audit fuer kuenftige
//!   Mutation-Endpoints (heute nicht erzeugt, aber Schema ist vorbereitet).
//!
//! Felder sind absichtlich flach gehalten; `resource_kind`/`resource_id`
//! folgen der kanonischen Form aus `shared::auth::Resource::storage_id`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// ISO-8601-Timestamp (UTC).
    #[sea_orm(column_type = "Text")]
    pub occurred_at: String,
    /// Klassifikation: `"deny"`, `"permission_added"`, `"permission_removed"`.
    #[sea_orm(column_type = "Text")]
    pub kind: String,
    /// User, dessen Aktion das Ereignis ausgeloest hat. Bei `deny` der
    /// abgelehnte User. `None` bei anonymen Aufrufen — eigentlich nicht
    /// erwartet, aber tolerant.
    #[sea_orm(column_type = "Text", nullable)]
    pub actor_user_id: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub resource_kind: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub resource_id: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub op: Option<String>,
    /// Frei-Form JSON fuer Zusatz-Kontext (z.B. die Permission-IDs, die
    /// veraendert wurden).
    #[sea_orm(column_type = "Text", nullable)]
    pub payload_json: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
