//! Persistenz der Named-Views (Q0005).
//!
//! Eine Row pro `(entity_type, view_name, layer, owner_id)`. Schichten
//! werden vom Resolver in `server/src/views.rs` zu einem `EntitySettings`-
//! Aequivalent gemerged.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entity_views")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub entity_type: String,
    #[sea_orm(column_type = "Text")]
    pub view_name: String,
    /// "global" | "group" | "user"  — als String gespeichert, weil
    /// die Wire-Form lower-case ist und kein `EnumIter`-Roundtrip
    /// gebraucht wird.
    #[sea_orm(column_type = "Text")]
    pub layer: String,
    /// NULL gdw `layer = "global"`. SeaORM macht aus dem UNIQUE-Index
    /// einen partiellen Index — Code-seitig zusaetzlich enforced in
    /// `views::insert_view`.
    #[sea_orm(column_type = "Text", nullable)]
    pub owner_id: Option<String>,
    /// JSON-Blob: `{ properties, defaultFilter, defaultSort, defaultPageSize }`.
    #[sea_orm(column_type = "Text")]
    pub payload: String,
    pub version: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub updated_by: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
