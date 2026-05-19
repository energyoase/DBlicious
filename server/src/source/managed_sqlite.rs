//! `ManagedSqliteSource` — heutiges Verhalten (eigene Tabellen, JSON-Blob
//! in `entities`-Tabelle) gewrappt unter die `Source`-Trait.

use async_trait::async_trait;

use shared::source::{EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::{Capabilities, PageQuery, Source, SourceError};

pub struct ManagedSqliteSource {
    name: String,
}

impl ManagedSqliteSource {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Source for ManagedSqliteSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> &'static str {
        "managed-sqlite"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,    // potentielle Faehigkeit, heute nicht ausgenutzt
            supports_introspection: false,  // wir kennen unser Schema durch SeaORM-Entities
            supports_composite_pk: false,   // entities-Tabelle hat (entity_type, id) als PK,
                                            // aus User-Sicht ist das aber Single-PK
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        // Delegiert an den bestehenden Boot-Pfad. Idempotent durch
        // `db::init`'s OnceLock-Slot.
        crate::db::init().await.map_err(SourceError::from)?;
        Ok(())
    }

    async fn list_page(
        &self,
        _binding: &EntityBinding,
        _query: &PageQuery,
    ) -> Result<EntityPage, SourceError> {
        unimplemented!("Task 5")
    }

    async fn get(
        &self,
        _binding: &EntityBinding,
        _id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        unimplemented!("Task 6")
    }

    async fn create(
        &self,
        _binding: &EntityBinding,
        _fields: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError> {
        unimplemented!("Task 6")
    }

    async fn update(
        &self,
        _binding: &EntityBinding,
        _id: &EntityId,
        _patch: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Option<Entity>, SourceError> {
        unimplemented!("Task 6")
    }

    async fn delete(
        &self,
        _binding: &EntityBinding,
        _id: &EntityId,
    ) -> Result<bool, SourceError> {
        unimplemented!("Task 6")
    }
}
