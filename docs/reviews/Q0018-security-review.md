# Q0018 Security Review — Q0014 Script-Filter Hardenings (H1/H2/H3)

**Item:** Q0018 — three client-side hardenings extending Q0014's already-cleared `script:`-filter path (numeric int-typing, global_search script-column exclusion, per-fetch run budget).
**Diff scope:** `git diff 82e52f5..7d8d8f9`
**Files (exactly 2):** `client/src/components/table/data_source.rs`, `client/tests/local_source_script_filter.rs`
**Reviewer:** security-review (Claude Opus 4.8)
**Date:** 2026-05-31
**Verdict:** cleared — 0 blocking security issues.
**Context (not a security pass):** code review `docs/reviews/Q0018-review.md` (approve, 0 blocking).
**Origin of H3:** Q0014 security-review `docs/reviews/Q0014-security-review.md` advisory #1 (per-row scripts had no aggregate op budget).

## Scope verification

`git diff --name-only 82e52f5..7d8d8f9` returns exactly the two files above. Confirmed against source at the reviewed SHA / working tree:

- **No sandbox / capability-model change.** `Sandbox::new`, `RhaiEngine::run`, `apply_limits` (`set_max_operations(50_000)`, `set_max_string_size/array/map`), the `disable_symbol("eval"/"import"/"print"/"debug")` strict config, `lookup_provider`, and the host-function set are all untouched. This item only adds logic *around* the already-gated call sites in `LocalSource`.
- **No server-side filtering added.** `QueryRoot::entities` still ignores sort/filter; all three hardenings live purely in the client `LocalSource` in-memory path. No GraphQL, no wire-format, no `shared` contract change.
- **`validator_id` / `filter_id` remain operator-authored.** They originate from the loaded `--data-dir` (`data::columns_for`), surfaced as the output-only, `require_permission(Read)`-gated `entity_columns` field. There is no input object / mutation argument accepting a `filter_id`; an untrusted client cannot inject one to redirect script execution. Runnable scripts are further constrained to `Active` entries in the `ScriptRegistry`. Unchanged by this diff.
- **No secrets / .env / auth / session / crypto / network surface touched.** The diff adds only in-memory control flow, a `log::warn!`, two `const`s, and a small struct. The test file adds no new shipped dependency (`futures` is already a dev-dependency from Q0014).

## H1 — INT-typed filter-value injection — type-confusion / injection / cast-UB: N/A, safe

`script_predicate` normalizes a whole-number `f64` `NumberEquals` value to an int-typed `serde_json::Number` before injecting it as `fields.selectedStackId`:

```rust
let sel_value = match selected {
    Some(v) if v.is_finite() && v.fract() == 0.0
        && v >= i64::MIN as f64 && v <= i64::MAX as f64 =>
        Value::Number((v as i64).into()),
    Some(v) => serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null),
    None => Value::Null,
};
```

- **No injection / code-as-data risk.** Verified against `client/src/script/engine/rhai.rs`: `selectedStackId` reaches the engine via `ScriptInputs.fields` → `json_to_dynamic` → `scope.push_constant`. It is pushed as a **Rhai data value**, never compiled as source. `eval`/`import` are `disable_symbol`'d in the strict config. The value being INT vs FLOAT does not change this — it is data either way. No type-confusion path that crosses a trust boundary: the marshalled type only affects whether the operator's own `row == sel` comparison short-circuits on Rhai's INT/FLOAT coercion. The change *removes* a dependency on that coercion (symmetric to the row `stackId`, which `json_to_dynamic` already marshals INT via `as_i64()` → `Dynamic::from(i)`), it does not introduce a new one.
- **No `as i64` UB.** The saturating `as i64` cast is reached **only** inside the guarded match arm, which already requires `is_finite()` (excludes NaN/±inf) and `v >= i64::MIN as f64 && v <= i64::MAX as f64` (excludes out-of-range whole floats like `1e30`). NaN/inf/out-of-range values all fall to the `from_f64` arm → FLOAT, or `None` → `Value::Null`. No path produces an i64 from a value the cast would clamp. Confirms the code-review "guarded" finding.
- **Outcome:** correctness/UX hardening (the operator's filter now matches reliably without relying on Rhai numeric coercion); zero security delta vs. the Q0014-cleared path.

## H2 — global_search excludes `script:`-filter columns — confidentiality: N/A

The `global_search` `columns.iter().any(...)` loop now returns `false` early for any column whose `filter_id` starts with `SCRIPT_PREFIX`, before `ops_for_named`/`matches_search`.

- **Not a boundary; availability/correctness only.** Skipping a `script:` column *removes* that column's raw-field value from the free-text needle match. The change can only ever cause the global search to return **fewer or equal** matches (the script column's raw `stackId` no longer spuriously matches a needle like `"2"`). It cannot widen what is shown, cannot bypass any per-row script predicate (explicit `predicates` still run via `passes`), and exposes nothing that wasn't already visible — the column's rendered cell content is governed by the formatter/display path, not by whether the raw value participates in global search. No confidentiality or authorization implication.
- **No new script execution.** The guard returns before any `ops_for_named`/script run, so H2 adds no execution surface (and keeps H3's budget cheap). Confirmed: `script_predicate` is not reachable from the global_search loop.

## H3 — per-fetch run budget (`ScriptBudget`) — DoS-hardening: sound, fail-open posture correct

This is the implementation of Q0014 security-review advisory #1. `ScriptBudget::new()` is constructed once per `fetch`, threaded `&mut` through the filter closure into `passes`. Each `script:`-filter predicate, when registry+host are present, checks `runs_left == 0` (→ `continue`, fail-open) else decrements `runs_left` and runs the script.

**Does the budget actually bound the resource it claims?**
- **Per-fetch, deterministic, WASM-neutral — yes.** The counter is a plain `usize` decremented once before each script run, with a `runs_left == 0` short-circuit. It is independent of any time source, so it is identical on native and WASM (unlike the optional `timeout_ms` wall-clock deadline, which degrades to no-op without a time source). It bounds the **number** of script runs per `fetch` to `MAX_SCRIPT_FILTER_RUNS`; combined with the unchanged per-run `set_max_operations(50_000)`, the aggregate per-fetch CPU is now hard-bounded at `MAX_SCRIPT_FILTER_RUNS × 50_000` ops regardless of `page_size` / dataset size. Previously the per-fetch bound scaled with `page_size` unbounded. This closes exactly the self-inflicted-DoS gap the Q0014 advisory flagged.
- **Active budget is correct per build.** `MAX_SCRIPT_FILTER_RUNS` = `5_000` under `#[cfg(not(debug_assertions))]` (release WASM — `[profile.release]` does not enable `debug-assertions`), `200` under `debug_assertions` (dev/test). Production ships 5_000, the integration test runs at 200 for speed. `#[allow(dead_code)]` on the prod const is required so the test profile (where it is `cfg`'d out) stays clippy-clean. No arithmetic underflow: `runs_left -= 1` runs only when `runs_left > 0`.

**Is fail-open the right security posture?**
- **Yes — fail-open here is availability-only, not a confidentiality or authorization leak.** On budget exhaustion the excess rows pass the filter *unfiltered*, i.e. the user sees **MORE** rows of their own already-fetched page, never fewer-and-hidden or cross-tenant data. `LocalSource` operates on data already delivered to this client by the GraphQL `DataSource` (itself permission-gated server-side); the budget governs only client-side display filtering of rows the user is already authorized to see. Passing them unfiltered cannot reveal anything the server did not already authorize. It is a UX/availability tradeoff (the filter silently becomes incomplete past the budget, signalled by a one-time `log::warn!`), consistent with the existing "no registry/host ⇒ row passes" semantics directly below it. Fail-**closed** (hiding excess rows) would arguably be worse UX with no security benefit, since the rows are not sensitive relative to what the client already holds.
- **Self-inflicted, not attacker-reachable — confirmed.** The trigger is the operator's own `script:`-filter (operator-authored `filter_id` from `--data-dir`, an `Active` registry script) running in the operator's own browser tab over their own fetched page. There is no remote/cross-user path: filters are not attacker-supplied, execution is client-side CSR/WASM, and the budget protects the same user's tab from their own expensive script × large page. No trust boundary is crossed.
- **One-time warn is bounded.** `budget.warned` flips on first exhaustion (per-`fetch`, not a global static), so the `log::warn!` fires at most once per fetch — no per-row log spam (which could itself be a minor DoS). Correct.

## Conclusion

All three hardenings are additive, client-side, and confined to the in-memory `LocalSource` filter path. None modifies the sandbox, capability model, host-function set, `json_to_dynamic` marshaller, `lookup_provider`, the per-run op/deadline limits, the wire format, or any server-side logic. H1 is a data-only int-typing with a guarded, UB-free `as i64` cast (no injection, inputs remain Rhai data with `eval`/`import` disabled). H2 only narrows free-text search results (availability/correctness, no confidentiality delta). H3 adds a deterministic, WASM-neutral per-fetch run budget that hard-bounds aggregate script CPU — directly implementing Q0014's DoS advisory — and its fail-open posture exposes only more of the user's own already-authorized rows (availability tradeoff, not a leak), triggered solely by the operator's own script in their own tab. No secrets/.env/auth/crypto/network surface is touched. `filter_id`/`validator_id` remain operator-authored, output-only metadata an untrusted client cannot inject.

**Verdict: cleared.**

### Advisory (non-blocking)

1. H3 fail-open makes the filter *silently incomplete* past the budget (signalled only by a one-time `log::warn!` to the console). This is correct for availability, but if a future requirement needs filter completeness to be trustworthy, surface a visible UI indicator rather than relying on console warn. Not security-relevant today (operator-authored, self-inflicted).
2. The `debug_assertions`-tied budget means an un-optimized `trunk serve` dev build uses 200 runs, not 5_000. Dev-only artifact, informational; carried over from the code review.
3. Keep the Q0014 invariant intact: this remains a client-side display filter; do not let any server-side path start trusting the client filter result for authorization or data-integrity.
