# Q0009 Skript-Sprache — Code-Review

Date: 2026-05-23
Reviewer: claude (feature-dev:code-reviewer Sub-Agent, Sandbox-Sicherheits-Fokus)
Scope: committeter Q0009-Code @ HEAD `4cba6b2` (alle 6 Phasen), gegen Spec `2026-05-23-q0009-skript-sprache-design.md`.
Verdict: **NEEDS-WORK** → **REMEDIATED** (2026-05-24)

Dieser Review deckt zugleich den `security_review.required`-Teil ab (Fokus lag auf Sandbox-Escape + Capability-Enforcement).

## Remediation-Status (2026-05-24)

Alle Blocker + Should-Fixes behoben:

| Punkt | Commit | Beweis |
|---|---|---|
| **B1** unmaskable uncatchbar | `ce03304` | `capability_denied_cannot_be_caught_by_try_catch` (try/catch schluckt CapabilityDenied NICHT) + `rhai_invariants::error_terminated_is_not_catchable` |
| **B2** Rhai-Packages | `1e4c78d` | `rhai_engine_actually_evaluates_arithmetic` + `..._string_and_array_ops` (echter eval-Pfad) |
| **B3** Gate im run-Pfad | `ce03304` | `run_and_persist`-Tests mit echten `db.entities`-Skripten: CapabilityDenied bei fehlendem Token im echten eval-Pfad + token_uses-Audit |
| **S4** token_eq exakt | `50a3476` | `reader_cannot_declare_composite_ui_node_scope` + `reader_may_declare_leaf_ui_node_scope` |
| **S5** Memory-Limits | `ce03304` | `apply_limits` setzt Rhai-Size-Limits aus `memory_kb`; `ErrorDataTooLarge → MemoryExceeded` |
| **S6** preview db.patch | `ce03304` | PreviewHostApi `db_patch → ServerOnlyFunction` |
| **S7** Timeout-Wert | `ce03304` | `ErrorTooManyOperations → Timeout{ echtes limit_ms }` |
| **S8** client audit.log | `ce03304` | `host_audit_log_on_client_is_server_only_function_error` |

Spike-Vorarbeit: `b2beb26` (Rhai-Invarianten gepinnt). Workspace-Build gruen, server script-Tests 43, client script-Tests 32.

## Blocker

### B1 — `unmaskable()` ist nur Klassifizierung, kein Enforcement (Confidence 95)
`server/src/script/engine/rhai.rs:75-88`, `shared/src/script/error.rs:74-84`

Spec §5.2/§10 verlangt: unmaskable Errors (`CapabilityDenied`, `Timeout`, `UiPrimitiveDenied`, `MemoryExceeded`) dürfen NICHT per Rhai `try`/`catch` gefangen werden. `ScriptError::unmaskable()` klassifiziert korrekt, wird aber **nirgends im run-Pfad aufgerufen**. Ein als native-function-error propagierter `CapabilityDenied` ist mit `try { db.entities("x") } catch(e) { 42 }` fangbar.

Fix: In `map_rhai_err` (server + client) nach `err.unmaskable()` prüfen und via `EvalAltResult::ErrorTerminated` (oder Terminierungs-Callback) die Catch-Barkeit verhindern.

### B2 — `Engine::new_raw()` ohne Package-Registrierung (Confidence 90)
`server/src/script/engine/rhai.rs:31-38`, `client/src/script/engine/rhai.rs:35-43`

Spec §5.1 verlangt die Module `Arithmetic`, `Logic`, `BasicString`, `BasicArray`, `BasicMap`. Implementierung lädt KEINE Packages. In Rhai 1.x hat `new_raw()` keine eingebauten Funktionen — nicht mal Integer-Arithmetik. `40 + 2` parst, aber `eval_ast_with_scope` müsste zur Laufzeit fehlschlagen. Dass die Tests grün sind, ist verdächtig (laufen sie wirklich durch den eval-Pfad?). **Zu verifizieren.**

Fix: In `configure_strict()` die 5 Packages via `…Package::new().register_into_engine(engine)` laden.

### B3 — Sandbox-Gate ist NICHT im echten Ausführungspfad verdrahtet (Confidence 95)
`server/src/script/provider_lookup.rs:116-148`, `server/src/script/run.rs:110-180`, `server/src/script/engine/rhai.rs:79`

Zentrale Lücke: `RhaiEngine::run()` ignoriert `_host` (underscore-prefixed). Es werden **keine Host-Funktionen** auf der Rhai-Engine registriert, und `Sandbox::gate()` wird während der Skript-Ausführung **nie** aufgerufen. Alle `gate()`-Tests prüfen die Sandbox-API isoliert; **kein Test** beweist, dass die Gate bei echter Engine-Ausführung feuert.

Konsequenz heute: ein Skript kann `db.entities(...)` gar nicht aufrufen (Funktion nicht registriert) → aktuell **sicher, aber funktional unvollständig**. Sobald Host-Funktionen verdrahtet werden, MUSS die Gate-Integration gleichzeitig kommen, sonst fehlt Capability-Enforcement im Produktionspfad.

Fix: Host-Funktionen als native fns via `engine.register_fn(...)` registrieren, die durch `sandbox.gate(token, || host.…())` laufen. Das ist der ausstehende Integrations-Schritt, auf den die „Provisorium"-Kommentare verweisen.

## Should-Fix

### S4 — `token_eq` ignoriert struct-variant-Felder → Tier-Bypass (Confidence 88)
`server/src/script/save.rs:99-118`

`matches!((a,b), (WriteEntity{..}, WriteEntity{..}))` ignoriert `validated`. Damit passiert `WriteEntity { validated: false }` die Validierung, als wäre es `validated: true`. Gleiches für `EmitUiNode { scope }` (Reader könnte `Composite` deklarieren) und `ReadAuditLog { own_only }` (Author könnte `own_only: false` = alle Logs). **Echter Security-Boundary-Bypass**, sobald Host-Wiring (B3) steht.

Fix: `token_eq` durch `a == b` ersetzen (`CapabilityToken` derived `PartialEq`).

### S5 — `MemoryExceeded` nie emittiert, kein Memory-Limit-Enforcement (Confidence 92)
`manifest.memory_kb` wird in `save.rs` validiert, aber zur Laufzeit nirgends durchgesetzt (kein `set_max_string_size`/`set_max_array_size`/`set_max_map_size`). Nur der Operation-Counter (CPU) bindet. Fix: in Engine-Factory die Rhai-Size-Limits aus `memory_kb` ableiten.

### S6 — preview `db_patch` returnt `CapabilityDenied` statt `ServerOnlyFunction` (Confidence 82)
`server/src/schema.rs:1488-1495`, `server/src/script/run.rs:82-102`. Inkonsistent mit Spec-Intent; aktuell nur durch den GraphQL-Pre-Check abgefangen (fragil, da Gate nicht feuert — siehe B3).

### S7 — `Timeout { limit_ms: 0 }` verliert echten Limit-Wert (Confidence 85)
`engine/rhai.rs:159` (+ client:122). `ErrorTooManyOperations` → `limit_ms: 0`; Audit-Log kann timeout nicht von 0-timeout unterscheiden. Fix: Manifest-`timeout_ms` an `map_rhai_err` durchreichen.

### S8 — client `audit.log` ist `server_only` aber wird nicht rejected (Confidence 80)
`client/src/script/host/audit.rs:36-38`. Anders als `db.patch` keine `ServerOnlyFunction`-Ablehnung. Defense-in-depth-Lücke / Doku-Ambiguität.

## Invarianten, die HALTEN (PASS)

- **Symbol-Disable real**: `new_raw()` + `disable_symbol` für `eval`/`import`/`print`/`debug`, je Test verifiziert. `internals`-Feature re-aktiviert nichts Gefährliches.
- **Server-only für `db.patch` (Client)**: korrekt `ServerOnlyFunction` + Registry-Flag.
- **`lift_capable`-Analyse**: AST-Walk prüft 1. Argument von `db.entities/entity`, dynamisch → false; konservativ korrekt; gute Testabdeckung.
- **preview persistiert keine Audit-Row**: Test zählt Rows vorher/nachher; `run_preview` ohne Insert.
- **Symmetrie-Test**: bidirektional inkl. server_only-Flag (runtime, nicht compile-time).

## Empfehlung

Architektur + Layering sind solide; die Wire-Typen, Sandbox-API, Persistenz und Tests sind gut gebaut. Aber Q0009 ist **funktional unvollständig** (B3: Engine↔Host nicht verdrahtet) und hat echte Enforcement-Bugs (B1 unmaskable, S4 token_eq), die scharf werden, sobald die Verdrahtung steht. **Vor produktiver Nutzung** der Skript-Ausführung: B1–B3 + S4 fixen, B2 verifizieren.
