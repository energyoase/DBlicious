//! Konkrete `HostApi`-Impl für den Client-Render-Pfad. Formatter-Scripts
//! brauchen nur `i18n_t` (Fluent-Lookup via `crate::i18n::t`). Die übrigen
//! Methoden sind in dieser Welle nicht erreichbar (ComputeOnly+ReadI18n).

use serde_json::Value;
use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct RenderHost;

impl HostApi for RenderHost {
    fn db_fetch(&self, _query: &Value) -> Result<Value, ScriptError> {
        Err(ScriptError::ServerOnlyFunction {
            name: "db.entities".into(),
        })
    }

    fn db_patch(&self, _entity_type: &str, _id: &str, _patch: &Value) -> Result<(), ScriptError> {
        Err(ScriptError::ServerOnlyFunction {
            name: "db.patch".into(),
        })
    }

    fn i18n_t(&self, key: &str, _args: &Value) -> Result<String, ScriptError> {
        Ok(crate::i18n::t(key))
    }

    fn audit_log(&self, _event: &str, _payload: &Value) -> Result<(), ScriptError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_host_is_host_api() {
        let h: std::sync::Arc<dyn HostApi> = std::sync::Arc::new(RenderHost);
        assert!(h.db_fetch(&serde_json::Value::Null).is_err());
    }
}
