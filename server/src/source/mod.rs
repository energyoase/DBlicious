//! Source-Architektur (Phase 0.6).
//!
//! Eine `Source` ist eine Datenquelle (lokale managed-sqlite, fremde
//! foreign-sqlite, spaeter postgres/rest/file). Die `SourceRegistry`
//! routet pro `EntityBinding` zur richtigen Implementierung. CRUD-
//! Resolver gehen ueber diesen Pfad.

pub mod managed_sqlite;
// `foreign_sqlite` kommt in Task 14.

use std::collections::BTreeMap;

use async_trait::async_trait;
use thiserror::Error;

use shared::source::{EntityBinding, EntityId};
use shared::{Entity, EntityPage, FilterCriteria, Sort};

/// Statischer Capability-Vektor pro Source-Implementierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub supports_write: bool,
    pub supports_transactions: bool,
    pub supports_sql_pushdown: bool,
    pub supports_introspection: bool,
    pub supports_composite_pk: bool,
}

/// Bündelt die Such-/Sort-/Pagination-Args fuer einen Listen-Aufruf.
#[derive(Debug, Clone, Default)]
pub struct PageQuery {
    pub page: i32,
    pub page_size: i32,
    pub sort: Option<Sort>,
    pub filter: FilterCriteria,
}

/// Fehler-Klasse fuer Source-Operationen.
#[derive(Debug, Error)]
pub enum SourceError {
    #[error("source or binding is read-only")]
    ReadOnly,
    #[error("locator not supported by this source: {0}")]
    UnsupportedLocator(String),
    #[error("source not found in registry: {0}")]
    UnknownSource(String),
    #[error("entity not found")]
    NotFound,
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("{0}")]
    Other(String),
}

/// Pluggable Datenquelle.
#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    /// Wird einmal beim Server-Start aufgerufen.
    async fn init(&mut self) -> Result<(), SourceError>;

    async fn list_page(
        &self,
        binding: &EntityBinding,
        query: &PageQuery,
    ) -> Result<EntityPage, SourceError>;

    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError>;

    async fn create(
        &self,
        binding: &EntityBinding,
        fields: serde_json::Map<String, serde_json::Value>,
        actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError>;

    async fn update(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
        patch: serde_json::Map<String, serde_json::Value>,
        actor_user_id: Option<&str>,
    ) -> Result<Option<Entity>, SourceError>;

    async fn delete(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<bool, SourceError>;
}

/// In-Process-Routing: pro Source-Name eine boxed Implementierung.
pub struct SourceRegistry {
    sources: BTreeMap<String, Box<dyn Source>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self { sources: BTreeMap::new() }
    }

    pub fn register(&mut self, source: Box<dyn Source>) {
        self.sources.insert(source.name().to_string(), source);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Source> {
        self.sources.get(name).map(|b| b.as_ref())
    }

    pub fn route(&self, binding: &EntityBinding) -> Result<&dyn Source, SourceError> {
        self.get(&binding.source)
            .ok_or_else(|| SourceError::UnknownSource(binding.source.clone()))
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.sources.keys().map(String::as_str)
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
