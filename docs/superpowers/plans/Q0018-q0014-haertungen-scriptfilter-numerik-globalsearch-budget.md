# Q0018 — Q0014-Härtungen: script-Filter INT-Coercion + global_search-Guard + Per-Fetch-Run-Budget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Three small, non-blocking robustness hardenings in `client/src/components/table/data_source.rs` — make whole-number `script:`-filter values reach Rhai as INT (H1), explicitly skip `script:`-filter columns in the `global_search` loop (H2), and cap the number of `script:`-filter runs per `fetch` with a fail-open run-count budget (H3).

**Architecture:** All production changes live in one file (`data_source.rs`). H1 rewrites the `selectedStackId` injection in `script_predicate` to emit an integer-typed `serde_json::Value::Number` for whole floats (so `json_to_dynamic` marshals it to Rhai INT, matching the row's INT `stackId`). H2 adds an early-return guard inside the existing `global_search` `columns.iter().any(...)` closure. H3 threads a `&mut` run-count budget from the `fetch` filter-pass into `passes`/`script_predicate`; once exhausted, remaining rows pass-through (fail-open, consistent with the existing "no script ⇒ keep row" semantics) and a one-time `console::warn` fires. A `#[cfg(test)]` override shrinks the budget so the H3 test stays sub-second. No changes to the sandbox, `set_max_operations`, `json_to_dynamic`, or `lookup_provider`.

**Tech Stack:** Rust (WASM client crate `client`), `serde_json`, real Rhai engine via `shared::script`, `web_sys::console` for the warning. Tests extend `client/tests/local_source_script_filter.rs` (E2E with the real Rhai engine + real `lookup_provider`, driven through `LocalSource::fetch`).

**Spec:** `docs/superpowers/specs/Q0018-q0014-haertungen-scriptfilter-numerik-globalsearch-budget-design.md`

**Scope guard:** Touch **only** `client/src/components/table/data_source.rs` and `client/tests/local_source_script_filter.rs`. Do **not** modify the sandbox/capability model, `json_to_dynamic`, `lookup_provider`, `set_max_operations`, or the `examples/d2v/...` script.

**Commits / bookkeeping:** This plan is executed under the ccm-loop controller. **Do not `git commit`, do not touch any queue file, and do not write to the audit log** — the parent controller handles all commits and bookkeeping. Where a normal plan would commit, this plan has a **Checkpoint** marker instead: stop, report the green verification output, and let the controller decide.

**Windows / target-dir caveat:** A local dev `server.exe`/build may hold a file-lock on `target/`. Run all client tests with `--target-dir target-test` (gitignored). If you see `E0786`, `STATUS_STACK_BUFFER_OVERRUN`, or rlib link errors, that is the **Q0016 transient `target-test` cache corruption**, not a code bug — run `cargo clean --target-dir target-test` (or use a fresh target dir) and re-run. Do not "fix" it in code.

---

## Verified Ist-Stand (line anchors re-checked against real code on 2026-05-31)

The spec's anchors hold. Concretely, in `client/src/components/table/data_source.rs`:

| Spec anchor | Real location | Status |
|---|---|---|
| `LocalSource::passes` Z. 248–297 | `passes` at **lines 248–297** | exact |
| per-predicate `script:`-Branch Z. 260–271 | **lines 260–271** | exact |
| `global_search`-Schleife Z. 282–295 | **lines 282–295** | exact |
| `script_predicate` Z. 372–408 | **lines 372–408** | exact |
| `from_f64` injection Z. 384–388 | **lines 384–388** | exact |
| `fetch` filter-pass `items.iter().filter(...)` Z. 334–338 | **lines 334–338** | exact |
| `apply_limits` `set_max_operations(50_000)` (rhai.rs) | rhai.rs **line 78** | exact (read-only ref) |

Other verified facts the plan relies on:
- `SCRIPT_PREFIX` is already imported into `data_source.rs` (`use crate::script::provider_lookup::{lookup_provider, LookupResult, SCRIPT_PREFIX};`, line 32). H2 reuses it.
- `passes` is **private** to the `impl LocalSource` block and only called once, from `fetch` (line 336). Adding budget params is a purely internal signature change — no public-API impact.
- `script_predicate` is a module-private free function (line 372), called only from `passes` (line 263).
- The d2v filter script (`examples/d2v/scripts/d2v_stack_filter.rhai`) ends with `if sel == () || sel == -1 { true } else { row == sel }` — it relies on `==` INT/FLOAT coercion today. The H1 test embeds its **own** stricter script as a string (using `type_of`) so the real script stays untouched.
- The test helper `row(id, stack: i64)` inserts `stackId` via `serde_json::json!(stack)` → row `stackId` arrives in Rhai as **INT**. `filter_for_stack(sel: f64)` builds `FilterPredicate::NumberEquals { value: sel }`.
- `web_sys` is available to the `client` crate (WASM target). The H3 warning uses `web_sys::console::warn_1(&JsValue::from_str(...))`.

**No code-divergence from the spec's anchors was found.**

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `client/src/components/table/data_source.rs` | H1: int-typed `selectedStackId`. H2: skip `script:` columns in `global_search`. H3: `MAX_SCRIPT_FILTER_RUNS` budget threaded through `passes`/`script_predicate`, fail-open + one-time warn, module-doc update. | Modify |
| `client/tests/local_source_script_filter.rs` | Three new E2E tests (H1 INT-vs-FLOAT pin, H2 global_search skip, H3 fail-open budget) plus a small embedded-script helper. Existing two tests stay green (regression guard). | Modify |

No new files. No new dependencies.

---

## Design note on H3 wiring (read before Task 5)

`passes` is called inside a closure passed to `Iterator::filter`. A `&mut usize` captured by that closure is fine because `filter`'s closure is `FnMut`, but to keep both the remaining-count and the one-time-warn flag together and borrow-friendly, we pass a single `&mut ScriptBudget` struct:

```rust
/// H3 (Q0018): Aggregat-Budget fuer `script:`-Filter-Runs pro `fetch`.
struct ScriptBudget {
    /// Verbleibende erlaubte `script:`-Filter-Runs in diesem `fetch`.
    runs_left: usize,
    /// Wurde das Budget in diesem `fetch` schon ueberschritten?
    /// Dient dem einmaligen Warning (kein Per-Zeile-Spam).
    warned: bool,
}
```

`passes` takes `budget: &mut ScriptBudget`. Only the `script:`-predicate path consumes the budget; built-in ops and `global_search` are untouched by it. When `runs_left == 0` and a `script:`-predicate would run, `passes` **keeps the row** for that column (fail-open) and emits the one-time warning. The production constant is `MAX_SCRIPT_FILTER_RUNS = 5_000`; a `#[cfg(test)]` override shrinks it.

The spec's NEEDS-DECISION (test budget injection) is resolved by the stated default: a `cfg(test)` override of the constant. **No blocking ambiguity remains.**

---

## Task 1: H1 — failing test pinning INT-vs-FLOAT compare

**Files:**
- Test: `client/tests/local_source_script_filter.rs` (add helper + test; keep existing helpers)

- [ ] **Step 1: Add an embedded type-strict filter script + a source builder using it**

Append to `client/tests/local_source_script_filter.rs` (after the existing `unselected_stack_passes_all_rows` test, end of file). This embeds a **type-strict** script as a string (the real d2v script stays untouched), then builds a `LocalSource` whose `stackId` column points at it:

```rust
// --- H1 (Q0018): typstrenger INT-vs-FLOAT-Vergleich ---------------------

/// Wie `d2v_stack_filter`, aber typstreng: erzwingt, dass `selectedStackId`
/// als INT ankommt (`type_of(row) == type_of(sel)`). Mit dem alten FLOAT-Pfad
/// waere `type_of(sel) == "f64"` != `type_of(row) == "i64"` => leeres Ergebnis.
const TYPED_STACK_SRC: &str = "\
let sel = fields.selectedStackId;\n\
let row = fields.stackId;\n\
if sel == () || sel == -1 { true } else { type_of(row) == type_of(sel) && row == sel }\n";

fn typed_stack_filter_script() -> Script {
    Script {
        id: ScriptId("typed_stack_filter".into()),
        kind: ScriptKind::Provider {
            slot: ProviderSlot::Filter,
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly],
            ..Default::default()
        },
        source: TYPED_STACK_SRC.into(),
        version: 1,
        state: ScriptState::Active,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    }
}

fn typed_columns() -> Vec<ColumnMeta> {
    vec![ColumnMeta {
        key: "stackId".into(),
        label_key: "k".into(),
        field_type: FieldType::Integer,
        sortable: true,
        filterable: true,
        comparator_id: None,
        filter_id: Some("script:typed_stack_filter".into()),
        editor_id: None,
        formatter_id: None,
        validator_id: None,
        action_ids: vec![],
    }]
}

fn typed_source() -> LocalSource {
    let reg = ScriptRegistry::new();
    reg.insert(typed_stack_filter_script());
    let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
        std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    LocalSource::with_script_registry(
        vec![row("a", 1), row("b", 2), row("c", 3)],
        &typed_columns(),
        std::sync::Arc::new(reg),
        host,
    )
}

#[test]
fn h1_selected_stack_reaches_script_as_int() {
    let src = typed_source();
    // filter_for_stack(2.0) baut NumberEquals { value: 2.0 } (f64). Mit dem Fix
    // wird der ganze Wert int-typisiert injiziert => Rhai-INT => type_of match.
    let ids = run(&src, filter_for_stack(2.0));
    assert_eq!(
        ids,
        vec!["b".to_string()],
        "selectedStackId muss als INT ankommen (type_of == row); vor dem Fix leer"
    );
}

#[test]
fn h1_sentinel_minus_one_passes_all_as_int() {
    let src = typed_source();
    // -1-Sentinel: jetzt INT -1; das `sel == -1`-Branch greift => alle 3 Zeilen.
    let ids = run(&src, filter_for_stack(-1.0));
    assert_eq!(ids.len(), 3, "INT -1 Sentinel => alle Stacks");
}
```

- [ ] **Step 2: Run the H1 tests to verify they FAIL**

Run: `cargo test -p client --target-dir target-test h1_`
Expected: `h1_selected_stack_reaches_script_as_int` **FAILS** (result empty, not `["b"]`) because the current `from_f64` path makes `selectedStackId` a Rhai FLOAT and `type_of(sel) == "f64" != "i64"`. (`h1_sentinel_minus_one_passes_all_as_int` may already pass via the `sel == -1` coercion branch — that's fine; the first test is the pin.)

If you instead hit `E0786`/`STATUS_STACK_BUFFER_OVERRUN`/rlib link errors, that's the Q0016 transient — `cargo clean --target-dir target-test` and re-run.

---

## Task 2: H1 — int-typed `selectedStackId` injection

**Files:**
- Modify: `client/src/components/table/data_source.rs:384-388` (the `sel_value` construction in `script_predicate`)

- [ ] **Step 1: Replace the `from_f64` injection with whole-float normalization**

In `script_predicate`, replace exactly these lines (currently 384–388):

```rust
    let sel_value = selected
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .unwrap_or(Value::Null);
    fields.insert("selectedStackId".into(), sel_value);
```

with:

```rust
    // H1 (Q0018): Ganzzahlige Filterwerte int-typisiert injizieren, damit
    // `selectedStackId` in `json_to_dynamic` als Rhai-INT marshalt — symmetrisch
    // zur Row-`stackId` (kommt aus `serde_json::json!(<i64>)` ebenfalls als INT).
    // So haengt der Vergleich `row == sel` nicht an Rhais INT/FLOAT-Coercion.
    // Echte Nachkommastellen / out-of-range / NaN/inf bleiben FLOAT bzw. Null.
    let sel_value = match selected {
        Some(v)
            if v.is_finite()
                && v.fract() == 0.0
                && v >= i64::MIN as f64
                && v <= i64::MAX as f64 =>
        {
            Value::Number((v as i64).into())
        }
        Some(v) => serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        None => Value::Null,
    };
    fields.insert("selectedStackId".into(), sel_value);
```

- [ ] **Step 2: Run the H1 tests to verify they PASS**

Run: `cargo test -p client --target-dir target-test h1_`
Expected: both `h1_*` tests **PASS**.

- [ ] **Step 3: Run the pre-existing script-filter tests (no regression)**

Run: `cargo test -p client --target-dir target-test --test local_source_script_filter`
Expected: `selected_stack_includes_only_matching_rows`, `unselected_stack_passes_all_rows`, and both `h1_*` tests PASS.

---

## Task 3: H2 — failing test for global_search skipping `script:` columns

**Files:**
- Test: `client/tests/local_source_script_filter.rs` (add a two-column source + test)

- [ ] **Step 1: Add a mixed-column source and the H2 test**

Append to `client/tests/local_source_script_filter.rs`. This builds a source with a normal text column (`name`, built-in ops) and a `script:`-filter column (`stackId`):

```rust
// --- H2 (Q0018): global_search klammert `script:`-Spalten aus -----------

fn named_row(id: &str, name: &str, stack: i64) -> Entity {
    let mut m = serde_json::Map::new();
    m.insert("name".into(), serde_json::json!(name));
    m.insert("stackId".into(), serde_json::json!(stack));
    Entity {
        id: id.into(),
        fields: m,
    }
}

fn mixed_columns() -> Vec<ColumnMeta> {
    vec![
        ColumnMeta {
            key: "name".into(),
            label_key: "k".into(),
            field_type: FieldType::Text,
            sortable: true,
            filterable: true,
            comparator_id: None,
            filter_id: None,
            editor_id: None,
            formatter_id: None,
            validator_id: None,
            action_ids: vec![],
        },
        ColumnMeta {
            key: "stackId".into(),
            label_key: "k".into(),
            field_type: FieldType::Integer,
            sortable: true,
            filterable: true,
            comparator_id: None,
            filter_id: Some("script:d2v_stack_filter".into()),
            editor_id: None,
            formatter_id: None,
            validator_id: None,
            action_ids: vec![],
        },
    ]
}

fn mixed_source() -> LocalSource {
    let reg = ScriptRegistry::new();
    reg.insert(stack_filter_script());
    let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
        std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    LocalSource::with_script_registry(
        vec![named_row("apple", "apple", 2), named_row("banana", "banana", 9)],
        &mixed_columns(),
        std::sync::Arc::new(reg),
        host,
    )
}

#[test]
fn h2_global_search_skips_script_columns() {
    let src = mixed_source();
    let filter = FilterCriteria {
        global_search: Some("2".into()),
        predicates: vec![],
    };
    let ids = run(&src, filter);
    // "2" passt auf KEINEN `name`; die `script:`-Spalte `stackId` (Rohwert 2 bei
    // "apple") ist aus der global_search ausgeklammert => kein Treffer.
    // Vor dem Fix wuerde "apple" ueber den stackId-Rohwert matchen.
    assert!(
        ids.is_empty(),
        "script:-Spalte darf nicht zur global_search beitragen, got {ids:?}"
    );
}

#[test]
fn h2_global_search_still_hits_text_column() {
    let src = mixed_source();
    let filter = FilterCriteria {
        global_search: Some("appl".into()),
        predicates: vec![],
    };
    let ids = run(&src, filter);
    // Treffer ueber die Text-Spalte `name` bleibt unveraendert.
    assert_eq!(ids, vec!["apple".to_string()]);
}
```

- [ ] **Step 2: Run the H2 tests to verify the skip test FAILS**

Run: `cargo test -p client --target-dir target-test h2_`
Expected: `h2_global_search_skips_script_columns` **FAILS** — current code searches the `stackId` raw value, so `global_search="2"` matches "apple". `h2_global_search_still_hits_text_column` already PASSES.

---

## Task 4: H2 — skip `script:` columns in the global_search loop

**Files:**
- Modify: `client/src/components/table/data_source.rs:282-295` (the `global_search` `columns.iter().any(...)` closure)

- [ ] **Step 1: Add the early-return guard inside the closure**

In `passes`, replace exactly the current global_search block (lines 282–295):

```rust
        if let Some(needle) = filter.global_search.as_deref().filter(|s| !s.is_empty()) {
            let hit = columns.iter().any(|(key, col)| {
                let value = entity.fields.get(key).cloned().unwrap_or(Value::Null);
                let ops = ops_for_named(
                    &col.field_type,
                    col.comparator_id.as_deref(),
                    col.filter_id.as_deref(),
                );
                ops.matches_search(&value, needle)
            });
            if !hit {
                return false;
            }
        }
```

with:

```rust
        if let Some(needle) = filter.global_search.as_deref().filter(|s| !s.is_empty()) {
            let hit = columns.iter().any(|(key, col)| {
                // H2 (Q0018): `script:`-Filter-Spalten sind boolesche Per-Row-
                // Praedikate ohne sinnvollen durchsuchbaren Textwert. Aus der
                // global_search explizit ausklammern (statt implizit ueber den
                // Roh-`field_type` zu suchen). Kein Skript-Run hier.
                if col
                    .filter_id
                    .as_deref()
                    .is_some_and(|fid| fid.starts_with(SCRIPT_PREFIX))
                {
                    return false;
                }
                let value = entity.fields.get(key).cloned().unwrap_or(Value::Null);
                let ops = ops_for_named(
                    &col.field_type,
                    col.comparator_id.as_deref(),
                    col.filter_id.as_deref(),
                );
                ops.matches_search(&value, needle)
            });
            if !hit {
                return false;
            }
        }
```

- [ ] **Step 2: Run the H2 tests to verify they PASS**

Run: `cargo test -p client --target-dir target-test h2_`
Expected: both `h2_*` tests PASS.

- [ ] **Step 3: Checkpoint — H1 + H2 verification**

Run, in order, and confirm green:

```
cargo fmt --manifest-path client/Cargo.toml
cargo clippy -p client --all-targets --target-dir target-test -- -D warnings
cargo test -p client --target-dir target-test --test local_source_script_filter
```

Expected: `fmt` makes no/whitespace-only changes; `clippy` clean (no warnings); all `local_source_script_filter` tests PASS. **Stop here and report** — the controller handles the commit.

---

## Task 5: H3 — failing test for the per-fetch run budget (fail-open)

**Files:**
- Test: `client/tests/local_source_script_filter.rs` (add a large-dataset fail-open test)

> The test relies on the `#[cfg(test)]` budget override added in Task 6. To keep TDD honest, write the test now; it will not compile/pass until the override + budget wiring exist, but its **logic** is fixed here. Because the override constant does not yet exist, this task's "run" step expects a **compile/assert failure**, which Task 6 resolves.

- [ ] **Step 1: Add the fail-open budget test**

Append to `client/tests/local_source_script_filter.rs`. The dataset is larger than the test-time budget; the script excludes **every** row (`selectedStackId = 999`, no row has stack 999). With a working budget, rows beyond the budget are passed through (fail-open) → non-empty result. Without the budget, the result would be empty.

```rust
// --- H3 (Q0018): Aggregat-Run-Budget greift (fail-open) -----------------

/// `client::components::table::data_source::TEST_MAX_SCRIPT_FILTER_RUNS` ist
/// der `#[cfg(test)]`-Override des Produktionsbudgets (klein, damit der Test
/// schnell bleibt). Re-exportiert fuer die Assertion-Schwelle.
use client::components::table::data_source::TEST_MAX_SCRIPT_FILTER_RUNS;

fn big_excluding_source(n: usize) -> LocalSource {
    let reg = ScriptRegistry::new();
    reg.insert(stack_filter_script());
    let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
        std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    // Alle Zeilen haben stack != 999 => das Filter-Skript schliesst jede Zeile
    // aus, solange das Budget reicht.
    let rows: Vec<Entity> = (0..n).map(|i| row(&format!("r{i}"), 1)).collect();
    LocalSource::with_script_registry(rows, &columns(), std::sync::Arc::new(reg), host)
}

#[test]
fn h3_budget_lets_excess_rows_pass_open() {
    let budget = TEST_MAX_SCRIPT_FILTER_RUNS;
    let n = budget + 50; // mehr Zeilen als Budget-Runs
    let src = big_excluding_source(n);
    // selectedStackId = 999 schliesst jede Zeile aus, solange ein Skript laeuft.
    let req = DataRequest {
        page: 1,
        page_size: (n as u32) + 10, // alle Zeilen auf einer Seite
        sort: None,
        filter: filter_for_stack(999.0),
    };
    let resp = futures::executor::block_on(src.fetch(req)).unwrap();
    // Ohne Budget waere total_count == 0 (alle ausgeschlossen). Mit Budget
    // werden die Zeilen jenseits des Budgets durchgelassen (fail-open).
    assert!(
        resp.total_count > 0,
        "Zeilen jenseits des Budgets muessen fail-open durchgelassen werden"
    );
    // Genauer: hoechstens `budget` Zeilen wurden tatsaechlich ausgewertet (und
    // ausgeschlossen); der Rest passiert. Also >= n - budget Durchlaeufer.
    assert!(
        resp.total_count >= (n - budget) as u64,
        "mindestens die {} Ueberschuss-Zeilen muessen durchgelassen werden, got {}",
        n - budget,
        resp.total_count
    );
}

#[test]
fn h3_within_budget_no_regression() {
    // Datenmenge <= Budget: Verhalten unveraendert (alle ausgeschlossen).
    let src = big_excluding_source(TEST_MAX_SCRIPT_FILTER_RUNS / 2);
    let ids = run(&src, filter_for_stack(999.0));
    assert!(ids.is_empty(), "innerhalb des Budgets bleibt der Filter strikt");
}
```

- [ ] **Step 2: Run the H3 tests to verify they FAIL (unresolved symbol)**

Run: `cargo test -p client --target-dir target-test h3_`
Expected: **compile error** — `TEST_MAX_SCRIPT_FILTER_RUNS` is not yet exported. This is the expected red state; Task 6 adds the export and wiring.

---

## Task 6: H3 — budget struct, constant, threading, fail-open + one-time warn, module-doc

**Files:**
- Modify: `client/src/components/table/data_source.rs` (module doc near top; new `ScriptBudget` + constants; `passes` signature + script-path; `fetch` filter-pass; `script_predicate` is **unchanged** in signature — budget logic stays in `passes`)

- [ ] **Step 1: Add the budget constants + `ScriptBudget` struct**

Add this block immediately **after** the `use` block (after line 33, before `#[derive(Debug, Clone, Default)] pub struct DataRequest`).

**Why `cfg(debug_assertions)` and not `cfg(test)`:** an integration test in `client/tests/` compiles `client` as a **dependency** — NOT with `cfg(test)` of the `client` crate — so a `#[cfg(test)]`-gated constant would be invisible and inert to it. `debug_assertions` is on for `cargo test`/dev builds and **off** for the size-tuned release WASM profile that ships, so the production budget (`5_000`) is what reaches users while tests run against the small override. The `pub const TEST_MAX_SCRIPT_FILTER_RUNS` is always compiled so the integration test can read the threshold as a constant.

```rust
/// H3 (Q0018): Aggregat-Budget fuer `script:`-Filter-Runs pro `fetch`.
/// Jeder einzelne Run ist bereits durch `set_max_operations(50_000)`
/// (rhai.rs::apply_limits) gedeckelt; dieses Budget begrenzt die *Anzahl*
/// der Runs ueber die Datenmenge, damit `n_rows x teures Skript` den eigenen
/// Browser-Tab nicht unbeobachtet blockiert. Reine Resource-Hygiene, **kein
/// Trust-Boundary** (self-inflicted, client-side). Bei Erschoepfung: fail-open
/// + einmaliges Warning.
const MAX_SCRIPT_FILTER_RUNS_PROD: usize = 5_000;

/// Kleiner Test-Override, damit der H3-Integrationstest in < 1 s laeuft.
/// `pub`, damit der Integrationstest die Schwelle als Konstante lesen kann.
/// Wird nur als aktives Budget verwendet, wenn `debug_assertions` aktiv ist
/// (also unter `cargo test`/dev-builds); Release-WASM nutzt den Prod-Wert.
pub const TEST_MAX_SCRIPT_FILTER_RUNS: usize = 200;

/// Das in diesem Build aktive Aggregat-Budget.
#[cfg(debug_assertions)]
const MAX_SCRIPT_FILTER_RUNS: usize = TEST_MAX_SCRIPT_FILTER_RUNS;
#[cfg(not(debug_assertions))]
const MAX_SCRIPT_FILTER_RUNS: usize = MAX_SCRIPT_FILTER_RUNS_PROD;

/// H3 (Q0018): pro `fetch` gefuehrtes Run-Budget fuer `script:`-Filter.
struct ScriptBudget {
    /// Verbleibende erlaubte `script:`-Filter-Runs in diesem `fetch`.
    runs_left: usize,
    /// Wurde das Budget in diesem `fetch` bereits ueberschritten? Dient dem
    /// einmaligen Warning (kein Per-Zeile-Spam).
    warned: bool,
}

impl ScriptBudget {
    fn new() -> Self {
        Self {
            runs_left: MAX_SCRIPT_FILTER_RUNS,
            warned: false,
        }
    }
}
```

> Rationale for `debug_assertions` instead of `cfg(test)`: an integration test in `client/tests/` compiles the `client` crate as a **dependency**, so crate-internal `#[cfg(test)]` items are NOT compiled in that build — the test could neither read the constant nor exercise the small budget. `debug_assertions` is on for `cargo test`/`cargo build` (dev profile) and **off** for the size-tuned release WASM profile, so the production budget (`5_000`) is what ships. `MAX_SCRIPT_FILTER_RUNS_PROD` is referenced by the `#[cfg(not(debug_assertions))]` arm, so it is not dead code in release builds; under dev builds add `#[allow(dead_code)]` if clippy flags it (see Step 6).

- [ ] **Step 2: Update the module doc to list all three script-resource bounds**

In the top-of-file module doc, after the existing `LocalSource`/`RemoteSource` bullet block (after line 9, before line 11's "Die Tabelle selbst ..."), insert:

```rust
//!
//! ## Script-Filter Resource-Schranken (Q0014 + Q0018-H3)
//! `script:`-Filter-Praedikate laufen pro Zeile durch die Rhai-Engine. Drei
//! Schranken begrenzen den Ressourcenverbrauch:
//!   (a) `set_max_operations(50_000)` **pro Run** (rhai.rs::apply_limits) —
//!       deckelt CPU-Ops eines einzelnen Skript-Laufs.
//!   (b) optionaler `manifest.timeout_ms` **pro Run** (sandbox.rs) als
//!       Wall-Clock-Deadline.
//!   (c) `MAX_SCRIPT_FILTER_RUNS` **aggregiert pro `fetch`** (Q0018-H3) —
//!       begrenzt die *Anzahl* der Filter-Runs ueber die Datenmenge; bei
//!       Erschoepfung fail-open (verbleibende Zeilen durchgelassen) + ein
//!       einmaliges `console::warn`. Reine Resource-Hygiene, kein
//!       Trust-Boundary.
```

- [ ] **Step 3: Extend `passes` to take the budget and consume it on the script path**

Change the `passes` signature (line 248) and the `script:`-predicate branch (lines 260–271). Current signature:

```rust
    fn passes(
        entity: &Entity,
        filter: &FilterCriteria,
        columns: &HashMap<String, ColumnLookup>,
        scripts: Option<&std::sync::Arc<ScriptRegistry>>,
        host: Option<&std::sync::Arc<dyn HostApi>>,
    ) -> bool {
```

becomes:

```rust
    fn passes(
        entity: &Entity,
        filter: &FilterCriteria,
        columns: &HashMap<String, ColumnLookup>,
        scripts: Option<&std::sync::Arc<ScriptRegistry>>,
        host: Option<&std::sync::Arc<dyn HostApi>>,
        budget: &mut ScriptBudget,
    ) -> bool {
```

Then replace the current script branch (lines 260–271):

```rust
            // Q0014: `script:`-Filter => Per-Row-Boolean-Praedikat statt Ops.
            if let Some(fid) = col.filter_id.as_deref() {
                if fid.starts_with(SCRIPT_PREFIX) {
                    if let (Some(reg), Some(h)) = (scripts, host) {
                        if !script_predicate(entity, &cf.predicate, fid, reg, h.clone()) {
                            return false;
                        }
                        continue; // Skript hat entschieden; Ops ueberspringen.
                    }
                    // Keine Registry/Host => Skript inaktiv => Zeile durchlassen.
                    continue;
                }
            }
```

with:

```rust
            // Q0014: `script:`-Filter => Per-Row-Boolean-Praedikat statt Ops.
            if let Some(fid) = col.filter_id.as_deref() {
                if fid.starts_with(SCRIPT_PREFIX) {
                    if let (Some(reg), Some(h)) = (scripts, host) {
                        // H3 (Q0018): Aggregat-Run-Budget pro `fetch`. Ist es
                        // erschoepft, laeuft kein weiteres Filter-Skript mehr;
                        // die Zeile wird durchgelassen (fail-open, konsistent
                        // mit der "kein Skript => Zeile durchlassen"-Semantik
                        // unten). Einmaliges Warning beim ersten Ueberschreiten.
                        if budget.runs_left == 0 {
                            if !budget.warned {
                                budget.warned = true;
                                web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(
                                    "dblicious: script-Filter Run-Budget pro fetch \
                                     erschoepft (MAX_SCRIPT_FILTER_RUNS); restliche \
                                     Zeilen werden ungefiltert durchgelassen.",
                                ));
                            }
                            continue; // fail-open
                        }
                        budget.runs_left -= 1;
                        if !script_predicate(entity, &cf.predicate, fid, reg, h.clone()) {
                            return false;
                        }
                        continue; // Skript hat entschieden; Ops ueberspringen.
                    }
                    // Keine Registry/Host => Skript inaktiv => Zeile durchlassen.
                    continue;
                }
            }
```

- [ ] **Step 4: Construct the budget in `fetch` and pass it into the filter closure**

Replace the current filter-pass in `fetch` (lines 333–338):

```rust
            // 1) Filter
            let mut filtered: Vec<Entity> = items
                .iter()
                .filter(|e| Self::passes(e, &req.filter, &columns, scripts.as_ref(), host.as_ref()))
                .cloned()
                .collect();
```

with:

```rust
            // 1) Filter
            // H3 (Q0018): ein Run-Budget pro `fetch` ueber alle Zeilen.
            let mut budget = ScriptBudget::new();
            let mut filtered: Vec<Entity> = items
                .iter()
                .filter(|e| {
                    Self::passes(
                        e,
                        &req.filter,
                        &columns,
                        scripts.as_ref(),
                        host.as_ref(),
                        &mut budget,
                    )
                })
                .cloned()
                .collect();
```

- [ ] **Step 5: Run the H3 tests to verify they PASS**

Run: `cargo test -p client --target-dir target-test h3_`
Expected: `h3_budget_lets_excess_rows_pass_open` and `h3_within_budget_no_regression` PASS. With `TEST_MAX_SCRIPT_FILTER_RUNS = 200` and `n = 250`, ~200 rows are evaluated (excluded) and ~50 pass through fail-open → `total_count >= 50`.

- [ ] **Step 6: Resolve any clippy dead-code / lint on the prod constant**

Run: `cargo clippy -p client --all-targets --target-dir target-test -- -D warnings`
If clippy flags `MAX_SCRIPT_FILTER_RUNS_PROD` as unused under `debug_assertions` builds, add `#[allow(dead_code)]` directly above it:

```rust
#[allow(dead_code)] // unter debug_assertions ungenutzt; aktiv im Release-Budget
const MAX_SCRIPT_FILTER_RUNS_PROD: usize = 5_000;
```

Re-run clippy; expected: clean.

---

## Task 7: Full verification + checkpoint

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt --manifest-path client/Cargo.toml`
Expected: no changes, or whitespace-only.

- [ ] **Step 2: Clippy (deny warnings)**

Run: `cargo clippy -p client --all-targets --target-dir target-test -- -D warnings`
Expected: clean, zero warnings.

- [ ] **Step 3: Full client test run**

Run: `cargo test -p client --target-dir target-test`
Expected: all tests PASS, including the five `local_source_script_filter` originals/new groups (`selected_stack_includes_only_matching_rows`, `unselected_stack_passes_all_rows`, `h1_*`, `h2_*`, `h3_*`).

If `E0786` / `STATUS_STACK_BUFFER_OVERRUN` / rlib link errors appear: that's the Q0016 transient `target-test` cache corruption — `cargo clean --target-dir target-test` and re-run. **Not a code bug.**

- [ ] **Step 4: Scope audit**

Run: `git status --short`
Expected: only `client/src/components/table/data_source.rs` and `client/tests/local_source_script_filter.rs` are modified. **No** changes to sandbox, `json_to_dynamic`, `lookup_provider`, `set_max_operations`, `examples/d2v/...`, or any queue/audit file. If anything else shows up, revert it.

- [ ] **Step 5: Final checkpoint — report, do NOT commit**

Report the green `fmt` / `clippy` / `test` output and the clean scope audit to the ccm-loop controller. **Do not `git commit`, do not move/edit any queue file, do not write the audit log** — the parent controller does all bookkeeping.

---

## Definition of Done (mirrors the spec)

- [ ] H1: `script_predicate` normalizes whole floats to int-typed JSON numbers; comment explains the symmetry to the row numerics (Task 2).
- [ ] H1-Test pins a type-strict INT-vs-FLOAT compare (green; was red pre-fix) (Tasks 1–2).
- [ ] H2: `global_search` loop explicitly skips `script:`-filter columns; guard + comment present (Task 4).
- [ ] H2-Test proves the skip (Tasks 3–4).
- [ ] H3: per-`fetch` run budget (`MAX_SCRIPT_FILTER_RUNS`) in the filter-pass; fail-open on exhaustion; one-time warning; module-doc documents all three bounds (Task 6).
- [ ] H3-Test proves the budget bites (fail-open beyond budget) (Tasks 5–6).
- [ ] `cargo fmt --check`, `cargo clippy -p client --all-targets -- -D warnings`, `cargo test -p client` green (Task 7).
- [ ] No change outside `data_source.rs` + `client/tests/`; in particular no change to sandbox/capability model, `json_to_dynamic`, `lookup_provider`, or `set_max_operations` (Task 7, Step 4).

---

## NEEDS-DECISION

None. The single non-blocking choice (H3 test budget injection) is resolved by the spec's stated default: a build-time override of the active budget constant. (Plan uses `cfg(debug_assertions)` rather than `cfg(test)` for the override because an integration test in `client/tests/` compiles `client` as a dependency — without `cfg(test)` of the crate — so a `#[cfg(test)]`-gated constant would be invisible and inert to it; `debug_assertions` is on for `cargo test`/dev and off for the release WASM profile that ships, preserving the production `5_000`. This is a mechanism detail within the spec's "implementer chooses the borrow-/test-friendly form" latitude, not a new decision.)
