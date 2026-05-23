//! `audit`-Host-Modul (Spec §7.6).
//!
//! `audit.log(event, payload)` delegiert an `HostApi::audit_log`. Capability
//! `WriteAuditLog` ist `server_only=true` in `ServerHostApiRegistry`; auf
//! Client-Seite gibt es keinen Aufrufpfad.

use serde_json::Value;

use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct AuditHost<'a> {
    host: &'a dyn HostApi,
}

impl<'a> AuditHost<'a> {
    pub fn new(host: &'a dyn HostApi) -> Self {
        Self { host }
    }

    pub fn log(&self, event: &str, payload: &Value) -> Result<(), ScriptError> {
        self.host.audit_log(event, payload)
    }
}
