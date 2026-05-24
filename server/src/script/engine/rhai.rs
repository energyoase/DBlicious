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

use std::sync::{Arc, Mutex};

use rhai::packages::{
    ArithmeticPackage, BasicArrayPackage, BasicMapPackage, BasicStringPackage, LogicPackage,
    Package,
};
use rhai::{ASTNode, Dynamic, Engine, EvalAltResult, Expr, Position, Scope, AST};

use shared::script::capability::CapabilityToken;
use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::error::ScriptError;
use shared::script::manifest::ScriptManifest;
use shared::script::model::ScriptKind;

use crate::script::sandbox::{Sandbox, TokenUse};

/// Owned-Wrapper, damit andere Module den AST ohne `rhai::*`-Import halten
/// koennen. Wird in Task 3.5 (Lift-Analyse) als Eingabe verwendet.
#[derive(Clone)]
pub struct RhaiAst(pub Arc<AST>);

pub struct RhaiEngine {
    /// Engine ohne Host-Funktionen — nur fuer `compile()` (Parsen). Der
    /// echte Ausfuehrungspfad in `run()` baut eine zweite Engine mit den
    /// durch die Sandbox-Gate gewickelten Host-fns.
    inner: Engine,
    /// Owned-Kopie des Manifests: liefert `run()` die Capabilities (Gate),
    /// das Timeout (Fehler-Mapping) und das Memory-Budget (Size-Limits).
    manifest: ScriptManifest,
}

impl RhaiEngine {
    /// Engine mit Default-Manifest (leere Capabilities, keine Limits). Fuer
    /// Compile-only- und reine Compute-Pfade. Der Gate-relevante
    /// Produktionspfad nutzt [`RhaiEngine::with_manifest`].
    pub fn new() -> Self {
        Self::with_manifest(&ScriptManifest::default())
    }

    /// Skript-spezifische Engine: kennt das Manifest, weil `run()` daraus
    /// die Capability-Gate, das Operation-/Memory-Limit und den
    /// Timeout-Wert ableitet.
    pub fn with_manifest(manifest: &ScriptManifest) -> Self {
        let mut engine = Engine::new_raw();
        configure_strict(&mut engine);
        apply_limits(&mut engine, manifest);
        Self {
            inner: engine,
            manifest: manifest.clone(),
        }
    }
}

impl Default for RhaiEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Setzt die Laufzeit-Limits aus dem Manifest (S5/S7). Das Operation-Limit
/// (CPU) ist konservativ fix; die Size-Limits (Speicher) werden aus
/// `memory_kb` abgeleitet. Ohne `memory_kb` bleiben die Rhai-Defaults
/// (unbegrenzt) — der Save-Pfad validiert die Obergrenze separat.
fn apply_limits(engine: &mut Engine, manifest: &ScriptManifest) {
    engine.set_max_operations(50_000);
    if let Some(kb) = manifest.memory_kb {
        let kb = kb as usize;
        // Grobe, dokumentierte Heuristik: 1 KB ~ 1024 Zeichen bzw. ~128
        // Container-Elemente. Kein exaktes Byte-Accounting (Rhai bietet
        // keins) — die Limits sind eine Schranke gegen Runaway-Allokation,
        // nicht ein praezises Speicher-Budget.
        engine.set_max_string_size(kb * 1024);
        engine.set_max_array_size(kb * 128);
        engine.set_max_map_size(kb * 128);
    }
}

fn configure_strict(engine: &mut Engine) {
    // Spec §5.1: kontrollierte Sprach-Packages. `Engine::new_raw()` bringt
    // in Rhai 1.x KEINE eingebauten Funktionen mit — kein `.len()`, kein
    // String-Concat, keine Map-Ops. Wir laden genau die fuenf erlaubten
    // Packages, bewusst NICHT `StandardPackage` (das brächte u.a.
    // print-/file-/eval-nahe Funktionen, die wir gerade ausschliessen).
    ArithmeticPackage::new().register_into_engine(engine);
    LogicPackage::new().register_into_engine(engine);
    BasicStringPackage::new().register_into_engine(engine);
    BasicArrayPackage::new().register_into_engine(engine);
    BasicMapPackage::new().register_into_engine(engine);

    // Symbol-Disable (Spec §7.5). Nach der Package-Registrierung, damit ein
    // Package das Symbol nicht versehentlich re-aktiviert (tut keines der
    // fuenf, aber die Reihenfolge macht die Garantie explizit).
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
        host: Arc<dyn HostApi>,
        ctx: ScriptCtx,
    ) -> Result<ScriptValue, ScriptError> {
        self.run_collecting(ast, host, ctx).0
    }
}

/// Pro-Run geteilter Zustand. Lebt in einem `Arc<Mutex<…>>`, das die
/// registrierten Host-Funktionen capturen — so laufen alle Host-Calls
/// durch dieselbe `Sandbox` (Gate + Token-Audit) und sehen denselben
/// `host`/`ctx`.
struct RunState {
    sandbox: Sandbox,
    host: Arc<dyn HostApi>,
}

/// Marker-Typ, der als Scope-Variable `db`/`ctx` im Skript erscheint. Die
/// Host-Methoden (`entities`/`entity`/`t`) sind auf ihm registriert; den
/// echten Zustand tragen sie ueber den gecaptureten `Arc<Mutex<RunState>>`.
#[derive(Clone)]
struct HostBridge;

impl RhaiEngine {
    /// Echter Ausfuehrungspfad: baut eine Engine **mit** Host-Funktionen,
    /// die durch `sandbox.gate(token, …)` laufen, evaluiert den AST und
    /// liefert zusaetzlich die Token-Use-Liste fuer den Audit-Eintrag.
    ///
    /// B3: die Gate feuert hier im echten eval-Pfad (nicht nur isoliert).
    /// B1: unmaskable Fehler werden als `ErrorTerminated` propagiert
    /// (per `try`/`catch` nicht fangbar) und am Ende zum echten
    /// `ScriptError` zurueckgemappt.
    pub fn run_collecting(
        &self,
        ast: &RhaiAst,
        host: Arc<dyn HostApi>,
        _ctx: ScriptCtx,
    ) -> (Result<ScriptValue, ScriptError>, Vec<TokenUse>) {
        let state = Arc::new(Mutex::new(RunState {
            sandbox: Sandbox::new(&self.manifest),
            host,
        }));

        let mut engine = Engine::new_raw();
        configure_strict(&mut engine);
        apply_limits(&mut engine, &self.manifest);
        register_host_fns(&mut engine, Arc::clone(&state));

        let mut scope = Scope::new();
        scope.push("db", HostBridge);
        scope.push("ctx", HostBridge);

        let res: Result<Dynamic, Box<EvalAltResult>> =
            engine.eval_ast_with_scope(&mut scope, &ast.0);

        let timeout_ms = self.manifest.timeout_ms.unwrap_or(0);
        let memory_kb = self.manifest.memory_kb.unwrap_or(0);
        let token_uses = state
            .lock()
            .map(|st| st.sandbox.token_uses().to_vec())
            .unwrap_or_default();

        let mapped = match res {
            Ok(v) => Ok(rhai_to_script_value(v)),
            Err(e) => Err(map_rhai_err(*e, timeout_ms, memory_kb)),
        };
        (mapped, token_uses)
    }
}

/// Registriert die Host-Methoden auf `HostBridge`, jede gewickelt in
/// `sandbox.gate(token, || host.…())`. Maskable Fehler werden
/// `ErrorRuntime` (per `catch` fangbar), unmaskable `ErrorTerminated`
/// (uncatchbar) — beide tragen den serialisierten `ScriptError` im
/// Payload, damit `map_rhai_err` ihn rekonstruieren kann.
fn register_host_fns(engine: &mut Engine, state: Arc<Mutex<RunState>>) {
    engine.register_type_with_name::<HostBridge>("HostBridge");

    // db.entities(entity_type) -> Json-Array
    let s = Arc::clone(&state);
    engine.register_fn(
        "entities",
        move |_b: &mut HostBridge, entity_type: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            gated_db_fetch(&s, entity_type, None)
        },
    );

    // db.entity(entity_type, id) -> Json
    let s = Arc::clone(&state);
    engine.register_fn(
        "entity",
        move |_b: &mut HostBridge,
              entity_type: &str,
              id: &str|
              -> Result<Dynamic, Box<EvalAltResult>> {
            gated_db_fetch(&s, entity_type, Some(id))
        },
    );

    // ctx.t(key) -> String  (ReadI18n)
    let s = Arc::clone(&state);
    engine.register_fn(
        "t",
        move |_b: &mut HostBridge, key: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            let host = {
                let st = s.lock().unwrap();
                Arc::clone(&st.host)
            };
            let key_owned = key.to_string();
            let mut st = s.lock().unwrap();
            let res = st.sandbox.gate(&CapabilityToken::ReadI18n, move || {
                host.i18n_t(&key_owned, &serde_json::Value::Null)
            });
            match res {
                Ok(s) => Ok(Dynamic::from(s)),
                Err(e) => Err(script_err_to_rhai(e)),
            }
        },
    );
}

/// Gemeinsamer Pfad fuer `db.entities`/`db.entity`: baut die Query, gated
/// gegen `ReadOwnEntities` und ruft `host.db_fetch`. Das Ergebnis
/// (serde_json) wird als Rhai-`Dynamic` (JSON-String) zurueckgegeben —
/// der Aufrufer parst bei Bedarf mit `json(...)`.
fn gated_db_fetch(
    state: &Arc<Mutex<RunState>>,
    entity_type: &str,
    id: Option<&str>,
) -> Result<Dynamic, Box<EvalAltResult>> {
    let host = {
        let st = state.lock().unwrap();
        Arc::clone(&st.host)
    };
    let mut query = serde_json::json!({ "entity": entity_type });
    if let Some(id) = id {
        query["id"] = serde_json::Value::String(id.to_string());
    }
    let mut st = state.lock().unwrap();
    let res = st.sandbox.gate(&CapabilityToken::ReadOwnEntities, move || {
        host.db_fetch(&query)
    });
    match res {
        Ok(v) => Ok(Dynamic::from(v.to_string())),
        Err(e) => Err(script_err_to_rhai(e)),
    }
}

/// Wandelt einen `ScriptError` in den passenden Rhai-Fehler: unmaskable
/// (Spec §10) wird `ErrorTerminated` (per `try`/`catch` NICHT fangbar),
/// alles andere `ErrorRuntime` (fangbar). Der serialisierte `ScriptError`
/// reist im `Dynamic`-Payload mit, damit `map_rhai_err` ihn rekonstruiert.
fn script_err_to_rhai(e: ScriptError) -> Box<EvalAltResult> {
    let payload = serde_json::to_string(&e).unwrap_or_default();
    if e.unmaskable() {
        Box::new(EvalAltResult::ErrorTerminated(
            Dynamic::from(payload),
            Position::NONE,
        ))
    } else {
        Box::new(EvalAltResult::ErrorRuntime(
            Dynamic::from(payload),
            Position::NONE,
        ))
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

fn map_rhai_err(e: EvalAltResult, timeout_ms: u32, memory_kb: u32) -> ScriptError {
    match e {
        // Host-fn-Fehler reisen als JSON-Payload im Terminated/Runtime-Wert
        // (siehe `script_err_to_rhai`). Erst versuchen, den echten
        // ScriptError zu rekonstruieren.
        EvalAltResult::ErrorTerminated(payload, _) | EvalAltResult::ErrorRuntime(payload, _) => {
            if let Ok(s) = payload.into_string() {
                if let Ok(err) = serde_json::from_str::<ScriptError>(&s) {
                    return err;
                }
                return ScriptError::HostError { source: s };
            }
            ScriptError::HostError {
                source: "runtime error".into(),
            }
        }
        // S7: echten Timeout-Wert statt 0 durchreichen.
        EvalAltResult::ErrorTooManyOperations(_) => ScriptError::Timeout {
            limit_ms: timeout_ms,
        },
        // S5: Size-Limit-Ueberschreitung → MemoryExceeded mit Budget.
        EvalAltResult::ErrorDataTooLarge(_, _) => ScriptError::MemoryExceeded {
            limit_kb: memory_kb,
        },
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

#[cfg(test)]
mod rhai_invariants {
    //! Pinnt die Rhai-Verhaltens-Invarianten, auf denen die Sandbox-
    //! Sicherheit beruht (Q0009 B1/B3): Host-Funktionen koennen ueber
    //! einen Custom-Type auf pro-Run-Shared-State zugreifen, `ErrorTerminated`
    //! ist per `try`/`catch` NICHT fangbar (unmaskable), `ErrorRuntime`
    //! schon (maskable). Bricht eine dieser Invarianten bei einem
    //! Rhai-Upgrade, faellt hier der Build — bevor die Enforcement-Luecke
    //! in Produktion landet. Lebt hier, weil rhai-Symbole nur in
    //! `engine::rhai` auftauchen duerfen (Spec §11).
    use std::sync::{Arc, Mutex};

    use rhai::{Dynamic, Engine, EvalAltResult, Position, Scope};

    #[derive(Clone)]
    struct DbProxy {
        seen: Arc<Mutex<Vec<String>>>,
    }

    #[test]
    fn custom_type_method_accesses_shared_state() {
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let mut engine = Engine::new_raw();
        engine.register_type_with_name::<DbProxy>("Db");
        let captured = Arc::clone(&seen);
        engine.register_fn(
            "entities",
            move |_db: &mut DbProxy, t: &str| -> Result<Dynamic, Box<EvalAltResult>> {
                captured.lock().unwrap().push(t.to_string());
                Ok(Dynamic::from(t.to_string()))
            },
        );
        let mut scope = Scope::new();
        scope.push(
            "db",
            DbProxy {
                seen: Arc::clone(&seen),
            },
        );
        let r: Result<Dynamic, _> = engine.eval_with_scope(&mut scope, r#"db.entities("product")"#);
        assert!(r.is_ok(), "db.entities() muss laufen: {r:?}");
        assert_eq!(seen.lock().unwrap().as_slice(), &["product".to_string()]);
    }

    #[test]
    fn error_terminated_is_not_catchable() {
        let mut engine = Engine::new_raw();
        // BasicArray/Logic fuer try/catch + Vergleich werden hier nicht
        // gebraucht; try/catch ist Core-Syntax.
        engine.register_fn("boom", || -> Result<(), Box<EvalAltResult>> {
            Err(Box::new(EvalAltResult::ErrorTerminated(
                Dynamic::from("denied".to_string()),
                Position::NONE,
            )))
        });
        let r: Result<i64, _> =
            engine.eval(r#"let x = 0; try { boom(); x = 1 } catch(e) { x = 42 } x"#);
        // Uncatchbar ⇒ eval bricht mit ErrorTerminated ab (nicht x=42).
        match r {
            Ok(v) => panic!("ErrorTerminated wurde gefangen (x={v}) — darf NICHT sein"),
            Err(e) => assert!(
                matches!(*e, EvalAltResult::ErrorTerminated(..)),
                "erwartete ErrorTerminated, war {e:?}"
            ),
        }
    }

    #[test]
    fn error_runtime_is_catchable() {
        // Kontroll-Test: ErrorRuntime (maskable) MUSS fangbar sein.
        let mut engine = Engine::new_raw();
        engine.register_fn("softfail", || -> Result<(), Box<EvalAltResult>> {
            Err(Box::new(EvalAltResult::ErrorRuntime(
                Dynamic::from("oops".to_string()),
                Position::NONE,
            )))
        });
        // `try/catch` liefert in Rhai `()` (Statement, kein Ausdruck) —
        // wir schreiben das Ergebnis daher in eine Aussen-Variable.
        let r: Result<i64, _> =
            engine.eval(r#"let x = 0; try { softfail(); x = 1 } catch(e) { x = 42 } x"#);
        match &r {
            Ok(v) => assert_eq!(*v, 42, "ErrorRuntime muss fangbar sein (catch lief)"),
            Err(e) => panic!("unerwarteter Fehler statt catch: {e:?}"),
        }
    }
}
