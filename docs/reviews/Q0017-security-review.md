# Q0017 — Security Review

**Scope:** `git diff 21c525c..82e52f5` (8 files)
**Verdict:** cleared
**Reviewer:** security-review (automated)
**Date:** 2026-05-31

## Change summary

Q0017 is Stage-3 of Q0014: it wires the already-built `script:`-column-validators
into the live editor's `ValidationSystem` at editor-load time, and re-runs them as a
client-side pre-flight gate on save.

Files:
- `client/src/routes/editor.rs` — at load, after `import_required_from`, optionally
  `fetch_columns(entity_type)` and call `register_script_validators(...)`; a
  `sys.clear(&entity_type)` precedes registration to prevent double-attach on re-open.
- `client/src/validation/mod.rs` — new `ValidationSystem::clear(entity_type)` +
  unit test.
- `client/src/validation/script_task.rs` — new pure helper
  `register_script_validators` that loops columns and reuses Q0014's
  `script_validator_task` (no new engine/lookup code) + tests.
- `client/locales/{de,en,fr}/main.ftl` — one static FTL string `validation-script`.
- `examples/d2v/entities/datev_entry/columns.json` — adds
  `"validatorId": "script:d2v_balance_validator"` to the `value` column.
- `client/tests/editor_validator_hookup.rs` — native integration test (real Rhai).

**Zero `shared/` and zero `server/` changes** (confirmed against the diff name list).

## Security focus findings

### 1. Capability escalation / sandbox bypass vs Q0014 — NONE

`register_script_validators` (script_task.rs:71) is a thin loop that, for each column
with a `script:`-prefixed `validator_id`, calls the *unchanged* Q0014
`script_validator_task`. That function builds a `TaskFn` that on invocation calls the
*unchanged* `lookup_provider(..., ProviderSlot::Validator, ...)`
(provider_lookup.rs:67). The full gating chain is untouched:

- `parse_script_id` → registry lookup → `state == Active` check → **slot-match check**
  (`ScriptKind::Provider { slot } if *slot == Validator`) → `RhaiEngine::compile` →
  `Sandbox::new(&manifest)` → `engine.run(...)`.
- All host-calls remain gated by `Sandbox::gate`, which enforces
  `manifest.capabilities.contains(token)` *first* (sandbox.rs:56) — a denied token
  returns `CapabilityError`, never silently grants.

`d2v_balance_validator`'s manifest is `tier=Reader`, `capabilities=[ComputeOnly,
ReadI18n]`. It performs no host-calls at all (pure arithmetic over `fields`). There is
no code path by which registering this validator at editor-load grants it
`WriteEntity` or any read beyond its manifest: the manifest is the single source of
truth at the `Sandbox::gate` choke point, and Q0017 adds no new gate, no new
token-granting, and no new `HostApi` surface. Same `registry`/`host`
(`ScriptRenderEnv`, script_renderer.rs:315) already provided app-wide for the
formatter/renderer path is reused — no new privileged context is constructed.

**Conclusion:** Running the validator at load + on save opens no new capability
relative to the already-cleared Q0014 path.

### 2. Client validation is NON-authoritative — invariant preserved

The save handler (editor.rs:155–217) runs the client `ValidationSystem` as a
*pre-flight* gate: if `result.has_blocking()` it returns early (better UX, fewer
round-trips). But when it does proceed, it calls `savable.update(...)` /
`savable.create(...)` (RemoteSource → GraphQL), and the server's response carries
`res.validation`; the UI honors `res.validation.has_blocking()` (editor.rs:201) and
`res.ok`. The server validate/permission path is **not touched** by this diff and
remains authoritative.

**Fail-open analysis:** the documented fail-open is "no `ScriptRenderEnv` in context
⇒ no script-validators registered ⇒ client pre-flight does not block." Likewise a
script compile/runtime error inside `lookup_provider` returns `Fallback` ⇒
`script_validator_task` maps anything that is not `Bool(false)` to `None` ⇒ no client
block. In every fail-open branch the save still goes through the server mutation,
which re-validates and can reject. There is **no path** where the client script
validator becomes the only gate — fail-open at worst skips a redundant client-side
check, never bypasses the server. Invalid data cannot be persisted via a client
fail-open.

### 3. Editor-load `fetch_columns` — no injection risk

`fetch_columns` (queries.rs:108) is a read-only GraphQL `columns` query. `validator_id`
is deserialized as `Option<String>` from server-supplied `ColumnMeta`, which originates
from operator-authored data-dir metadata (`examples/.../columns.json`), not from a
runtime mutation argument. There is no `ColumnMeta` `InputObject` and no mutation that
accepts a `validator_id` — an end-user cannot inject a validator id at runtime. The
string is only ever used as a registry key (`script:<id>` → `ScriptId`); a non-existent
or non-`script:` id degrades to `NotAScriptId` / `Fallback::Missing` (no block, audit-
logged), never to code execution outside the registered, manifest-gated scripts. The
Rhai *source* executed is the registered `Script.source` from the registry, never the
`validator_id` string.

### 4. `ValidationSystem::clear(entity_type)` — no cross-entity leak

`clear` (mod.rs:54) does `self.tasks.remove(entity_type)` — a single `HashMap` key
removal scoped to the passed type. The new unit test
(`clear_removes_only_the_given_entity_types_tasks`) asserts that clearing
`datev_entry` leaves `other` intact. The editor-load closure always calls
`clear(&entity_type)` with the same `entity_type` it then registers against, so there
is no state-confusion (clearing the wrong entity) or stale-task leak across entity
types. The clear-before-register pattern correctly prevents duplicate-task buildup on
repeated opens of the App-lifetime `ValidationSystem`.

### 5. Logging / FTL hygiene — no secret/PII leak

The new FTL strings are static, parameter-free messages ("Validierung
fehlgeschlagen." / "Validation failed." / "Échec de la validation."). The validator
error surfaces only `SCRIPT_VALIDATION_KEY = "validation.script"` with the field
`target` (the column key, e.g. `value`) — no field values, no script internals.
Script-error/fallback paths log via the existing Q0014 `push_fallback` audit queue
(unchanged here), which records `script_id` + reason enum, not user data. No new
`log::error!`/`console` call introduced by this diff emits secrets or PII.

### 6. No sensitive surface touched

Confirmed by reading the full diff: no `.env`, no secrets, no real-DB / SQL, no
auth/permission logic, no crypto, no IPC, no network/transport config. The only network
interaction is the pre-existing read-only `fetch_columns` GraphQL query, invoked one
extra time at editor-load. CORS, sessions, and the server schema are untouched.

## Advisory notes (non-blocking)

- **Defense-in-depth (informational):** as with Q0014, the server side does not yet
  run the equivalent `script:` validator in the authoritative path for this column
  (the server validate path is generic/required-field based). This is acceptable
  because the change is explicitly client-pre-flight-only and the server still
  performs its own validation/permission checks; nothing here weakens that. If/when a
  server-side script-validator lands it should mirror the same slot+manifest gating
  (provider_lookup symmetry already noted in the code).
- **Minor:** `fetch_columns` is fetched separately from `fetch_editor` at load,
  adding one round-trip per editor open. No security impact; performance-only.

## Conclusion

The change reuses the already-security-cleared Q0014 sandbox/capability/lookup path
verbatim, adds only a pure registration loop, a scoped `clear`, static FTL strings, and
operator-authored example metadata. It does not introduce capability escalation, does
not make client validation authoritative, exposes no injection surface, and leaks no
secrets/PII. **Verdict: cleared.**
