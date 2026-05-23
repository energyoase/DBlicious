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
            supports_ddl: true,             // Designer-Schemas laufen ueber diese Source.
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
        binding: &EntityBinding,
        query: &PageQuery,
    ) -> Result<EntityPage, SourceError> {
        let entity_type = match &binding.locator {
            shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
            other => {
                return Err(SourceError::UnsupportedLocator(format!("{other:?}")));
            }
        };
        let page = crate::data::entities_page_raw(
            entity_type,
            query.page,
            query.page_size,
            query.sort.clone(),
            query.filter.clone(),
        )
        .await;
        Ok(page)
    }

    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        let entity_type = match &binding.locator {
            shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        let key = match id {
            EntityId::Single(s) => s.clone(),
            EntityId::Composite(_) => {
                return Err(SourceError::UnsupportedLocator(
                    "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
                ));
            }
        };
        Ok(crate::data::entity_by_id_raw(entity_type, &key).await)
    }

    async fn create(
        &self,
        binding: &EntityBinding,
        id: Option<String>,
        fields: serde_json::Map<String, serde_json::Value>,
        actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let entity_type = match &binding.locator {
            shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        Ok(crate::data::create_entity_raw(entity_type, id, fields, actor_user_id).await)
    }

    async fn update(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
        patch: serde_json::Map<String, serde_json::Value>,
        actor_user_id: Option<&str>,
    ) -> Result<Option<Entity>, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let entity_type = match &binding.locator {
            shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        let key = match id {
            EntityId::Single(s) => s.clone(),
            EntityId::Composite(_) => {
                return Err(SourceError::UnsupportedLocator(
                    "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
                ));
            }
        };
        Ok(crate::data::update_entity_raw(entity_type, &key, patch, actor_user_id).await)
    }

    async fn delete(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<bool, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let entity_type = match &binding.locator {
            shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
            other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        };
        let key = match id {
            EntityId::Single(s) => s.clone(),
            EntityId::Composite(_) => {
                return Err(SourceError::UnsupportedLocator(
                    "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
                ));
            }
        };
        Ok(crate::data::delete_entity_raw(entity_type, &key).await)
    }

    async fn apply_schema(
        &self,
        schema: &shared::DbSchema,
    ) -> Result<usize, SourceError> {
        // Delegiert an den bestehenden ddl-Pfad (CREATE TABLE IF NOT EXISTS).
        // try_apply_schema toleriert Statement-Level-Fehler intern und liefert
        // die Anzahl erfolgreich ausgefuehrter Statements.
        Ok(crate::ddl::try_apply_schema(schema).await)
    }
}
