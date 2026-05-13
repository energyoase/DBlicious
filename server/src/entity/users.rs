//! User-Tabelle.
//!
//! Spiegelt `shared::SecurityUser` — bis auf `group_ids`, die ueber die
//! n:n-Verknuepfungstabelle `user_groups` aufgeloest werden.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text", unique)]
    pub username: String,
    #[sea_orm(column_type = "Text")]
    pub display_name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub locale: Option<String>,
    pub active: bool,
    /// Argon2-PHC-String. `None` waere ungueltig — wird bei Seed-Insert
    /// immer gefuellt.
    #[sea_orm(column_type = "Text", nullable)]
    pub password_hash: Option<String>,
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
