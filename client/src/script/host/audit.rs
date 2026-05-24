//! `audit`-Host-Modul (Spec §7.6) — Client-Pendant.
//!
//! `audit.log(event, payload)` ist in `ClientHostApiRegistry` als
//! `server_only=true` markiert. Analog zu `db.patch` (Spec §5.3) lehnt der
//! Client-Host einen Skript-Aufruf daher **hart** mit `ServerOnlyFunction`
//! ab (S8, Q0009-Review) — Defence-in-depth, selbst wenn ein Author-tier-
//! Manifest `WriteAuditLog` halten sollte.
//!
//! Davon getrennt: der Renderer puffert **interne** Run-Events lokal ueber
//! [`crate::script::audit_queue::push`] und schiebt sie beim Heartbeat
//! hoch. Das laeuft NICHT ueber diese Schicht — `AuditHost::log` ist
//! ausschliesslich der (verbotene) Skript-Einstiegspunkt.

use serde_json::Value;

use shared::script::error::ScriptError;

pub struct AuditHost;

impl AuditHost {
    pub fn new() -> Self {
        Self
    }

    /// Server-only: ein Client-Skript darf `audit.log` nicht aufrufen.
    /// Gibt `ServerOnlyFunction` zurueck (konsistent mit `db.patch`), bevor
    /// irgendein Effekt entsteht.
    pub fn log(&self, _event: &str, _payload: &Value) -> Result<(), ScriptError> {
        Err(ScriptError::ServerOnlyFunction {
            name: "audit.log".into(),
        })
    }
}

impl Default for AuditHost {
    fn default() -> Self {
        Self::new()
    }
}
