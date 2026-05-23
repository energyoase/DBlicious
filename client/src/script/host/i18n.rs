//! `i18n`-Host-Modul (Spec §7.3) — Client-Pendant.
//!
//! Delegiert an die `HostApi::i18n_t`-Methode des aufrufenden Hosts.
//! `crate::i18n::t` ist ein Leptos-Signal-Reader und kann nur innerhalb
//! eines Leptos-Owner-Kontexts aufgerufen werden; das `HostApi`-Trait
//! abstrahiert das weg — der Renderer (Phase 5) liefert einen Host-Wrapper,
//! der den `t`-Lookup tatsaechlich ausfuehrt. In Tests wird die `MockHostApi`
//! aus `shared::script::testing` benutzt, identisch zum Server-Test-Setup.

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
