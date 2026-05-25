//! `FileSource` — statische JSON-Datei als Datenquelle (Phase 0.6 B3).
//!
//! Layout der Datei (JSON):
//! ```json
//! {
//!   "tables": {
//!     "products": [ { "id": "p1", "fields": { "name": "Hammer" } }, ... ],
//!     "...":      [ ... ]
//!   }
//! }
//! ```
//!
//! Beim `init()` wird die Datei einmal eingelesen. Mutate-Operationen
//! aktualisieren den In-Memory-Stand und schreiben die Datei im selben
//! Format zurueck (atomarer Write via Tempfile + rename, falls Source nicht
//! read-only ist).
//!
//! Akzeptiert `BindingLocator::Table { table }`. Filter/Sort/Pagination
//! laufen in Rust analog zu MemorySource.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use shared::source::{BindingLocator, EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::{Capabilities, PageQuery, Source, SourceError};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct FileLayout {
    #[serde(default)]
    tables: BTreeMap<String, Vec<Entity>>,
}

pub struct FileSource {
    name: String,
    path: PathBuf,
    store: Arc<RwLock<FileLayout>>,
}

impl FileSource {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            store: Arc::new(RwLock::new(FileLayout::default())),
        }
    }

    fn persist(&self) -> Result<(), SourceError> {
        let layout = self.store.read().clone();
        let json = serde_json::to_string_pretty(&layout)
            .map_err(|e| SourceError::Other(format!("serialise: {e}")))?;
        // Atomarer Write: tmp-File + rename.
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, json).map_err(|e| SourceError::Other(format!("write tmp: {e}")))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| SourceError::Other(format!("rename: {e}")))?;
        Ok(())
    }

    fn ensure_table_locator(locator: &BindingLocator) -> Result<String, SourceError> {
        match locator {
            BindingLocator::Table { table } => Ok(table.clone()),
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }
}

#[async_trait]
impl Source for FileSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> &'static str {
        "file"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true, // pro-Binding via read_only steuerbar
            supports_transactions: false,
            supports_sql_pushdown: false,
            supports_introspection: false,
            supports_composite_pk: true,
            supports_ddl: false,
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        if !self.path.exists() {
            *self.store.write() = FileLayout::default();
            return Ok(());
        }
        let raw = std::fs::read_to_string(&self.path)
            .map_err(|e| SourceError::Other(format!("read {}: {e}", self.path.display())))?;
        let layout: FileLayout = serde_json::from_str(&raw)
            .map_err(|e| SourceError::Other(format!("parse {}: {e}", self.path.display())))?;
        *self.store.write() = layout;
        Ok(())
    }

    async fn list_page(
        &self,
        binding: &EntityBinding,
        query: &PageQuery,
    ) -> Result<EntityPage, SourceError> {
        let table = Self::ensure_table_locator(&binding.locator)?;
        let rows = self
            .store
            .read()
            .tables
            .get(&table)
            .cloned()
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
            reference_labels: Default::default(),
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
            .tables
            .get(&table)
            .and_then(|t| t.iter().find(|e| e.id == key).cloned()))
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
        let id_str = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let entity = Entity { id: id_str, fields };
        self.store
            .write()
            .tables
            .entry(table)
            .or_default()
            .push(entity.clone());
        self.persist()?;
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
        let updated = {
            let mut store = self.store.write();
            let Some(entries) = store.tables.get_mut(&table) else {
                return Ok(None);
            };
            let Some(existing) = entries.iter_mut().find(|e| e.id == key) else {
                return Ok(None);
            };
            for (k, v) in patch {
                existing.fields.insert(k, v);
            }
            existing.clone()
        };
        self.persist()?;
        Ok(Some(updated))
    }

    async fn delete(&self, binding: &EntityBinding, id: &EntityId) -> Result<bool, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = Self::ensure_table_locator(&binding.locator)?;
        let key = id.encode();
        let removed = {
            let mut store = self.store.write();
            let Some(entries) = store.tables.get_mut(&table) else {
                return Ok(false);
            };
            let before = entries.len();
            entries.retain(|e| e.id != key);
            before != entries.len()
        };
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }
}
