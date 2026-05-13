//! Roles-Tabelle (Phase 0.7).
//!
//! Eine Role ist ein benanntes Permission-Buendel. Permissions referenzieren
//! sie ueber `Subject::Role { id }`; User/Groups bekommen Rollen ueber
//! [`super::role_assignments`].

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "roles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub name_key: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description_key: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::role_assignments::Entity")]
    Assignments,
}

impl Related<super::role_assignments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Assignments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
