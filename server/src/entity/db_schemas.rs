//! Designer-Schema-Snapshots.
//!
//! `saveDbSchema` schreibt hier eine neue Zeile. `updated_at` ist die
//! Tiebreaker-Spalte fuer "der neueste gewinnt"; die ID ist der `name`,
//! womit nur ein Snapshot pro Schema existiert.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "db_schemas")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub name: String,
    /// `shared::DbSchema` als JSON.
    #[sea_orm(column_type = "Text")]
    pub schema_json: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
