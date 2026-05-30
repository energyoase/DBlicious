# Code Review — Q0014: `validator_id`-Slot + `script:`-Prefix-Filterpfad

**Scope:** `c45abc6..0774e1b` (9 implementation commits, 21 files, +548/-3)
**Reviewer:** Claude (code-review skill, adapted to local-diff + notes-file output)
**Date:** 2026-05-31
**Verdict:** approve

---

## Summary

Stage-2 framework work that makes Q0013's dormant metadata live. Two locked sub-scopes:
- **Lücke A:** additive `validator_id: Option<String>` on `shared::ColumnMeta` + `FieldTypeDefaults`, threaded through server GraphQL re-wrap and client `COLUMNS_QUERY`/deser, plus a new `ColumnMeta` wire-pin and a live `script:` validator path through the real `ValidationSystem::run`.
- **Lücke B:** `script:`-aware per-row boolean predicate branch in `LocalSource::passes`, a `filters/mod.rs` guard so a `script:` filterId renders no built-in widget, and an end-to-end `LocalSource` filter test.

The change is clean, additive, well-tested against the real Rhai engine, and respects the documented deferrals. No blocking issues found.

---

## Detailed findings

### 1. Wire-contract correctness & consistency — PASS

- `shared/src/lib.rs`: `validator_id` added with `#[serde(default, skip_serializing_if = "Option::is_none")]`, mirroring `formatter_id`. Serializes camelCase `validatorId` (struct uses `#[serde(rename_all = "camelCase")]` at the type level — confirmed via the new wire-pin asserting `"validatorId"`).
- `shared/src/settings.rs`: `FieldTypeDefaults.validator_id` and `allowed_validator_ids` added with the same skip-if-empty/None guards, mirroring the formatter analogues.
- `server/src/schema.rs` GraphQL re-wrap and `server/src/data.rs::convert_column` both thread `validator_id` through. `client/src/graphql/queries.rs`: `COLUMNS_QUERY` requests `validatorId`, `RawColumnMeta` carries `#[serde(default)] validator_id`, and `fetch_columns` maps it through. Server-emit and client-decode agree.
- **New wire-pin** `shared/tests/column_meta_wire_format.rs` correctly pins all three required properties: camelCase roundtrip, `None`→skipped, and missing-field→`None` (proving additivity). Also covers `FieldTypeDefaults` roundtrip + empty default. Strong pin.
- **DATA_DIR_FORMAT correctly NOT bumped** — stays `1` (`shared/src/lib.rs:20`, untouched by the diff). Correct per CLAUDE.md: additive loader changes do not increment the constant.

### 2. Implementation-shape decisions — PASS

- `ScriptRegistry` is `#[derive(Debug, Default)]` — **not** `Clone` (`client/src/script/registry.rs:25`). `Arc<ScriptRegistry>` is therefore the correct sharing mechanism, used consistently in `LocalSource`, `script_validator_task`, and both tests.
- `Arc<dyn HostApi>` is injected via constructor (`LocalSource::with_script_registry`, `script_validator_task(... host ...)`). `MockHostApi` is `testing`-feature-gated and appears in `data_source.rs` **only in a doc comment** (line 210) — it does NOT leak into production code. Verified by grep: the only `MockHostApi` constructions in `client/src` are under `#[cfg(test)]`/test modules; production `data_source.rs`/`script_task.rs` reference only `dyn HostApi`. `RenderHost` is named as the production injectee in the doc comments; actual injection happens at the deferred wiring site.
- `client/Cargo.toml`: `futures = "0.3"` added as a **dev-dependency** (used by `futures::executor::block_on` in the integration test); `shared` already carried `features=["testing"]` as a dev-dependency. Production build stays free of both. Cargo.lock change is the matching additive entry.

### 3. `LocalSource::passes` script branch — PASS

- `client/src/components/table/data_source.rs`: the new branch fires only when `col.filter_id` starts with `SCRIPT_PREFIX`. On the script path it calls `script_predicate` and `continue`s (skipping the built-in ops), so the non-script ops path is byte-for-byte unchanged → no regression. Confirmed by the `plain_integer_still_gets_number_range` test in `filters/mod.rs` and the existing ops tests still passing (87/108 suites green per verification log).
- When `scripts`/`host` are `None` (the `LocalSource::new` path, which is what production currently constructs), the script branch `continue`s and lets the row through — i.e. a `script:` filter with no registry is inert, not an error. Sensible default.
- `script_predicate` maps `FilterPredicate::NumberEquals { value }` into `selectedStackId` injected into a cloned `fields` map, sets `value: Null`, runs `lookup_provider(ProviderSlot::Filter, ...)`. Error handling: only `LookupResult::Ok { value: Bool(b) }` is honored; Fallback / NotAScriptId / non-Bool all return `true` ("do not exclude"). This is the documented fail-open convention and matches the validator path's mirror convention (fail-open = "validator not active").
- The `d2v_stack_filter.rhai` script reads `fields.selectedStackId` / `fields.stackId` and the sentinel `-1`/`()` = "all stacks" — exactly the contract the predicate injects.

  **Non-blocking observation (verified, benign):** `selectedStackId` is injected via `serde_json::Number::from_f64(value)`, which produces a float-backed JSON number even for whole values (`from_f64(2.0).as_i64()` returns `None` — empirically confirmed). `engine::json_to_dynamic` therefore maps it to a Rhai `FLOAT(2.0)`, while the row's `stackId` (an integer literal in seed data) maps to a Rhai `INT(2)`. The script's `row == sel` and `sel == -1` thus compare INT vs FLOAT. **Both end-to-end tests pass**, so Rhai 1.24 performs cross-type numeric equality here — the predicate is correct as written. The reliance on Rhai's INT/FLOAT `==` coercion is a latent fragility (a future engine in strict-numeric mode, or a non-integer stackId domain, could surprise), but it is correct against the pinned engine and the integer-stackId domain. Optional hardening: normalize whole floats to integers before injection, or round in the script. Not blocking.

### 4. `filters/mod.rs` guard — PASS

- `resolve_filter_id` gets an early `return None` when `column.filter_id` starts with `SCRIPT_PREFIX`, placed before the "server-Vorgabe gewinnt" registry lookup. So a `script:` filterId no longer falls through to a range/text widget (it isn't a registered built-in id, so previously it would have hit the field-type default and mis-rendered a number-range). The early-return is correct and minimal.
- Covered by the new `script_filter_renders_no_builtin_widget` test (→ `None`) and `plain_integer_still_gets_number_range` (→ `number_range::ID`, proving no regression).

### 5. The +1-line struct-literal updates — PASS / COMPLETE

All `ColumnMeta` literals across the workspace got `validator_id: None` and all `FieldTypeDefaults` paths default cleanly:
- Production: `server/src/data.rs` (threads `c.validator_id`), `server/src/schema.rs` (field added), `client/src/builder/project.rs` (`None` — correct, builder has no validator UI yet), `client/src/components/registries/resolve.rs` (adds `"validator"` arms to both column-override and field-type-default match, mirroring formatter — correct).
- Tests: `server/src/reference.rs`, `server/tests/int_enum.rs`, `server/tests/reference_resolver.rs`, `client/src/components/table/builder_preview.rs`, `client/src/components/table/column_editor/mod.rs` — all `None`.

`None`/`Vec::new()` are the right explicit values for these literals (none of these sites produce a validator). The compiler would reject any missed literal, so completeness is structurally guaranteed; the chosen values are also semantically sensible. No site was missed.

### 6. Project conventions — PASS

- German comments throughout the new code (`script_task.rs`, the `LocalSource` additions, the wire-pin header, the filter guard). Consistent with the repo.
- No `DesignSystem` bypass — the change touches data/validation/filter logic only, no component styling.
- `FieldType` tagged enum untouched — no change to the `#[serde(tag = "kind")]` contract; the wire-pin's `fieldType: { "kind": "integer" }` confirms the existing form.
- `resolve.rs` adds `"validator"` to the resolution chain with **no client fallback** (the `_ => None` for the static fallback layer), matching the spec note "Q0014 kennt keine statischen Validator-IDs". Tested by `validator_has_no_client_fallback`.

### 7. Deferrals — correctly out of scope (NOT flagged)

- The `editor.rs` on-save validator wiring is explicitly deferred to a follow-up (spec §"Lücke A" line 70, §"Out of scope" lines 245–253). Its absence is intended.
- The `EntityListPage` `RemoteSource → LocalSource` live-wiring of the script filter is likewise deferred (spec lines 181–187, 253). Consequently `with_script_registry` has no production call site yet — this is the framework being put in place, not a gap. `with_script_registry` and `script_validator_task` are `pub` and reachable for the follow-up.

---

## Verification performed by reviewer

- Re-ran `cargo test -p client --test local_source_script_filter` (target-test) → 2 passed. Confirms the end-to-end Rhai filter path works including the INT/FLOAT comparison.
- Empirically confirmed `serde_json::Number::from_f64(2.0).as_i64() == None` (the basis of the non-blocking observation in §3).
- Confirmed clean working tree after removing the throwaway probe.
- Static checks (fmt/clippy/full test matrix) per the ccm-execute verification log; not re-run here.

## Non-blocking suggestions

1. (§3) `script_predicate` injects the filter value as a float via `from_f64`, relying on Rhai 1.24's INT/FLOAT `==` coercion for correctness. Consider normalizing whole floats to integers before injecting `selectedStackId` (or rounding in the script) to make the predicate robust against engine numeric-strictness changes. Correct today; purely defensive.
2. (§3) During `global_search`, a `script:`-filter column is still run through `ops_for_named`/`matches_search` with the fallback ops (the script branch only guards the per-predicate loop, not the global-search loop). Benign today (unknown `script:` id → default ops), and global-search + script-filter is not in Q0014 scope, but worth a comment or a future guard if global search over script-filtered columns becomes a real path.
