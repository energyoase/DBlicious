//! Per-User-Wahl einer Implementations-ID (Phase 1.5.3).
//!
//! Wenn `field_type_defaults` mehrere `allowed_*_ids` zulaesst, kann der User
//! eine davon waehlen. Diese Tabelle persistiert die Wahl pro
//! `(user_id, entity_type, property, registry)`.
//!
//! `registry` ist eine Registry-Art: `"filter"`, `"editor"`, `"formatter"`,
//! `"action"` — analog zu [`shared::auth::Resource::ImplementationId.registry`].

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_implementation_choices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub user_id: String,
    #[sea_orm(column_type = "Text")]
    pub entity_type: String,
    #[sea_orm(column_type = "Text")]
    pub property: String,
    #[sea_orm(column_type = "Text")]
    pub registry: String,
    #[sea_orm(column_type = "Text")]
    pub chosen_id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
