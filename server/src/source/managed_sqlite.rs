//! `ManagedSqliteSource` — heutiges Verhalten (eigene Tabellen, JSON-Blob
//! in `entities`-Tabelle) gewrappt unter die `Source`-Trait.
//!
//! Seit Phase 0.6 B4 zusaetzlich `BindingLocator::Table`-Support: eine
//! Entitaet kann auf eine echte SQL-Tabelle innerhalb der managed-sqlite
//! DB zeigen (statt JSON-Blob in `entities`). Code-Pfad teilt die SQL-
//! Helfer mit `foreign_sqlite` (siehe `super::sql`).

use async_trait::async_trait;
use sea_orm::DbBackend;

use shared::source::{BindingLocator, EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::sql;
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
            supports_sql_pushdown: true, // im Table-Locator-Pfad echt genutzt (B4)
            supports_introspection: false, // wir kennen unser Schema durch SeaORM-Entities
            supports_composite_pk: true, // im Table-Locator-Pfad voll unterstuetzt (B4);
            // GenericEntityRow bleibt Single-PK.
            supports_ddl: true, // Designer-Schemas laufen ueber diese Source.
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
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => Ok(crate::data::entities_page_raw(
                entity_type,
                query.page,
                query.page_size,
                query.sort.clone(),
                query.filter.clone(),
            )
            .await),
            BindingLocator::Table { table } => {
                sql::relational_list_page(
                    &crate::db::conn(),
                    DbBackend::Sqlite,
                    binding,
                    table,
                    query,
                    None,
                )
                .await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => {
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
            BindingLocator::Table { table } => {
                sql::relational_get(
                    &crate::db::conn(),
                    DbBackend::Sqlite,
                    binding,
                    table,
                    id,
                    None,
                )
                .await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
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
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => {
                Ok(crate::data::create_entity_raw(entity_type, id, fields, actor_user_id).await)
            }
            BindingLocator::Table { table } => {
                sql::relational_create(
                    &crate::db::conn(),
                    DbBackend::Sqlite,
                    binding,
                    table,
                    fields,
                )
                .await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
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
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => {
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
            BindingLocator::Table { table } => {
                sql::relational_update(
                    &crate::db::conn(),
                    DbBackend::Sqlite,
                    binding,
                    table,
                    id,
                    patch,
                    None,
                )
                .await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    async fn delete(&self, binding: &EntityBinding, id: &EntityId) -> Result<bool, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        match &binding.locator {
            BindingLocator::GenericEntityRow { entity_type } => {
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
            BindingLocator::Table { table } => {
                sql::relational_delete(&crate::db::conn(), DbBackend::Sqlite, binding, table, id)
                    .await
            }
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    async fn apply_schema(&self, schema: &shared::DbSchema) -> Result<usize, SourceError> {
        // Delegiert an den bestehenden ddl-Pfad (CREATE TABLE IF NOT EXISTS).
        // try_apply_schema toleriert Statement-Level-Fehler intern und liefert
        // die Anzahl erfolgreich ausgefuehrter Statements.
        Ok(crate::ddl::try_apply_schema(schema).await)
    }
}
