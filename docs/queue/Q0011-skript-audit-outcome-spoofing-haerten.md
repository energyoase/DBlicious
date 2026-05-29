---
id: Q0011
created: 2026-05-25T00:00:00Z
status: done
priority: medium
title: "Skript-Audit-Outcome-Spoofing härten: Host-Fehler nicht durch den Rhai-Fehlerwert transportieren"
spec: null
plan: docs/superpowers/plans/Q0011-skript-audit-outcome-spoofing-haerten.md
pending_question_id: null
resume_step: null
parent: Q0009
artifacts: []
source: review
fingerprint: null
requirements: null
assigned_worker: null
type: bug
review:
  status: null
  reviewer: null
  notes_path: null
  requested_at: null
  decided_at: null
security_review:
  required: true
  status: null
  notes_path: null
diagnosis_path: docs/superpowers/diagnoses/Q0011-skript-audit-outcome-spoofing-haerten-diagnosis.md
design_path: null
linked_issue: null
linked_pr: null
verification_ops: []
tags:
  - scripting
  - sandbox
  - rhai
  - audit
  - security-hardening
  - q0009-followup
---

## Description

Befund aus der Q0009-Security-Re-Review (2026-05-25,
[`docs/reviews/Q0009-review.md`](../reviews/Q0009-review.md) §Security-Re-Review).
**Nicht-blockierend, kein Sandbox-Escape** — Q0009 wurde mit diesem als
Follow-up ausgegliederten Befund als CLEARED/APPROVE archiviert.

### Das Problem

Host-Funktions-Fehler reisen heute als serialisierter `ScriptError` im
Payload eines Rhai-`EvalAltResult` (`ErrorRuntime`/`ErrorTerminated`) und
werden in `map_rhai_err` (`server/src/script/engine/rhai.rs:365-398`) per
`serde_json::from_str::<ScriptError>(payload)` zurück-deserialisiert.

Rhais `throw <expr>` erzeugt selbst ein `ErrorRuntime(value, …)`. Ein Skript
kann also `throw "{\"kind\":\"timeout\",\"limit_ms\":99999}"` (oder jeden
anderen `ScriptError`-`kind`) werfen — `map_rhai_err` matcht den
`ErrorRuntime(payload, _)`-Arm, deserialisiert erfolgreich und meldet einen
**vom Skript frei gewählten** `ScriptError` als Run-Ergebnis.

### Wirkung — und Grenze der Wirkung

Begrenzt auf **Audit-Fidelity**: das Skript kann den gemeldeten
`outcome`/`error`-Wert seines *eigenen* Runs fälschen (landet in
`script_audit_log.outcome` bzw. im Preview-`error_json`).

**Ausdrücklich KEIN** Sandbox-Escape / Capability-Bypass:

- Ein echter Deny ist `ErrorTerminated` (per `try`/`catch` uncatchbar) — das
  Skript erlangt nach einem echten Deny nie wieder Kontrolle, kann also einen
  unmaskable Fehler **nicht** zu maskable downgraden.
- Der `token_uses`-Audit-Ledger wird aus der echten `Sandbox` gezogen
  (`run_collecting`), unabhängig vom geworfenen Payload — ein gefälschter
  `outcome` löscht keine echte `Denied`/`Ok`-Token-Nutzung.
- Kein Codepfad gewährt einen Seiteneffekt aufgrund des zurückgemappten
  `ScriptError` (nur Reporting).

### Empfohlener Fix

Host-originierte Fehler nicht über den Rhai-Fehlerwert (den ein Skript per
`throw` erzeugen kann) zurück-transportieren, sondern out-of-band:

- Option A: den echten `ScriptError` des Host-Calls im `RunState` ablegen
  (z.B. `last_host_error: Option<ScriptError>`) und nach dem Run von dort
  lesen, statt aus dem Rhai-Payload zu deserialisieren.
- Option B: Host-Payloads mit einem Sentinel kennzeichnen, das ein Skript
  nicht selbst erzeugen kann (z.B. ein nicht-serde-darstellbarer
  `Dynamic`-Typ statt eines JSON-Strings).

Ein script-`throw` soll als generischer `HostError`/`ScriptThrew` enden,
nicht als beliebig wählbarer typisierter `ScriptError`.

### Akzeptanzkriterien

1. Ein Skript-`throw` eines beliebigen JSON-Strings kann den gemeldeten
   `ScriptError`-`kind` des Runs **nicht** mehr frei bestimmen (insbesondere
   nicht `timeout`/`memoryExceeded`/`capabilityDenied` vortäuschen).
2. Echte Host-Fehler (CapabilityDenied, Timeout, MemoryExceeded, HostError)
   werden weiterhin korrekt gemeldet.
3. Neuer Test im `script_*`-Set, der den Spoof-Pfad pinnt (Skript wirft einen
   gefälschten `ScriptError`-JSON → Run-Outcome ist NICHT der gefälschte Wert).

## Affected files (zu erwarten)

- `server/src/script/engine/rhai.rs` — `map_rhai_err`, `script_err_to_rhai`,
  `RunState`, `run_collecting`.
- ggf. `client/src/script/engine/rhai.rs` (Symmetrie).
- Tests: `server/tests/script_run.rs` oder `server/tests/script_engine.rs`.

## Notes

- Minor-Begleitbefunde aus derselben Re-Review (kein eigenes Item nötig,
  hier vermerkt): heuristisches Memory-Limit in `apply_limits` (dokumentiert,
  akzeptiert — kein Byte-genaues Budget); client `map_rhai_err` hardcodet
  `Timeout{limit_ms:0}` (kosmetisch, Client hat kein Manifest/Compute-only).

## Log
- 2026-05-29T09:30:24Z — ccm-debug: status new → diagnosed, diagnosis=docs/superpowers/diagnoses/Q0011-skript-audit-outcome-spoofing-haerten-diagnosis.md
- 2026-05-29T13:20:37Z — ccm-plan: status diagnosed → planned, plan=docs/superpowers/plans/Q0011-skript-audit-outcome-spoofing-haerten.md
- 2026-05-29T13:25:47Z — ccm-execute: status planned → executing (pre-approved via plan transition)
- 2026-05-29T14:00:03Z — ccm-execute: status executing → done, final_sha=13064f0 (Option B Sentinel-Dynamic, server+client; verification green) — awaiting review
