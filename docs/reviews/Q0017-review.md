# Code Review — Q0017 (validator_id on-save Editor-Hookup, Stage-3 von Q0014)

- **Diff scope:** `git diff 21c525c..82e52f5` (8 files, +311/-3)
- **Reviewer:** code-review skill (adapted to local queue-item flow; no GitHub/queue/audit touched)
- **Date:** 2026-05-31
- **Verdict:** approve

## Scope verification (locked plan vs. diff)

Ansatz A as locked. All scope items present and matching the spec/plan:

| Plan item | Status |
|---|---|
| Editor zieht zusätzlich `fetch_columns` + registriert | OK (`editor.rs:111,130-141`) |
| Pure `register_script_validators` helper, reuses Q0014 `script_validator_task` | OK (`script_task.rs:67-90`) |
| ZERO shared/server changes | OK — `git diff --name-only` shows only `client/`, `examples/d2v/`, `client/locales/`. No `shared/` or `server/` path. |
| `ValidationSystem::clear` for re-load idempotency | OK (`mod.rs:57-59`), called first in load closure (`editor.rs:122`) |
| Fail-open via `Option<ScriptRenderEnv>` | OK (`editor.rs:111` `use_context::<ScriptRenderEnv>()`, guarded `if let Some(env)`) |
| `ScriptCtx::default()` | OK, consistent with all other call sites |
| d2v value column gets `validatorId=script:d2v_balance_validator` | OK (`columns.json`) |
| FTL `validation-script` de/en/fr | OK (all three at line 71, section `## Validation`) |
| native `#[test]` in `client/tests/editor_validator_hookup.rs` | OK (3 tests) |
| editor on-save UI mount test NOT in scope | Correctly omitted; tests exercise `register_script_validators` + `run` directly |

## Focus-area findings

### Correctness of `register_script_validators` + Load-closure wiring — PASS
- Iterates columns, `col.validator_id.as_deref()` → `script_validator_task` (returns `None` for non-`script:` prefix, so static IDs are silently skipped — matches Q0014 semantics). Only `script:`-prefixed columns register a task. Verified against `script_task.rs:34-36`.
- Load closure greift `script_env_for_load` **vor** `LocalResource::new` und moved den `Option<ScriptRenderEnv>` in die async-Closure — exactly the plan's "Env vor LocalResource greifen, kein Context-Zugriff im async-Body" Vorgabe (§5.2). No `use_context` in the async body.
- `env.registry` / `env.host` fields exist on `ScriptRenderEnv` (`script_renderer.rs:316,320`); call site types line up.
- `validation_for_load` and `validation_for_save` are clones of the same `ValidationSystemHandle` (`Arc<Mutex<ValidationSystem>>`) from `use_validation_system()` — registration at load and query at save hit the same instance. Wiring is sound.

### Fail-open when no ScriptRenderEnv — PASS
- `use_context::<ScriptRenderEnv>()` returns `Option`; `if let Some(env)` guards registration. No env ⇒ no script validators ⇒ save stays allowed. No `expect`/panic path. Matches §8 Plan-Vorgabe.
- Inner `if let Ok(columns) = fetch_columns(...)` also fail-open: a transient `fetch_columns` error ⇒ no registration ⇒ save allowed (server authoritative). Acceptable (see non-blocking note 1).

### `ValidationSystem::clear` correctness (no leak/duplicate on re-load) — PASS
- `clear` = `self.tasks.remove(entity_type)` — removes only the given type's task vec; other types untouched. Unit test `clear_removes_only_the_given_entity_types_tasks` proves exactly this (datev_entry empties, `other` survives).
- Called **first** in the load closure (`editor.rs:122`), before both `import_required_from` and the script-validator registration. This also fixes the pre-existing re-load duplication of `import_required_from`'s required-tasks (noted in spec §8) — a latent bug, now covered. Ordering clear → import_required → script-register is correct.

### Client validation stays NON-authoritative — PASS (invariant from Q0014 security review upheld)
- The diff adds no server code and changes no save path. `on_save` still calls `RemoteSource.savable().create/update` after the client check (`editor.rs:169-185`); server remains the authoritative gate. Client check is a pure UX early-warning. `has_blocking()` only short-circuits the client spawn; it does not replace the server validation that runs on the actual write.
- Fail-open: script error / missing / non-bool ⇒ `_ => None` ⇒ no block ⇒ save proceeds to server. A broken/missing client script can at worst drop a warning, never force an illegal write.

### FTL key present + consistent — PASS
- `validation-script` present in de (`Validierung fehlgeschlagen.`), en (`Validation failed.`), fr (`Échec de la validation.`), all at line 71 in the `## Validation` section. `t("validation.script")` normalises to Fluent ID `validation-script` (i18n dot→dash), matching `SCRIPT_VALIDATION_KEY = "validation.script"`. Key now renders instead of raw fallback.

### columns.json edit valid — PASS
- Only the `value` column changed: appended `"validatorId": "script:d2v_balance_validator"` to the existing object; JSON well-formed, other columns untouched. `validatorId` is the camelCase wire name for `ColumnMeta.validator_id`.

### Test genuinely proves block-on-violation AND pass-on-valid — PASS
- `unbalanced_record_is_blocked`: SOLL 100 vs HABEN 90 → real Rhai engine returns `Bool(false)` → `ValidationMessage::error` (Severity::Error → `blocks()` true) → `has_blocking()` true; asserts message target `value` and key `validation.script`. Verified `ValidationMessage::error` sets `Severity::Error` (`shared/validation.rs:53-60`) and `has_blocking` checks `severity.blocks()`.
- `balanced_record_passes`: SOLL 100 vs HABEN 100 → `Bool(true)` → `_ => None` → `is_empty()` true.
- `non_script_and_missing_validator_register_no_task`: confirms only `value` registers; `description` (None) and `key` (`static-x`, non-script prefix) add no task.
- Tests use the **real** Rhai engine (`MockHostApi`, real `ScriptRegistry`, the actual `d2v_balance_validator.rhai` via `include_str!`) — not a mock predicate. They call `register_script_validators` — the same function the editor uses — so they exercise the genuine path. An in-module duplicate test in `script_task.rs` (`registers_only_script_validators`) covers the registration count as well.

### No DesignSystem bypass — PASS
- No styling/CSS changes in the diff at all. No inline `style="..."` added. N/A but clean.

### German comments — PASS
- New comments (`editor.rs:108-110,120-121,127-128`, `mod.rs:53-56`, `script_task.rs:67-70`) are German; identifiers English. Matches CLAUDE.md convention.

### ScriptCtx::default() for the balance validator (ComputeOnly+ReadI18n — does it get i18n?) — PASS, acceptable
- `d2v_balance_validator.rhai` reads only `fields.{value,valueType,partnerValue,partnerValueType}` and returns a bool; it **never calls `i18n_t`**. The declared `ReadI18n` capability is unused by this specific script. `ScriptCtx::default()` carries empty locale/user/tenant, which the script does not consult — so the default ctx is functionally sufficient for the balance validator.
- It is also the established pattern across the codebase: the table filter path (`data_source.rs:507`) and every `provider_lookup.rs` call use `ScriptCtx::default()`. The spec §5.3 explicitly accepts default as sufficient and consistent. Note: the user-facing `validation-script` message is rendered by the client i18n (`t(...)`), not from inside the script, so locale correctness of the message is unaffected by the ctx.

## Non-blocking suggestions

1. **Transient `fetch_columns` failure silently drops script validators on re-load.** Because `clear(entity_type)` runs unconditionally at the top of the load closure but `fetch_columns` is fail-open, a transient GraphQL error during a re-open leaves the editor with required-tasks but no script validators until the next successful load. This is safe (server authoritative, UX-only warning) and matches the documented fail-open posture, but a one-line `log::debug!`/`warn!` on the `Err` arm of `fetch_columns` would make the degraded-warning state observable. Optional.

2. **Minor duplication between the two tests.** `client/tests/editor_validator_hookup.rs` and the inline `register_tests` module in `script_task.rs` build near-identical `validator_script`/`col` fixtures. Could share a fixture, but the integration test is intentionally standalone (crate-external, proves the public API) — leaving it duplicated is a defensible trade-off. Optional.

## Conclusion

Approve. The change is a precise last-mile wiring of Q0014's `script:` validators into the live editor's `ValidationSystem` at load time. Scope is exactly as locked (Ansatz A), no shared/server contract touched, fail-open is correct and keeps client validation non-authoritative (Q0014 security invariant upheld), `clear` correctly prevents re-load duplication (and incidentally fixes the latent `import_required_from` re-load dup), FTL keys are present/consistent in all three locales, the columns.json edit is valid, and the tests genuinely prove block-on-violation and pass-on-valid through the real Rhai engine. `ScriptCtx::default()` is acceptable for the balance validator (it consumes no ctx/i18n). German-comment and no-DesignSystem-bypass conventions honoured. No blocking issues.
