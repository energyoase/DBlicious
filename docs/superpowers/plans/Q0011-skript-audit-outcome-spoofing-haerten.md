# Q0011 — Skript-Audit-Outcome-Spoofing haerten — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ein Rhai-Skript darf den gemeldeten `ScriptError`-`kind` seines eigenen Runs nicht mehr per `throw "<json>"` faelschen — Host-originierte Fehler werden ueber einen nicht-skript-erzeugbaren Kanal transportiert, alle echten Host-Fehler bleiben unveraendert korrekt gemeldet.

**Architecture:** Primaerer Mechanismus = **Option B** (Sentinel-`Dynamic`) aus der Diagnose §5. Host-Fehler reisen nicht mehr als JSON-String im `EvalAltResult`-Payload, sondern als geboxter Custom-Rust-Typ `HostErrorPayload(ScriptError)`. `map_rhai_err` rekonstruiert den echten Fehler per `payload.try_cast::<HostErrorPayload>()` statt `into_string()` + `serde_json::from_str`. Ein Skript-`throw` kann nur primitive/Map-`Dynamic`s erzeugen, nie diesen Typ — damit ist der Payload ueber den Rust-Typ authentifiziert. Der bisherige `ErrorRuntime`/`ErrorTerminated`-Arm liefert fuer alles Nicht-Authentifizierte einen generischen `HostError`, **nie** einen skript-gewaehlten typisierten Fehler. Der Fix wird **symmetrisch auf Server und Client** angewendet (Diagnose §6 „Client-Symmetrie"). Falls B's Custom-Cast ohne `rhai/serde`-Feature unerwartet zickt, ist **Option A** (Diagnose §5) der dokumentierte Fallback (Task 1 entscheidet das via Spike).

**Tech Stack:** Rust (workspace `shared`/`server`/`client`), `rhai 1.24.0` (Cargo.lock:5891, `serde`-Feature bewusst AUS), `serde`/`serde_json`, `cargo test`, `cargo clippy`, `cargo fmt`.

**Variant-Entscheidung (Akzeptanzkriterium 1):** Der unerkannte/skript-erzeugte Wurf wird **als `ScriptError::HostError { source }` gemeldet**, nicht als neue `ScriptThrew`-Variante. Begruendung: (a) Akzeptanzkriterium 1 verlangt nur, dass der gefaelschte `kind` nicht waehlbar ist — `HostError` erfuellt das; (b) `outcome_tag` (`server/src/script/run.rs:47`) mappt `HostError` bereits auf `"hostError"`, und der Nicht-JSON-Pfad (`throw "irgendwas"`) liefert heute schon `HostError` (rhai.rs:411) — eine eigene Variante wuerde `ScriptError` (`shared/src/script/error.rs`), `outcome_tag`, das Wire-Format-Pin (`shared/tests/script_wire_format.rs`) und beide Engine-Arme zusaetzlich aufblaehen, ohne ein Kriterium zu erfuellen. Trade-off: ein gefaelschter Wurf ist im Audit nicht von einem echten internen `HostError` unterscheidbar; das ist akzeptabel, weil der Wurf ohnehin bedeutet „Skript hat einen unstrukturierten Fehler ausgeloest". Sollte der Reviewer eine Trennung verlangen, ist `ScriptThrew { source: String }` der minimale Folgeschritt (eine Variante + ein `outcome_tag`-Arm + ein Wire-Format-Fall).

---

## File Structure

Geaenderte Produktions-Dateien (Diagnose §6 „Betroffene Codeareale"):

- `server/src/script/engine/rhai.rs` — neuer Typ `HostErrorPayload`, `script_err_to_rhai:291-304` (Payload-Konstruktion), `map_rhai_err:401-434` (Payload-Rekonstruktion). `RunState:145-148` / `run_collecting:165-206` bleiben unberuehrt (Option B braucht kein Seitenkanal-Feld).
- `client/src/script/engine/rhai.rs` — identischer neuer Typ `HostErrorPayload`, `script_err_to_rhai:226-239`, `map_rhai_err:294-325`. Symmetrisch zum Server.

Geaenderte/neue Test-Dateien:

- `server/tests/script_gate_integration.rs` — neuer synchroner Engine-Spoof-Pin (kein DB, kein `#[serial]`).
- `server/tests/script_run.rs` — neuer E2E-Outcome-Pin ueber `run_and_persist` (`fresh_test_setup().await` + `#[serial]`).
- `client/tests/script_engine.rs` — Client-Spiegel des Spoof-Pins (Symmetrie).

**Keine** Aenderung an `shared/src/script/error.rs` (Variant-Entscheidung: `HostError` wiederverwenden), an `run.rs::outcome_tag` (mappt `HostError` schon korrekt) oder am Symmetrie-Test `server/tests/script_symmetry.rs` (der pinnt die HostApi-Registry, nicht den Fehlertransport — er muss nur gruen bleiben).

---

## Task 1: Spike — `Dynamic`-Custom-Cast ohne `rhai/serde`-Feature verifizieren

> Diagnose §5 Option B (Minuspunkt) und §6 „Offene Detailfrage": vor der Festlegung muss empirisch bestaetigt sein, dass `Dynamic::from(custom_type)` + `try_cast::<custom_type>()` ohne das (bewusst deaktivierte) `rhai/serde`-Feature sauber funktioniert. Erwartet: ja (reines Custom-Type-Boxing). Dieser Task ist eine **Wegwerf-Probe**, sie wird am Ende wieder entfernt.

**Files:**
- Test (temporaer): `server/tests/script_gate_integration.rs` (Probe-Test, wird in Task 1 Step 4 wieder geloescht)

- [ ] **Step 1: Wegwerf-Probe-Test als 3-Zeilen-Spike schreiben**

Ans Ende von `server/tests/script_gate_integration.rs` anfuegen:

```rust
#[test]
fn spike_custom_dynamic_roundtrips_without_serde_feature() {
    // SPIKE (Q0011 Task 1): beweist, dass ein Custom-Rust-Typ ueber rhai::Dynamic
    // geboxt und per try_cast zurueckgeholt werden kann, OHNE das rhai/serde-Feature.
    // Erfolg => Option B ist gangbar. Fehlschlag/Compile-Error => Fallback Option A.
    #[derive(Clone)]
    struct Probe(u32);
    let d: rhai::Dynamic = rhai::Dynamic::from(Probe(7));
    let back: Probe = d.try_cast::<Probe>().expect("custom type muss zurueckcastbar sein");
    assert_eq!(back.0, 7);
    // Gegenprobe: ein String-Dynamic darf NICHT als Probe castbar sein.
    let s: rhai::Dynamic = rhai::Dynamic::from("nope".to_string());
    assert!(s.try_cast::<Probe>().is_none(), "String darf nicht als Probe casten");
}
```

- [ ] **Step 2: Probe laufen lassen**

Run: `cargo test -p server --test script_gate_integration spike_custom_dynamic_roundtrips_without_serde_feature`
(Falls `server.exe`-Lock unter Windows: `cargo test --target-dir target-test -p server --test script_gate_integration spike_custom_dynamic_roundtrips_without_serde_feature`.)
Expected: PASS — `try_cast::<Probe>()` liefert `Some(Probe(7))`, der String-Dynamic `None`.

- [ ] **Step 3: Entscheidung dokumentieren**

- **PASS (erwartet):** Weiter mit Option B (Tasks 2–7).
- **Compile-Error / FAIL:** Option B ist nicht gangbar. STOP und auf **Fallback Option A** (Diagnose §5) umschwenken: `RunState` (`server/src/script/engine/rhai.rs:145-148`) um `last_host_error: Option<ScriptError>` erweitern, in `gated_db_fetch`/`t`-fn vor dem `script_err_to_rhai`-Wurf `state.lock().last_host_error = Some(e.clone())` setzen, in `run_collecting` bei `Err(_)` zuerst `last_host_error` lesen und nur sonst generisch mappen; `map_rhai_err` verliert in beiden Faellen den `serde_json::from_str`-Zweig. Die Tasks 4–7 (Tests) bleiben identisch, nur die Produktions-Schritte (Task 2/3/5/6) werden durch die A-Variante ersetzt. Die Reihenfolge-Subtilitaet aus Diagnose §5 (catch-und-spaeter-throw ueberschreibt `last_host_error`) muss dann via Test abgedeckt werden.

- [ ] **Step 4: Probe-Test wieder entfernen**

Den in Step 1 hinzugefuegten `spike_custom_dynamic_roundtrips_without_serde_feature`-Test wieder loeschen — er ist eine Wegwerf-Probe, kein Dauer-Pin. (Der echte Schutz wird in Task 4 gepinnt.)

- [ ] **Step 5: Kein Commit**

Dieser Task laesst keinen Code zurueck (Probe entfernt). Nichts committen.

---

## Task 2: Spoof-Pin-Test RED — Engine-Ebene (server)

> Diagnose §4 Test 1 (Spoof-Pin, Akzeptanzkriterium 1+3) und §1 „Konkreter Reproducer (TDD-tauglich, synchron)". Stil von `server/tests/script_gate_integration.rs` (Engine direkt, kein DB, kein `#[serial]`). TDD: zuerst rot, gegen den heutigen Bug.

**Files:**
- Test: `server/tests/script_gate_integration.rs` (anfuegen)

- [ ] **Step 1: Failing-Test mit parametrisierter Helper schreiben**

Ans Ende von `server/tests/script_gate_integration.rs` anfuegen (Imports `Arc`, `ScriptCtx`, `ScriptEngine`, `ScriptInputs`, `MockHostApi`, `CapabilityToken`, `ScriptError`, `ScriptManifest`, `ScriptTier` sind oben in der Datei bereits vorhanden):

```rust
/// Q0011: ein Skript-`throw "<json>"` darf den gemeldeten ScriptError-`kind`
/// NICHT bestimmen. Egal welchen typisierten Fehler die JSON behauptet, der
/// Run muss als generischer HostError enden (Akzeptanzkriterium 1).
fn assert_forged_throw_maps_to_host_error(forged_json: &str) {
    let manifest = manifest_with(vec![CapabilityToken::ComputeOnly]);
    let engine = server::script::engine::RhaiEngine::with_manifest(&manifest);
    // throw "<escaped-json>" — der String-Payload landet im ErrorRuntime.
    let source = format!(r#"throw "{}""#, forged_json.replace('"', r#"\""#));
    let ast = engine.compile(&source, &manifest).expect("compile");
    let host = Arc::new(MockHostApi::new());
    let res = engine.run(&ast, ScriptInputs::default(), host, ScriptCtx::default());
    assert!(
        !matches!(
            res,
            Err(ScriptError::Timeout { .. })
                | Err(ScriptError::MemoryExceeded { .. })
                | Err(ScriptError::CapabilityDenied { .. })
                | Err(ScriptError::ServerOnlyFunction { .. })
        ),
        "gefaelschter throw bestimmte den kind ({forged_json}): {res:?}"
    );
    assert!(
        matches!(res, Err(ScriptError::HostError { .. })),
        "erwartete generischen HostError fuer ({forged_json}), war: {res:?}"
    );
}

#[test]
fn forged_throw_does_not_determine_reported_error_kind() {
    assert_forged_throw_maps_to_host_error(r#"{"kind":"timeout","limit_ms":99999}"#);
    assert_forged_throw_maps_to_host_error(r#"{"kind":"memoryExceeded","limit_kb":99999}"#);
    assert_forged_throw_maps_to_host_error(
        r#"{"kind":"capabilityDenied","token":"readOwnEntities"}"#,
    );
    assert_forged_throw_maps_to_host_error(r#"{"kind":"serverOnlyFunction","name":"x"}"#);
}

/// Kontroll-Pin (Diagnose §4 Test 4): ein Nicht-JSON-Wurf muss schon heute —
/// und auch nach dem Fix — als HostError{source:"..."} enden. Der Fix darf den
/// Nicht-JSON-Pfad nicht brechen.
#[test]
fn non_json_throw_still_maps_to_host_error() {
    let manifest = manifest_with(vec![CapabilityToken::ComputeOnly]);
    let engine = server::script::engine::RhaiEngine::with_manifest(&manifest);
    let ast = engine.compile(r#"throw "plain-non-json""#, &manifest).expect("compile");
    let host = Arc::new(MockHostApi::new());
    let res = engine.run(&ast, ScriptInputs::default(), host, ScriptCtx::default());
    match res {
        Err(ScriptError::HostError { source }) => assert!(
            source.contains("plain-non-json"),
            "HostError.source soll den Wurf-String tragen, war: {source}"
        ),
        other => panic!("erwartete HostError mit Wurf-String, war: {other:?}"),
    }
}
```

- [ ] **Step 2: Test laufen lassen und Rotfaerbung bestaetigen**

Run: `cargo test -p server --test script_gate_integration forged_throw_does_not_determine_reported_error_kind`
(Windows-Lock-Fallback: `--target-dir target-test` voranstellen wie in Task 1.)
Expected: FAIL — heute deserialisiert `map_rhai_err` den gefaelschten JSON zu z.B. `ScriptError::Timeout { limit_ms: 99999 }` (Diagnose §1 Schritt 4–5), die `!matches!`-Assertion schlaegt fehl. Der Kontroll-Test `non_json_throw_still_maps_to_host_error` ist bereits jetzt **gruen** (rhai.rs:411) — das ist erwartet und beweist, dass nur der JSON-Pfad kaputt ist.

- [ ] **Step 3: Commit (Red-Test)**

```bash
git add server/tests/script_gate_integration.rs
git commit -m "test(script): RED Spoof-Pin fuer Q0011 (forged throw)"
```

---

## Task 3: Sentinel-Payload einfuehren — Server (Option B), Spoof-Pin GREEN

> Diagnose §5 Option B + Empfehlung §5. Kern-Fix: Host-Fehler als `HostErrorPayload(ScriptError)`-Dynamic transportieren, `map_rhai_err` per `try_cast` rekonstruieren, JSON-Deser-Zweig entfernen.

**Files:**
- Modify: `server/src/script/engine/rhai.rs` — neuer Typ + `script_err_to_rhai:291-304` + `map_rhai_err:401-434`

- [ ] **Step 1: Sentinel-Typ definieren**

In `server/src/script/engine/rhai.rs` direkt vor `fn script_err_to_rhai` (aktuell Zeile 291) einfuegen:

```rust
/// Sentinel-Transport fuer host-originierte `ScriptError`s (Q0011).
///
/// Host-Fehler reisen als geboxter Custom-Typ im `EvalAltResult`-Payload, NICHT
/// mehr als JSON-String. Ein Skript-`throw` kann nur primitive/Map-`Dynamic`s
/// erzeugen — niemals diesen Typ. Damit ist der gemeldete `ScriptError`-`kind`
/// authentifiziert ueber den Rust-Typ statt ueber Vertrauen in den Payload.
/// `try_cast::<HostErrorPayload>()` braucht das `rhai/serde`-Feature NICHT
/// (reines Custom-Type-Boxing, in Task 1 verifiziert).
#[derive(Clone)]
struct HostErrorPayload(ScriptError);
```

- [ ] **Step 2: `script_err_to_rhai` auf Sentinel umstellen**

`server/src/script/engine/rhai.rs:291-304` ersetzen:

```rust
fn script_err_to_rhai(e: ScriptError) -> Box<EvalAltResult> {
    let unmaskable = e.unmaskable();
    let payload = Dynamic::from(HostErrorPayload(e));
    if unmaskable {
        Box::new(EvalAltResult::ErrorTerminated(payload, Position::NONE))
    } else {
        Box::new(EvalAltResult::ErrorRuntime(payload, Position::NONE))
    }
}
```

(Hinweis: `e.unmaskable()` muss VOR dem `Dynamic::from(HostErrorPayload(e))` gelesen werden, weil `e` in den Payload moved.)

- [ ] **Step 3: `map_rhai_err` auf `try_cast` umstellen, JSON-Deser entfernen**

Den `ErrorTerminated`/`ErrorRuntime`-Arm in `server/src/script/engine/rhai.rs:406-416` ersetzen:

```rust
        // Host-fn-Fehler reisen als authentifizierter Sentinel-Payload
        // (`HostErrorPayload`, siehe `script_err_to_rhai`). NUR dieser Typ
        // rekonstruiert den echten ScriptError. Alles andere (insbesondere ein
        // Skript-`throw "<json>"`, das nur primitive/Map-Dynamics erzeugt) wird
        // generisch als HostError gemeldet — der gemeldete `kind` ist damit
        // NICHT mehr vom Skript waehlbar (Q0011, Akzeptanzkriterium 1).
        EvalAltResult::ErrorTerminated(payload, _) | EvalAltResult::ErrorRuntime(payload, _) => {
            match payload.try_cast::<HostErrorPayload>() {
                Some(HostErrorPayload(err)) => err,
                None => ScriptError::HostError {
                    source: format!("{remaining}"),
                },
            }
        }
```

Wichtig: `payload.try_cast` konsumiert `payload`; fuer den `None`-Zweig braucht es den urspruenglichen Fehler-Text. Da der Match-Arm `e` bereits in `payload` zerlegt hat, muss der Fehler-Text vor dem `try_cast` gesichert werden. Konkrete, kompilierbare Form des gesamten `match e`-Bodys (ersetzt `map_rhai_err`-Rumpf `402-433`):

```rust
fn map_rhai_err(e: EvalAltResult, timeout_ms: u32, memory_kb: u32) -> ScriptError {
    // Lesbare Beschreibung VOR dem Move sichern (fuer den HostError-Fallback,
    // wenn der Payload kein authentifizierter HostErrorPayload ist).
    let descr = e.to_string();
    match e {
        EvalAltResult::ErrorTerminated(payload, _) | EvalAltResult::ErrorRuntime(payload, _) => {
            match payload.try_cast::<HostErrorPayload>() {
                Some(HostErrorPayload(err)) => err,
                None => ScriptError::HostError { source: descr },
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
```

(Anmerkung zum Kontroll-Test `non_json_throw_still_maps_to_host_error`: `EvalAltResult::to_string()` fuer ein `ErrorRuntime("plain-non-json")` enthaelt den Wurf-String `plain-non-json` — die `source.contains("plain-non-json")`-Assertion in Task 2 bleibt damit erfuellt. Falls `to_string()` den String unerwartet nicht enthaelt, im `None`-Zweig stattdessen `payload`-vorab-Klon nutzen: vor `try_cast` `let raw = payload.clone().into_string().ok();` und `source: raw.unwrap_or(descr)`.)

- [ ] **Step 4: Spoof-Pin + Kontroll-Test laufen lassen (GREEN)**

Run: `cargo test -p server --test script_gate_integration`
(Windows-Lock-Fallback: `--target-dir target-test`.)
Expected: PASS — `forged_throw_does_not_determine_reported_error_kind` und `non_json_throw_still_maps_to_host_error` gruen; die Bestands-Tests `db_call_without_token_is_denied_at_real_execution`, `db_call_with_token_returns_seeded_data`, `capability_denied_is_not_catchable_via_try_catch` bleiben gruen (echte Denies reisen jetzt als `HostErrorPayload(CapabilityDenied)` und werden korrekt rekonstruiert).

- [ ] **Step 5: Inline-Engine-Tests in rhai.rs gegenpruefen**

Run: `cargo test -p server script::engine::rhai`
Expected: PASS — `rhai_invariants::error_terminated_is_not_catchable` / `error_runtime_is_catchable` nutzen einen `Dynamic::from(String)`-Payload direkt (nicht `script_err_to_rhai`) und pruefen nur Catchbarkeit, kein Mapping — sie bleiben gruen.

- [ ] **Step 6: Commit (Server-Fix)**

```bash
git add server/src/script/engine/rhai.rs server/tests/script_gate_integration.rs
git commit -m "fix(script): authentifiziere Host-Fehler via Sentinel-Dynamic (server, Q0011)"
```

---

## Task 4: E2E-Outcome-Pin ueber `run_and_persist` (Audit-Row)

> Diagnose §4 Test 2 (E2E-Outcome-Pin). Harness wie der Rest von `server/tests/script_run.rs`: `fresh_test_setup().await` + `#[serial]` + `persist_script_with_caps`. Beweist, dass die Audit-Spalte `outcome` nicht gefaelscht wird und `tokens_used` unbeeinflusst bleibt (Akzeptanzkriterium 3 end-to-end).

**Files:**
- Test: `server/tests/script_run.rs` (anfuegen)

- [ ] **Step 1: E2E-Pin-Test schreiben**

Ans Ende von `server/tests/script_run.rs` anfuegen (alle Imports/Helper — `run_and_persist`, `persist_script_with_caps`, `script_audit_log`, `ScriptCtx`, `CapabilityToken`, `serial` — sind oben in der Datei vorhanden):

```rust
#[tokio::test]
#[serial]
async fn run_and_persist_does_not_report_forged_outcome() {
    // Q0011 E2E: ein Skript wirft einen gefaelschten Timeout-JSON. Die Audit-Row
    // darf NICHT "timeout" melden (erwartet "hostError") und der Token-Ledger
    // bleibt unangetastet.
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();
    let script = persist_script_with_caps(
        "r-forge",
        r#"throw "{\"kind\":\"timeout\",\"limit_ms\":1}""#,
        vec![CapabilityToken::ComputeOnly],
    )
    .await;

    let mock = Arc::new(shared::script::testing::MockHostApi::new());
    let rec = run_and_persist(&db, &script, ScriptCtx::default(), mock)
        .await
        .expect("run");

    assert_ne!(rec.outcome, "timeout", "gefaelschter Timeout darf nicht ins Outcome");
    assert_eq!(rec.outcome, "hostError", "ein Skript-throw endet als hostError");

    let rows = script_audit_log::Entity::find()
        .filter(script_audit_log::Column::ScriptId.eq("r-forge"))
        .all(&db)
        .await
        .expect("query");
    assert_eq!(rows.len(), 1);
    assert_ne!(rows[0].outcome, "timeout");
    assert_eq!(rows[0].outcome, "hostError");
    // Ledger unbeeinflusst: throw ruft keine Host-fn → leeres Token-Array.
    let tokens: serde_json::Value =
        serde_json::from_str(&rows[0].tokens_used).expect("tokens JSON");
    assert_eq!(tokens.as_array().unwrap().len(), 0);
}
```

- [ ] **Step 2: E2E-Pin laufen lassen (GREEN, dank Task 3)**

Run: `cargo test -p server --test script_run run_and_persist_does_not_report_forged_outcome`
(Windows-Lock-Fallback: `--target-dir target-test`.)
Expected: PASS — der Server-Fix aus Task 3 wirkt auch ueber `run_collecting`/`run_and_persist`, also meldet die Audit-Row `"hostError"` statt `"timeout"`.

- [ ] **Step 3: Non-Regress der echten Host-Fehler im Run-Pfad (Akzeptanzkriterium 2)**

Run: `cargo test -p server --test script_run`
Expected: PASS — insbesondere `run_and_persist_records_capability_denied_outcome` (echter Deny → `"capabilityDenied"`), `run_and_persist_records_ok_outcome_with_token_uses` (Ledger korrekt), `capability_denied_cannot_be_caught_by_try_catch` und `run_and_persist_handles_compile_failure_without_panic` bleiben gruen.

- [ ] **Step 4: Commit (E2E-Pin)**

```bash
git add server/tests/script_run.rs
git commit -m "test(script): E2E-Pin gegen gefaelschtes Audit-Outcome (Q0011)"
```

---

## Task 5: Symmetrischer Fix — Client (Option B)

> Diagnose §6 „Client-Symmetrie": der Client traegt denselben Mechanismus (`map_rhai_err:294-325`, `serde_json::from_str` Zeile 301; `script_err_to_rhai:226-239`) und ist genauso spoofbar (Preview/Compute). Der Fix muss symmetrisch erfolgen. Der Client hat KEINE `Sandbox`/`token_uses` (eigene Inline-`gate`), aber denselben Fehlertransport.

**Files:**
- Modify: `client/src/script/engine/rhai.rs` — neuer Typ + `script_err_to_rhai:226-239` + `map_rhai_err:294-325`

- [ ] **Step 1: Sentinel-Typ definieren (Client)**

In `client/src/script/engine/rhai.rs` direkt vor `fn script_err_to_rhai` (aktuell Zeile 226) einfuegen — wortgleich zum Server (Task 3 Step 1):

```rust
/// Sentinel-Transport fuer host-originierte `ScriptError`s (Q0011, symmetrisch
/// zum Server). Host-Fehler reisen als geboxter Custom-Typ im
/// `EvalAltResult`-Payload, nicht als JSON-String — ein Skript-`throw` kann
/// diesen Typ nicht erzeugen, der gemeldete `kind` ist damit nicht spoofbar.
#[derive(Clone)]
struct HostErrorPayload(ScriptError);
```

- [ ] **Step 2: `script_err_to_rhai` auf Sentinel umstellen (Client)**

`client/src/script/engine/rhai.rs:226-239` ersetzen (identisch zum Server):

```rust
fn script_err_to_rhai(e: ScriptError) -> Box<EvalAltResult> {
    let unmaskable = e.unmaskable();
    let payload = Dynamic::from(HostErrorPayload(e));
    if unmaskable {
        Box::new(EvalAltResult::ErrorTerminated(payload, Position::NONE))
    } else {
        Box::new(EvalAltResult::ErrorRuntime(payload, Position::NONE))
    }
}
```

- [ ] **Step 3: `map_rhai_err` auf `try_cast` umstellen (Client)**

`client/src/script/engine/rhai.rs:294-325` (gesamter `map_rhai_err`-Rumpf) ersetzen — identische Struktur zum Server (Task 3 Step 3), die `timeout_ms`/`memory_kb`-Arme bleiben unveraendert:

```rust
fn map_rhai_err(e: EvalAltResult, timeout_ms: u32, memory_kb: u32) -> ScriptError {
    let descr = e.to_string();
    match e {
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
```

- [ ] **Step 4: Client-Bestandstests gegenpruefen (Non-Regress)**

Run: `cargo test -p client --test script_engine`
Expected: PASS — und die Inline-`run_inputs`-Tests in `client/src/script/engine/rhai.rs` (`ctx_t_requires_read_i18n`, `ctx_t_denial_is_not_catchable`, `ctx_t_with_read_i18n_calls_host`) bleiben gruen: `ctx_t_denial_is_not_catchable` haengt nur an Unmaskable-Catchbarkeit + korrekter `CapabilityDenied`-Rekonstruktion, die der Sentinel jetzt liefert.

- [ ] **Step 5: Commit (Client-Fix)**

```bash
git add client/src/script/engine/rhai.rs
git commit -m "fix(script): authentifiziere Host-Fehler via Sentinel-Dynamic (client, Q0011)"
```

---

## Task 6: Client-Spoof-Pin (Symmetrie-Beweis)

> Diagnose §4 Test 1, auf den Client gespiegelt (der Client ist ueber Preview/Compute genauso spoofbar). Pinnt, dass der Client-Fix dieselbe Garantie traegt.

**Files:**
- Test: `client/tests/script_engine.rs` (anfuegen)

- [ ] **Step 1: Imports/Setup pruefen**

Read: `client/tests/script_engine.rs` — pruefen, welche Helper/Imports schon da sind (`RhaiEngine`, `ScriptEngine`, `ScriptInputs`, `ScriptCtx`, `MockHostApi`, `ScriptError`, `ScriptManifest`, `CapabilityToken`, `ScriptTier`). Fehlende `use`-Zeilen ergaenzen, bevor der Test geschrieben wird (kein Code raten — erst lesen).

- [ ] **Step 2: Client-Spoof-Pin schreiben**

Ans Ende von `client/tests/script_engine.rs` anfuegen (Engine-Pfad: `client::script::engine::RhaiEngine`, analog zur Server-Variante):

```rust
#[test]
fn client_forged_throw_does_not_determine_reported_error_kind() {
    // Q0011 Symmetrie: der Client-Engine-Pfad (Preview/Compute) darf einen
    // gefaelschten throw-JSON nicht als typisierten ScriptError melden.
    let manifest = shared::script::ScriptManifest {
        manifest_version: 1,
        tier: shared::script::ScriptTier::Reader,
        capabilities: vec![shared::script::CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let engine = client::script::engine::RhaiEngine::with_manifest(&manifest);
    let ast = engine
        .compile(r#"throw "{\"kind\":\"timeout\",\"limit_ms\":99999}""#, &manifest)
        .expect("compile");
    let host = std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    let res = engine.run(
        &ast,
        shared::script::engine::ScriptInputs::default(),
        host,
        shared::script::engine::ScriptCtx::default(),
    );
    assert!(
        !matches!(res, Err(shared::script::ScriptError::Timeout { .. })),
        "Client: gefaelschter throw bestimmte den kind: {res:?}"
    );
    assert!(
        matches!(res, Err(shared::script::ScriptError::HostError { .. })),
        "Client: erwartete generischen HostError, war: {res:?}"
    );
}
```

(Falls `client/tests/script_engine.rs` bereits einen passenden Helper/Importblock hat, an dessen Stil angleichen — die qualifizierten `shared::…`-Pfade oben funktionieren in jedem Fall ohne Zusatz-`use`.)

- [ ] **Step 3: Client-Pin laufen lassen (GREEN, dank Task 5)**

Run: `cargo test -p client --test script_engine client_forged_throw_does_not_determine_reported_error_kind`
Expected: PASS.

- [ ] **Step 4: Commit (Client-Pin)**

```bash
git add client/tests/script_engine.rs
git commit -m "test(script): Client-Spoof-Pin fuer Symmetrie (Q0011)"
```

---

## Task 7: Workspace-Verifikation + Symmetrie + Lint-Gate

> Diagnose §3 Schritt 4 (Gegenprobe kein Regress) + §6 (Symmetrie-Pin `script_symmetry.rs` muss gruen bleiben). Abschluss-Gate mit den Projekt-Befehlen aus CLAUDE.md.

**Files:** keine Aenderung — reine Verifikation.

- [ ] **Step 1: Voller Workspace-Testlauf**

Run: `cargo test --workspace`
(Windows: falls ein Dev-Server `server.exe` lockt — `cargo test --workspace --target-dir target-test`. `target-test/` ist gitignored.)
Expected: PASS — insbesondere:
- neue Pins: `forged_throw_does_not_determine_reported_error_kind`, `non_json_throw_still_maps_to_host_error`, `run_and_persist_does_not_report_forged_outcome`, `client_forged_throw_does_not_determine_reported_error_kind`.
- Non-Regress echte Host-Fehler (Akzeptanzkriterium 2): `script_gate_integration.rs::*`, `script_run.rs::run_and_persist_records_capability_denied_outcome` / `..._records_ok_outcome_with_token_uses`, `script_engine.rs::timeout_constraint_fires_when_deadline_exceeded` / `engine_max_operations_kicks_in_on_runaway_loop`.
- Symmetrie: `script_symmetry.rs::server_and_client_host_api_registries_are_symmetric` (unberuehrt, muss gruen bleiben).
- Wire-Format: `shared/tests/script_wire_format.rs` (unberuehrt, weil `ScriptError` nicht geaendert wurde).

- [ ] **Step 2: Lint-/Format-Gate (spiegelt die git-Hooks)**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS — kein Format-Diff, keine Clippy-Warnung. Falls Clippy `dead_code` fuer `HostErrorPayload.0` meldet (das Feld wird via Pattern `HostErrorPayload(err)` gelesen — sollte nicht passieren), das Lesen ist echt; andernfalls `#[allow(dead_code)]` NICHT setzen, sondern die Verwendung pruefen.

- [ ] **Step 3: Akzeptanzkriterien gegen die Queue-Item-Liste abhaken**

`docs/queue/Q0011-skript-audit-outcome-spoofing-haerten.md` §Akzeptanzkriterien:
1. Skript-`throw` bestimmt den `kind` nicht mehr → Task 2/3 (Engine) + Task 4 (E2E) + Task 6 (Client). ✓
2. Echte Host-Fehler weiterhin korrekt → Task 3 Step 4/5 + Task 4 Step 3 + Task 7 Step 1 (Non-Regress gruen). ✓
3. Neuer Pin-Test im `script_*`-Set → vier neue Tests (Task 2/4/6). ✓

- [ ] **Step 4: Kein eigener Commit**

Reine Verifikation; die Code-/Test-Commits sind in Tasks 2–6 erfolgt. Falls Step 2 ein `cargo fmt`-Diff erzeugte, dieses als separaten `style(script): cargo fmt`-Commit nachziehen.

---

## Self-Review

**1. Spec-Coverage (Diagnose + Queue-Item):**
- Spike fuer Option-B-Cast ohne `serde`-Feature (Diagnose §6 offene Frage) → Task 1, mit Option-A-Fallback dokumentiert (Diagnose §5). ✓
- Sentinel-`Dynamic` statt JSON-String, `try_cast` statt `from_str` (Diagnose §5 Option B) → Task 3 (Server) + Task 5 (Client). ✓
- `script_err_to_rhai` UND `map_rhai_err` auf BEIDEN Seiten (Diagnose §6 Symmetrie) → Task 3 + Task 5. ✓
- Variant-Entscheidung `HostError` vs. `ScriptThrew` (Diagnose §5 Empfehlung, Akzeptanzkriterium 1) → Header „Variant-Entscheidung" + Task 3 Step 3. ✓
- Primaerer Spoof-Pin synchron, Engine-Ebene, parametrisiert (Diagnose §4 Test 1) → Task 2. ✓
- E2E-Outcome-Pin ueber `run_and_persist` (Diagnose §4 Test 2) → Task 4. ✓
- Non-Regress echte Host-Fehler (Diagnose §4 Test 3, Akzeptanzkriterium 2) → Task 3/4/7. ✓
- Kontroll-Assertion Nicht-JSON-Wurf (Diagnose §4 Test 4) → Task 2 `non_json_throw_still_maps_to_host_error`. ✓
- TDD-Reihenfolge: Pin zuerst RED (Task 2), dann Fix bis GREEN (Task 3), dann E2E + Symmetrie + Gate. ✓
- Verifikations-Befehle aus CLAUDE.md (`cargo test --workspace` / `--target-dir target-test` Windows-Lock; `cargo fmt --check && cargo clippy …`) → Task 7. ✓

**2. Placeholder-Scan:** keine TBD/TODO/„appropriate"-Platzhalter; jeder Code-Schritt zeigt vollstaendigen Code, jeder Run-Schritt zeigt Befehl + erwartetes Ergebnis.

**3. Typ-Konsistenz:** `HostErrorPayload(ScriptError)` identisch in Task 3 Step 1 (Server) und Task 5 Step 1 (Client); `try_cast::<HostErrorPayload>()` und das `Some(HostErrorPayload(err))`-Pattern konsistent in Task 3 Step 3 und Task 5 Step 3; Test-Namen (`forged_throw_does_not_determine_reported_error_kind`, `non_json_throw_still_maps_to_host_error`, `run_and_persist_does_not_report_forged_outcome`, `client_forged_throw_does_not_determine_reported_error_kind`) durchgehend identisch referenziert (Task 2/4/6/7).
