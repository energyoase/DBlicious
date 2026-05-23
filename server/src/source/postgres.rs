//! `PostgresSource` — Phase 0.6 B2: relationale Source ueber `postgres://`-
//! Connection. Delegiert die CRUD-Logik an `super::sql::relational_*`,
//! Backend ist `DbBackend::Postgres`.
//!
//! Beachte: Introspektion ist heute nicht implementiert; das Schema kommt
//! ausschliesslich aus `EntityBinding.column_map` + `primary_key`. Mit
//! `column_map = {}` lehnt `relational_list_page` ab. Eine spaetere
//! Introspect-Erweiterung kann `information_schema.tables` nutzen.

use async_trait::async_trait;
use sea_orm::{Database, DatabaseConnection, DbBackend};

use shared::source::{BindingLocator, EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::sql;
use super::{Capabilities, PageQuery, Source, SourceError};

pub struct PostgresSource {
    name: String,
    url:  String,
    conn: Option<DatabaseConnection>,
}

impl PostgresSource {
    pub fn new(name: String, url: String) -> Self {
        Self { name, url, conn: None }
    }
}

#[async_trait]
impl Source for PostgresSource {
    fn name(&self) -> &str { &self.name }
    fn kind(&self) -> &'static str { "postgres" }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write:        true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: false, // TODO: information_schema.tables
            supports_composite_pk: true,
            supports_ddl:          false,  // Schema-Verwaltung durch externe Tools.
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        let mut opts = sea_orm::ConnectOptions::new(self.url.clone());
        opts.sqlx_logging(false);
        self.conn = Some(Database::connect(opts).await?);
        Ok(())
    }

    async fn list_page(
        &self,
        binding: &EntityBinding,
        query: &PageQuery,
    ) -> Result<EntityPage, SourceError> {
        let table = expect_table(&binding.locator)?;
        let conn = self.conn.as_ref().ok_or_else(|| {
            SourceError::Other("PostgresSource: init() not called".into())
        })?;
        sql::relational_list_page(conn, DbBackend::Postgres, binding, &table, query, None).await
    }

    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        let table = expect_table(&binding.locator)?;
        let conn = self.conn.as_ref().ok_or_else(|| {
            SourceError::Other("PostgresSource: init() not called".into())
        })?;
        sql::relational_get(conn, DbBackend::Postgres, binding, &table, id, None).await
    }

    async fn create(
        &self,
        binding: &EntityBinding,
        _id: Option<String>,
        fields: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = expect_table(&binding.locator)?;
        let conn = self.conn.as_ref().ok_or_else(|| {
            SourceError::Other("PostgresSource: init() not called".into())
        })?;
        sql::relational_create(conn, DbBackend::Postgres, binding, &table, fields).await
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
        let table = expect_table(&binding.locator)?;
        let conn = self.conn.as_ref().ok_or_else(|| {
            SourceError::Other("PostgresSource: init() not called".into())
        })?;
        sql::relational_update(conn, DbBackend::Postgres, binding, &table, id, patch, None).await
    }

    async fn delete(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<bool, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let table = expect_table(&binding.locator)?;
        let conn = self.conn.as_ref().ok_or_else(|| {
            SourceError::Other("PostgresSource: init() not called".into())
        })?;
        sql::relational_delete(conn, DbBackend::Postgres, binding, &table, id).await
    }
}

fn expect_table(locator: &BindingLocator) -> Result<String, SourceError> {
    match locator {
        BindingLocator::Table { table } => Ok(table.clone()),
        other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    }
}
