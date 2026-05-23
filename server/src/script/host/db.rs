//! `db`-Host-Modul (Spec §7.1).
//!
//! Pre-Resolved Calls. Der Rhai-Adapter (spaeterer Task) baut den
//! Lazy-Builder-Pattern (`db.entities(...).where(...).order_by(...)`) um
//! diese Funktionen herum: das gesammelte Query-Object wird hier als
//! `serde_json::Value` reingereicht.
//!
//! Sandbox-Gating passiert in `Sandbox::gate(...)` — diese Schicht hat keine
//! Capability-Pruefung, sie macht nur die Brueke zur `HostApi`.

use serde_json::Value;

use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct DbHost<'a> {
    host: &'a dyn HostApi,
}

impl<'a> DbHost<'a> {
    pub fn new(host: &'a dyn HostApi) -> Self {
        Self { host }
    }

    /// `db.entities(entity_type, query)` — Rueckgabe ist eine JSON-Array
    /// mit Maps. `query` ist der zusammengebaute Builder-State
    /// (where/order_by/limit) als Map; das `entity`-Feld wird hier
    /// injiziert, damit der `HostApi::db_fetch` einen einheitlichen
    /// Eingangsvertrag hat.
    pub fn fetch_entities(&self, entity_type: &str, query: &Value) -> Result<Value, ScriptError> {
        let mut q = query.clone();
        if !q.is_object() {
            q = serde_json::json!({});
        }
        q.as_object_mut()
            .unwrap()
            .insert("entity".into(), Value::String(entity_type.into()));
        self.host.db_fetch(&q)
    }

    pub fn patch_entity(
        &self,
        entity_type: &str,
        id: &str,
        patch: &Value,
    ) -> Result<(), ScriptError> {
        self.host.db_patch(entity_type, id, patch)
    }
}
