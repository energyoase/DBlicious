//! Source-Architektur (Phase 0.6).
//!
//! Eine `Source` ist eine Datenquelle (lokale managed-sqlite, fremde
//! foreign-sqlite, spaeter postgres/rest/file). Die `SourceRegistry`
//! routet pro `EntityBinding` zur richtigen Implementierung. CRUD-
//! Resolver gehen ueber diesen Pfad.

pub mod config;
pub mod managed_sqlite;
pub mod foreign_sqlite;
pub mod introspect;
pub mod mysql;
pub mod postgres;
pub mod sql;

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
    /// Source akzeptiert `apply_schema`-Aufrufe vom Designer. Sources, die
    /// nur fremde Schemata bedienen (foreign-sqlite, rest, …) liefern hier
    /// `false` und lehnen DDL mit [`SourceError::ReadOnly`] ab.
    pub supports_ddl: bool,
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
        id: Option<String>,
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

    /// Wendet ein vom Designer geliefertes Schema an. Default: Ablehnung
    /// mit [`SourceError::ReadOnly`] — nur Sources, deren Capabilities
    /// `supports_ddl = true` sind, ueberschreiben das.
    async fn apply_schema(
        &self,
        _schema: &shared::DbSchema,
    ) -> Result<usize, SourceError> {
        Err(SourceError::ReadOnly)
    }
}

/// In-Process-Routing: pro Source-Name eine boxed Implementierung.
pub struct SourceRegistry {
    sources: BTreeMap<String, std::sync::Arc<dyn Source>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self { sources: BTreeMap::new() }
    }

    pub fn register(&mut self, source: Box<dyn Source>) {
        let arc: std::sync::Arc<dyn Source> = source.into();
        self.sources.insert(arc.name().to_string(), arc);
    }

    /// Borrows a reference. Prefer [`Self::route`] for async callers.
    pub fn get(&self, name: &str) -> Option<&dyn Source> {
        self.sources.get(name).map(|a| a.as_ref())
    }

    /// Returns a cloned `Arc` so callers can release the registry read-lock
    /// before calling async methods.
    pub fn route(&self, binding: &EntityBinding) -> Result<std::sync::Arc<dyn Source>, SourceError> {
        self.sources
            .get(&binding.source)
            .cloned()
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

// =============================================================================
// Globaler Prozess-Slot + Boot-Funktion
// =============================================================================

use parking_lot::RwLock;
use std::sync::OnceLock;

use crate::source::config::SourceConfig;
use crate::source::managed_sqlite::ManagedSqliteSource;

static REGISTRY: OnceLock<RwLock<SourceRegistry>> = OnceLock::new();

fn slot() -> &'static RwLock<SourceRegistry> {
    REGISTRY.get_or_init(|| RwLock::new(SourceRegistry::new()))
}

/// Liefert eine Read-Lock-Sicht auf die Registry. Lebt prozessweit.
pub fn registry() -> parking_lot::RwLockReadGuard<'static, SourceRegistry> {
    slot().read()
}

/// Baut die Registry aus der Konfiguration auf. Idempotent — bei zweitem
/// Aufruf wird die alte Registry komplett ersetzt (nur Tests benutzen das,
/// um zwischen `fresh_test_setup`-Aufrufen zurueckzusetzen).
pub async fn boot_registry(
    sources: &std::collections::BTreeMap<String, SourceConfig>,
) -> Result<(), SourceError> {
    let mut reg = SourceRegistry::new();
    for (name, cfg) in sources {
        let mut src: Box<dyn Source> = match cfg.kind.as_str() {
            "managed-sqlite" => Box::new(ManagedSqliteSource::new(name.clone())),
            "foreign-sqlite" => {
                let url = cfg.url.clone().ok_or_else(|| {
                    SourceError::Other(format!("source '{name}' (foreign-sqlite) requires url"))
                })?;
                Box::new(crate::source::foreign_sqlite::ForeignSqliteSource::new(name.clone(), url))
            }
            "postgres" => {
                let url = cfg.url.clone().ok_or_else(|| {
                    SourceError::Other(format!("source '{name}' (postgres) requires url"))
                })?;
                Box::new(crate::source::postgres::PostgresSource::new(name.clone(), url))
            }
            "mysql" => {
                let url = cfg.url.clone().ok_or_else(|| {
                    SourceError::Other(format!("source '{name}' (mysql) requires url"))
                })?;
                Box::new(crate::source::mysql::MySqlSource::new(name.clone(), url))
            }
            other => return Err(SourceError::Other(format!("unknown source kind: {other}"))),
        };
        src.init().await?;
        reg.register(src);
    }
    *slot().write() = reg;
    Ok(())
}

/// Reset fuer Tests — analog zu `db::reset`.
pub fn reset() {
    *slot().write() = SourceRegistry::new();
}
