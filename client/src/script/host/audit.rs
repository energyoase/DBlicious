//! `audit`-Host-Modul (Spec §7.6) — Client-Pendant.
//!
//! `audit.log(event, payload)` ist in `ClientHostApiRegistry` als
//! `server_only=true` markiert. Anders als beim `db.patch`-Pfad ist hier ein
//! Aufruf vom Skript jedoch **kein Fehler**: das Skript kann nicht aktiv
//! `audit.log` rufen (das Sandbox-Capability-Gate blockt `WriteAuditLog` auf
//! Reader-Tier-Skripten ohnehin), aber der Renderer kann sich vorbehalten,
//! interne Run-Events lokal zu puffern und beim naechsten Heartbeat
//! hochzuschieben.
//!
//! Diese Schicht ist daher dual:
//!   - Ueber `HostApi::audit_log` wird die Server-Implementierung gerufen.
//!     Im Client-Test gegen `MockHostApi` funktioniert das wie auf dem
//!     Server.
//!   - Im echten Client-Run wird `crate::script::audit_queue::push` benutzt,
//!     und der Heartbeat in Phase 6 entlaedt die Queue.

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

    /// Default-Pfad: delegiert an die `HostApi`-Impl. Der Renderer-Host
    /// (Phase 5) wird intern in seine `audit_log`-Impl an die
    /// `audit_queue::push`-Funktion delegieren — die Buffer-Logik bleibt
    /// damit ausserhalb dieser Schicht.
    pub fn log(&self, event: &str, payload: &Value) -> Result<(), ScriptError> {
        self.host.audit_log(event, payload)
    }
}
