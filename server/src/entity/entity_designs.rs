//! Builder-Design-Persistenz (Phase 1.6).
//!
//! Append-only Tabelle: jeder `saveEntityDesign` schreibt eine neue Zeile mit
//! monoton wachsender `version` pro `entity_type`. Die aktive Version ist
//! `MAX(version) WHERE entity_type = ?`. Alte Versionen bleiben fuer Revert
//! und Audit erhalten.
//!
//! `state` ist das vom Client gelieferte JSON-Dokument (`tree.nodes` +
//! `projection`). Der Server kennt die Form nicht typisiert — er nimmt sie
//! als opaque JSON-Blob und versteht nur den projizierten Teil
//! (`projection.columns/settings/editor`), den `data::*` liest.
//!
//! `created_by` ist eine User-ID. Beim DB-Boot-Snapshot ist das die
//! reservierte ID `"system"` (siehe `data::ensure_system_user`).
//!
//! `locked` = 1 markiert einen Codegen-Snapshot (Phase 4); diese Versionen
//! sind unveraenderlich. Heute nur Skeleton-Feld — kein Producer schreibt 1.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entity_designs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub entity_type: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub version: i32,
    /// Voller `state`-Blob als JSON-Text.
    #[sea_orm(column_type = "Text")]
    pub state_json: String,
    /// Form-Version des `state`-Blobs. `shared::builder::*::migrate_state` ist
    /// dafuer zustaendig, alte Werte hochzumigrieren.
    pub schema_version: i32,
    /// ISO-8601-UTC.
    #[sea_orm(column_type = "Text")]
    pub created_at: String,
    /// User-ID (oder `"system"` fuer Boot-Snapshot).
    #[sea_orm(column_type = "Text")]
    pub created_by: String,
    #[sea_orm(default_value = false)]
    pub locked: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
