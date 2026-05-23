//! Append-only Versions-Historie pro Skript (Q0009 Phase 3.1).
//!
//! Jeder erfolgreiche oder fehlgeschlagene Save-Schritt erzeugt eine neue
//! Zeile. Composite-PK: `(script_id, version)`. `state_at_save` und
//! `last_error` sind ein Snapshot des Zustands zum Save-Zeitpunkt — die
//! "Head"-Zeile in [`super::script`] wird einfach ueberschrieben, die
//! Geschichte bleibt hier.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "script_versions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub script_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub version: i32,
    /// `shared::script::ScriptManifest` als JSON.
    #[sea_orm(column_type = "Text")]
    pub manifest_json: String,
    /// Rhai-Source zur Save-Zeit.
    #[sea_orm(column_type = "Text")]
    pub source: String,
    /// `draft` | `active` | `locked`.
    #[sea_orm(column_type = "Text")]
    pub state_at_save: String,
    /// `shared::script::ScriptError` als JSON, wenn diese Version mit Fehler
    /// gespeichert wurde; sonst NULL.
    #[sea_orm(column_type = "Text", nullable)]
    pub last_error: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub saved_by: String,
    /// ISO-8601 UTC (RFC-3339).
    #[sea_orm(column_type = "Text")]
    pub saved_at: String,
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
