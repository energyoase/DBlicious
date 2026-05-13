//! Generische Entity-Tabelle.
//!
//! Composite-PK aus `(entity_type, id)`. `fields_json` traegt den
//! kompletten Feldmap der [`shared::Entity`] als JSON-String. `hash` ist
//! redundant zur on-the-fly-Berechnung in [`shared::compute_hash`], wird
//! aber gespeichert, um Concurrency-Checks ohne JSON-Parse durchzufuehren.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entities")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub entity_type: String,
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub fields_json: String,
    #[sea_orm(column_type = "Text")]
    pub hash: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
