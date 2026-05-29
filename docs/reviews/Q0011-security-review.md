# Q0011 — Security-Review (Script-Sandbox-Haertung)

- Datum: 2026-05-29
- Reviewer: claude (`security-review`-Skill, Security-Verdikt-Pass — separat vom
  Korrektheits-Review in `docs/reviews/Q0011-review.md`)
- Scope: Q0011-Implementierungs-Diff `064846e..13064f0` (5 Dateien, +191/-46):
  `server/src/script/engine/rhai.rs`, `client/src/script/engine/rhai.rs` +
  drei Test-Dateien.
- Eltern: Q0009-Security-Re-Review (Audit-Fidelity-Spoofing, ausgegliedert als
  Q0011). Threat-Model: nicht-vertrauenswuerdige, user-authored Rhai-Skripte in
  einer capability-gated Sandbox.
- Akzeptierter Scope: **Audit-Fidelity-Haertung** — kein Sandbox-Escape, kein
  Capability-Bypass (so von Q0009 klassifiziert).

> **VERDIKT: CLEARED** — der Spoof-Vektor (skript-frei waehlbarer
> `ScriptError`-`kind` via `throw "<json>"`) ist an der Wurzel geschlossen.
> Keine geschwaechte Sandbox-Garantie, kein Information-Disclosure-Regress,
> keine neue skript-erreichbare Angriffsflaeche. Eine Rhai-Semantik-Nuance
> (catch+rethrow eines *echten* maskable Host-Fehlers) wurde empirisch geprueft
> und ist **keine** Faelschung — siehe A1 (advisory).

---

## 1. Schliesst der Fix den Spoofing-Vektor vollstaendig? (Fokus 1) — JA

Der Mechanismus: Host-Fehler reisen jetzt als geboxter Custom-Typ
`Dynamic::from(HostErrorPayload(ScriptError))` im `EvalAltResult`-Payload und
werden in `map_rhai_err` ausschliesslich per `payload.try_cast::<HostErrorPayload>()`
rekonstruiert. Der frueher vom Skript steuerbare Pfad
(`payload.into_string()` + `serde_json::from_str::<ScriptError>`) ist entfernt.
Ein nicht erkannter Wurf faellt in den `None`-Zweig → generischer
`ScriptError::HostError { source: descr }`.

**Empirisch verifiziert gegen `rhai 1.24.0`** (exakte Pin aus Cargo.lock,
Feature-Set `std,sync,internals`, *ohne* `serde` — identisch zu beiden Crates).
Wegwerf-Probe mit demselben Boxing-Shape wie der Fix:

- `throw "{\"kind\":\"timeout\"}"` → Payload-Typ `string`,
  `try_cast::<HostErrorPayload>() == false` → generischer `HostError`.
- `throw #{ kind: "timeout" }` (Map) → Payload-Typ `map`, `try_cast == false`.
- `throw 42` (Int) → Payload-Typ `i64`, `try_cast == false`.
- `throw [1,2,3]` (Array) → Payload-Typ `array`, `try_cast == false`.

Damit ist die im Fokus genannte Liste (Nicht-String-Dynamics: Maps, Arrays,
Ints) abgedeckt: **keiner** dieser skript-erzeugbaren Werte besteht den Cast.
Ein Skript kann den Custom-Typ syntaktisch nicht benennen oder konstruieren
(`HostErrorPayload` ist nicht als Rhai-Typ registriert, siehe §6), also gibt es
keinen Wurf, der „zufaellig round-trippt". Der gemeldete `kind` stammt
ausschliesslich aus einem vom Host gesetzten Rust-Wert.

Die vier kritischen kinds (`timeout`, `memoryExceeded`, `capabilityDenied`,
`serverOnlyFunction`) sind im gepinnten Test
`forged_throw_does_not_determine_reported_error_kind` parametrisiert
abgesichert; der Client-Symmetrie-Pin und der E2E-Audit-Pin ebenso.

### Nested/catch-then-rethrow (explizit geprueft)

- **Verschachtelter Wurf / Wurf-nach-Catch eines forged Werts:**
  `try { softfail() } catch(e) { throw "{\"kind\":\"timeout\"}" }` → der
  geworfene String ist weiterhin `string`, `try_cast == false` → `HostError`.
  Ein Catch „upgraded" einen forged Wurf nicht zu typisiert.
- **Round-trip-Wert:** existiert nicht, weil der einzige Cast-fester Typ der
  private, nicht-registrierte Custom-Typ ist.

Restpfad-Befund: **kein** skript-steuerbarer Wert erreicht einen typisierten
`ScriptError`-`kind` im Run-Outcome. Vektor geschlossen.

## 2. Hat der Fix eine bestehende Garantie geschwaecht? (Fokus 2) — NEIN

Die Wirkungs-Grenze aus der Diagnose haengt an drei Saeulen — alle bleiben:

- **Echte Denies bleiben uncatchbar.** Unmaskable Fehler (`CapabilityDenied`,
  `Timeout`, `MemoryExceeded`, `UiPrimitiveDenied`, via `ScriptError::unmaskable()`)
  werden weiterhin als `EvalAltResult::ErrorTerminated` propagiert. Empirisch
  bestaetigt: ein `ErrorTerminated(HostErrorPayload)` ist per `try`/`catch`
  **nicht** fangbar — der `catch`-Block laeuft nicht, die `eval` bricht mit dem
  unveraenderten `ErrorTerminated` (Payload immer noch `HostErrorPayload`) ab.
  Der Fix tauscht nur den *Inhalt* des Payloads (Custom-Typ statt JSON-String),
  nicht die *Variante* (`ErrorTerminated` vs `ErrorRuntime`) — die
  maskable/unmaskable-Entscheidung bleibt `e.unmaskable()`, korrekt VOR dem Move
  gelesen. Die Inline-Invarianten `error_terminated_is_not_catchable` /
  `error_runtime_is_catchable` haengen an direktem `Dynamic::from(String)` und
  sind vom Transport-Wechsel unberuehrt (bleiben gruen).
- **Token-Ledger unabhaengig.** `run_collecting` liest `token_uses` aus
  `state.lock().sandbox.token_uses()` — voellig getrennt vom Error-Payload und
  von `map_rhai_err`. Der `Denied`/`Ok`-Push passiert in `Sandbox::gate`
  (`sandbox.rs:57-83`), nicht ueber den Fehlerkanal. Ein gespooftes/gefaerbtes
  Outcome kann eine echte Token-Nutzung nicht aus dem Ledger loeschen.
- **Kein Seiteneffekt am remappten Fehler.** `RunRecord.error` /
  `PreviewRecord.error` sind reines Reporting; kein Capability-/DB-/Token-Pfad
  keyt auf den zurueckgemappten `ScriptError`.

Host-Fehler-Konstruktion laeuft **nicht** ueber einen skript-erreichbaren Pfad:
`script_err_to_rhai` ist privat und wird nur von den Host-fn-Wrappern (`t`,
`gated_db_fetch`) aufgerufen. Der neue `HostErrorPayload` leakt keine
Capability und ist aus Skript-Raum weder konstruier- noch beobachtbar (§6).

## 3. Audit-Log-Integritaet (Fokus 3) — OK

Kann ein Skript `outcome`/`tokens_used` so beeinflussen, dass boesartige
Aktivitaet verborgen wird? Nein:

- `outcome` stammt aus `outcome_tag(&ScriptError)` (`run.rs:33-51`), gespeist vom
  authentifizierten Rust-Wert. Ein forged Wurf landet als `hostError` — der E2E-Pin
  `run_and_persist_does_not_report_forged_outcome` assertiert `outcome != "timeout"`
  und `== "hostError"`.
- `tokens_used` ist payload-unabhaengig (§2). Der E2E-Pin assertiert zusaetzlich
  ein leeres Token-Array fuer den reinen `throw` (keine Host-fn aufgerufen).

Ein Angreifer kann also weder einen echten Deny/Timeout aus dem Ledger tilgen
noch ein „sauberes" Outcome ueber eine echte Capability-Nutzung legen.

## 4. Information-Disclosure (Fokus 4) — KEIN REGRESS

Der neue Fallback `HostError { source: descr }` setzt `descr = e.to_string()`
(Rhai-`EvalAltResult`-Formatierung). Fuer einen forged Wurf ist das z.B.
`"Runtime error: {\"kind\":\"timeout\"} (line 1, position 1)"` — also der
**skript-eigene** Wurf-String plus Position, keine Host-Internals, keine
Dateipfade, keine Secrets, keine Capability-Internals. Der alte Pfad legte den
rohen Skript-String (`source: s`) ab; der neue legt die Rhai-Beschreibung ab —
beide sind skript-abgeleitet. Kein neuer Leak-Kanal. (Echte Host-Fehler reisen
weiter im `HostErrorPayload` und werden 1:1 zurueckgecastet, nicht ueber
`descr` — deren `source` ist dasselbe wie vor dem Fix.)

## 5. Symmetrie als Security-Property (Fokus 5) — OK

Server und Client tragen den **identischen** Mechanismus: gleicher privater
`struct HostErrorPayload(ScriptError)`, gleiche `script_err_to_rhai`-Konstruktion
via `Dynamic::from`, gleiche `map_rhai_err`-Rekonstruktion via `try_cast`. Kein
verbleibender `serde_json::from_str::<ScriptError>`-Pfad auf einer Seite
(verifiziert per Grep ueber beide Crates). Der Client-Gate (`RunState::gate`,
`client/.../rhai.rs:176-187`) liefert ebenfalls `CapabilityDenied` (unmaskable →
`ErrorTerminated` → uncatchbar). Der Client (Preview/Compute) ist damit gleich
gehaertet; der Symmetrie-Pin `client_forged_throw_does_not_determine_reported_error_kind`
deckt den Client-Pfad eigenstaendig ab. Keine ausnutzbare Divergenz.

## 6. Neue Angriffsflaeche durch den Custom-Typ (Fokus 6) — KEINE

`HostErrorPayload` wird **nicht** via `register_type[_with_name]` registriert
(nur `HostBridge` ist registriert). Folgen:

- Kein skript-aufrufbarer Konstruktor, keine Methode, kein Feld auf dem Typ.
- Ein Skript kann den Typ syntaktisch nicht benennen → kann ihn nicht erzeugen
  und keine `try_cast`/Downcast-Operation darauf ausloesen.
- Der einzige `try_cast::<HostErrorPayload>()` lebt host-seitig in `map_rhai_err`
  und ist total (`Option`), kein Panic-Risiko.
- Boxing eines `Clone + 'static`-Typs in `Dynamic` braucht das `rhai/serde`-Feature
  nicht (bestaetigt: Build/Tests gruen ohne `serde`-Feature).

Der Typ ist ein reiner, opaker Transport. Keine neue script-callable Surface.

---

## Verifikation

- Wegwerf-Probe gegen `rhai 1.24.0` (Pin + `std,sync,internals`, kein `serde`):
  Probes A–E (catch-rethrow eines echten Payloads, `type_of(e)` im catch,
  forged String/Map/Int/Array, `ErrorTerminated`-Uncatchbarkeit, forged-nach-catch).
  Ergebnisse wie oben zitiert; Probe danach entfernt (kein Artefakt im Tree).
- Gepinnte Tests gruen: `server/tests/script_gate_integration.rs` (5),
  `server/tests/script_run.rs` (5) — inkl. der vier neuen Q0011-Pins —
  via `--target-dir target-test` (Windows-Lock, gemaess CLAUDE.md).

## Advisory (nicht-blockierend)

- **A1 (Rhai-Catch-Semantik, geprueft, KEIN Defekt):** Ein *maskable* Host-Fehler
  (`ErrorRuntime(HostErrorPayload)`, z.B. `HostError`/`InternalPanic`/
  `ServerOnlyFunction`/`ValidationFailed`) ist per `catch(e)` fangbar; `e` wird
  an den **echten** `HostErrorPayload`-`Dynamic` gebunden (`type_of(e) ==
  "...::HostErrorPayload"`), und `throw e` propagiert ihn so, dass `try_cast`
  *erfolgreich* bleibt. Das ist **keine Faelschung**: das Skript kann nur einen
  kind re-melden, den der Host fuer **diesen** Run tatsaechlich erzeugt hat — es
  kann keinen `timeout`/`capabilityDenied` erfinden (die sind unmaskable →
  `ErrorTerminated` → uncatchbar, Probe D). Re-Reporting eines echten
  `HostError` als `HostError` ist audit-neutral. Zudem ist dieses Verhalten
  **kein Regress**: der alte JSON-String-Pfad reproduzierte denselben kind beim
  catch+rethrow ebenso. Kein Token-Ledger-Effekt. → reine Anmerkung.
- **A2 (Audit-Granularitaet):** Ein forged `throw` ist im Audit nicht von einem
  echten internen `HostError` unterscheidbar (bewusste Plan-Entscheidung:
  `HostError` statt neuer `ScriptThrew`-Variante). Liegt im akzeptierten Scope
  (Audit-Fidelity-Haertung); eine spaetere `ScriptThrew`-Variante waere der
  minimale Trennschritt, ist aber NICHT blocking.
- **A3 (stale doc-comment, server):** Der Doc-Kommentar von `register_host_fns`
  (`server/.../rhai.rs` ~Z.209-213) sagt noch „beide tragen den serialisierten
  `ScriptError` im Payload". Nach dem Fix ist es der `HostErrorPayload`-Custom-Typ.
  Rein dokumentarisch, kein Verhaltens-/Security-Bug.

## Fazit

Alle sechs Security-Fokusbereiche sind sauber. Der Forge-Vektor (skript-frei
gewaehlter `kind`) ist an der Wurzel geschlossen, ohne eine Sandbox- oder
Audit-Garantie zu schwaechen und ohne neue skript-erreichbare Flaeche. Die
einzige Rhai-Nuance (A1) wurde empirisch entkraeftet. **CLEARED.**
