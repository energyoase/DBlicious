//! `RestSource` — generischer HTTP-CRUD-Adapter (Phase 0.6 B3).
//!
//! Konvention pro Tabelle (Locator `Table { table }`) bzw. pro Pfad
//! (Locator `RestEndpoint { path }`):
//!
//! | Operation     | HTTP           | Body / Response                       |
//! |---------------|----------------|---------------------------------------|
//! | `list_page`   | `GET <base>/<path>?page=...&pageSize=...&sortBy=...&sortDir=...` | Response: `{ "items": [...], "totalCount": N }` |
//! | `get`         | `GET <base>/<path>/<id>`           | Response: `Entity`           |
//! | `create`      | `POST <base>/<path>`               | Body + Response: `Entity`    |
//! | `update`      | `PATCH <base>/<path>/<id>`         | Body: patch; Response: `Entity` |
//! | `delete`      | `DELETE <base>/<path>/<id>`        | 2xx → true, 404 → false      |
//!
//! Filter-Pushdown: heute begrenzt — alle predicates landen als
//! `?filter=<json-base64>` (out-of-scope für MVP; Server kann es ignorieren
//! und lokal nachfiltern). MVP unterstützt nur die pagination/sort-Params.

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use shared::source::{BindingLocator, EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::{Capabilities, PageQuery, Source, SourceError};

pub struct RestSource {
    name: String,
    base_url: String,
    client: Client,
}

impl RestSource {
    pub fn new(name: String, base_url: String) -> Self {
        Self {
            name,
            base_url: base_url.trim_end_matches('/').into(),
            client: Client::new(),
        }
    }

    fn endpoint(&self, locator: &BindingLocator) -> Result<String, SourceError> {
        match locator {
            BindingLocator::Table { table } => Ok(format!("{}/{}", self.base_url, table)),
            BindingLocator::RestEndpoint { path } => Ok(format!(
                "{}/{}",
                self.base_url,
                path.trim_start_matches('/')
            )),
            other => Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
        }
    }

    fn http_err(prefix: &str, e: reqwest::Error) -> SourceError {
        SourceError::Other(format!("{prefix}: {e}"))
    }
}

#[derive(Deserialize)]
struct ListPayload {
    #[serde(default)]
    items: Vec<Entity>,
    #[serde(default, alias = "total")]
    total_count: u64,
}

#[async_trait]
impl Source for RestSource {
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> &'static str {
        "rest"
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
        let url = self.endpoint(&binding.locator)?;
        let page = query.page.max(1);
        let page_size = query.page_size.max(1);
        let mut req = self.client.get(&url).query(&[
            ("page", page.to_string()),
            ("pageSize", page_size.to_string()),
        ]);
        if let Some(s) = &query.sort {
            let dir = match s.direction {
                shared::SortDirection::Asc => "asc",
                shared::SortDirection::Desc => "desc",
            };
            req = req.query(&[("sortBy", s.field.as_str()), ("sortDir", dir)]);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| Self::http_err("GET list", e))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(SourceError::Other(format!("GET {url} → {status}")));
        }
        let payload: ListPayload = resp
            .json()
            .await
            .map_err(|e| Self::http_err("decode list", e))?;
        Ok(EntityPage {
            items: payload.items,
            total_count: payload.total_count,
            page: page as u32,
            page_size: page_size as u32,
        })
    }

    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        let url = format!("{}/{}", self.endpoint(&binding.locator)?, id.encode());
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Self::http_err("GET", e))?;
        match resp.status() {
            StatusCode::NOT_FOUND => Ok(None),
            s if s.is_success() => {
                let e: Entity = resp
                    .json()
                    .await
                    .map_err(|e| Self::http_err("decode entity", e))?;
                Ok(Some(e))
            }
            other => Err(SourceError::Other(format!("GET {url} → {other}"))),
        }
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
        let url = self.endpoint(&binding.locator)?;
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::Value::Object(fields))
            .send()
            .await
            .map_err(|e| Self::http_err("POST", e))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(SourceError::Other(format!("POST {url} → {status}")));
        }
        resp.json()
            .await
            .map_err(|e| Self::http_err("decode created", e))
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
        let url = format!("{}/{}", self.endpoint(&binding.locator)?, id.encode());
        let resp = self
            .client
            .patch(&url)
            .json(&serde_json::Value::Object(patch))
            .send()
            .await
            .map_err(|e| Self::http_err("PATCH", e))?;
        match resp.status() {
            StatusCode::NOT_FOUND => Ok(None),
            s if s.is_success() => {
                let e: Entity = resp
                    .json()
                    .await
                    .map_err(|e| Self::http_err("decode updated", e))?;
                Ok(Some(e))
            }
            other => Err(SourceError::Other(format!("PATCH {url} → {other}"))),
        }
    }

    async fn delete(&self, binding: &EntityBinding, id: &EntityId) -> Result<bool, SourceError> {
        if binding.read_only {
            return Err(SourceError::ReadOnly);
        }
        let url = format!("{}/{}", self.endpoint(&binding.locator)?, id.encode());
        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| Self::http_err("DELETE", e))?;
        match resp.status() {
            StatusCode::NOT_FOUND => Ok(false),
            s if s.is_success() => Ok(true),
            other => Err(SourceError::Other(format!("DELETE {url} → {other}"))),
        }
    }
}
