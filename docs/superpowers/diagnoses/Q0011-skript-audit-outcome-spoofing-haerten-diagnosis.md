# Q0011 — Diagnose: Skript-Audit-Outcome-Spoofing via Rhai-`throw`

- Item: `docs/queue/Q0011-skript-audit-outcome-spoofing-haerten.md`
- Typ: bug | Prioritaet: medium | Quelle: review | `security_review.required: true`
- Eltern-Item: Q0009 (`docs/reviews/Q0009-review.md` §Security-Re-Review)
- Datum: 2026-05-29
- Methode: superpowers:systematic-debugging — Code gelesen und Kern-Annahme
  (Rhai-`throw`-Semantik) **empirisch gegen das gepinnte `rhai 1.24.0`**
  (Cargo.lock:5891) verifiziert, nicht nur dem Item vertraut.

> **VERDIKT: BUG BESTAETIGT** — der beschriebene Spoof reproduziert exakt.
> Korrektur ggue. Item: die im Item genannten Zeilennummern stimmen nicht mehr
> (Item: `map_rhai_err:365-398`; tatsaechlich `map_rhai_err:401-434`, der
> betroffene Match-Arm `406-416`). Sachverhalt und Wirkungs-Grenze des Items
> sind ansonsten korrekt. Eine zusaetzliche Nuance (Spoofing greift auf der
> **serde-Ebene**, nicht auf der Rhai-Varianten-Ebene) ist unten praezisiert.

---

## 1. Reproducer

### Validierte Mechanik (Kette aus echtem Code)

1. Ein Skript fuehrt `throw "<json>"` aus, z.B.
   `throw "{\"kind\":\"timeout\",\"limit_ms\":99999}"`.
2. Rhai erzeugt daraus `EvalAltResult::ErrorRuntime(payload, _)`, wobei
   `payload` ein `Dynamic::from(String)` mit exakt dem geworfenen String ist.
   **Empirisch bestaetigt** gegen `rhai 1.24.0`:
   - `throw "<string>"` -> `ErrorRuntime`, `payload.is::<String>() == true`,
     `payload.into_string() == Ok("<string>")` (Byte-gleich zum Wurf).
   - `throw #{ ... }` (Map) -> `ErrorRuntime`, aber `payload.into_string()`
     schlaegt fehl (kein String). Relevant fuer Fix-Option B (s.u.).
   - `throw` liefert **immer** `ErrorRuntime`, **nie** `ErrorTerminated`.
3. `run_collecting` faengt das `Err(e)` und ruft
   `map_rhai_err(*e, timeout_ms, memory_kb)`
   (`server/src/script/engine/rhai.rs:203`).
4. `map_rhai_err` matcht den zusammengefassten Arm
   `ErrorTerminated(payload, _) | ErrorRuntime(payload, _)`
   (`server/src/script/engine/rhai.rs:406`), zieht den String
   (`payload.into_string()`, Zeile 407) und ruft
   `serde_json::from_str::<ScriptError>(&s)` (Zeile 408).
5. `ScriptError` ist `#[serde(tag = "kind", rename_all = "camelCase")]`
   (`shared/src/script/error.rs:21-22`). `{"kind":"timeout","limit_ms":99999}`
   deserialisiert daher erfolgreich zu `ScriptError::Timeout { limit_ms: 99999 }`
   und wird `return`'t (Zeile 409) — der gefaelschte Fehler ist das Run-Ergebnis.
6. `run_and_persist`/`run_preview` mappen diesen `ScriptError` ueber
   `outcome_tag` (`server/src/script/run.rs:33-51`) auf die
   `script_audit_log.outcome`-Spalte bzw. das Preview-`error`-Feld
   (`run.rs:140-148`, `run.rs:203-211`). `Timeout` -> `outcome = "timeout"`.

Ergebnis: Die Audit-Zeile bzw. das Preview-Ergebnis meldet `timeout`, obwohl
weder ein Operation-Limit noch ein Deadline ausgeloest hat — ein **vom Skript
frei gewaehlter** `ScriptError`-`kind`.

### Konkreter Reproducer (TDD-tauglich, synchron)

Analog zu `server/tests/script_gate_integration.rs` (Engine direkt, ohne DB):

```rust
let manifest = ScriptManifest { tier: Reader,
    capabilities: vec![CapabilityToken::ComputeOnly], ..Default::default() };
let engine = RhaiEngine::with_manifest(&manifest);
let ast = engine
    .compile(r#"throw "{\"kind\":\"timeout\",\"limit_ms\":99999}""#, &manifest)
    .unwrap();
let host = Arc::new(MockHostApi::new());
let res = engine.run(&ast, ScriptInputs::default(), host, ScriptCtx::default());
// HEUTE (Bug):  res == Err(ScriptError::Timeout { limit_ms: 99999 })   <-- gespooft
// SOLL (Fix):   res == Err(ScriptError::HostError { .. })  (generisch, kind NICHT timeout)
```

Annahmen explizit: `throw` ist Rhai-Core-Syntax und braucht keines der fuenf
geladenen Packages (bestaetigt — Probe lief auf `Engine::new_raw()` ohne
Packages). Der Run-Pfad braucht hier **kein** Capability-Token, weil `throw`
keine Host-fn ist; die Gate ist fuer den Spoof irrelevant.

### End-to-End-Variante (mit Audit-Row)

Ueber `run_and_persist` (vgl. `server/tests/script_run.rs`): ein gespeichertes
Skript mit Source `throw "{\"kind\":\"timeout\",\"limit_ms\":1}"` erzeugt eine
Audit-Row mit `outcome == "timeout"` statt `"hostError"`.

---

## 2. Root-Cause-Hypothese

**Ein-Satz:** `map_rhai_err` rekonstruiert den Run-`ScriptError`, indem es den
String-Payload eines `ErrorRuntime` per `serde_json::from_str::<ScriptError>`
deserialisiert — und dieser Payload ist nicht vom Host authentifiziert, weil
Rhais `throw <string>` denselben Payload-Kanal (String im `ErrorRuntime`)
benutzt wie der Host-Fehler-Transport in `script_err_to_rhai`.

Konkrete Belege (`<file>:<line>`):

- **Spoof-Senke:** `server/src/script/engine/rhai.rs:406-409`
  ```rust
  EvalAltResult::ErrorTerminated(payload, _) | EvalAltResult::ErrorRuntime(payload, _) => {
      if let Ok(s) = payload.into_string() {
          if let Ok(err) = serde_json::from_str::<ScriptError>(&s) {
              return err;            // <-- vertraut dem Skript-erzeugbaren Payload
  ```
- **Spoof-Quelle (legitimer Gegenpart):** `script_err_to_rhai`
  `server/src/script/engine/rhai.rs:291-304` — Host-Fehler werden als
  `serde_json::to_string(&e)` in genau dieses `ErrorRuntime`/`ErrorTerminated`
  gelegt. Der Transport-Kanal ist also **nicht vom Skript unterscheidbar**.
- **Kollabierender Match-Arm:** dass `ErrorTerminated` und `ErrorRuntime`
  denselben Arm teilen (`rhai.rs:406`), heisst: auch wenn `throw` nur
  `ErrorRuntime` erzeugt, wird der deserialisierte Wert **nicht** danach
  gefiltert, ob er ein maskable/unmaskable kind ist. Der gefaelschte JSON darf
  `"kind":"capabilityDenied"` oder `"timeout"` behaupten und landet als solcher
  im Outcome. **Das Spoofing greift auf der serde-Ebene, nicht auf der
  Rhai-Varianten-Ebene** — Korrektur/Praezisierung der Item-Formulierung.
- **Wirkungs-Senke:** `outcome_tag` (`server/src/script/run.rs:33-51`) mappt
  den `kind` direkt auf die Outcome-Spalte; es gibt keine Plausibilisierung
  ("war ueberhaupt ein echter Timeout/Deny im Sandbox-Ledger?").

**Warum es nur Audit-Fidelity ist (validiert, deckt sich mit Item & Review):**

- Echte Denies sind `ErrorTerminated` und per `try`/`catch` uncatchbar — gepinnt
  durch `rhai_invariants::error_terminated_is_not_catchable`
  (`server/src/script/engine/rhai.rs:516-537`) und end-to-end
  `script_gate_integration.rs::capability_denied_is_not_catchable_via_try_catch`.
  Nach einem echten Deny erlangt das Skript nie wieder Kontrolle, kann also den
  echten Fehler nicht ueberschreiben (nur einen *eigenen* Wurf faerben).
- Der `token_uses`-Ledger kommt aus der echten `Sandbox`
  (`run_collecting`: `state.lock().…sandbox.token_uses()`, `rhai.rs:196-199`),
  voellig unabhaengig vom geworfenen Payload. Ein gespooftes `outcome` loescht
  keine echte `Ok`/`Denied`-Token-Nutzung in `tokens_used`.
- Kein Codepfad gewaehrt einen **Seiteneffekt** anhand des zurueckgemappten
  `ScriptError` — `RunRecord.error`/`PreviewRecord.error` sind reines Reporting.

---

## 3. Beweis-Strategie

So wird die Hypothese bestaetigt bzw. widerlegt:

1. **Repro-Test (bestaetigt heute den Bug):** der unter §1 gezeigte Engine-Test.
   Heutiges Verhalten: `Err(ScriptError::Timeout{limit_ms:99999})`. Laeuft der
   Test rot gegen den geforderten Soll-Zustand (`HostError`), ist der Bug
   bewiesen.
2. **Empirie zur Rhai-Annahme (bereits durchgefuehrt):** isoliertes
   `rhai 1.24.0`-Probe-Programm — `throw "<json>"` -> `ErrorRuntime` mit
   String-Payload (byte-gleich); `throw #{…}` -> `ErrorRuntime` ohne
   String-Payload. Damit ist die einzige nicht-offensichtliche Annahme der
   Reproduktionskette extern verifiziert.
3. **serde-Ebene:** `ScriptError` traegt `#[serde(tag="kind",
   rename_all="camelCase")]` (`shared/src/script/error.rs:21-22`) — die
   camelCase-`kind`-Strings (`timeout`, `memoryExceeded`, `capabilityDenied`)
   sind exakt die, die `outcome_tag` ausgibt. Ein `roundtrip`-Assert
   (`from_str::<ScriptError>("{\"kind\":\"timeout\",\"limit_ms\":1}") ==
   Timeout{limit_ms:1}`) belegt die Deserialisierbarkeit.
4. **Gegenprobe (kein Regress):** Bestands-Tests
   `script_gate_integration.rs` (echter Deny -> `CapabilityDenied`),
   `script_run.rs::run_and_persist_records_capability_denied_outcome` und
   `…_records_ok_outcome_with_token_uses` muessen nach einem Fix **gruen
   bleiben** — sie beweisen, dass echte Host-Fehler weiterhin korrekt gemeldet
   werden (Akzeptanzkriterium 2).

---

## 4. Test-Strategy (TDD-ready fuer die spaetere Implementierung)

Empfohlene Datei: **`server/tests/script_run.rs`** (E2E ueber `run_and_persist`,
gleiche Harness-Konvention: `fresh_test_setup().await` + `#[serial]` +
`save_script`/direkter Seed) **plus** ein schneller synchroner Engine-Test in
`server/tests/script_gate_integration.rs` (kein DB-Setup, kein `#[serial]`).

Neue Tests (Assertion-Form):

1. **Spoof-Pin (Kern, Akzeptanzkriterium 1+3):**
   `forged_throw_does_not_determine_reported_error_kind`
   - Source: `throw "{\"kind\":\"timeout\",\"limit_ms\":99999}"`.
   - Assertion: `!matches!(res, Err(ScriptError::Timeout { .. }))` und
     positiv `matches!(res, Err(ScriptError::HostError { .. }))` (bzw. die im
     Fix gewaehlte generische Variante, z.B. ein neues `ScriptThrew`).
   - Parametrisierte Varianten fuer `memoryExceeded`, `capabilityDenied`,
     `serverOnlyFunction` — keiner darf den gemeldeten `kind` bestimmen.
2. **E2E-Outcome-Pin (Audit-Row):** in `script_run.rs`,
   `run_and_persist_does_not_report_forged_outcome` — Skript wirft den
   Timeout-JSON, Assertion `rec.outcome != "timeout"` (erwartet `"hostError"`),
   und `tokens_used` bleibt korrekt (Ledger unbeeinflusst).
3. **Nicht-Regress fuer echte Host-Fehler (Akzeptanzkriterium 2):** bereits
   abgedeckt durch `script_gate_integration.rs::db_call_without_token_is_denied_at_real_execution`
   und `script_run.rs::run_and_persist_records_capability_denied_outcome`; ggf.
   einen expliziten Timeout-Test ergaenzen (Manifest `timeout_ms` klein +
   Endlosschleife -> echter `Timeout`, vgl.
   `script_engine.rs::engine_max_operations_kicks_in_on_runaway_loop`).
4. **Klartext-Wurf weiterhin als HostError:** `throw "irgendwas"` (kein
   gueltiges JSON) muss schon heute `HostError{source:"irgendwas"}` ergeben
   (`rhai.rs:411`) — als Kontroll-Assertion mitnehmen, damit der Fix den
   Nicht-JSON-Pfad nicht bricht.

Hinweis: TDD-Reihenfolge — Test 1 zuerst rot schreiben (er ist heute rot gegen
das Soll), dann fixen, bis 1+2 gruen und 3+4 unveraendert gruen.

---

## 5. Moegliche Fixes

### Option A — echten Host-Fehler out-of-band im `RunState` ablegen (EMPFOHLEN)

`RunState` (`server/src/script/engine/rhai.rs:145-148`) um
`last_host_error: Option<ScriptError>` erweitern. In `script_err_to_rhai` (bzw.
direkt in den Host-fn-Wrappern `gated_db_fetch:281-284`, `t`-fn `252-255`) den
echten `ScriptError` zusaetzlich in `state.lock().last_host_error` ablegen,
bevor der `EvalAltResult` propagiert wird. In `run_collecting` nach dem eval:

```rust
let mapped = match res {
    Ok(v)  => Ok(rhai_to_script_value(v)),
    Err(e) => {
        // 1) echten Host-Fehler bevorzugen (vom Host gesetzt, nicht vom Skript erzeugbar)
        if let Some(host_err) = state.lock().ok().and_then(|s| s.last_host_error.clone()) {
            Err(host_err)
        } else {
            // 2) alles andere (inkl. throw) generisch mappen — KEINE serde-Deser. mehr
            Err(map_rhai_err_generic(*e, timeout_ms, memory_kb))
        }
    }
};
```

`map_rhai_err` verliert den `serde_json::from_str::<ScriptError>`-Zweig; der
`ErrorRuntime`/`ErrorTerminated`-Arm wird zu
`ScriptError::HostError { source: s }` (oder einem neuen `ScriptThrew`).

- **+** Schliesst die Luecke an der Wurzel: der Outcome stammt aus einer
  Quelle, die das Skript nicht beschreiben kann.
- **+** `throw` landet sauber als generischer `HostError`.
- **+** Engine-Limit-Fehler (`ErrorTooManyOperations`, `ErrorDataTooLarge`)
  bleiben unveraendert (kommen aus eigenen Match-Armen, nicht aus `throw`).
- **Achtung Reihenfolge/Mehrfach-Calls:** `last_host_error` muss die *zum
  abbrechenden Fehler gehoerende* Quelle sein. Da ein maskable `ErrorRuntime`
  vom Skript per `catch` geschluckt werden kann, koennte ein *frueheres*,
  gefangenes Host-`last_host_error` einen *spaeteren* `throw` ueberschreiben.
  Sauber: `last_host_error` nur als Wahrheit nehmen, wenn der **propagierte**
  `EvalAltResult` tatsaechlich der Host-Fehler ist. Robuster Weg → Kombination
  mit Option B (Sentinel) zur Identitaet, A zum Transport.

### Option B — Sentinel-Dynamic statt JSON-String

Host-Payloads als nicht-serde-darstellbaren Custom-Typ transportieren, z.B.
`Dynamic::from(HostErrorPayload(ScriptError))` mit
`register_type::<HostErrorPayload>()` (oder direkt `Dynamic::from(ScriptError)`,
da `Dynamic` beliebige `Clone + 'static`-Typen boxed). In `map_rhai_err` dann
`payload.try_cast::<HostErrorPayload>()` statt `into_string()` +
`from_str`. Ein Skript-`throw` produziert nur primitive/Map-Dynamics, **nie**
diesen Typ — empirisch gestuetzt: `throw #{…}` liefert bereits einen Payload,
dessen `into_string()` fehlschlaegt; ein Custom-Typ ist erst recht
unerzeugbar aus Skript-Syntax.

- **+** Authentifiziert den Payload ueber den Rust-Typ statt ueber Vertrauen.
- **+** Kein zusaetzliches `RunState`-Feld, keine Reihenfolge-Subtilitaeten.
- **-** `Dynamic` mit Nicht-`String`-Custom-Typ: pruefen, ob die geladenen
  Packages / `try_cast` das ohne `serde`-Feature sauber unterstuetzen
  (das `rhai`-`serde`-Feature ist bewusst AUS — `Dynamic::from`/`try_cast`
  fuer Custom-Typen brauchen es aber **nicht**, nur das `internals`/Default).
- **-** Beruehrt `script_err_to_rhai` (Server **und** Client) sowie beide
  `map_rhai_err` — etwas groesserer Diff, aber lokal.

### Empfehlung

**Option B als primaerer Mechanismus** (Authentizitaet ueber den Typ, kein
Reihenfolge-Problem, symmetrisch fuer Server+Client), optional gestuetzt durch
A. Begruendung: B adressiert direkt das Root-Cause (Payload-Kanal ist
skript-erzeugbar), waehrend A einen Seitenkanal anlegt, dessen Korrektheit von
der genauen Fehler-Reihenfolge abhaengt (catch-und-spaeter-throw). Falls B's
`Dynamic`-Custom-Cast im `no-serde`-Setup unerwartet zickt, ist A der sichere
Fallback. In beiden Faellen: der `ErrorRuntime`/`ErrorTerminated`-Arm darf
**keinen** vom Skript steuerbaren `ScriptError`-`kind` mehr zurueckgeben — ein
nicht erkannter Wurf endet als generischer `HostError` (oder neuer
`ScriptThrew`).

---

## 6. Risiko-Einschaetzung

- **Regress-Wahrscheinlichkeit: niedrig–mittel.** Der Aenderungs-Kern ist eine
  einzelne Funktion (`map_rhai_err`) plus ggf. `script_err_to_rhai`/`RunState`.
  Die Bestands-Tests pinnen die kritischen Pfade scharf: echte Denies
  (`script_gate_integration.rs`, `script_run.rs`), Timeout/Operations
  (`script_engine.rs`), Symmetrie (`script_symmetry.rs`). Solange diese gruen
  bleiben, ist das Verhalten bei *echten* Host-Fehlern unveraendert.
- **Betroffene Codeareale:**
  - `server/src/script/engine/rhai.rs` — `map_rhai_err:401-434`,
    `script_err_to_rhai:291-304`, `RunState:145-148`, `run_collecting:165-206`.
  - **Client-Symmetrie:** `client/src/script/engine/rhai.rs` traegt den
    **identischen** Mechanismus — `map_rhai_err:294-325` mit demselben
    kollabierten Arm (`299-305`) und `serde_json::from_str::<ScriptError>`
    (Zeile 301), und `script_err_to_rhai:226-239`. Der Client ist genauso
    spoofbar (Preview/Compute), daher sollte der Fix **symmetrisch** erfolgen.
    Der Symmetrie-Test `script_symmetry.rs` und `client/.../run_inputs::*`
    sind die Pins. Hinweis: der Client hat im Item bereits den Minor-Vermerk
    `Timeout{limit_ms:0}` — das ist davon unabhaengig.
- **Reine Reporting-Aenderung:** kein Seiteneffekt-Pfad haengt am
  zurueckgemappten `ScriptError`; der Fix kann den gemeldeten `kind` aendern,
  ohne Capability-/DB-/Token-Verhalten zu beruehren. `token_uses`/`tokens_used`
  sind unabhaengig (`run_collecting:196-199`) und bleiben unangetastet.
- **Beobachtbares Verhaltens-Delta nach Fix:** ein Skript, das heute z.B.
  `throw "{\"kind\":\"timeout\",…}"` wirft und `outcome="timeout"` erzeugte,
  meldet danach `outcome="hostError"` (bzw. `scriptThrew`). Das ist die
  *gewollte* Aenderung (Akzeptanzkriterium 1) und betrifft nur die
  Audit-Fidelity boesartiger/fehlerhafter Skripte — kein legitimer Pfad
  verlaesst sich darauf.
- **Offene Detailfrage fuer die Implementierung (kein Blocker fuer die
  Diagnose):** ob bei Option B `Dynamic::try_cast::<ScriptError>()` ohne das
  `rhai/serde`-Feature funktioniert (erwartet: ja, da reines Custom-Type-Boxing)
  — vor Festlegung in der Plan-/Execute-Phase mit einem 3-Zeilen-Probe
  bestaetigen.
