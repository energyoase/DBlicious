//! `MemorySource` — In-Memory-Mock fuer Tests und Demos (Phase 0.6 B3).
//!
//! Hält die Daten pro `(table, id)` in einer `BTreeMap`. Filter/Sort/
//! Pagination laufen in Rust (kein Pushdown). Akzeptiert
//! `BindingLocator::Table { table }`; andere Locator-Kinds werden
//! abgelehnt.
//!
//! Use cases:
//!   - Unit-Tests, die eine Source brauchen, aber keinen DB-Roundtrip
//!   - Demo-Examples ohne Persistenz
//!   - Plugin-Sandbox-Tests

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use shared::source::{BindingLocator, EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::{Capabilities, PageQuery, Source, SourceError};

type Table = BTreeMap<String, Entity>;
type Store = BTreeMap<String, Table>;

pub struct MemorySource {
    name: String,
    store: Arc<RwLock<Store>>,
}

impl MemorySource {
    pub fn new(name: String) -> Self {
        Self {
            name,
            store: Arc::new(RwLock::new(Store::new())),
        }
    }

    /// Seed-Helper fuer Tests: setzt eine Tabelle initial.
    pub fn seed_table(&self, table: &str, rows: impl IntoIterator<Item = Entity>) {
        let mut store = self.store.write();
        let entry = store.entry(table.to_string()).or_default();
        for r in rows {
            entry.insert(r.id.clone(), r);
        }
    }

    fn ensure_table_locator(locator: &BindingLocator) -> Result<String, SourceError> {
        match locator {
            BindingLocator::Table { table } => Ok(table.clone()),
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }
}

#[async_trait]
impl Source for MemorySource {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> &'static str {
        "memory"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true,
            supports_transactions: false,
            supports_sql_pushdown: false,
            supports_introspection: false,
            supports_composite_pk: true,
            supports_ddl: false,
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        Ok(())
    }

    async fn list_page(
        &self,
        binding: &EntityBinding,
        query: &PageQuery,
    ) -> Result<EntityPage, SourceError> {
        let table = Self::ensure_table_locator(&binding.locator)?;
        let store = self.store.read();
        let rows = store
            .get(&table)
            .map(|t| t.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let total = rows.len() as u64;
        let page = query.page.max(1) as usize;
        let page_size = query.page_size.max(1) as usize;
        let offset = (page - 1) * page_size;
        let items: Vec<Entity> = rows.into_iter().skip(offset).take(page_size).collect();
        Ok(EntityPage {
            items,
            total_count: total,
            page: page as u32,
            page_size: page_size as u32,
        })
    }

    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        let table = Self::ensure_table_locator(&binding.locator)?;
        let key = id.encode();
        Ok(self
            .store
            .read()
            .get(&table)
            .and_then(|t| t.get(&key).cloned()))
    }

    async fn create(
        &self,
        binding: &EntityBinding,
        id: Option<String>,
        fields: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = Self::ensure_table_locator(&binding.locator)?;
        // ID-Ableitung: Caller-Vorgabe → PK-Spalten aus fields → leerer String.
        let id_str = id
            .or_else(|| {
                let parts: Vec<String> = binding
                    .primary_key
                    .iter()
                    .map(|k| {
                        fields
                            .get(k)
                            .map(|v| match v {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            })
                            .unwrap_or_default()
                    })
                    .collect();
                if parts.is_empty() || parts.iter().all(String::is_empty) {
                    None
                } else if parts.len() == 1 {
                    Some(parts.into_iter().next().unwrap())
                } else {
                    Some(shared::source::EntityId::Composite(parts).encode())
                }
            })
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let entity = Entity {
            id: id_str.clone(),
            fields,
        };
        self.store
            .write()
            .entry(table)
            .or_default()
            .insert(id_str, entity.clone());
        Ok(entity)
    }

    async fn update(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
        patch: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Option<Entity>, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = Self::ensure_table_locator(&binding.locator)?;
        let key = id.encode();
        let mut store = self.store.write();
        let table_map = store.entry(table).or_default();
        let Some(existing) = table_map.get_mut(&key) else {
            return Ok(None);
        };
        for (k, v) in patch {
            existing.fields.insert(k, v);
        }
        Ok(Some(existing.clone()))
    }

    async fn delete(&self, binding: &EntityBinding, id: &EntityId) -> Result<bool, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = Self::ensure_table_locator(&binding.locator)?;
        let key = id.encode();
        Ok(self
            .store
            .write()
            .get_mut(&table)
            .map(|t| t.remove(&key).is_some())
            .unwrap_or(false))
    }
}
