//! Permission-Tabelle (Phase 0.7).
//!
//! Flache Speicherform fuer `shared::auth::Permission`. Resource und Subject
//! werden ueber ihre `_kind`/`_id`-Spalten kodiert; siehe
//! [`shared::auth::Resource::storage_id`] und
//! [`shared::auth::Subject::kind_str`] fuer das genaue Format.
//!
//! `tenant_id` ist immer NULL, solange Multi-Tenancy nicht aktiv ist
//! (Phase-0.7-Schema-Vorbereitung, kein Enforcement).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "permissions")]
pub struct Model {
    /// Synthetisches Auto-Increment-PK; vereinfacht die Wartung gegenueber
    /// einem zusammengesetzten Schluessel ueber alle inhaltlichen Spalten.
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub subject_kind: String,
    #[sea_orm(column_type = "Text")]
    pub subject_id: String,
    #[sea_orm(column_type = "Text")]
    pub resource_kind: String,
    #[sea_orm(column_type = "Text")]
    pub resource_id: String,
    #[sea_orm(column_type = "Text")]
    pub op: String,
    #[sea_orm(column_type = "Text")]
    pub effect: String,
    #[sea_orm(default_value = 0)]
    pub priority: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
