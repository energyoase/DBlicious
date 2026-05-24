# Q0009 Review-Remediation (Engine↔Host-Verdrahtung) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Die offenen Blocker/Should-Fixes aus `docs/reviews/Q0009-review.md` schließen, die alle am Engine↔Host-Ausführungspfad hängen: **B3** (Host-Funktionen via `sandbox.gate()` im echten eval-Pfad), **B1** (unmaskable Errors uncatchbar), **S5** (Memory-Limits), **S7** (echter Timeout-Wert), **S6** (preview `db_patch`-Fehler), **S8** (client `audit.log`-Ablehnung).

**Architecture:** `ScriptEngine::run` nimmt den Host künftig als `Arc<dyn HostApi + Send + Sync>` statt `&dyn HostApi` — nötig, weil Rhais `register_fn`-Closures `'static` sind. Pro Run wird ein `RunState { sandbox, host }` als `Arc<Mutex<…>>` geteilt; die registrierten Host-Funktionen (`db.entities`/`db.entity`/`ui.*`/`ctx.t`) laufen durch `sandbox.gate(token, || host.…())`. Unmaskable `ScriptError` werden als `EvalAltResult::ErrorTerminated` (uncatchbar) propagiert und am Ende via Payload zurückgemappt.

**Tech Stack:** Rust, `shared` (Trait), `server` + `client` (Rhai-Adapter), Rhai 1.24, `cargo test`.

**Review-Bezug:** `docs/reviews/Q0009-review.md` (B1, B3, S5, S6, S7, S8). B2 + S4 sind bereits gefixt (Commits `1e4c78d`, `50a3476`).

**Verifizierte Rhai-API (Spike-Recherche 2026-05-24, rhai 1.24):**
- `EvalAltResult::ErrorTerminated(Dynamic, Position)` — **uncatchbar** per `try`/`catch` (terminiert das Skript). Trägt einen `Dynamic`-Payload → wir transportieren darin den serialisierten `ScriptError`-Tag.
- `EvalAltResult::ErrorDataTooLarge(String, Position)` — Size-Limit-Überschreitung → `MemoryExceeded`.
- `EvalAltResult::ErrorTooManyOperations(Position)` — Operation-Limit → `Timeout`.
- `Engine::register_fn` / `register_custom_syntax` vorhanden; Closures müssen `'static + Send + Sync` (sync-Feature aktiv).
- `Engine::set_max_string_size/array_size/map_size/operations` für die Limits.

**Decisions aufgenommen:**
- **`Arc<dyn HostApi>` statt `&dyn`** (Trait-Bruch): die einzig saubere Art, den Host in `'static` Rhai-Closures zu halten. Betrifft `shared` (Trait + `Send+Sync`-Bound), `server`/`client` (Impl + Aufrufer + Tests). Alternative (thread-local Host) wäre fragiler und schlechter testbar.
- **`db` als Custom-Type mit Methoden** (`db.entities(t)`, `db.entity(t,id)`): die `analyze_lift_capability`-Analyse sucht `Expr::MethodCall{name:"entities"/"entity"}` — der Wire-Vertrag ist also Methoden-Syntax, kein freies `db_entities(...)`. Ein Spike (Task 1) nagelt fest, wie ein Custom-Type mit `&mut RunState`-Zugriff in Rhai registriert wird.
- **unmaskable → `ErrorTerminated`**: maskable Fehler bleiben `ErrorRuntime` (per `catch` fangbar), unmaskable werden `ErrorTerminated`. `map_rhai_err` dröselt beide zurück auf den ursprünglichen `ScriptError`.

---

## File Structure

**Modifizieren:**
- `shared/src/script/engine.rs` — `run`-Signatur auf `Arc<dyn HostApi + Send + Sync>`; `HostApi: Send + Sync`.
- `server/src/script/engine/rhai.rs` — Engine-Factory mit Manifest (Limits), Host-fn-Registrierung durch Gate, `RunState`, `map_rhai_err` mit Kontext + unmaskable-Rückmapping.
- `server/src/script/run.rs` — `run_and_persist`/`run_preview`: Host als `Arc`, Body-Closure entfällt bzw. ruft echten `engine.run`.
- `server/src/script/provider_lookup.rs` — Aufrufer auf `Arc<dyn HostApi>` (Zeile ~116-148).
- `server/src/schema.rs` — alle `run_and_persist`/`run_preview`/`provider_lookup`-Aufrufer; S6 (preview `db_patch` → `ServerOnlyFunction`).
- `client/src/script/engine/rhai.rs` — gleicher Engine-Umbau (Symmetrie), soweit der Client einen run-Pfad hat.
- `client/src/script/host/audit.rs` — S8: `log()` → `ServerOnlyFunction`.
- `shared/src/script/testing.rs` — `MockHostApi` muss `Send + Sync` + `Arc`-fähig sein.

**Neu (Tests):**
- `server/tests/script_gate_integration.rs` — beweist, dass die Gate bei **echter** Engine-Ausführung feuert (der vom Review vermisste Test).

---

## Architektur-Details

### Trait-Änderung (`shared/src/script/engine.rs`)

```rust
use std::sync::Arc;

pub trait HostApi: Send + Sync {
    fn db_fetch(&self, query: &serde_json::Value) -> Result<serde_json::Value, ScriptError>;
    fn db_patch(&self, entity_type: &str, id: &str, patch: &serde_json::Value) -> Result<(), ScriptError>;
    fn i18n_t(&self, key: &str, args: &serde_json::Value) -> Result<String, ScriptError>;
    fn audit_log(&self, event: &str, payload: &serde_json::Value) -> Result<(), ScriptError>;
}

pub trait ScriptEngine {
    type Ast: Clone + Send + Sync;
    fn compile(&self, source: &str, manifest: &ScriptManifest) -> Result<Self::Ast, ScriptError>;
    fn run(
        &self,
        ast: &Self::Ast,
        host: Arc<dyn HostApi>,
        ctx: ScriptCtx,
    ) -> Result<ScriptValue, ScriptError>;
}
```

### RunState + Host-fn-Registrierung (`server/src/script/engine/rhai.rs`)

```rust
use std::sync::{Arc, Mutex};

/// Pro-Run geteilter Zustand: Sandbox (Token-Audit + Gate) + Host. Wird
/// in die registrierten Host-Funktionen via Arc<Mutex<…>> gecaptured.
struct RunState {
    manifest: ScriptManifest,   // owned: kein 'm-Lifetime in 'static-Closures
    host: Arc<dyn HostApi>,
    token_uses: Vec<TokenUse>,
}
```

`run()` baut pro Aufruf eine Engine (oder klont eine Basis + registriert die fns mit dem frischen `Arc<Mutex<RunState>>`), registriert die Host-Methoden, ruft `eval_ast_with_scope`, mappt Fehler. Die `Sandbox`-Gate-Logik (capability-check + token_uses-push) wird in `RunState` integriert oder die `Sandbox` owned das manifest.

> **Spike Task 1 entscheidet:** ob `Sandbox` auf owned-manifest umgestellt wird oder die Gate-Logik in `RunState` zieht; und wie `db` als Custom-Type mit `&mut RunState`-Zugriff registriert wird (Rhai custom-type-methods + captured Arc).

### unmaskable → ErrorTerminated

In der registrierten Host-fn:

```rust
match sandbox_gate_result {
    Ok(v) => Ok(rhai_value(v)),
    Err(e) if e.unmaskable() => {
        // uncatchbar: ScriptError-Tag im Payload transportieren
        Err(Box::new(EvalAltResult::ErrorTerminated(
            rhai::Dynamic::from(serde_json::to_string(&e).unwrap_or_default()),
            rhai::Position::NONE,
        )))
    }
    Err(e) => Err(Box::new(EvalAltResult::ErrorRuntime(
        rhai::Dynamic::from(serde_json::to_string(&e).unwrap_or_default()),
        rhai::Position::NONE,
    ))),
}
```

`map_rhai_err` erkennt `ErrorTerminated`/`ErrorRuntime` mit JSON-Payload und deserialisiert zurück zu `ScriptError`; `ErrorTooManyOperations → Timeout{timeout_ms}`, `ErrorDataTooLarge → MemoryExceeded`.

### Memory/Timeout-Limits (S5/S7)

Engine-Factory liest `manifest.memory_kb`/`timeout_ms`:
```rust
if let Some(kb) = manifest.memory_kb {
    engine.set_max_string_size((kb as usize) * 1024);
    engine.set_max_array_size((kb as usize) * 128);
    engine.set_max_map_size((kb as usize) * 128);
}
```
`timeout_ms` wird im Engine-Struct gespeichert und in `map_rhai_err` für `Timeout{limit_ms}` genutzt.

---

## Tasks

### Task 1: Spike — Rhai Custom-Type `db` mit Shared-RunState

**Files:** temporär `server/src/script/engine/rhai.rs`

- [ ] **Schritt 1:** Minimaler Spike: einen Custom-Type registrieren, dessen Methode `entities(&mut self, t: &str)` auf einen via Closure gecaptured `Arc<Mutex<RunState>>` zugreift und einen Wert zurückgibt. Verifizieren: `db.entities("product")` aus einem Skript ruft die Rust-fn und der `RunState` ist sichtbar/mutierbar.
- [ ] **Schritt 2:** Verifizieren, dass `EvalAltResult::ErrorTerminated` aus der fn NICHT von `try { db.entities("x") } catch(e) { 42 }` gefangen wird (eval-Ergebnis ist Error, nicht 42).
- [ ] **Schritt 3:** Befund als `//! VERIFIED:`-Kommentar festhalten. Gate: wenn Custom-Type-Methoden + Shared-State nicht in <1.5h tragen → alternativen Ansatz (globale `register_fn` + `db`-Objekt via Scope-Variable) dokumentieren und Plan-Tasks anpassen.

```
cargo check -p server --target-dir target-q0009
```

### Task 2: Trait-Bruch `Arc<dyn HostApi>` (shared)

**Files:** `shared/src/script/engine.rs`, `shared/src/script/testing.rs`

- [ ] **Schritt 1:** `HostApi: Send + Sync` + `run(... host: Arc<dyn HostApi> ...)` wie oben. `use std::sync::Arc;`.
- [ ] **Schritt 2:** `MockHostApi` (testing.rs) auf `Send + Sync` prüfen; interne `RefCell` → `Mutex` falls nötig (für Sync).
- [ ] **Schritt 3:** `cargo check -p shared --target-dir target-q0009` — erwartet Folgefehler in server/client (nächste Tasks).
- [ ] **Schritt 4:** Commit `refactor(script): run() takes Arc<dyn HostApi> (Q0009 B3 prep)`.

### Task 3: Server-Engine — Host-fns durch Gate + Limits + Fehler-Mapping (B3/B1/S5/S7)

**Files:** `server/src/script/engine/rhai.rs`

- [ ] **Schritt 1:** `RunState` + Engine-Factory mit Manifest-Limits (S5/S7-Felder). `run()` registriert `db.entities`/`db.entity` (Token `ReadOwnEntities`), `ui.text`/`ui.vstack`/… (`EmitUiNode`), `ctx.t` (`ReadI18n`) als Custom-Type-Methoden, die durch die Gate laufen (Befund Task 1).
- [ ] **Schritt 2:** unmaskable → `ErrorTerminated`, maskable → `ErrorRuntime` (JSON-Payload). `map_rhai_err(self, e)`: Payload-Deserialisierung; `ErrorTooManyOperations → Timeout{self.timeout_ms}`; `ErrorDataTooLarge → MemoryExceeded`.
- [ ] **Schritt 3:** `token_uses` aus `RunState` nach dem Run auslesen (für Audit). `run_and_persist` zieht sie von dort statt aus der externen `body`-Closure.
- [ ] **Schritt 4:** `cargo check -p server --target-dir target-q0009`.

### Task 4: run.rs + provider_lookup + schema-Aufrufer (B3-Integration)

**Files:** `server/src/script/run.rs`, `server/src/script/provider_lookup.rs`, `server/src/schema.rs`

- [ ] **Schritt 1:** `run_and_persist`/`run_preview`: Host als `Arc<dyn HostApi>`, Body-Closure entfernen — direkt `engine.run(&ast, host, ctx)` rufen; `token_uses` aus dem Run-Ergebnis.
- [ ] **Schritt 2:** `provider_lookup` (Zeile ~116-148): `host: Arc<dyn HostApi>` durchreichen.
- [ ] **Schritt 3:** Alle `schema.rs`-Aufrufer anpassen (Host als `Arc`).
- [ ] **Schritt 4:** `cargo check -p server --target-dir target-q0009`.

### Task 5: Gate-Integrations-Test (B3-Beweis)

**Files:** `server/tests/script_gate_integration.rs`

- [ ] **Schritt 1:** Test: Skript `db.entities("product")` mit Manifest **ohne** `ReadOwnEntities` → Run endet mit `CapabilityDenied` (Gate feuert bei echter Ausführung). Mit Token → Run liefert die gemockten Daten.
- [ ] **Schritt 2:** Test: `try { db.entities("x") } catch(e) { 42 }` ohne Token → Ergebnis ist **nicht** 42 (B1: unmaskable nicht fangbar).
- [ ] **Schritt 3:** `cargo test -p server --test script_gate_integration --target-dir target-q0009` → grün.
- [ ] **Schritt 4:** Commit `feat(script): wire host fns through sandbox gate (Q0009 B3+B1+S5+S7)`.

### Task 6: Client-Engine-Symmetrie

**Files:** `client/src/script/engine/rhai.rs`, `client/tests/script_engine.rs`

- [ ] **Schritt 1:** Gleicher Engine-Umbau wie Server, soweit der Client-run-Pfad ihn nutzt (mind. Trait-Konformität + Limits + Fehler-Mapping). Falls der Client keine echten Host-fns registriert (Reads gehen über GraphQL im Renderer), mindestens die Signatur + map_rhai_err angleichen.
- [ ] **Schritt 2:** `cargo test -p client --test script_engine --target-dir target-q0009`.
- [ ] **Schritt 3:** Commit `feat(script): client engine symmetry for gate wiring (Q0009)`.

### Task 7: S6 + S8 (Fehler-Konsistenz)

**Files:** `server/src/schema.rs`, `server/src/script/run.rs`, `client/src/script/host/audit.rs`

- [ ] **Schritt 1 (S6):** preview `db_patch`-Pfad → `ServerOnlyFunction` statt `CapabilityDenied` (konsistent mit Spec-Intent). Test im preview-Bereich.
- [ ] **Schritt 2 (S8):** client `audit.rs::log()` → `ServerOnlyFunction`-Ablehnung analog `db.patch`. **Achtung:** der bestehende Test `host_audit_log_via_mock_records_event` erwartet Delegation — Test anpassen oder einen separaten „echten Client-Run lehnt ab"-Pfad einführen (Doku-Ambiguität aus dem Review auflösen).
- [ ] **Schritt 3:** `cargo test` für die betroffenen Suites.
- [ ] **Schritt 4:** Commit `fix(script): consistent ServerOnlyFunction for preview patch + client audit (Q0009 S6+S8)`.

### Task 8: Review-Doc + Workspace-Verifikation

**Files:** `docs/reviews/Q0009-review.md`

- [ ] **Schritt 1:** Im Review-Doc die gefixten Punkte als ✅ markieren (B1, B2, B3, S4, S5, S6, S7, S8) mit Commit-Refs.
- [ ] **Schritt 2:** `cargo test --workspace --target-dir target-q0009 -j 2` — bei flaky Fails (Parallel-Sessions) betroffene Tests einzeln re-runnen (Memory `parallel-claude-sessions`). Exit-Code direkt erfassen, **nie** `| tail`.
- [ ] **Schritt 3:** Commit `docs(review): Q0009 blocker remediation done`.

---

## Self-Review (gegen Review-Doc)

- B1 (unmaskable) → Task 3 Schritt 2 + Task 5 Schritt 2 ✓
- B3 (Gate im run-Pfad) → Task 3 + Task 4 + Task 5 ✓
- S5 (Memory) → Task 3 Schritt 1 ✓
- S7 (Timeout-Wert) → Task 3 Schritt 2 ✓
- S6 (preview patch) → Task 7 Schritt 1 ✓
- S8 (client audit) → Task 7 Schritt 2 ✓
- B2, S4 → bereits gefixt (Commits `1e4c78d`, `50a3476`) ✓

## Risiken

- **Trait-Bruch-Blast-Radius:** `Arc<dyn HostApi>` betrifft alle run-Aufrufer + Tests in client+server. Task 2-4 inkrementell mit `cargo check` zwischen den Schritten.
- **Rhai-Shared-State (Task 1 Gate):** Custom-Type + `&mut RunState` via Arc<Mutex> ist das Hauptrisiko. Falls es nicht trägt, Fallback auf globale `register_fn` mit `db`-Scope-Objekt — Plan-Tasks dann anpassen.
- **65-min-Builds durch Parallel-Sessions:** eigener `target-q0009`-dir reduziert Lock-Konkurrenz; nach dem ersten Build inkrementell schnell.
- **MockHostApi Send+Sync:** falls interne Interior-Mutability `RefCell` nutzt, auf `Mutex` umstellen (Task 2).

## Was NICHT in diesem Plan ist

- Neue Host-Funktionen über den Review-Scope hinaus.
- WASM-Skript-Pfad (weiterhin hart rejected).
- Distributed-Locking / Multi-Server.
