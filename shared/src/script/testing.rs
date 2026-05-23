//! `MockHostApi` — Test-Doppel fuer Symmetrie- und Sandbox-Tests.
//!
//! Server- und Client-Crate testen jeweils ihren echten Host gegen die
//! gleiche Reference-Implementation: identische Inputs -> identische Outputs.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::script::engine::HostApi;
use crate::script::error::ScriptError;

#[derive(Debug, Default)]
pub struct MockHostApi {
    inner: Mutex<MockInner>,
}

#[derive(Debug, Default)]
struct MockInner {
    entities: BTreeMap<String, Value>,
    audit_log: Vec<(String, Value)>,
    patch_log: Vec<(String, String, Value)>,
    t_calls: Vec<(String, Value)>,
}

impl MockHostApi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed_entities(&self, entity: impl Into<String>, data: Value) {
        self.inner
            .lock()
            .unwrap()
            .entities
            .insert(entity.into(), data);
    }

    pub fn audit_log_calls(&self) -> Vec<(String, Value)> {
        self.inner.lock().unwrap().audit_log.clone()
    }

    pub fn patch_log(&self) -> Vec<(String, String, Value)> {
        self.inner.lock().unwrap().patch_log.clone()
    }

    pub fn t_calls(&self) -> Vec<(String, Value)> {
        self.inner.lock().unwrap().t_calls.clone()
    }
}

impl HostApi for MockHostApi {
    fn db_fetch(&self, query: &Value) -> Result<Value, ScriptError> {
        let entity = query
            .get("entity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ScriptError::HostError {
                source: "query.entity fehlt".into(),
            })?;
        Ok(self
            .inner
            .lock()
            .unwrap()
            .entities
            .get(entity)
            .cloned()
            .unwrap_or(Value::Array(Vec::new())))
    }
    fn db_patch(&self, entity_type: &str, id: &str, patch: &Value) -> Result<(), ScriptError> {
        self.inner
            .lock()
            .unwrap()
            .patch_log
            .push((entity_type.into(), id.into(), patch.clone()));
        Ok(())
    }
    fn i18n_t(&self, key: &str, args: &Value) -> Result<String, ScriptError> {
        self.inner
            .lock()
            .unwrap()
            .t_calls
            .push((key.into(), args.clone()));
        Ok(format!("[t:{key}]"))
    }
    fn audit_log(&self, event: &str, payload: &Value) -> Result<(), ScriptError> {
        self.inner
            .lock()
            .unwrap()
            .audit_log
            .push((event.into(), payload.clone()));
        Ok(())
    }
}
