//! Engine-agnostische Trait-Schnittstellen. **Wichtige Forward-Compat-Regel
//! (Spec §11):** dieses Modul darf **nirgendwo** das Wort `rhai` enthalten.
//! Der Server- und Client-seitige Engine-Adapter haellt das Rhai-Wissen in
//! seinem eigenen `engine::rhai`-Submodul.

use std::sync::Arc;

use crate::script::error::ScriptError;
use crate::script::manifest::ScriptManifest;

#[derive(Debug, Clone, Default)]
pub struct ScriptCtx {
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub locale: String,
}

/// Engine-spezifischer kompilierter AST (associated type, damit der Trait
/// engine-agnostisch bleibt).
pub trait ScriptEngine {
    type Ast: Clone + Send + Sync;
    fn compile(&self, source: &str, manifest: &ScriptManifest) -> Result<Self::Ast, ScriptError>;
    /// Fuehrt das Skript aus. Der Host kommt als `Arc<dyn HostApi>`, weil die
    /// Engine-Adapter ihn in `'static` Native-Function-Closures capturen
    /// muessen (Rhai `register_fn` verlangt `'static + Send + Sync`). Ein
    /// `&dyn HostApi` mit Run-Lifetime liesse sich dort nicht halten.
    fn run(
        &self,
        ast: &Self::Ast,
        host: Arc<dyn HostApi>,
        ctx: ScriptCtx,
    ) -> Result<ScriptValue, ScriptError>;
}

/// Rueckgabewert eines Skript-Runs — engine-neutral.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    String(String),
    Number(f64),
    Bool(bool),
    Json(serde_json::Value),
    Unit,
}

/// Engine-agnostischer Host. Beide Crates implementieren ihn — Server mit
/// echten SeaORM-Calls, Client mit GraphQL-Calls.
///
/// `Send + Sync`, damit der Host als `Arc<dyn HostApi>` in `'static`
/// Engine-Closures (Rhai `register_fn`) gehalten werden kann.
pub trait HostApi: Send + Sync {
    fn db_fetch(&self, query: &serde_json::Value) -> Result<serde_json::Value, ScriptError>;
    fn db_patch(
        &self,
        entity_type: &str,
        id: &str,
        patch: &serde_json::Value,
    ) -> Result<(), ScriptError>;
    fn i18n_t(&self, key: &str, args: &serde_json::Value) -> Result<String, ScriptError>;
    fn audit_log(&self, event: &str, payload: &serde_json::Value) -> Result<(), ScriptError>;
}
