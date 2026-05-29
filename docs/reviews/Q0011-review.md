# Q0011 Skript-Audit-Outcome-Spoofing haerten — Code-Review

Date: 2026-05-29
Reviewer: claude (code-review:code-review Skill, Korrektheits-/Qualitaets-Fokus)
Scope: Q0011-Implementierungs-Diff `064846e..13064f0` (5 Dateien, +191/-46), gegen
Diagnose (`docs/superpowers/diagnoses/Q0011-…-diagnosis.md`), Plan
(`docs/superpowers/plans/Q0011-…-haerten.md`) und Queue-Item-Akzeptanzkriterien
(`docs/queue/Q0011-skript-audit-outcome-spoofing-haerten.md`).
Hinweis: Der dedizierte `security_review.required`-Pass (`ccm-security-review`)
folgt separat — dieser Review deckt Korrektheit/Qualitaet ab, nicht den formalen
Security-Verdikt.

Verdict: **APPROVE**

## Geprueftes

Geaenderte Produktion:
- `server/src/script/engine/rhai.rs` — neuer Sentinel-Typ `HostErrorPayload(ScriptError)`,
  `script_err_to_rhai` (Payload-Konstruktion via `Dynamic::from`), `map_rhai_err`
  (Rekonstruktion via `try_cast`, JSON-Deser entfernt).
- `client/src/script/engine/rhai.rs` — wortgleicher Sentinel-Typ + identische
  Umstellung von `script_err_to_rhai` und `map_rhai_err`.

Geaenderte Tests:
- `server/tests/script_gate_integration.rs` — `forged_throw_does_not_determine_reported_error_kind`
  (parametrisiert ueber timeout/memoryExceeded/capabilityDenied/serverOnlyFunction)
  + Kontroll-Pin `non_json_throw_still_maps_to_host_error`.
- `server/tests/script_run.rs` — E2E `run_and_persist_does_not_report_forged_outcome`
  (Audit-Row `outcome != "timeout"`, `== "hostError"`, Token-Ledger leer).
- `client/tests/script_engine.rs` — Symmetrie-Pin `client_forged_throw_does_not_determine_reported_error_kind`.

## Akzeptanzkriterien

1. **Forged `throw` kann den `kind` nicht mehr bestimmen** — ERFUELLT.
   `map_rhai_err` rekonstruiert den `ScriptError` ausschliesslich aus einem
   `HostErrorPayload`-Cast (`payload.try_cast::<HostErrorPayload>()`). Ein
   Skript-`throw "<json>"` erzeugt einen String-`Dynamic` (Diagnose §1, empirisch
   gegen rhai 1.24.0 belegt), der den Cast NICHT besteht und im `None`-Zweig als
   generischer `HostError { source }` endet. Die Diskriminierung erfolgt am
   Rust-Typ, NICHT am `serde-kind` und NICHT am Match-Arm (`ErrorTerminated` vs.
   `ErrorRuntime`) — daher kann auch ein `throw`, der `"kind":"capabilityDenied"`
   (ein unmaskable kind) behauptet, keinen typisierten Fehler rekonstruieren:
   `throw` liefert immer `ErrorRuntime` mit String-Payload, der Cast schlaegt fehl.
2. **Echte Host-Fehler weiterhin korrekt** — ERFUELLT. `script_err_to_rhai` legt
   den echten `ScriptError` in `HostErrorPayload` und propagiert ihn als
   `ErrorTerminated`/`ErrorRuntime` (maskable-Entscheidung via `e.unmaskable()`,
   korrekt VOR dem Move gelesen). `map_rhai_err` castet ihn 1:1 zurueck.
   Bestaetigt durch gruene Non-Regress-Tests (`..._records_capability_denied_outcome`,
   `..._records_ok_outcome_with_token_uses`, `capability_denied_is_not_catchable_via_try_catch`).
   Timeout/MemoryExceeded kommen weiterhin aus den eigenen Match-Armen
   (`ErrorTooManyOperations`/`ErrorDataTooLarge`) — vom Sentinel unberuehrt.
3. **Neuer Spoof-Pin-Test** — ERFUELLT. Vier neue Tests; der Kern-Pin ist
   parametrisiert ueber alle vier kritischen kinds und assertiert BEIDE Seiten:
   negativ (`!matches!` typisierte Fehler) UND positiv (`matches!(HostError)`).
   Der E2E-Pin schliesst den Audit-Pfad ein.

## Detail-Pruefungen

- **Symmetrie Server↔Client:** Beide `map_rhai_err` UND beide `script_err_to_rhai`
  sind konsistent umgestellt; der Sentinel-Typ ist auf beiden Seiten identisch
  (eigener `struct HostErrorPayload(ScriptError)` pro Crate — korrekt, da privat
  und nicht ueber die Wire-Grenze transportiert). Kein verbleibender
  `serde_json::from_str::<ScriptError>`-Pfad auf einer Seite. Beide Crate-Engines
  sind damit gleich gehaertet.
- **Non-JSON `throw` erhalten:** `throw "plain-non-json"` → `HostError { source }`
  mit `source = e.to_string()`. Die Beschreibung wird VOR dem Move gesichert
  (`let descr = e.to_string();`) und enthaelt empirisch den Wurf-String
  (Test `non_json_throw_still_maps_to_host_error` gruen — `source.contains("plain-non-json")`).
- **Engine-Limit-Arme:** `ErrorTooManyOperations`/`ErrorDataTooLarge`/`ErrorParsing`
  bleiben unveraendert in ihren eigenen Armen — der Sentinel betrifft nur den
  zusammengefassten `ErrorTerminated|ErrorRuntime`-Arm.
- **Panic-/`unwrap`-Risiko:** Keine neuen `unwrap`/`expect`/`panic` in der
  Produktion. `try_cast` ist total (`Option`), `to_string()` ist infallibel.
  Reihenfolge-Subtilitaet (Option-B braucht keinen Seitenkanal, daher kein
  catch-und-spaeter-throw-Problem wie bei der Diagnose-Option A) korrekt vermieden.
- **Verifikation reproduziert:** server `script_gate_integration` (5) + `script_run`
  (5) + lib `script::engine::rhai`-Invarianten (5) + client `script_engine` (21)
  lokal gruen nachgestellt (`--target-dir target-test` wegen Windows-Lock/OOM gemaess
  CLAUDE.md). Die inline-Invarianten `error_terminated_is_not_catchable` /
  `error_runtime_is_catchable` haengen an direktem `Dynamic::from(String)` und sind
  vom Transport-Wechsel unberuehrt — bleiben gruen.

## Non-blocking Hinweise

- **N1 (stale comment, server):** Der Doc-Kommentar von `register_host_fns`
  (`server/src/script/engine/rhai.rs` ~Z.209-213) sagt noch „beide tragen den
  serialisierten `ScriptError` im Payload" — der Payload ist nach dem Fix der
  Sentinel-Custom-Typ, nicht mehr serialisiert. Rein dokumentarisch, kein
  Verhaltens-Bug. Sollte bei Gelegenheit auf `HostErrorPayload` angepasst werden.
- **N2 (Audit-Granularitaet):** Ein gefaelschter `throw` ist im Audit nicht von
  einem echten internen `HostError` unterscheidbar (bewusste Variant-Entscheidung
  im Plan: `HostError` statt neuer `ScriptThrew`-Variante). Akzeptabel — der Wurf
  bedeutet ohnehin „Skript hat unstrukturierten Fehler ausgeloest"; ein
  Folge-Item `ScriptThrew` ist der minimale Trennschritt, falls je gewuenscht.
- **N3 (Mikro):** `let descr = e.to_string();` wird fuer JEDEN Fehler berechnet,
  auch wenn der Cast gleich erfolgreich ist (Real-Host-Fehler) — eine kleine,
  verworfene Allokation pro Fehler-Run. Vernachlaessigbar (Fehlerpfad, kein
  Hot-Loop); eine Verlagerung in den `None`-Zweig ist moeglich, aber dort steht
  `payload` nach `try_cast` nicht mehr zur Verfuegung. Nicht aenderungswuerdig.

## Fazit

Der Fix adressiert die Root-Cause exakt: der Outcome stammt aus einer Quelle, die
das Skript nicht beschreiben kann (Rust-Typ statt skript-erzeugbarem JSON-String).
Symmetrisch auf Server und Client, keine neuen Panic-/Lint-Risiken, alle drei
Akzeptanzkriterien erfuellt, Bestands-Pins gruen. Die Hinweise N1-N3 sind
nicht-blockierend. **APPROVE.**
