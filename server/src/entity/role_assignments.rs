//! Role-Assignment-Tabelle (Phase 0.7).
//!
//! Verbindet User oder Group mit einer Role. `subject_kind` ist
//! `"user"` oder `"group"` — `"role"` (Role-Hierarchie) ist bewusst nicht
//! erlaubt; der Loader validiert das.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "role_assignments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub subject_kind: String,
    #[sea_orm(column_type = "Text")]
    pub subject_id: String,
    #[sea_orm(column_type = "Text")]
    pub role_id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::roles::Entity",
        from = "Column::RoleId",
        to = "super::roles::Column::Id"
    )]
    Role,
}

impl Related<super::roles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Role.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
