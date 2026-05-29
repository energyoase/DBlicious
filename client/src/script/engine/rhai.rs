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
//! Deadlines leben in `sandbox.rs` und nutzen dort `web_sys::Performance::now`
//! (Browser) bzw. `Instant` (native Tests).

use std::sync::{Arc, Mutex};

use rhai::packages::{
    ArithmeticPackage, BasicArrayPackage, BasicMapPackage, BasicStringPackage, LogicPackage,
    Package,
};
use rhai::{Dynamic, Engine, EvalAltResult, Position, Scope, AST};

use shared::script::capability::CapabilityToken;
use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptInputs, ScriptValue};
use shared::script::error::ScriptError;
use shared::script::manifest::ScriptManifest;
use shared::script::model::ScriptKind;

/// Owned-Wrapper, damit andere Module den AST ohne `rhai::*`-Import halten
/// koennen. Spiegelt das Server-Pendant.
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

/// Setzt die Laufzeit-Limits aus dem Manifest. Das Operation-Limit
/// (CPU) ist konservativ fix; die Size-Limits (Speicher) werden aus
/// `memory_kb` abgeleitet. Ohne `memory_kb` bleiben die Rhai-Defaults
/// (unbegrenzt).
fn apply_limits(engine: &mut Engine, manifest: &ScriptManifest) {
    engine.set_max_operations(50_000);
    if let Some(kb) = manifest.memory_kb {
        let kb = kb as usize;
        engine.set_max_string_size(kb * 1024);
        engine.set_max_array_size(kb * 128);
        engine.set_max_map_size(kb * 128);
    }
}

fn configure_strict(engine: &mut Engine) {
    // Spec §5.1: kontrollierte Sprach-Packages (identisch zum Server).
    // `Engine::new_raw()` bringt in Rhai 1.x keine eingebauten Funktionen
    // mit — kein `.len()`, kein String-Concat, keine Map-Ops. Wir laden
    // genau die fuenf erlaubten Packages, bewusst NICHT `StandardPackage`.
    ArithmeticPackage::new().register_into_engine(engine);
    LogicPackage::new().register_into_engine(engine);
    BasicStringPackage::new().register_into_engine(engine);
    BasicArrayPackage::new().register_into_engine(engine);
    BasicMapPackage::new().register_into_engine(engine);

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
        inputs: ScriptInputs,
        host: Arc<dyn HostApi>,
        _ctx: ScriptCtx,
    ) -> Result<ScriptValue, ScriptError> {
        // Pro-Run geteilter Zustand fuer die Host-Funktionen.
        let state = Arc::new(Mutex::new(RunState {
            capabilities: self.manifest.capabilities.clone(),
            host,
        }));

        let mut engine = Engine::new_raw();
        configure_strict(&mut engine);
        apply_limits(&mut engine, &self.manifest);
        register_host_fns(&mut engine, Arc::clone(&state));

        let mut scope = Scope::new();
        scope.push("ctx", HostBridge);
        scope.push_constant("value", json_to_dynamic(&inputs.value));
        scope.push_constant(
            "fields",
            json_to_dynamic(&serde_json::Value::Object(inputs.fields.clone())),
        );

        let res: Result<Dynamic, Box<EvalAltResult>> =
            engine.eval_ast_with_scope(&mut scope, &ast.0);

        let timeout_ms = self.manifest.timeout_ms.unwrap_or(0);
        let memory_kb = self.manifest.memory_kb.unwrap_or(0);

        match res {
            Ok(v) => Ok(rhai_to_script_value(v)),
            Err(e) => Err(map_rhai_err(*e, timeout_ms, memory_kb)),
        }
    }
}

/// Pro-Run geteilter Zustand. Lebt in einem `Arc<Mutex<…>>`, das die
/// registrierten Host-Funktionen capturen. Haelt eine owned Kopie der
/// Capabilities statt einer `Sandbox<'m>`-Referenz, damit der State in
/// den `Arc<Mutex<…>>` wandern kann.
struct RunState {
    capabilities: Vec<CapabilityToken>,
    host: Arc<dyn HostApi>,
}

impl RunState {
    /// Inline Capability-Gate: prueft, ob `token` in `capabilities` liegt.
    /// Bei Fehlen: unmittelbares `CapabilityDenied` zurueck.
    fn gate<T, F>(&mut self, token: &CapabilityToken, body: F) -> Result<T, ScriptError>
    where
        F: FnOnce() -> Result<T, ScriptError>,
    {
        if !self.capabilities.contains(token) {
            return Err(ScriptError::CapabilityDenied {
                token: token.clone(),
            });
        }
        body()
    }
}

/// Marker-Typ, der als Scope-Variable `ctx` im Skript erscheint. Die
/// Host-Methode `t` ist auf ihm registriert; den echten Zustand traegt
/// sie ueber den gecaptureten `Arc<Mutex<RunState>>`.
#[derive(Clone)]
struct HostBridge;

/// Registriert die Host-Methoden auf `HostBridge`. Nur `ctx.t` — der Client
/// hat keinen `db.*`-Zugriff (der laeuft ueber den GraphQL-`DataSource`-Adapter).
fn register_host_fns(engine: &mut Engine, state: Arc<Mutex<RunState>>) {
    engine.register_type_with_name::<HostBridge>("HostBridge");

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
            let res = st.gate(&CapabilityToken::ReadI18n, move || {
                host.i18n_t(&key_owned, &serde_json::Value::Null)
            });
            match res {
                Ok(s) => Ok(Dynamic::from(s)),
                Err(e) => Err(script_err_to_rhai(e)),
            }
        },
    );
}

/// Sentinel-Transport fuer host-originierte `ScriptError`s (Q0011, symmetrisch
/// zum Server). Host-Fehler reisen als geboxter Custom-Typ im
/// `EvalAltResult`-Payload, nicht als JSON-String — ein Skript-`throw` kann
/// diesen Typ nicht erzeugen, der gemeldete `kind` ist damit nicht spoofbar.
#[derive(Clone)]
struct HostErrorPayload(ScriptError);

/// Wandelt einen `ScriptError` in den passenden Rhai-Fehler: unmaskable
/// (Spec §10) wird `ErrorTerminated` (per `try`/`catch` NICHT fangbar),
/// alles andere `ErrorRuntime` (fangbar). Der `ScriptError` reist als
/// authentifizierter `HostErrorPayload`-Custom-Typ im `Dynamic`-Payload mit,
/// damit `map_rhai_err` ihn per `try_cast` rekonstruiert (Q0011).
fn script_err_to_rhai(e: ScriptError) -> Box<EvalAltResult> {
    let unmaskable = e.unmaskable();
    let payload = Dynamic::from(HostErrorPayload(e));
    if unmaskable {
        Box::new(EvalAltResult::ErrorTerminated(payload, Position::NONE))
    } else {
        Box::new(EvalAltResult::ErrorRuntime(payload, Position::NONE))
    }
}

/// Public Entry-Point fuer Wasm-Skripte: heute hartes Reject (identisch zum
/// Server). Phase 2 hat die `ScriptKind::Wasm`-Variante absichtlich
/// reserviert; bevor irgendein Pfad das anfasst, schlaegt der Compile-Schritt
/// mit `ScriptError::WasmEngineNotAvailable` fehl.
pub fn compile_wasm(_kind: &ScriptKind) -> Result<(), ScriptError> {
    Err(ScriptError::WasmEngineNotAvailable)
}

/// serde_json::Value -> rhai::Dynamic. Manuell, weil das rhai-`serde`-
/// Feature bewusst NICHT aktiviert ist (nur std/sync/internals). Zahlen:
/// ganzzahlig -> INT, sonst FLOAT. Arrays/Objects rekursiv.
fn json_to_dynamic(v: &serde_json::Value) -> Dynamic {
    match v {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else {
                Dynamic::from(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s.clone()),
        serde_json::Value::Array(a) => {
            let arr: rhai::Array = a.iter().map(json_to_dynamic).collect();
            Dynamic::from(arr)
        }
        serde_json::Value::Object(o) => {
            let mut map = rhai::Map::new();
            for (k, val) in o {
                map.insert(k.as_str().into(), json_to_dynamic(val));
            }
            Dynamic::from(map)
        }
    }
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
    let descr = e.to_string();
    match e {
        // Host-fn-Fehler reisen als authentifizierter Sentinel-Payload
        // (`HostErrorPayload`, siehe `script_err_to_rhai`). NUR dieser Typ
        // rekonstruiert den echten ScriptError. Alles andere (insbesondere ein
        // Skript-`throw "<json>"`, das nur primitive/Map-Dynamics erzeugt) wird
        // generisch als HostError gemeldet — der gemeldete `kind` ist damit
        // NICHT mehr vom Skript waehlbar (Q0011, Akzeptanzkriterium 1).
        EvalAltResult::ErrorTerminated(payload, _) | EvalAltResult::ErrorRuntime(payload, _) => {
            match payload.try_cast::<HostErrorPayload>() {
                Some(HostErrorPayload(err)) => err,
                None => ScriptError::HostError { source: descr },
            }
        }
        EvalAltResult::ErrorTooManyOperations(_) => ScriptError::Timeout {
            limit_ms: timeout_ms,
        },
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
mod run_inputs {
    use super::*;
    use shared::script::capability::{CapabilityToken, ScriptTier};
    use shared::script::engine::ScriptInputs;
    use shared::script::manifest::ScriptManifest;
    use shared::script::testing::MockHostApi;

    fn manifest_with(caps: Vec<CapabilityToken>) -> ScriptManifest {
        ScriptManifest {
            tier: ScriptTier::Reader,
            capabilities: caps,
            ..Default::default()
        }
    }

    #[test]
    fn value_is_bound() {
        let eng = RhaiEngine::new();
        let ast = eng.compile("value", &ScriptManifest::default()).unwrap();
        let inputs = ScriptInputs {
            value: serde_json::json!("SOLL"),
            fields: Default::default(),
        };
        let host = std::sync::Arc::new(MockHostApi::new());
        let out = eng.run(&ast, inputs, host, ScriptCtx::default()).unwrap();
        assert_eq!(out, ScriptValue::String("SOLL".into()));
    }

    #[test]
    fn ctx_t_requires_read_i18n() {
        let eng = RhaiEngine::with_manifest(&manifest_with(vec![CapabilityToken::ComputeOnly]));
        let ast = eng
            .compile(r#"ctx.t("k")"#, &ScriptManifest::default())
            .unwrap();
        let host = std::sync::Arc::new(MockHostApi::new());
        let err = eng
            .run(&ast, ScriptInputs::default(), host, ScriptCtx::default())
            .unwrap_err();
        assert!(
            matches!(err, ScriptError::CapabilityDenied { .. }),
            "war {err:?}"
        );
    }

    #[test]
    fn ctx_t_with_read_i18n_calls_host() {
        let eng = RhaiEngine::with_manifest(&manifest_with(vec![CapabilityToken::ReadI18n]));
        let ast = eng
            .compile(r#"ctx.t("hello")"#, &ScriptManifest::default())
            .unwrap();
        let host = std::sync::Arc::new(MockHostApi::new());
        let out = eng
            .run(&ast, ScriptInputs::default(), host, ScriptCtx::default())
            .unwrap();
        assert_eq!(out, ScriptValue::String("[t:hello]".into()));
    }

    /// Sicherheitsinvariante (Spec §10): ein `CapabilityDenied` aus `ctx.t`
    /// muss `ErrorTerminated` sein — per Rhai `try/catch` NICHT fangbar.
    /// Wuerde `script_err_to_rhai` faelschlicherweise `ErrorRuntime` liefern,
    /// wuerde `run()` `Ok("swallowed")` zurueckgeben statt `Err(CapabilityDenied)`.
    #[test]
    fn ctx_t_denial_is_not_catchable() {
        let eng = RhaiEngine::with_manifest(&manifest_with(vec![CapabilityToken::ComputeOnly]));
        let ast = eng
            .compile(
                r#"let x = "init"; try { x = ctx.t("k") } catch(e) { x = "swallowed" } x"#,
                &ScriptManifest::default(),
            )
            .unwrap();
        let host = std::sync::Arc::new(MockHostApi::new());
        let res = eng.run(&ast, ScriptInputs::default(), host, ScriptCtx::default());
        match res {
            Ok(v) => panic!("CapabilityDenied wurde gefangen (x={v:?}) — darf NICHT sein"),
            Err(e) => assert!(
                matches!(e, ScriptError::CapabilityDenied { .. }),
                "war {e:?}"
            ),
        }
    }
}
