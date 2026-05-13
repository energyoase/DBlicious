//! Plugin-Storage (Phase 2.5).
//!
//! Pro Plugin eine Zeile: ID, Versions-String aus dem Manifest, das ganze
//! Manifest als JSON-Text (fuer Capabilities-Pruefung + Anzeige im Admin-UI),
//! der WASM-Bytecode als Blob, und ein `enabled`-Flag. Disabled Plugins
//! werden geladen aber nicht getriggert.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "plugins")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub version: String,
    /// `shared::plugin::PluginManifest` als JSON.
    #[sea_orm(column_type = "Text")]
    pub manifest_json: String,
    /// WASM-Bytecode. Heute als BLOB direkt in der DB — fuer grosse Plugins
    /// spaeter ggf. auf Filesystem auslagern (Spalte wuerde dann den Pfad
    /// halten). Akzeptabel bis ~MB-Bereich pro Plugin. SeaORM mappt
    /// `Vec<u8>` ohne explizite column_type auf BLOB.
    pub wasm_blob: Vec<u8>,
    #[sea_orm(default_value = true)]
    pub enabled: bool,
    /// ISO-8601 — wann das Plugin hochgeladen oder aktualisiert wurde.
    #[sea_orm(column_type = "Text")]
    pub installed_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::plugin_invocations::Entity")]
    Invocations,
}

impl Related<super::plugin_invocations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invocations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
