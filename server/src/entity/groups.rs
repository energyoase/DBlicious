//! Group-Tabelle.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub name_key: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description_key: Option<String>,
    /// `Vec<shared::Permission>` als JSON-String.
    #[sea_orm(column_type = "Text")]
    pub permissions_json: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_groups::Entity")]
    UserGroups,
}

impl Related<super::user_groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserGroups.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
