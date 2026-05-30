# Q0014 Security Review

**Item:** Q0014 — `validator_id` slot + `script:`-Prefix-Filterpfad (Stage-2 framework work activating Q0013's dormant script metadata).
**Diff scope:** `c45abc6..0774e1b`
**Reviewer:** security-review (Claude Opus 4.8)
**Date:** 2026-05-31
**Verdict:** cleared — 0 blocking security issues.
**Prior code review (context, not a security pass):** `docs/reviews/Q0014-review.md` (approve, 0 blocking).

## Scope summary

The change is additive framework wiring across 21 files. Substantive logic lives in:

- `shared/src/lib.rs`, `shared/src/settings.rs` — additive `validator_id` / `allowed_validator_ids` fields (`#[serde(default, skip_serializing_if = ...)]`).
- `server/src/schema.rs`, `server/src/data.rs`, `client/src/graphql/queries.rs` — thread `validator_id` through the GraphQL re-wrap and client deser.
- `client/src/validation/script_task.rs` (new) — Lücke A: builds a `TaskFn` that runs a `script:`-validator through `lookup_provider`.
- `client/src/components/table/data_source.rs` — Lücke B: per-row `script:`-filter predicate branch in `LocalSource::passes` + `script_predicate`.
- `client/src/components/table/filters/mod.rs` — guard so a `script:`-filter renders no built-in input widget.
- `client/src/components/registries/resolve.rs` — `"validator"` resolution arm mirroring `"formatter"`.

The remaining ~10 files are one-line `validator_id: None` struct-field additions in constructors/test fixtures. Trivial, no security relevance.

## Focus-area findings

### 1. Script execution surface — capability escalation / sandbox bypass — N/A (no new vector)

The two new call sites (`script_validator_task` → `lookup_provider(..., ProviderSlot::Validator, ...)` and `script_predicate` → `lookup_provider(..., ProviderSlot::Filter, ...)`) route through the **exact same** `client::script::provider_lookup::lookup_provider` function as the pre-existing formatter path. That function unconditionally enforces, in order:

1. `script:`-prefix parse (else `NotAScriptId`, no run);
2. `registry.get` (missing → `Fallback`);
3. `script.state == Active` (Draft/Locked → `Fallback`);
4. `ScriptKind::Provider { slot } == expected_slot` (mismatch → `Fallback` + audit) — a Formatter script cannot be driven as a Validator or Filter and vice versa;
5. compile → `Sandbox::new(manifest)` deadline check → `engine.run(...)`.

Capabilities are not chosen by the call site — they come from the script's own manifest, enforced inside `RhaiEngine::run`/`RunState::gate` and `Sandbox::gate` (`CapabilityDenied` on any token not declared). A script cannot gain `WriteEntity` or read-beyond-capability merely by being referenced as a validator or filter; the slot it occupies does not widen its token set. The client engine registers **only** `ctx.t` (gated on `ReadI18n`) as a host function — there is no `db.*` / write surface on the client at all (writes go through the GraphQL `DataSource` adapter, untouched here). The two example scripts are `ComputeOnly+ReadI18n` (validator) and `ComputeOnly` (filter), well within these gates. **This item extends where already-sandboxed scripts run; it does not modify the sandbox, engine, capability model, or host-function set.** No new escalation/bypass vector vs. the formatter path.

### 2. Input to scripts — injection / type-confusion — N/A (data, not code)

Row fields and the synthesized `selectedStackId` reach the engine via `ScriptInputs { value, fields }` → `scope.push_constant("value"/"fields", json_to_dynamic(...))` in `RhaiEngine::run`. They are pushed as **Rhai data values**, never compiled as source. `eval` and `import` symbols are `disable_symbol`'d in `configure_strict`, so a field whose content is a string of Rhai source cannot be turned into executable code. `selectedStackId` is constructed from a typed `FilterPredicate::NumberEquals { value }` (an `f64`) via `serde_json::Number::from_f64`, not from free text — no type-confusion path. No untrusted-data-as-code risk.

### 3. DoS / resource — per-row script execution — acceptable (bounded by existing limits)

`LocalSource::passes` runs the filter script once per row, over a single already-fetched result page (`LocalSource` operates on `Rc<Vec<Entity>>` in memory, not the whole table). Each individual run is bounded by the existing sandbox limits, unchanged by this item:

- `engine.set_max_operations(50_000)` — a hard deterministic CPU bound per run that aborts runaway/infinite-loop scripts (`ErrorTooManyOperations`);
- optional manifest `timeout_ms` wall-clock deadline (native `Instant`; WASM `performance.now()`, degrading safely to no-op if no time source — operation limit then remains the sole but still-present bound);
- optional `memory_kb`-derived string/array/map size caps.

Worst case is bounded by `page_size × 50_000` operations. The per-row cost is the documented, intended design of a row-predicate filter; a buggy/malicious script can make filtering slow on one client's own page but cannot escape the operation/deadline limits or affect the server or other users (all execution is client-side CSR/WASM). Advisory, not blocking: there is no aggregate cross-row budget, so a script near the per-run ceiling multiplied by a large `page_size` could noticeably slow the user's own browser tab — a self-inflicted client-side UX cost given operator-authored scripts, not a security exposure.

### 4. Error handling / info-leak — fail-open is a correctness concern, not a security one here

Both new branches are fail-open on `Fallback` / `NotAScriptId` / non-`Bool` return:
- validator: any non-`Bool(false)` outcome ⇒ `None` (no blocking message);
- filter: any non-`Bool` outcome ⇒ `true` (row kept).

For the **validator**, fail-open means a script that errors or returns a non-bool will silently *accept* the row rather than block it. In a general security model "validation that silently passes" can be a concern — but here it is **not** a privilege/trust boundary: this is client-side, advisory editor validation of operator-authored business rules (e.g. SOLL/HABEN balance). The server does **not** rely on this client validator for authorization or data-integrity enforcement; server-side CRUD/permission checks (`require_permission`) are independent and untouched. So fail-open here is a correctness/UX behavior (invalid-but-not-security-sensitive data could be submitted and would still pass any real server-side checks), consistent with the formatter path's fallback philosophy and explicitly documented in the code. Not a security issue.

Error messages: failures go through `push_fallback(reason)` into the audit queue. The `reason` carries `script_id`, slot name, and a `format!("{e:?}")` of the engine error — i.e. script identity and Rhai engine diagnostics, not row data or secrets. No sensitive-data leak into logs. The blocking validator message uses a static Fluent key (`validation.script`), carrying no field content.

### 5. Wire / GraphQL — operator-authored metadata, not attacker-supplied — confirmed safe

`validator_id` and `filter_id` originate from the loaded `--data-dir` (`data::columns_for` → `convert_column`), surfaced as the **output-only** `entity_columns` GraphQL field (`require_permission(Read)`-gated). There is no `ColumnMeta` `InputObject` and no mutation argument that accepts a `validator_id` / `filter_id`; an untrusted client cannot inject a `validator_id` at runtime to point execution at an arbitrary script. The set of runnable scripts is further constrained to entries already present in the `ScriptRegistry` and in `Active` state. The wire format is pinned (`shared/tests/column_meta_wire_format.rs`) including backward-compat (missing `validatorId` ⇒ `None`).

### 6. Secrets / .env / DB / auth / crypto surface — none touched — confirmed

No `.env`, credential, DB-connection, session/auth, or crypto code is in the diff. `Cargo.lock` / `client/Cargo.toml` add only `futures = "0.3"` as a **dev-dependency** (used by the new async filter tests); not shipped in the client binary. Server-side auth (`require_permission`, session handling) is unchanged.

## Conclusion

The change is a thin, additive activation of already-sandboxed script execution at two new, capability-gated call sites that reuse the existing `lookup_provider` + `Sandbox` + Rhai-engine machinery verbatim. No new capability-escalation or sandbox-bypass vector, no code-injection path (inputs are data, `eval`/`import` disabled), DoS bounded by the existing per-run operation/deadline limits, and `validator_id`/`filter_id` are operator-authored output-only metadata that an untrusted client cannot inject. Fail-open on validator error is a correctness/UX property, not a trust-boundary weakness (server-side checks are authoritative and untouched). No secrets/auth/crypto surface is touched.

**Verdict: cleared.**

### Advisory (non-blocking)
- Per-row filter scripts have no aggregate cross-row operation budget; with a large `page_size` and a script near the 50k-op ceiling, the user's own browser tab could slow noticeably (self-inflicted, client-side only). Consider an aggregate budget or page-size cap if untrusted scripts ever become a goal.
- Validator fail-open means client-side validation silently passes on script error; acceptable only because the server is the authoritative validator. Keep that invariant — do not let any server-side write path start trusting this client validator.
