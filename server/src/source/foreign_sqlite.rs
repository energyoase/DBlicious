//! `ForeignSqliteSource` — liest/schreibt eine fremde SQLite-DB. Legt
//! KEIN eigenes Schema an; introspiziert beim `init`.

use async_trait::async_trait;

use shared::source::{EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::{Capabilities, PageQuery, Source, SourceError};

pub struct ForeignSqliteSource {
    name: String,
    url: String,
    conn: Option<sea_orm::DatabaseConnection>,
    introspected: super::introspect::Schema,
}

impl ForeignSqliteSource {
    pub fn new(name: String, url: String) -> Self {
        Self {
            name,
            url,
            conn: None,
            introspected: super::introspect::Schema::default(),
        }
    }
}

#[async_trait]
impl Source for ForeignSqliteSource {
    fn name(&self) -> &str { &self.name }
    fn kind(&self) -> &'static str { "foreign-sqlite" }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: true,
            supports_composite_pk: true,
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        let mut opts = sea_orm::ConnectOptions::new(self.url.clone());
        opts.sqlx_logging(false);
        let conn = sea_orm::Database::connect(opts).await?;
        self.introspected = super::introspect::read_schema(&conn).await?;
        self.conn = Some(conn);
        Ok(())
    }

    async fn list_page(&self, _b: &EntityBinding, _q: &PageQuery)
        -> Result<EntityPage, SourceError> { unimplemented!("Task 16") }
    async fn get(&self, _b: &EntityBinding, _id: &EntityId)
        -> Result<Option<Entity>, SourceError> { unimplemented!("Task 17") }
    async fn create(&self, _b: &EntityBinding, _id: Option<String>,
        _f: serde_json::Map<String, serde_json::Value>, _a: Option<&str>)
        -> Result<Entity, SourceError> { unimplemented!("Task 18") }
    async fn update(&self, _b: &EntityBinding, _id: &EntityId,
        _f: serde_json::Map<String, serde_json::Value>, _a: Option<&str>)
        -> Result<Option<Entity>, SourceError> { unimplemented!("Task 18") }
    async fn delete(&self, _b: &EntityBinding, _id: &EntityId)
        -> Result<bool, SourceError> { unimplemented!("Task 18") }
}
