//! Skript-Storage (Q0009 Phase 3.1).
//!
//! Pro Skript eine Zeile mit der aktuellen, "Head"-Version. Die volle
//! Versionshistorie liegt in [`super::script_version`] (append-only). Pro Run
//! schreibt der Sandbox-Driver einen Eintrag in [`super::script_audit_log`].
//!
//! Felder:
//!   - `id` — ID gemaess [`shared::script::ScriptId`].
//!   - `kind` — `provider`/`component`/`wasm`, gespeichert als
//!     camelCase-String und gegen das Manifest abgeglichen.
//!   - `kind_json` — vollstaendiges `shared::script::ScriptKind` als JSON
//!     (inkl. `slot` bei `provider`). `kind` bleibt als Backward-Compat-Tag
//!     erhalten; `kind_json` wird fuer den GQL-Output genutzt.
//!   - `manifest` — `shared::script::ScriptManifest` als JSON.
//!   - `source` — Rhai-Source (bei `wasm` ein leerer String — Spec §11).
//!   - `version` — Monotone Versionsnummer. `save_script` rejiziert
//!     nicht-monoton wachsende Versionen.
//!   - `state` — `draft`/`active`/`locked` (lowercase, vgl.
//!     `ScriptState`-Wire-Format).
//!   - `last_error` — Optional letzter Compile-/Manifest-/Tier-Fehler
//!     (als JSON). Wird beim erfolgreichen Save geleert.
//!   - `created_by` — User-ID des Anlegers.
//!   - `created_at` — ISO-8601 UTC (RFC-3339).
//!   - `updated_at` — ISO-8601 UTC (RFC-3339).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "scripts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    /// camelCase-Tag wie auf der Wire-Form (`provider` / `component` /
    /// `wasm`). Inner-Felder leben im `manifest_json`/`source`.
    #[sea_orm(column_type = "Text")]
    pub kind: String,
    /// Vollstaendiges `shared::script::ScriptKind` als JSON (inkl. `slot`
    /// bei `provider`). Neue Zeilen setzen dieses Feld; alte Zeilen in
    /// persisted File-DBs liefern leer (Fallback auf `kind`-Tag).
    #[sea_orm(column_type = "Text")]
    pub kind_json: String,
    /// `shared::script::ScriptManifest` als JSON.
    #[sea_orm(column_type = "Text")]
    pub manifest_json: String,
    /// Rhai-Source. Bei `kind=wasm` heute leer (reserved).
    #[sea_orm(column_type = "Text")]
    pub source: String,
    pub version: i32,
    /// `draft` | `active` | `locked`.
    #[sea_orm(column_type = "Text")]
    pub state: String,
    /// `shared::script::ScriptError` als JSON, wenn der letzte Save in einen
    /// Draft-Zustand muendete; sonst NULL.
    #[sea_orm(column_type = "Text", nullable)]
    pub last_error: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub created_by: String,
    /// ISO-8601 UTC (RFC-3339).
    #[sea_orm(column_type = "Text")]
    pub created_at: String,
    /// ISO-8601 UTC (RFC-3339).
    #[sea_orm(column_type = "Text")]
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::script_version::Entity")]
    Versions,
    #[sea_orm(has_many = "super::script_audit_log::Entity")]
    AuditLog,
}

impl Related<super::script_version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Versions.def()
    }
}

impl Related<super::script_audit_log::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AuditLog.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
