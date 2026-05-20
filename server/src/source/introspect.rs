//! SQLite-Schema-Introspektion via sqlite_master + PRAGMA.
//!
//! Task 14 liefert nur das Datenmodell + einen `read_schema`-Stub, der
//! ein leeres `Schema` zurueckgibt. Die tatsaechliche Introspektion via
//! `PRAGMA table_info` kommt in Task 15.

use std::collections::BTreeMap;

use sea_orm::DatabaseConnection;

use super::SourceError;

#[derive(Debug, Default, Clone)]
pub struct Schema {
    pub tables: BTreeMap<String, TableMeta>,
}

#[derive(Debug, Clone)]
pub struct TableMeta {
    pub name: String,
    pub columns: Vec<ColumnMeta>,
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub sql_type: String,
    pub not_null: bool,
}

pub async fn read_schema(_db: &DatabaseConnection) -> Result<Schema, SourceError> {
    // Task 15 implementiert das.
    Ok(Schema::default())
}
