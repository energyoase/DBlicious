# Formatter-Provider-Script in den Render-Pfad verdrahten — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ein `formatter_id = "script:<id>"` auf einer Spalte führt beim Zellen-Render ein Provider-Formatter-Script aus, das den Zellenwert sieht und via `ctx.t` lokalisiert — demonstriert am d2v `valueType` (SOLL/HABEN → „Soll"/„Haben").

**Architecture:** `ScriptEngine::run` bekommt einen `ScriptInputs { value, fields }`-Parameter (symmetrisch shared/client/server). Der Server-Engine hat die Host-fn-/Gate-/`ctx.t`-Maschinerie bereits (`run_collecting`/`register_host_fns`); der Client-Engine wird auf dieselbe Struktur gehoben (Sandbox-Gate, `ctx.t` → `host.i18n_t`, `value`/`fields` als Scope-Variablen). `FieldCell` ruft bei `script:`-Formatter-ID `lookup_provider(Formatter, inputs)` und rendert dessen String; jeder Fehler fällt sauber auf `DefaultFieldRegistry` zurück.

**Tech Stack:** Rust, Rhai 1.x (`["std","sync","internals"]`, **kein** `serde`-Feature), Leptos/WASM (Client), serde_json. Tests isoliert via `--target-dir target-<task>verify -j 2` (Parallel-Session-Schutz, siehe Memory).

**Spec:** `docs/superpowers/specs/2026-05-25-provider-render-wiring-design.md`.

**Konventionen aus dem Repo (zwingend):**
- Engine-Adapter-Regel (Q0009 §11): das Wort `rhai` darf NUR in `*/script/engine/rhai.rs` vorkommen. Neue Rhai-Helper (`json_to_dynamic`) leben dort.
- Tests isoliert: nie `cargo test --workspace` im geteilten `target-test`; immer frisches `--target-dir target-prvverify -j 2`, Exit via `; echo "EXIT=$?"`, nie `| tail`.
- Commits mit explizitem Pathspec (`git commit -m "..." -- <dateien>`), nie `git add -A`.
- Commit-Trailer: `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`.

---

## File Structure

- **Create** `shared/src/script/engine.rs` → `ScriptInputs` (im selben File wie `ScriptEngine`).
- **Modify** `shared/src/script/engine.rs` → `ScriptEngine::run`-Signatur (+`inputs`).
- **Modify** `server/src/script/engine/rhai.rs` → `run`/`run_collecting`-Signatur (+`inputs`), `value`/`fields` in den Scope, `json_to_dynamic`-Helper.
- **Modify** `client/src/script/engine/rhai.rs` → Run-Pfad auf Server-Parität heben: Sandbox + `register_host_fns` (`t`-Gate) + Scope `value`/`fields`/`ctx`, `json_to_dynamic`.
- **Create** `client/src/script/render_host.rs` → konkrete `HostApi`-Impl (`RenderHost`) deren `i18n_t` an `crate::i18n::t` delegiert (übrige Methoden für Formatter ungenutzt).
- **Modify** `client/src/script/provider_lookup.rs` → `lookup_provider`-Signatur (+`inputs`), an `engine.run` durchreichen.
- **Modify** `client/src/components/table/formatters.rs` (`FieldCell`) → Formatter-ID-Resolution + Script-Aufruf + Fallback.
- **Modify** alle übrigen `run(...)`/`lookup_provider(...)`-Aufrufer (server `run.rs`, server/client `provider_lookup`-Tests, `client/src/components/script_renderer.rs`) → `ScriptInputs::default()` durchreichen.
- **Modify** `examples/d2v/entities/datev_entry/columns.json` → `valueType.formatterId`.
- **Create** `examples/d2v/scripts/d2v_value_type_label.rhai` + `.manifest.json`.
- **Modify** `examples/d2v/translatables/{entries,values}.json` → SOLL/HABEN-Labels.
- **Test** `server/tests/loader_d2v.rs`, `client/src/script/engine/rhai.rs` (inline), `client/src/script/provider_lookup.rs` (inline), `server/src/script/engine/rhai.rs` (inline).

---

## Task 1: `ScriptInputs`-Typ + `run`-Signatur (compile-grün, kein Verhaltenswechsel)

Signatur-Änderung an `ScriptEngine::run` bricht alle Aufrufer gleichzeitig — daher ein Commit, der den Typ einführt, beide Engines + alle Aufrufer auf `ScriptInputs::default()` zieht und grün bleibt. Noch **kein** Binding, **keine** Verhaltensänderung.

**Files:**
- Modify: `shared/src/script/engine.rs`
- Modify: `server/src/script/engine/rhai.rs:130-138,164-169`
- Modify: `client/src/script/engine/rhai.rs:94-113`
- Modify: `client/src/script/provider_lookup.rs:67-148`
- Modify: alle Aufrufer (siehe Schritt 4)

- [ ] **Step 1: `ScriptInputs` in shared definieren**

In `shared/src/script/engine.rs`, nach dem `ScriptCtx`-Struct (nach Zeile 16):

```rust
/// Pro-Aufruf-Eingaben für einen Provider-Slot-Run (Formatter/Filter/...).
/// Bewusst getrennt von [`ScriptCtx`] (ambient: user/tenant/locale): ein
/// Component-Script hat keinen Zellenwert, ein Formatter schon.
#[derive(Debug, Clone, Default)]
pub struct ScriptInputs {
    /// Der Zellenwert wie der Client ihn hält (nach Server-Grenz-Decode,
    /// z.B. directionalEnum 1 -> "SOLL"). NICHT vorformatiert.
    pub value: serde_json::Value,
    /// Die gesamte Zeile (`Entity.fields`) für kreuzfeld-Formatter.
    pub fields: serde_json::Map<String, serde_json::Value>,
}
```

- [ ] **Step 2: Trait-Signatur erweitern**

In `shared/src/script/engine.rs`, `ScriptEngine::run` (Zeile 27-32) zu:

```rust
    fn run(
        &self,
        ast: &Self::Ast,
        inputs: ScriptInputs,
        host: Arc<dyn HostApi>,
        ctx: ScriptCtx,
    ) -> Result<ScriptValue, ScriptError>;
```

- [ ] **Step 3: Beide Engine-Impls + `run_collecting` an die Signatur anpassen (noch ohne Binding)**

Server `server/src/script/engine/rhai.rs`:
- `run` (Zeile 130-138): Parameter `inputs: ScriptInputs` einfügen, an `run_collecting` weiterreichen: `self.run_collecting(ast, inputs, host, ctx).0`.
- `run_collecting` (Zeile 164-169): Parameter `_inputs: ScriptInputs` einfügen (in diesem Task noch ungenutzt, Unterstrich).
- Import ergänzen: `use shared::script::engine::{..., ScriptInputs};`.

Client `client/src/script/engine/rhai.rs` `run` (Zeile 94-113): Parameter `_inputs: ScriptInputs` einfügen (ungenutzt), Import `ScriptInputs` ergänzen.

- [ ] **Step 4: Alle Aufrufer auf `ScriptInputs::default()` ziehen**

Suchen mit `rg "\.run\(|run_collecting\(|lookup_provider\(" --type rust`. Jeden `engine.run(ast, host, ctx)` → `engine.run(ast, ScriptInputs::default(), host, ctx)`; jeden `run_collecting(ast, host, ctx)` analog. Bekannte Stellen:
- `server/src/script/provider_lookup.rs` (Server-Run-Aufruf)
- `server/src/script/run.rs`
- `client/src/script/provider_lookup.rs:137` (`engine.run(&ast, host, ctx)`)
- `client/src/components/script_renderer.rs` (render_decision → run)
- Tests in `server/tests/script_*.rs`, `server/src/script/**` inline-Tests, `client/src/script/provider_lookup.rs` inline-Tests (`lookup_provider(...)`-Aufrufe — hier zunächst nur, falls `lookup_provider` selbst die Signatur ändert; siehe Step 5).

Außerdem `client/src/script/provider_lookup.rs::lookup_provider` (Zeile 67-73): Parameter `inputs: ScriptInputs` ergänzen und an `engine.run` (Zeile 137) durchreichen: `engine.run(&ast, inputs, host, ctx)`. Alle `lookup_provider(...)`-Aufrufe (inline-Tests Zeile ~230-340) auf `ScriptInputs::default()` ergänzen.

- [ ] **Step 5: Workspace kompiliert + bestehende Tests grün**

Run: `cargo build --workspace --target-dir target-prvverify -j 2 2>&1 | tail -5; echo "EXIT=${PIPESTATUS[0]}"`
Expected: `Finished`, EXIT=0.
Run: `cargo test -p shared -p server -p client --target-dir target-prvverify -j 2 2>&1 | rg "test result|^error"; echo "EXIT=${PIPESTATUS[0]}"`
Expected: alle `test result: ok`, EXIT=0. (Reine Signatur-Ergänzung, kein Verhaltenswechsel.)

- [ ] **Step 6: Commit**

```bash
git commit -m "refactor(script): ScriptInputs-Parameter in ScriptEngine::run (symmetrisch, no-op)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>" -- shared/src/script/engine.rs server/src/script/engine/rhai.rs client/src/script/engine/rhai.rs client/src/script/provider_lookup.rs server/src/script/run.rs client/src/components/script_renderer.rs
```
(Pathspec an die tatsächlich geänderten Aufrufer-Dateien anpassen.)

---

## Task 2: `value`/`fields` in den Server-Scope binden (+ `json_to_dynamic`)

Der Server-Engine hat den Scope (`run_collecting`, Zeile 180-182). Hier wird `value`/`fields` gebunden und der JSON→Dynamic-Helper eingeführt. Rhai hat **kein** `serde`-Feature — der Helper ist manuell.

**Files:**
- Modify: `server/src/script/engine/rhai.rs`
- Test: `server/src/script/engine/rhai.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Failing test — Script liest `value`**

In `server/src/script/engine/rhai.rs`, im `#[cfg(test)] mod` (neben `rhai_invariants`, oder neues `mod input_binding`):

```rust
#[cfg(test)]
mod input_binding {
    use super::*;
    use shared::script::engine::{ScriptInputs, ScriptValue};
    use shared::script::testing::MockHostApi;

    #[test]
    fn value_is_bound_into_scope() {
        let eng = RhaiEngine::new();
        let ast = eng.compile(r#"value"#, &ScriptManifest::default()).unwrap();
        let inputs = ScriptInputs {
            value: serde_json::json!("SOLL"),
            fields: serde_json::Map::new(),
        };
        let host = std::sync::Arc::new(MockHostApi::new());
        let out = eng.run(&ast, inputs, host, ScriptCtx::default()).unwrap();
        assert_eq!(out, ScriptValue::String("SOLL".into()));
    }

    #[test]
    fn fields_map_is_bound_and_indexable() {
        let eng = RhaiEngine::new();
        let ast = eng.compile(r#"fields["k"]"#, &ScriptManifest::default()).unwrap();
        let mut m = serde_json::Map::new();
        m.insert("k".into(), serde_json::json!("v"));
        let inputs = ScriptInputs { value: serde_json::Value::Null, fields: m };
        let host = std::sync::Arc::new(MockHostApi::new());
        let out = eng.run(&ast, inputs, host, ScriptCtx::default()).unwrap();
        assert_eq!(out, ScriptValue::String("v".into()));
    }
}
```

- [ ] **Step 2: Test verifizieren (fail)**

Run: `cargo test -p server --lib --target-dir target-prvverify -j 2 input_binding 2>&1 | rg "test result|FAILED|value|fields"; echo "EXIT=${PIPESTATUS[0]}"`
Expected: FAIL (`value`/`fields` nicht im Scope → ErrorVariableNotFound o.ä.).

- [ ] **Step 3: `json_to_dynamic`-Helper + Binding implementieren**

In `server/src/script/engine/rhai.rs` einen Helper hinzufügen (rhai-Symbole sind hier erlaubt):

```rust
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
```

In `run_collecting` den Parameter `_inputs` → `inputs` (entkommentieren) und vor `eval_ast_with_scope` (nach Zeile 182) binden:

```rust
        scope.push("db", HostBridge);
        scope.push("ctx", HostBridge);
        scope.push_constant("value", json_to_dynamic(&inputs.value));
        scope.push_constant(
            "fields",
            json_to_dynamic(&serde_json::Value::Object(inputs.fields.clone())),
        );
```

(`push_constant`, weil das Script seine Eingaben nicht mutieren soll.)

- [ ] **Step 4: Test verifizieren (pass)**

Run: `cargo test -p server --lib --target-dir target-prvverify -j 2 input_binding 2>&1 | rg "test result|FAILED"; echo "EXIT=${PIPESTATUS[0]}"`
Expected: `test result: ok. 2 passed`, EXIT=0.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(script/server): value+fields in den Engine-Scope binden (json_to_dynamic)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>" -- server/src/script/engine/rhai.rs
```

---

## Task 3: Client-Engine auf Server-Parität — Sandbox-Gate, `ctx.t`, `value`/`fields`

Der Client-Engine (`client/src/script/engine/rhai.rs`) ist heute reine Compute (ignoriert host/ctx, registriert nichts). Diese Task hebt den `run`-Pfad auf die Server-Struktur: Sandbox + `register_host_fns` (nur `t` — Formatter braucht keine `db.*`-Reads), Scope `value`/`fields`/`ctx`, `json_to_dynamic`, Fehler-Mapping. Vorlage 1:1: `server/src/script/engine/rhai.rs:140-297,365-398`.

**Files:**
- Modify: `client/src/script/engine/rhai.rs`
- Test: `client/src/script/engine/rhai.rs` (inline)

- [ ] **Step 1: Failing test — `value` + `ctx.t`**

In `client/src/script/engine/rhai.rs` inline-Test:

```rust
#[cfg(test)]
mod run_inputs {
    use super::*;
    use shared::script::capability::{CapabilityToken, ScriptTier};
    use shared::script::engine::ScriptInputs;
    use shared::script::manifest::ScriptManifest;
    use shared::script::testing::MockHostApi;

    fn manifest_with(caps: Vec<CapabilityToken>) -> ScriptManifest {
        ScriptManifest { tier: ScriptTier::Reader, capabilities: caps, ..Default::default() }
    }

    #[test]
    fn value_is_bound() {
        let eng = RhaiEngine::new();
        let ast = eng.compile("value", &ScriptManifest::default()).unwrap();
        let inputs = ScriptInputs { value: serde_json::json!("SOLL"), fields: Default::default() };
        let host = std::sync::Arc::new(MockHostApi::new());
        let out = eng.run(&ast, inputs, host, ScriptCtx::default()).unwrap();
        assert_eq!(out, ScriptValue::String("SOLL".into()));
    }

    #[test]
    fn ctx_t_requires_read_i18n() {
        // Ohne ReadI18n -> CapabilityDenied (unmaskable, per try/catch nicht fangbar).
        let eng = RhaiEngine::with_manifest(&manifest_with(vec![CapabilityToken::ComputeOnly]));
        let ast = eng.compile(r#"ctx.t("k")"#, &ScriptManifest::default()).unwrap();
        let host = std::sync::Arc::new(MockHostApi::new());
        let err = eng.run(&ast, ScriptInputs::default(), host, ScriptCtx::default()).unwrap_err();
        assert!(matches!(err, ScriptError::CapabilityDenied { .. }), "war {err:?}");
    }

    #[test]
    fn ctx_t_with_read_i18n_calls_host() {
        // MockHostApi.i18n_t echo't den Key (siehe shared::script::testing).
        let eng = RhaiEngine::with_manifest(&manifest_with(vec![CapabilityToken::ReadI18n]));
        let ast = eng.compile(r#"ctx.t("hello")"#, &ScriptManifest::default()).unwrap();
        let host = std::sync::Arc::new(MockHostApi::new());
        let out = eng.run(&ast, ScriptInputs::default(), host, ScriptCtx::default()).unwrap();
        // Erwartung an MockHostApi::i18n_t-Verhalten prüfen/anpassen
        // (gibt typischerweise den Key oder einen Marker zurück).
        assert!(matches!(out, ScriptValue::String(_)));
    }
}
```

(Hinweis: `RhaiEngine::with_manifest` existiert client-seitig heute nicht — wird in Step 3 mit eingeführt, analog Server. Falls `MockHostApi::i18n_t`-Verhalten unbekannt: vorab in `shared/src/script/testing.rs` nachsehen und die Assertion exakt darauf setzen.)

- [ ] **Step 2: Test verifizieren (fail/compile-fail)**

Run: `cargo test -p client --lib --target-dir target-prvverify -j 2 run_inputs 2>&1 | rg "test result|error|FAILED"; echo "EXIT=${PIPESTATUS[0]}"`
Expected: Compile-Fehler (`with_manifest` fehlt) bzw. FAIL — beweist, dass der Pfad noch nicht existiert.

- [ ] **Step 3: Client-Run-Pfad auf Server-Parität bringen**

In `client/src/script/engine/rhai.rs` (Vorlage: Server-Datei):
1. Imports: `use std::sync::Mutex;`, `use rhai::{Dynamic, Position, Scope};`, `use shared::script::capability::CapabilityToken;`, `use shared::script::engine::ScriptInputs;`, `use crate::script::sandbox::Sandbox;`.
2. `RhaiEngine` um `manifest: ScriptManifest` erweitern; `with_manifest(manifest)` + `new()` = `with_manifest(&Default::default())` (wie Server Zeile 43-63). `apply_limits` aus Manifest (Server Zeile 75-87) übernehmen.
3. `RunState { sandbox: Sandbox, host: Arc<dyn HostApi> }`, `HostBridge` (Server Zeile 144-153) übernehmen.
4. `run` ruft einen neuen `run_collecting(ast, inputs, host, ctx)`-Pfad (Server Zeile 164-199), der eine Engine mit `register_host_fns` baut, `scope.push("ctx", HostBridge)`, `value`/`fields` per `push_constant(json_to_dynamic(...))` bindet, evaluiert, `map_rhai_err` anwendet. (Client braucht den `db`-Scope/`entities` NICHT — nur `ctx` mit `t`.)
5. `register_host_fns` (nur `t`): Server Zeile 231-250 1:1 — `engine.register_type_with_name::<HostBridge>("HostBridge")`, dann `t` via `sandbox.gate(&CapabilityToken::ReadI18n, || host.i18n_t(&key, &Null))`, Fehler via `script_err_to_rhai`.
6. `script_err_to_rhai` (Server Zeile 284-297) + `map_rhai_err` mit `timeout_ms`/`memory_kb` (Server Zeile 365-398) + `json_to_dynamic` (aus Task 2, hier kopieren — die `rhai`-Isolation erlaubt das nur in dieser Datei) übernehmen.

`compile`/`rhai_to_script_value`/`compile_wasm` bleiben. Der Kommentar „keine Host-fns" (heutige Zeile 100-105) wird ersetzt.

- [ ] **Step 4: Test verifizieren (pass)**

Run: `cargo test -p client --lib --target-dir target-prvverify -j 2 run_inputs 2>&1 | rg "test result|FAILED"; echo "EXIT=${PIPESTATUS[0]}"`
Expected: `test result: ok. 3 passed`, EXIT=0.
Run zusätzlich die bestehenden provider_lookup-Tests: `cargo test -p client --lib --target-dir target-prvverify -j 2 provider_lookup 2>&1 | rg "test result|FAILED"; echo "EXIT=${PIPESTATUS[0]}"` → ok.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(script/client): Run-Pfad auf Server-Paritaet — Sandbox-Gate + ctx.t + value/fields

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>" -- client/src/script/engine/rhai.rs
```

---

## Task 4: Konkrete Client-`HostApi`-Impl (`RenderHost`)

`FieldCell` braucht ein `Arc<dyn HostApi>`, dessen `i18n_t` an `crate::i18n::t` delegiert. `crate::i18n::t` ist ein Leptos-Signal-Reader (nur im Owner-Kontext aufrufbar) — der `HostApi`-Wrapper kapselt das. Für einen Formatter werden nur `i18n_t` gebraucht; `db_*`/`audit_log` liefern für diese Welle `ServerOnlyFunction`/`HostError` (nicht erreichbar bei ComputeOnly+ReadI18n).

**Files:**
- Create: `client/src/script/render_host.rs`
- Modify: `client/src/script/mod.rs` (Modul anmelden)
- Test: `client/src/script/render_host.rs` (inline)

- [ ] **Step 1: Prüfen, ob bereits eine Impl existiert**

Run: `rg "impl HostApi for" client/src; echo "EXIT=$?"`
Wenn eine renderer-seitige Impl mit `i18n_t -> crate::i18n::t` existiert: diese in Task 6 wiederverwenden, Task 4 entfällt (im Plan vermerken). Sonst weiter mit Step 2.

- [ ] **Step 2: `RenderHost` schreiben**

`client/src/script/render_host.rs`:

```rust
//! Konkrete `HostApi`-Impl für den Client-Render-Pfad. Formatter-Scripts
//! brauchen nur `i18n_t` (Fluent-Lookup via `crate::i18n::t`). Die übrigen
//! Methoden sind in dieser Welle nicht erreichbar (ComputeOnly+ReadI18n).

use serde_json::Value;
use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct RenderHost;

impl HostApi for RenderHost {
    fn db_fetch(&self, _query: &Value) -> Result<Value, ScriptError> {
        Err(ScriptError::ServerOnlyFunction { name: "db.entities".into() })
    }
    fn db_patch(&self, _e: &str, _id: &str, _p: &Value) -> Result<(), ScriptError> {
        Err(ScriptError::ServerOnlyFunction { name: "db.patch".into() })
    }
    fn i18n_t(&self, key: &str, _args: &Value) -> Result<String, ScriptError> {
        Ok(crate::i18n::t(key))
    }
    fn audit_log(&self, _event: &str, _payload: &Value) -> Result<(), ScriptError> {
        Ok(())
    }
}
```

(Die exakten `ScriptError`-Varianten/Felder gegen `shared/src/script/error.rs` prüfen — `ServerOnlyFunction { name }` ist der in Q0009 verwendete; ggf. anpassen.)

In `client/src/script/mod.rs`: `pub mod render_host;`.

- [ ] **Step 3: Test — `i18n_t` delegiert**

Inline-Test (ohne Leptos-Owner schwierig, daher minimal — nur dass `RenderHost` `HostApi` erfüllt und konstruierbar ist; der echte `i18n::t`-Pfad wird in Task 7 im Browser geprüft):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_host_is_host_api() {
        let h: std::sync::Arc<dyn HostApi> = std::sync::Arc::new(RenderHost);
        assert!(h.db_fetch(&serde_json::Value::Null).is_err());
    }
}
```

- [ ] **Step 4: Verifizieren + Commit**

Run: `cargo test -p client --lib --target-dir target-prvverify -j 2 render_host 2>&1 | rg "test result|FAILED"; echo "EXIT=${PIPESTATUS[0]}"` → ok.
```bash
git commit -m "feat(script/client): RenderHost (HostApi) — i18n_t via crate::i18n::t

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>" -- client/src/script/render_host.rs client/src/script/mod.rs
```

---

## Task 5: `FieldCell` ruft den Formatter-Slot

`FieldCell` (`client/src/components/table/formatters.rs:19-42`) resolved heute direkt die `FieldRegistry`. Hier: Formatter-ID auflösen, bei `script:`-Prefix `lookup_provider(Formatter, inputs)`, Ok → String rendern, sonst Default-Pfad.

**Files:**
- Modify: `client/src/components/table/formatters.rs`
- Modify: ggf. Aufrufer von `FieldCell` (Spalten-`formatter_id` durchreichen) — `client/src/components/table/` (TableView/Row).
- Test: `client/src/components/table/formatters.rs` (inline, soweit ohne Leptos-Render möglich) + Browser-Augenschein in Task 7.

- [ ] **Step 1: `FieldCell`-Signatur um `formatter_id` erweitern**

`FieldCell` bekommt `formatter_id: Option<String>` (aus der `ColumnMeta` der Spalte; der Aufrufer in der Tabellen-Row reicht `col.formatter_id.clone()` durch — bzw. `resolve_implementation_id(col, defaults, "formatter")`, falls Defaults berücksichtigt werden sollen).

- [ ] **Step 2: Script-Branch im Render**

In `FieldCell` vor dem `registry.render(...)`:

```rust
    let script_out = move || -> Option<String> {
        let fid = formatter_id.clone()?;
        let raw = fid.strip_prefix(crate::script::provider_lookup::SCRIPT_PREFIX)?;
        let _ = raw; // nur Prefix-Gate; lookup_provider parst selbst
        let registry = use_context::<crate::script::registry::ScriptRegistry>()?;
        let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
            std::sync::Arc::new(crate::script::render_host::RenderHost);
        let ctx = shared::script::engine::ScriptCtx {
            locale: locale.get().to_string(),
            ..Default::default()
        };
        let inputs = shared::script::engine::ScriptInputs {
            value: value.clone(),
            fields: fields.clone(),
        };
        use crate::script::provider_lookup::{lookup_provider, script_value_to_display, LookupResult};
        match lookup_provider(&fid, shared::script::model::ProviderSlot::Formatter, inputs, &registry, host, ctx) {
            LookupResult::Ok { value } => Some(script_value_to_display(&value)),
            _ => None, // Fallback/NotAScriptId -> Default-Pfad
        }
    };
```

Render: wenn `script_out()` `Some(s)` → `<span>{s}</span>`, sonst der bisherige `registry.render(...)`-Pfad. (`ScriptRegistry` muss im Leptos-Context provided sein — prüfen wo das geschieht; falls nicht, in `app.rs`/`EntityListPage` ergänzen. Wenn keine Registry im Context: `None` → Default-Pfad, kein Crash.)

- [ ] **Step 3: Verifizieren — kein Regress für Nicht-Script-Spalten**

Run: `cargo build -p client --target-dir target-prvverify -j 2 2>&1 | tail -3; echo "EXIT=${PIPESTATUS[0]}"` → Finished.
Run: `cargo test -p client --lib --target-dir target-prvverify -j 2 2>&1 | rg "test result|FAILED"; echo "EXIT=${PIPESTATUS[0]}"` → alle ok (bestehende Field/Table-Tests grün; `formatter_id=None` → unverändertes Verhalten).

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(client): FieldCell ruft Formatter-Provider-Script (Fallback auf Default)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>" -- client/src/components/table/formatters.rs
```
(Pathspec um geänderte Aufrufer-Dateien ergänzen.)

---

## Task 6: d2v — Translatables, Formatter-ID, Script + Manifest

**Files:**
- Modify: `examples/d2v/translatables/entries.json`, `examples/d2v/translatables/values.json`
- Modify: `examples/d2v/entities/datev_entry/columns.json`
- Create: `examples/d2v/scripts/d2v_value_type_label.rhai`
- Create: `examples/d2v/scripts/d2v_value_type_label.manifest.json`
- Test: `server/tests/loader_d2v.rs`

- [ ] **Step 1: Translatables-Format prüfen**

`examples/d2v/translatables/entries.json` + `values.json` lesen, um das exakte Schema zu treffen (Key-Eintrag in `entries`, pro-Sprache-Wert in `values`). Die SOLL/HABEN-Keys `field.datev_entry.value_type.soll` / `.haben` analog zu einem bestehenden Eintrag ergänzen (de: „Soll"/„Haben"; en: „Debit"/„Credit"; fr: „Débit"/„Crédit").

- [ ] **Step 2: Script + Manifest anlegen**

`examples/d2v/scripts/d2v_value_type_label.rhai`:
```rhai
if value == "SOLL" { ctx.t("field.datev_entry.value_type.soll") }
else if value == "HABEN" { ctx.t("field.datev_entry.value_type.haben") }
else { "" }
```

`examples/d2v/scripts/d2v_value_type_label.manifest.json`:
```json
{
  "kind": { "kind": "provider", "slot": "formatter" },
  "manifest": {
    "manifestVersion": 1,
    "tier": "reader",
    "capabilities": [ { "kind": "computeOnly" }, { "kind": "readI18n" } ]
  }
}
```

- [ ] **Step 3: `formatterId` an die Spalte**

`examples/d2v/entities/datev_entry/columns.json`, valueType-Spalte: `"formatterId": "script:d2v_value_type_label"` ergänzen (nach `filterable`).

- [ ] **Step 4: Loader-Test — Script seedet Active + Formatter-Slot**

In `server/tests/loader_d2v.rs` ergänzen:

```rust
#[test]
fn d2v_value_type_formatter_script_loads_active() {
    let set = example::load(&d2v_dir()).expect("laden");
    let s = set
        .scripts
        .get("d2v_value_type_label")
        .expect("Script fehlt im Set");
    // Manifest geparst (kein manifest_error), Slot Formatter, Caps korrekt.
    assert!(s.manifest_error.is_none(), "manifest_error: {:?}", s.manifest_error);
    assert!(matches!(
        s.kind,
        shared::script::ScriptKind::Provider { slot: shared::script::ProviderSlot::Formatter }
    ));
}
```
(Feldnamen gegen `ScriptSeed`/`ExampleSet` in `server/src/example/loader.rs` prüfen — `set.scripts` ist die `BTreeMap<String, ScriptSeed>` aus `load_scripts`; ggf. den Zugriffspfad anpassen. Seeding-State `Active` wird beim DB-Seed gesetzt; der reine Loader liefert `ScriptSeed` — der Test prüft Parse + Slot + Caps.)

- [ ] **Step 5: Verifizieren**

Run: `cargo test -p server --test loader_d2v --target-dir target-prvverify -j 2 2>&1 | rg "test result|FAILED|value_type"; echo "EXIT=${PIPESTATUS[0]}"` → ok.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(examples/d2v): ValueType-Formatter-Script + Translatables (Soll/Haben)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>" -- examples/d2v/translatables/entries.json examples/d2v/translatables/values.json examples/d2v/entities/datev_entry/columns.json examples/d2v/scripts/d2v_value_type_label.rhai examples/d2v/scripts/d2v_value_type_label.manifest.json server/tests/loader_d2v.rs
```

---

## Task 7: Symmetrie + End-to-End-Verifikation

**Files:**
- Modify: ggf. `server/tests/script_symmetry.rs` (nur falls `t`/`i18n_t` nicht bereits symmetrisch gelistet)
- Manuell: Dev-Server + Browser

- [ ] **Step 1: Symmetrie-Test grün**

Run: `cargo test -p server --test script_symmetry --target-dir target-prvverify -j 2 2>&1 | rg "test result|FAILED|Drift"; echo "EXIT=${PIPESTATUS[0]}"`
Expected: ok. Falls Drift (z.B. `t`/`i18n_t` einseitig): die Client-`HostApiRegistry::functions()` (in `client/src/script/mod.rs`) an die Server-Liste angleichen (gleicher Name, `ReadI18n`-Token, `server_only=false`). Dann erneut grün.

- [ ] **Step 2: Gesamter Workspace grün (isoliert)**

Run: `cargo test --workspace --target-dir target-prvfinal -j 2 2>&1 | rg "test result: ok|^error|FAILED|test result: FAILED" | tail -60; echo "EXIT=${PIPESTATUS[0]}"`
Expected: nur `test result: ok`, EXIT=0. (Frisches Dir gegen Parallel-Session-Korruption; bei Phantom-Fehlern siehe Memory [[parallel-claude-sessions]].)

- [ ] **Step 3: Manueller Browser-Augenschein (nicht automatisierbar — explizit so berichten)**

Dev-Server gegen die echte d2v.db starten (`D2V_LEGACY_URL` auf `docs/production db/d2v.db` setzen), `trunk serve`, datev_entry-Liste öffnen. Erwartung: Spalte „Werttyp" zeigt lokalisiertes „Soll"/„Haben" (Script-Formatter), nicht „SOLL"/„HABEN" (Default-Fallback). Locale-Wechsel de↔en ändert das Label. Bei Script-Fehler: Fallback zeigt „SOLL"/„HABEN" + Audit-Queue-Log in der Konsole. **Dieses Ergebnis wird als manuell verifiziert berichtet — kein automatisierter Test deckt das Browser-Rendering.**

- [ ] **Step 4: Abschluss**

`superpowers:finishing-a-development-branch` aufrufen (Tests verifizieren → Optionen). Branch ist `dev` (shared) — Promotion/Push ist eine separate User-Entscheidung (nicht auto).

---

## Self-Review

**1. Spec-Coverage:**
- §2.1 ScriptInputs + run-Signatur → Task 1, 2, 3. ✅
- §2.2 ctx.t (ReadI18n-gated, via i18n::t) → Task 3 (Engine-Wiring), Task 4 (RenderHost). ✅
- §2.3 FieldCell-Wiring + Fallback → Task 5. ✅
- §2.4 d2v Config/Script/Translatables → Task 6. ✅
- §3 Datenfluss → über Task 5+6 abgedeckt; §4 Fehlerbehandlung → Fallback in Task 5, gate-Fehler Task 3. ✅
- §5 Tests → je Task; Symmetrie Task 7; Browser manuell Task 7. ✅
- §1 Abgrenzung (nur Formatter) → kein Filter/Validator-Task. ✅

**2. Placeholder-Scan:** Code in jedem implementierenden Step vorhanden. Zwei bewusste Verifikations-Schritte mit „prüfen/anpassen" (MockHostApi::i18n_t-Verhalten, ScriptError-Varianten, set.scripts-Zugriffspfad, bestehende HostApi-Impl) — das sind reale Unsicherheiten gegen fremden/parallel veränderten Code, kein Hand-wave: jeweils mit konkretem rg-/Datei-Check hinterlegt.

**3. Typ-Konsistenz:** `ScriptInputs { value: Value, fields: Map }`, `ScriptEngine::run(ast, inputs, host, ctx)`, `lookup_provider(id, slot, inputs, registry, host, ctx)`, `ProviderSlot::Formatter`, `CapabilityToken::ReadI18n`, `script_value_to_display` — durchgängig identisch über Task 1/3/5 verwendet. `RhaiEngine::with_manifest` client neu (Task 3), genutzt in Task-3-Tests. ✅

**Bewusste Risiko-Notiz:** Task 1 ist ein breiter Signatur-Ripple (viele Aufrufer). Falls eine Parallel-Session denselben Engine-Pfad anfasst → Merge sorgfältig, Aufrufer-Liste via `rg` frisch ermitteln statt der Plan-Liste blind vertrauen.
