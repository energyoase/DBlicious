//! Rhai-Engine-Adapter (Client-Pendant zu `server::script::engine::rhai`).
//!
//! Aufbau gemaess Spec §5.1: `Engine::new_raw()` plus expliziter
//! `configure_strict` — nichts ausser dem Notwendigen wird registriert,
//! `eval`/`import`/`print`/`debug` werden per `disable_symbol` verboten.
//!
//! Spec §11-Garantie: dies ist die **einzige** Datei im Client-Crate, die
//! das Wort `rhai` ausserhalb des Cargo-Manifests enthaelt. Andere Module
//! sprechen ueber den `ScriptEngine`-Trait.
//!
//! Es gibt absichtlich **keinen async-Pfad** im Client: das deterministische
//! `set_max_operations`-Limit deckt die Run-Time-Begrenzung. Wall-clock-
//! Deadlines lebt in `sandbox.rs` und nutzt dort `web_sys::Performance::now`
//! (Browser) bzw. `Instant` (native Tests).

use std::sync::Arc;

use rhai::{Engine, EvalAltResult, AST};

use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::error::ScriptError;
use shared::script::manifest::ScriptManifest;
use shared::script::model::ScriptKind;

/// Owned-Wrapper, damit andere Module den AST ohne `rhai::*`-Import halten
/// koennen. Spiegelt das Server-Pendant.
#[derive(Clone)]
pub struct RhaiAst(pub Arc<AST>);

pub struct RhaiEngine {
    inner: Engine,
}

impl RhaiEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new_raw();
        configure_strict(&mut engine);
        // Konservatives Operation-Limit — identisch zum Server. Der Sandbox-
        // Pfad pro Run setzt zusaetzlich Deadlines, kann das spaeter
        // herunterskalieren.
        engine.set_max_operations(50_000);
        Self { inner: engine }
    }
}

impl Default for RhaiEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn configure_strict(engine: &mut Engine) {
    // Symbol-Disable (Spec §7.5).
    engine.disable_symbol("eval");
    engine.disable_symbol("import");
    engine.disable_symbol("print");
    engine.disable_symbol("debug");
}

impl ScriptEngine for RhaiEngine {
    type Ast = RhaiAst;

    fn compile(&self, source: &str, _manifest: &ScriptManifest) -> Result<Self::Ast, ScriptError> {
        match self.inner.compile(source) {
            Ok(mut ast) => {
                ast.set_source(source.to_string());
                Ok(RhaiAst(Arc::new(ast)))
            }
            Err(e) => {
                let pos = e.position();
                Err(ScriptError::ParseFailed {
                    line: pos.line().unwrap_or(0) as u32,
                    col: pos.position().unwrap_or(0) as u32,
                    msg: format!("{e}"),
                })
            }
        }
    }

    fn run(
        &self,
        ast: &Self::Ast,
        _host: &dyn HostApi,
        _ctx: ScriptCtx,
    ) -> Result<ScriptValue, ScriptError> {
        let mut scope = rhai::Scope::new();
        let res: Result<rhai::Dynamic, Box<EvalAltResult>> =
            self.inner.eval_ast_with_scope(&mut scope, &ast.0);
        match res {
            Ok(v) => Ok(rhai_to_script_value(v)),
            Err(e) => Err(map_rhai_err(*e)),
        }
    }
}

/// Public Entry-Point fuer Wasm-Skripte: heute hartes Reject (identisch zum
/// Server). Phase 2 hat die `ScriptKind::Wasm`-Variante absichtlich
/// reserviert; bevor irgendein Pfad das anfasst, schlaegt der Compile-Schritt
/// mit `ScriptError::WasmEngineNotAvailable` fehl.
pub fn compile_wasm(_kind: &ScriptKind) -> Result<(), ScriptError> {
    Err(ScriptError::WasmEngineNotAvailable)
}

fn rhai_to_script_value(v: rhai::Dynamic) -> ScriptValue {
    if v.is::<bool>() {
        return ScriptValue::Bool(v.as_bool().unwrap_or(false));
    }
    if let Ok(n) = v.as_int() {
        return ScriptValue::Number(n as f64);
    }
    if let Ok(f) = v.as_float() {
        return ScriptValue::Number(f);
    }
    if v.is::<String>() {
        return ScriptValue::String(v.into_string().unwrap_or_default());
    }
    ScriptValue::Unit
}

fn map_rhai_err(e: EvalAltResult) -> ScriptError {
    match e {
        EvalAltResult::ErrorTooManyOperations(_) => ScriptError::Timeout { limit_ms: 0 },
        EvalAltResult::ErrorParsing(_, p) => ScriptError::ParseFailed {
            line: p.line().unwrap_or(0) as u32,
            col: p.position().unwrap_or(0) as u32,
            msg: "parse".into(),
        },
        other => ScriptError::HostError {
            source: format!("{other}"),
        },
    }
}
