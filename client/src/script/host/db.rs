//! `db`-Host-Modul (Spec §7.1) — Client-Pendant.
//!
//! Identische Eingangsverträge wie das Server-Pendant (`HostApi::db_fetch`,
//! `HostApi::db_patch`), aber:
//!   - Reads gehen durch — der GraphQL-Pfad ist im Renderer-Host hinterlegt.
//!   - Writes werden auf Host-Ebene mit `ServerOnlyFunction` abgelehnt,
//!     bevor der HostApi-Call ausgeloest wird. Damit verfehlt ein Client-
//!     Script, das `db.patch` aufruft, vor jedem Netzwerk-Round-Trip.
//!     Hintergrund: `ClientHostApiRegistry` markiert `db.patch` als
//!     `server_only=true`; die Sandbox-Capability-Pruefung wuerde es zwar
//!     bei korrekt restriktivem Manifest blockieren, aber ein
//!     Author-tier-Manifest, das `WriteEntity` haelt, wuerde sonst durch-
//!     gehen — diese Schicht ist der zweite Riegel (Defence-in-depth).
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

    /// `db.entities(entity_type, query)` — Form identisch zum Server. Das
    /// `entity`-Feld wird hier injiziert, damit `HostApi::db_fetch` einen
    /// einheitlichen Eingangsvertrag hat.
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

    /// `db.patch(entity, id, patch)` — auf dem Client immer abgelehnt
    /// (Spec §5.3: `WriteEntity` ist `server_only=true`). Der Fehler-
    /// Bezeichner `name` verwendet den Wire-Namen aus
    /// `ClientHostApiRegistry::functions` (`"db.patch"`), damit die
    /// Diagnose direkt aus dem Audit-Log lesbar ist.
    pub fn patch_entity(
        &self,
        _entity_type: &str,
        _id: &str,
        _patch: &Value,
    ) -> Result<(), ScriptError> {
        Err(ScriptError::ServerOnlyFunction {
            name: "db.patch".into(),
        })
    }
}
