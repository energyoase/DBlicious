//! Skript-Audit-Log (Q0009 Phase 3.4).
//!
//! Eine Zeile pro Skript-Run. Das `tokens_used`-Feld haellt die Liste der
//! waehrend des Runs aufgerufenen `CapabilityToken`s inkl. Outcome
//! (`ok`/`denied`) als JSON-Array — gespeist aus
//! [`crate::script::sandbox::Sandbox::token_uses`].
//!
//! Persistente Sicht ist append-only: weder Update noch Delete im
//! Produktivpfad. Der `reset()`-Hook in `db.rs` ist nur fuer Tests.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "script_audit_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub script_id: String,
    /// Welche Skript-Version wurde ausgefuehrt — Cross-Referenz auf
    /// `script_versions(script_id, version)`.
    pub script_version: i32,
    /// Run-Identifier (ULID). Im Mock/Test ein zufaelliger String.
    #[sea_orm(column_type = "Text")]
    pub run_id: String,
    /// Aufrufender User, optional (z.B. System-Tasks).
    #[sea_orm(column_type = "Text", nullable)]
    pub user_id: Option<String>,
    /// ISO-8601 UTC (RFC-3339).
    #[sea_orm(column_type = "Text")]
    pub started_at: String,
    /// ISO-8601 UTC (RFC-3339).
    #[sea_orm(column_type = "Text")]
    pub finished_at: String,
    /// `ok` | `capabilityDenied` | `timeout` | `internalPanic` |
    /// `parseFailed` | `manifestInvalid` | `tierExceeded` | ... — wir
    /// uebernehmen den Tag-Wert der `ScriptError`-Variante (camelCase) und
    /// nutzen `ok` fuer den Success-Pfad.
    #[sea_orm(column_type = "Text")]
    pub outcome: String,
    /// Token-Liste als JSON. Format pro Eintrag:
    /// `{"token": <CapabilityToken-Wire>, "outcome": "ok"|"denied"}`.
    #[sea_orm(column_type = "Text")]
    pub tokens_used: String,
    /// Beliebige skript-spezifische Events (heute nur fuer
    /// `audit.log()`-Calls aus den Skripten selbst gefuellt). Wenn leer,
    /// `[]`.
    #[sea_orm(column_type = "Text")]
    pub custom_events: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::script::Entity",
        from = "Column::ScriptId",
        to = "super::script::Column::Id"
    )]
    Script,
}

impl Related<super::script::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Script.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
