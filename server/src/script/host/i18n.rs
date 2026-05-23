//! `i18n`-Host-Modul (Spec §7.3).
//!
//! Delegiert an die `HostApi`-Implementation des aufrufenden Kontexts. Der
//! Server-Pfad nutzt heute den Fluent-Bundle-Lookup; im Test wird gegen
//! `MockHostApi` gespielt, das `"[t:<key>]"` zurueckgibt.

use serde_json::Value;

use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct I18nHost<'a> {
    host: &'a dyn HostApi,
}

impl<'a> I18nHost<'a> {
    pub fn new(host: &'a dyn HostApi) -> Self {
        Self { host }
    }

    pub fn t(&self, key: &str, args: &Value) -> Result<String, ScriptError> {
        self.host.i18n_t(key, args)
    }
}
