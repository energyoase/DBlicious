//! Plugin-Audit-Log (Phase 2.8).
//!
//! Ein Eintrag pro Plugin-Aufruf: welches Plugin, welcher Trigger/Function,
//! Dauer in Millisekunden, Outcome (`ok`, `error`, `timeout`,
//! `forbidden`, `capability_violation`), optional Input-Hash zur
//! Reproduktion ohne den vollen Payload zu loggen.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "plugin_invocations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub plugin_id: String,
    #[sea_orm(column_type = "Text")]
    pub function_name: String,
    /// `shared::plugin::TriggerKind` als camelCase-String.
    #[sea_orm(column_type = "Text")]
    pub trigger_kind: String,
    /// Welche Entity wurde getrieben (None bei `CustomAction` ohne Entity).
    #[sea_orm(column_type = "Text", nullable)]
    pub entity_type: Option<String>,
    /// User, der den Trigger ausgeloest hat.
    #[sea_orm(column_type = "Text", nullable)]
    pub actor_user_id: Option<String>,
    /// ISO-8601-UTC.
    #[sea_orm(column_type = "Text")]
    pub started_at: String,
    pub duration_ms: i32,
    /// `ok` | `error` | `timeout` | `forbidden` | `capability_violation`.
    #[sea_orm(column_type = "Text")]
    pub outcome: String,
    /// Optionaler Hash des Inputs (z.B. `compute_hash`-aequivalent) fuer
    /// Replay-Analyse. Kein Klartext-Payload — DSGVO-freundlich.
    #[sea_orm(column_type = "Text", nullable)]
    pub input_hash: Option<String>,
    /// Bei `outcome != "ok"`: maschinen-lesbarer Fehlercode + Message.
    #[sea_orm(column_type = "Text", nullable)]
    pub error_code: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub error_message: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::plugins::Entity",
        from = "Column::PluginId",
        to = "super::plugins::Column::Id"
    )]
    Plugin,
}

impl Related<super::plugins::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Plugin.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
