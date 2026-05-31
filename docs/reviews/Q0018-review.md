# Q0018 Code Review — Q0014 Script-Filter Hardenings (H1/H2/H3)

- **Diff scope:** `git diff 82e52f5..7d8d8f9`
- **Files (exactly 2):** `client/src/components/table/data_source.rs`, `client/tests/local_source_script_filter.rs`
- **Spec:** `docs/superpowers/specs/Q0018-q0014-haertungen-scriptfilter-numerik-globalsearch-budget-design.md`
- **Plan:** `docs/superpowers/plans/Q0018-q0014-haertungen-scriptfilter-numerik-globalsearch-budget.md`
- **Verdict:** approve

## Scope verification

- `git diff --name-only 82e52f5..7d8d8f9` returns exactly the two expected files. No other files touched.
- No change to sandbox/capability model, `json_to_dynamic`, `lookup_provider`, `set_max_operations`, or any server-side filtering. Matches spec §"Out of Scope" / DoD last checkbox.
- Comments are German; identifiers English — consistent with CLAUDE.md convention.

## H1 — INT-typed filter-value injection (correctness)

`script_predicate` now normalizes whole-number `f64` filter values to an
int-typed `serde_json::Number` before injecting `selectedStackId`:

```rust
let sel_value = match selected {
    Some(v) if v.is_finite() && v.fract() == 0.0
        && v >= i64::MIN as f64 && v <= i64::MAX as f64 =>
        Value::Number((v as i64).into()),
    Some(v) => serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null),
    None => Value::Null,
};
```

**Premise verified against `82e52f5:client/src/script/engine/rhai.rs::json_to_dynamic`:**
the marshaller does `if let Some(i) = n.as_i64() { Dynamic::from(i) /* INT */ } else { Dynamic::from(n.as_f64()...) /* FLOAT */ }`.
An int-typed `Number` (built via `i64::into()`) yields `Some` from `as_i64()` → Rhai INT;
a `from_f64` Number yields `None` → Rhai FLOAT. So the fix achieves exactly the
INT marshalling claimed, symmetric to the row `stackId` (which originates from
`serde_json::json!(<i64>)`, also int-typed).

**Edge cases — all handled correctly (no silent wrong-typing):**
- Non-whole (`2.5`): `fract() != 0.0` → falls to `from_f64` → stays FLOAT. Correct.
- `NaN`/`inf`: `is_finite()` guard fails → `from_f64` arm; `from_f64(NaN/inf)`
  returns `None` → `Value::Null` (= "no stack selected" → script returns true).
  Correct and matches spec.
- Out-of-`i64`-range whole floats (e.g. `1e30`): range guard fails → `from_f64`
  arm → FLOAT, avoiding UB from the `as i64` saturating cast. Correct; the
  explicit `>= i64::MIN as f64 && v <= i64::MAX as f64` guard is the right
  defensive bound.
- Negative whole (sentinel `-1.0`): `fract()==0.0`, in range → INT `-1`. The
  script's `sel == -1` branch fires → all rows pass. Verified by
  `h1_sentinel_minus_one_passes_all_as_int`.
- Non-`NumberEquals` predicates: `selected = None` → `Value::Null`. Other
  predicate kinds are unaffected (they never reach this int path). No regression.

**Does not break other predicates:** the only behavioral change is the marshalled
numeric *type* of `selectedStackId`; for any consumer that relied on Rhai's
INT/FLOAT `==` coercion (the existing `d2v_stack_filter.rhai`, `row == sel`),
the result is unchanged (existing `selected_stack_includes_only_matching_rows`
stays green). The fix only removes the dependency on that coercion.

**H1 test red-before-fix:** `TYPED_STACK_SRC` asserts `type_of(row) == type_of(sel)`.
Pre-fix `sel` is FLOAT (`f64`) and `row` is INT (`i64`) → type mismatch → empty
result → test (expecting `["b"]`) fails. Genuinely red before the fix. ✓

## H2 — global_search skips `script:`-filter columns

The `columns.iter().any(...)` global-search loop now returns `false` early for
any column whose `filter_id` starts with `SCRIPT_PREFIX`:

```rust
if col.filter_id.as_deref().is_some_and(|fid| fid.starts_with(SCRIPT_PREFIX)) {
    return false;
}
```

- **No script run here** (matches spec intent / keeps H3 cheap) — the guard
  returns before `ops_for_named`/`matches_search`.
- **No over-skip of legit text search:** the guard is keyed strictly on
  `filter_id` carrying the `script:` prefix. Columns with `filter_id: None`
  (e.g. the `name` Text column) or a non-script named filter id are not skipped
  and still participate. `any()` returning `false` for the script column only
  removes that column's contribution; other columns are still evaluated.
- Confirmed by both tests: `h2_global_search_skips_script_columns` (needle `"2"`
  no longer matches the raw `stackId` of the script column) and
  `h2_global_search_still_hits_text_column` (needle `"appl"` still hits `name`).
  The first is red-before-fix (pre-fix the script column's raw `field_type`
  Integer ops would match `stackId==2`). ✓

## H3 — per-fetch run budget (`ScriptBudget`)

- **Threading correct:** `ScriptBudget::new()` is created once per `fetch`
  (inside the async block), passed `&mut` through the filter closure into
  `passes`. Each script-filter predicate decrements `runs_left` exactly once
  *before* running the script (guarded by `runs_left == 0` check). Single
  predicate per row in the tests → one decrement per row.
- **Fail-open is panic-free:** on exhaustion the code does `continue` (skip the
  predicate, row passes), consistent with the existing "no registry/host =>
  pass" semantics directly below it. No `unwrap`/index/arithmetic that can
  panic; `runs_left -= 1` only runs when `runs_left > 0`, so no underflow.
- **Counting is exact:** with `n = budget + 50` excluding rows, the first
  `budget` rows consume the budget and are excluded by the script; the
  remaining `50` hit `runs_left == 0` → pass. `total_count == 50 == n - budget`,
  satisfying both `> 0` and `>= n - budget`. Without the budget `total_count`
  would be 0, so the test truly exercises the new path (red-before-fix). ✓
  `h3_within_budget_no_regression` confirms strict behavior when `n <= budget`.
- **One-time warn is genuinely one-time:** guarded by `if !budget.warned`,
  `warned` flips true on first exhaustion. Because `budget` is per-`fetch`
  (not a global static), the warn fires at most once per fetch and correctly
  resets for the next fetch. Uses `log::warn!` (not the per-line spam path).
  Not a trust boundary — documented as such in the module doc and the
  const doc-comment.

### debug_assertions override soundness (key concern)

```rust
const MAX_SCRIPT_FILTER_RUNS_PROD: usize = 5_000;          // #[allow(dead_code)]
pub const TEST_MAX_SCRIPT_FILTER_RUNS: usize = 200;
#[cfg(debug_assertions)]      const MAX_SCRIPT_FILTER_RUNS = TEST_MAX_SCRIPT_FILTER_RUNS;
#[cfg(not(debug_assertions))] const MAX_SCRIPT_FILTER_RUNS = MAX_SCRIPT_FILTER_RUNS_PROD;
```

- Verified `7d8d8f9:Cargo.toml` `[profile.release]` does **not** set
  `debug-assertions = true`. Cargo's default leaves `debug_assertions` **off**
  in `release` and **on** in `dev`/`test`. Therefore:
  - Production WASM (`trunk build --release`) → `debug_assertions` off →
    active budget = **5_000** (prod). The caller's concern ("production ships
    5_000, not 200") holds. ✓
  - `cargo test -p client` (dev/test profile) → `debug_assertions` on →
    active budget = **200**, keeping the H3 test < 1 s.
- `#[allow(dead_code)]` on `MAX_SCRIPT_FILTER_RUNS_PROD` is necessary and
  correct: under `debug_assertions` the `cfg(not(debug_assertions))` reference
  is compiled out, so the const is unused in the test profile and would
  otherwise break the `clippy -D warnings` baseline.
- Minor non-blocking nuance: a *debug* (non-release) production serve
  (`trunk serve` without `--release`) would also ship 200. That is a dev build,
  not a shipped artifact, so acceptable. Tying the threshold to `debug_assertions`
  rather than `cfg(test)` is a deliberate, documented choice (the integration
  test runs as a separate crate where `cfg(test)` of the lib would not apply);
  `debug_assertions` is the correct lever for an integration-test-visible
  override. No change required.

## CLAUDE.md compliance

- German comments / English identifiers: ✓
- No hard-coded styles / DesignSystem bypass introduced (not applicable to this
  file).
- No change to the FieldType wire format, SortDirection casing, or shared
  contract.
- Module doc updated to enumerate all three resource limits (per-run
  `set_max_operations`, per-run `timeout_ms`, per-fetch `MAX_SCRIPT_FILTER_RUNS`)
  — matches DoD.

## Definition-of-Done check

All DoD checkboxes are satisfied by the diff: H1 normalization + comment; H1
typed test; H2 guard + comment; H2 test; H3 budget in fetch pass + fail-open +
one-time warn + module doc; H3 fail-open test; scope confined to the two files
with no sandbox/capability/marshaller/lookup/op-limit changes.

## Non-blocking suggestions

1. The `debug_assertions`-tied budget means an un-optimized `trunk serve` dev
   build uses 200 runs, which could surprise a developer testing large local
   datasets. Purely informational; not worth a code change.
2. `(v as i64)` is guarded by the explicit range check so it is safe; if a
   future reader prefers self-documenting intent, `v as i64` could be written
   via a checked path, but the current guard is correct and clearer alongside
   the comment.

## Conclusion

All three hardenings are correct, scoped exactly as specified, free of panics,
and each backed by a test that is genuinely red before its fix. No blocking
issues. **approve.**
