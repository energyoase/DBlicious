//! Editor-Metadaten pro Entity-Typ.
//!
//! Heute optional gefuettert (in-Memory-Defaults aus `data::editor_for`
//! gewinnen, wenn die Tabelle leer ist).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "metadata_editor")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub entity_type: String,
    /// `shared::EditorMeta` als JSON.
    #[sea_orm(column_type = "Text")]
    pub meta_json: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
