//! Rhai-Engine-Adapter. Trait-Impl von
//! [`shared::script::engine::ScriptEngine`].
//!
//! Aufbau gemaess Spec §5.1: `Engine::new_raw()` plus expliziter
//! `configure_strict` — nichts ausser dem Notwendigen wird registriert,
//! `eval`/`import`/`print`/`debug` werden per `disable_symbol` verboten.
//!
//! Spec §11-Garantie: dies ist die **einzige** Datei im Workspace, die das
//! Wort `rhai` ausserhalb der Cargo-Manifeste enthalten darf. Andere Module
//! sprechen ueber den `ScriptEngine`-Trait.

use std::sync::Arc;

use rhai::{ASTNode, Engine, EvalAltResult, Expr, AST};

use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::error::ScriptError;
use shared::script::manifest::ScriptManifest;
use shared::script::model::ScriptKind;

/// Owned-Wrapper, damit andere Module den AST ohne `rhai::*`-Import halten
/// koennen. Wird in Task 3.5 (Lift-Analyse) als Eingabe verwendet.
#[derive(Clone)]
pub struct RhaiAst(pub Arc<AST>);

pub struct RhaiEngine {
    inner: Engine,
}

impl RhaiEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new_raw();
        configure_strict(&mut engine);
        // Konservatives Operation-Limit. Der Sandbox-Pfad pro Run setzt
        // zusaetzlich Deadlines und kann das spaeter herunterskalieren.
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

/// Public Entry-Point fuer Wasm-Skripte: heute hartes Reject. Phase 2 hat
/// die `ScriptKind::Wasm`-Variante absichtlich reserviert; bevor irgendein
/// Pfad das anfasst, schlaegt der Compile-Schritt mit
/// `ScriptError::WasmEngineNotAvailable` fehl.
pub fn compile_wasm(_kind: &ScriptKind) -> Result<(), ScriptError> {
    Err(ScriptError::WasmEngineNotAvailable)
}

/// Lift-Capability-Analyse (Phase 3.3, Spec §4): durchsucht den AST nach
/// allen Aufrufen von `db.entities(...)` und `db.entity(...)` und liefert
/// `true` genau dann, wenn jeder solche Call seinen *ersten* Argument-
/// Knoten als String-Literal hat. Sobald irgendein dynamischer Ausdruck
/// (Variable, Funktionsaufruf, String-Interpolation, ...) gefunden wird,
/// kollabiert das Ergebnis zu `false`.
///
/// Hintergrund: Lift = "der Server kann die Daten ohne Skript-Run
/// vorab-streamen". Dafuer muss er statisch wissen, *welche* Entity-Typen
/// das Skript anfassen wird — daher die Konstanz-Forderung.
///
/// Diese Funktion lebt bewusst hier neben dem Engine-Adapter (Spec §11):
/// die Inspektion benoetigt `rhai::*`-Internals, die in Modulen ausserhalb
/// `engine::rhai` nicht auftauchen duerfen.
pub fn analyze_lift_capability(ast: &RhaiAst) -> bool {
    use std::cell::Cell;
    let lift_capable = Cell::new(true);
    let mut visit = |path: &[ASTNode]| -> bool {
        if let Some(ASTNode::Expr(Expr::MethodCall(call, _))) = path.last() {
            // Method-Calls: `obj.method(args)` — name traegt nur den
            // Method-Anteil, nicht den vollqualifizierten Pfad. Daher
            // matcht "entities" auch `db.entities()` und (theoretisch)
            // `something_else.entities()`. Wir akzeptieren das: der
            // Server registriert `entities`/`entity` nur auf `db`, ein
            // anderer Receiver wuerde im Compile-Pfad scheitern.
            let name = call.name.as_str();
            if name == "entities" || name == "entity" {
                if let Some(first) = call.args.first() {
                    if !matches!(first, Expr::StringConstant(..)) {
                        lift_capable.set(false);
                        // Walk terminieren: ein dynamischer Arg reicht.
                        return false;
                    }
                }
            }
        }
        true
    };
    ast.0.walk(&mut visit);
    lift_capable.get()
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
