# Q0017 — `validator_id` on-save Editor-Hookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `script:`-Column-Validators (`ColumnMeta.validator_id`) into the live Editor's `ValidationSystem` at load time so a failing validation blocks save and shows the `validation.script` `ValidationMessage`.

**Architecture:** Ansatz A (spec §3, decided). The Editor-Load closure additionally calls `fetch_columns` and registers script-validators via a new **pure** function `register_script_validators` that reuses Q0014's `script_validator_task`. ZERO `shared`/server changes, no new GraphQL query. `ValidationSystem` stays non-reactive (`Arc<Mutex<…>>`) and synchronous — registration mirrors the existing `import_required_from` `required`-task path; the lazy lookup at save-time keeps the async-filled `ScriptRegistry` race-free (fail-open). The Editor reads the env via `use_context::<ScriptRenderEnv>()` (Option, fail-open) so it never panics when no script env is mounted.

**Tech Stack:** Rust, Leptos 0.7 (CSR/WASM client), `shared` wire types, Rhai script engine (via Q0014 `lookup_provider`), Project Fluent (`.ftl`) i18n, native `#[test]` integration tests under `client/tests/`.

---

## Code-divergence from the spec's anchors (verified against real code, 2026-05-31)

The plan below uses the **real** API; these differ from names the spec speculated about:

1. **`use_script_render_env()` does NOT exist.** The codebase accesses the env via `use_context::<ScriptRenderEnv>()` returning `Option<ScriptRenderEnv>` (e.g. `script_renderer.rs:188`, `formatters.rs:55`). This plan uses the `Option` form per the explicit plan-directive (fail-open: no env ⇒ no script-validators ⇒ save allowed). The spec's `expect`-based `use_script_render_env` is intentionally NOT used.
2. **`env.script_ctx()` does NOT exist; `env.make_ctx()` does** (`script_renderer.rs:330-338`, builds `ScriptCtx { user_id, tenant_id, locale }`). Per the resolved deferred choice (1), the balance validator reads no ctx fields, so this plan passes **`ScriptCtx::default()`** (consistent with the table-filter path `data_source.rs:399`). `env.make_ctx()` remains a trivial later upgrade — noted, not used.
3. **`ValidationSystem` mutex is at `validation/mod.rs:207`** (spec said 206) — cosmetic.
4. **The `fr` locale exists** (`client/locales/fr/main.ftl`). The spec DoD-4 lists de/en/fr; the dispatch note mentioned de+en. Since `fr/main.ftl` is real and already carries the full `validation-*` block, this plan adds `validation-script` to **all three** (de/en/fr) — consistent with DoD-4 and avoids a raw-key render under the French locale.

These are the resolved deferred minor choices (spec §10):
- **(1) ScriptCtx:** `ScriptCtx::default()` (spec-recommended default; balance validator is ctx-agnostic).
- **(2) Re-load idempotency:** add `ValidationSystem::clear(entity_type)` and call it at the top of the Load closure before `import_required_from` + `register_script_validators` (spec-preferred option (a)).
- **(3) FTL wording:** de/en/fr written below (sensible, matching the existing validation-block tone).

No NEEDS-DECISION items — scope is fully resolvable.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `client/src/validation/mod.rs` | `ValidationSystem::clear(entity_type)` for re-load idempotency | Modify |
| `client/src/validation/script_task.rs` | NEW pure `register_script_validators` (next to `script_validator_task`) | Modify |
| `client/src/routes/editor.rs` | Load closure: grab env before `LocalResource`, `clear` + `fetch_columns` + register after `import_required_from` | Modify |
| `client/locales/de/main.ftl` | `validation-script` key (de) | Modify |
| `client/locales/en/main.ftl` | `validation-script` key (en) | Modify |
| `client/locales/fr/main.ftl` | `validation-script` key (fr) | Modify |
| `examples/d2v/entities/datev_entry/columns.json` | `value` column → `validatorId: "script:d2v_balance_validator"` | Modify |
| `client/tests/editor_validator_hookup.rs` | NEW native `#[test]` proving block-on-violation + pass-on-valid via `register_script_validators` | Create |

**No** server code, **no** `shared` wire type.

---

## Verification sequence (run after each implementing task and at the end)

Use a separate target dir to avoid the Windows `server.exe` file-lock and the Q0016 cache issue:

```
cargo fmt
cargo clippy -p client --target-dir target-test -- -D warnings
cargo test -p client --target-dir target-test
```

Plus the regression guard (unchanged wire contract):
```
cargo test -p shared --target-dir target-test
```

> **Q0016 transient note:** If `cargo test`/`clippy` against `target-test` emits **E0786**, **STATUS_STACK_BUFFER_OVERRUN**, or rlib link errors, that is the known Q0016 `target-test` cache corruption — NOT a code bug. Fix by `cargo clean --target-dir target-test` (or use a fresh target dir name) and re-run. Do not "fix" code in response to those errors.

---

## Task 1: `ValidationSystem::clear(entity_type)` (re-load idempotency)

**Files:**
- Modify: `client/src/validation/mod.rs` (add method to `impl ValidationSystem`, after `register`/`extend`, before `run`)
- Test: covered by `client/tests/editor_validator_hookup.rs` (Task 4) — the re-register-idempotency assertion exercises `clear`. No standalone test file.

- [ ] **Step 1: Write the failing test (inline unit test at the bottom of `mod.rs`)**

Add this module at the end of `client/src/validation/mod.rs`:

```rust
#[cfg(test)]
mod clear_tests {
    use super::*;

    fn noop_task(target: &str) -> ValidationTask {
        let t = target.to_string();
        ValidationTask {
            target: target.into(),
            task: Arc::new(move |_t, _v, _f| {
                Some(ValidationMessage::error(t.clone(), "validation.test"))
            }),
        }
    }

    #[test]
    fn clear_removes_only_the_given_entity_types_tasks() {
        let mut sys = ValidationSystem::new();
        sys.register("datev_entry", noop_task("value"));
        sys.register("other", noop_task("x"));

        sys.clear("datev_entry");

        // datev_entry has no tasks left -> empty result.
        let empty = serde_json::Map::new();
        assert!(sys.run("datev_entry", &empty).is_empty());
        // unrelated entity type untouched.
        assert!(!sys.run("other", &empty).is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p client --target-dir target-test clear_removes_only`
Expected: FAIL — `no method named clear found for struct ValidationSystem`.

- [ ] **Step 3: Add the `clear` method**

In `client/src/validation/mod.rs`, inside `impl ValidationSystem`, insert after the `extend` method (around line 59, before `run`):

```rust
    /// Entfernt alle Tasks fuer `entity_type`. Wird beim Editor-Re-Load
    /// aufgerufen, damit erneutes Oeffnen desselben Editors (die
    /// `ValidationSystem`-Instanz lebt App-weit via Context) nicht denselben
    /// Task ein zweites Mal anhaengt. Andere Entity-Typen bleiben unberuehrt.
    pub fn clear(&mut self, entity_type: &str) {
        self.tasks.remove(entity_type);
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p client --target-dir target-test clear_removes_only`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add client/src/validation/mod.rs
git commit -m "feat(validation): add ValidationSystem::clear(entity_type) for editor re-load idempotency"
```

---

## Task 2: pure `register_script_validators` helper

**Files:**
- Modify: `client/src/validation/script_task.rs` (add the new pure fn after `script_validator_task`)
- Test: `client/tests/editor_validator_hookup.rs` (Task 4) is the primary proof. A small inline `#[cfg(test)]` smoke test here keeps the unit fast.

- [ ] **Step 1: Write the failing test (inline at the bottom of `script_task.rs`)**

Append to `client/src/validation/script_task.rs`:

```rust
#[cfg(test)]
mod register_tests {
    use super::*;
    use crate::script::registry::ScriptRegistry;
    use crate::validation::ValidationSystem;
    use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
    use shared::script::testing::MockHostApi;
    use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptTier};

    const BALANCE_SRC: &str =
        include_str!("../../../examples/d2v/scripts/d2v_balance_validator.rhai");

    fn validator_script(id: &str) -> Script {
        Script {
            id: ScriptId(id.into()),
            kind: ScriptKind::Provider {
                slot: ProviderSlot::Validator,
            },
            manifest: ScriptManifest {
                manifest_version: 1,
                tier: ScriptTier::Reader,
                capabilities: vec![CapabilityToken::ComputeOnly, CapabilityToken::ReadI18n],
                ..Default::default()
            },
            source: BALANCE_SRC.into(),
            version: 1,
            state: ScriptState::Active,
            last_error: None,
            created_by: "u-1".into(),
            created_at: "2026-05-23T00:00:00Z".into(),
            updated_at: "2026-05-23T00:00:00Z".into(),
        }
    }

    fn col(key: &str, validator_id: Option<&str>) -> shared::ColumnMeta {
        shared::ColumnMeta {
            key: key.into(),
            label_key: format!("field.{key}"),
            field_type: shared::FieldType::Text,
            sortable: false,
            filterable: false,
            comparator_id: None,
            filter_id: None,
            editor_id: None,
            formatter_id: None,
            validator_id: validator_id.map(str::to_string),
            action_ids: Vec::new(),
        }
    }

    #[test]
    fn registers_only_script_validators() {
        let reg = ScriptRegistry::new();
        reg.insert(validator_script("d2v_balance_validator"));
        let host = std::sync::Arc::new(MockHostApi::new());
        let columns = vec![
            col("value", Some("script:d2v_balance_validator")),
            col("description", None),
            col("key", Some("static-noop")), // non-script -> skipped
        ];

        let mut sys = ValidationSystem::new();
        register_script_validators(
            &mut sys,
            "datev_entry",
            &columns,
            std::sync::Arc::new(reg),
            host,
            ScriptCtx::default(),
        );

        // Exactly the "value" task got registered: unbalanced blocks.
        let mut f = serde_json::Map::new();
        f.insert("value".into(), serde_json::json!(100.0));
        f.insert("valueType".into(), serde_json::json!("SOLL"));
        f.insert("partnerValue".into(), serde_json::json!(90.0));
        f.insert("partnerValueType".into(), serde_json::json!("HABEN"));
        let res = sys.run("datev_entry", &f);
        assert!(res.has_blocking(), "unbalanced value must block");
        assert_eq!(res.messages.len(), 1, "only the value validator registers");
        assert_eq!(res.messages[0].target.as_deref(), Some("value"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p client --target-dir target-test registers_only_script_validators`
Expected: FAIL — `cannot find function register_script_validators in this scope`.

- [ ] **Step 3: Implement `register_script_validators`**

In `client/src/validation/script_task.rs`, add `use crate::validation::{ValidationSystem, ValidationTask};` to the imports (the file already imports `crate::validation::TaskFn`; widen it), then add after `script_validator_task` (after line 65):

```rust
/// Registriert fuer jede Column mit `validator_id` (Prefix `script:`) einen
/// [`ValidationTask`] im `sys`, gebunden an den Entity-Typ. Columns ohne
/// (Script-)Validator werden ignoriert (`script_validator_task` liefert `None`).
/// Reine Funktion — kein Leptos, daher ohne Mount testbar (wie Q0014).
pub fn register_script_validators(
    sys: &mut ValidationSystem,
    entity_type: &str,
    columns: &[shared::ColumnMeta],
    registry: Arc<ScriptRegistry>,
    host: Arc<dyn HostApi>,
    ctx: ScriptCtx,
) {
    for col in columns {
        let Some(vid) = col.validator_id.as_deref() else {
            continue;
        };
        if let Some(task) =
            script_validator_task(&col.key, vid, registry.clone(), host.clone(), ctx.clone())
        {
            sys.register(
                entity_type,
                ValidationTask {
                    target: col.key.clone(),
                    task,
                },
            );
        }
    }
}
```

Adjust the existing import line:
```rust
use crate::validation::{TaskFn, ValidationSystem, ValidationTask};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p client --target-dir target-test registers_only_script_validators`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add client/src/validation/script_task.rs
git commit -m "feat(validation): add pure register_script_validators helper (Q0014 reuse)"
```

---

## Task 3: data-dir — d2v `value` column references the validator

**Files:**
- Modify: `examples/d2v/entities/datev_entry/columns.json` (the `value` row)

This is a pure data-dir change (no code). It makes the balance validator wire end-to-end in the Editor (DoD-3). The test in Task 4 loads the validator by id but does NOT depend on this file; this change is what the live Editor consumes.

- [ ] **Step 1: Edit the `value` column**

In `examples/d2v/entities/datev_entry/columns.json`, change the `value` row to add `validatorId`:

```json
  { "key": "value",                    "labelKey": "field.datev_entry.value",                      "fieldType": { "kind": "money", "currencyCodeField": null }, "sortable": true, "filterable": true, "validatorId": "script:d2v_balance_validator" },
```

- [ ] **Step 2: Validate JSON is well-formed**

Run: `cargo test -p server --target-dir target-test loader`
Expected: PASS (the d2v/shop loader tests parse the data-dir; malformed JSON would fail). If no loader test touches d2v, fall back to `python -c "import json;json.load(open('examples/d2v/entities/datev_entry/columns.json'))"` — expect no output (valid JSON).

- [ ] **Step 3: Commit**

```bash
git add examples/d2v/entities/datev_entry/columns.json
git commit -m "feat(d2v): wire script:d2v_balance_validator onto datev_entry.value column"
```

---

## Task 4: native integration test (DoD-5) — block-on-violation + pass-on-valid

**Files:**
- Create: `client/tests/editor_validator_hookup.rs`

Mirrors `client/tests/validation_script_task.rs` but drives the **new** `register_script_validators` path (the exact entry point the Editor uses), plus a non-script negative control. This proves the registered task blocks an unbalanced record and passes a balanced one — the same `run`/`has_blocking` coupling the on-save block (`editor.rs:136`) consumes.

- [ ] **Step 1: Write the test file**

Create `client/tests/editor_validator_hookup.rs`:

```rust
//! Q0017 (Stage-3 von Q0014): beweist, dass `register_script_validators` —
//! die Funktion, die der Editor-Load benutzt — einen `script:`-Validator aus
//! einer `ColumnMeta` in das echte `ValidationSystem` registriert, sodass ein
//! verletzender Datensatz blockiert und ein valider durchlaeuft. Reale
//! Rhai-Engine, kein Mock-Praedikat; kein Leptos-Mount noetig.

use std::sync::Arc;

use client::script::registry::ScriptRegistry;
use client::validation::script_task::register_script_validators;
use client::validation::ValidationSystem;
use shared::script::engine::ScriptCtx;
use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
use shared::script::testing::MockHostApi;
use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptTier};

const BALANCE_SRC: &str = include_str!("../../examples/d2v/scripts/d2v_balance_validator.rhai");

fn validator_script(id: &str, source: &str) -> Script {
    Script {
        id: ScriptId(id.into()),
        kind: ScriptKind::Provider {
            slot: ProviderSlot::Validator,
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly, CapabilityToken::ReadI18n],
            ..Default::default()
        },
        source: source.into(),
        version: 1,
        state: ScriptState::Active,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    }
}

fn col(key: &str, validator_id: Option<&str>) -> shared::ColumnMeta {
    shared::ColumnMeta {
        key: key.into(),
        label_key: format!("field.{key}"),
        field_type: shared::FieldType::Text,
        sortable: false,
        filterable: false,
        comparator_id: None,
        filter_id: None,
        editor_id: None,
        formatter_id: None,
        validator_id: validator_id.map(str::to_string),
        action_ids: Vec::new(),
    }
}

fn fields(v: f64, vt: &str, pv: f64, pt: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    m.insert("value".into(), serde_json::json!(v));
    m.insert("valueType".into(), serde_json::json!(vt));
    m.insert("partnerValue".into(), serde_json::json!(pv));
    m.insert("partnerValueType".into(), serde_json::json!(pt));
    m
}

/// Baut ein `ValidationSystem` ueber den Editor-Pfad: Columns -> register.
fn build_system() -> ValidationSystem {
    let reg = ScriptRegistry::new();
    reg.insert(validator_script("d2v_balance_validator", BALANCE_SRC));
    let host = Arc::new(MockHostApi::new());
    let columns = vec![
        col("value", Some("script:d2v_balance_validator")),
        // Negativ-Kontrolle: kein Validator -> kein Task.
        col("description", None),
        // Negativ-Kontrolle: nicht-script-Prefix -> kein Task.
        col("key", Some("static-x")),
    ];
    let mut sys = ValidationSystem::new();
    register_script_validators(
        &mut sys,
        "datev_entry",
        &columns,
        Arc::new(reg),
        host,
        ScriptCtx::default(),
    );
    sys
}

#[test]
fn unbalanced_record_is_blocked() {
    let sys = build_system();
    // SOLL 100 vs HABEN 90 -> unbalanciert -> false -> blockierender Fehler.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 90.0, "HABEN"));
    assert!(res.has_blocking(), "unbalanced record must block save");
    let msg = res
        .messages
        .iter()
        .find(|m| m.target.as_deref() == Some("value"))
        .expect("there must be a message targeting `value`");
    assert_eq!(
        msg.message_key, "validation.script",
        "script validator uses SCRIPT_VALIDATION_KEY"
    );
}

#[test]
fn balanced_record_passes() {
    let sys = build_system();
    // SOLL 100 vs HABEN 100 -> balanciert -> true -> kein Fehler.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 100.0, "HABEN"));
    assert!(res.is_empty(), "balanced record must pass, got: {res:?}");
}

#[test]
fn non_script_and_missing_validator_register_no_task() {
    // Nur die `value`-Column erzeugt einen Task; description/key nicht.
    let sys = build_system();
    // Balanciert -> wenn nur der eine value-Task laeuft, ist das Ergebnis leer.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 100.0, "HABEN"));
    assert!(
        res.is_empty(),
        "non-script and missing validators must not add tasks"
    );
}
```

> Note on `ValidationMessage` field name (verified): the message-key field is `message_key` (`shared/src/validation.rs:44`), and `ValidationMessage::error(target, message_key)` sets it as the second arg (`validation.rs:53`). The assertion uses `msg.message_key`. `target` is `Option<String>` (`validation.rs:46`), hence `m.target.as_deref()`.

- [ ] **Step 2: Run test to verify it fails first (compile-gated)**

Run: `cargo test -p client --target-dir target-test --test editor_validator_hookup`
Expected: FAIL initially only if Task 2 not yet merged. After Task 2 it should compile; the three asserts must PASS. (If `msg.key` is the wrong accessor, the compile error tells you — fix per the note above.)

- [ ] **Step 3: Run the full client test suite**

Run: `cargo test -p client --target-dir target-test`
Expected: PASS — new file + Q0014 `validation_script_task.rs` both green.

- [ ] **Step 4: Commit**

```bash
git add client/tests/editor_validator_hookup.rs
git commit -m "test(client): editor validator hookup blocks unbalanced, passes balanced (Q0017 DoD-5)"
```

---

## Task 5: Editor-Load wiring (`editor.rs`)

**Files:**
- Modify: `client/src/routes/editor.rs` (imports; grab env before `LocalResource`; clear + fetch_columns + register inside the Load closure after `import_required_from`)

This is the glue that makes the live Editor consume the validators. There is no native DOM test for the Leptos mount (spec §6.2 — the project runs no wasm-bindgen browser harness); correctness of the registered path is proven by Task 4, and the on-save block (`has_blocking()` → `return`, `editor.rs:136`) is pre-existing, unchanged behavior. Manual verification is in the final step.

- [ ] **Step 1: Add imports**

In `client/src/routes/editor.rs`, extend the imports:

```rust
use crate::components::script_renderer::ScriptRenderEnv;
use crate::graphql::queries::{fetch_columns, fetch_editor, fetch_entity_by_id};
use crate::validation::script_task::register_script_validators;
```

(The existing `use crate::graphql::queries::{fetch_editor, fetch_entity_by_id};` line at `editor.rs:25` is replaced by the three-item form above.)

Also add the `ScriptCtx` import for `ScriptCtx::default()`:

```rust
use shared::script::engine::ScriptCtx;
```

- [ ] **Step 2: Grab the env before `LocalResource::new` (fail-open Option)**

In `EditorBody`, alongside the existing `validation_for_load`/`header_for_load` clones (around `editor.rs:102-103`), add — using `use_context` (Option), NOT an `expect` helper:

```rust
    // Q0017: ScriptRenderEnv fail-open greifen (None => keine Script-Validatoren,
    // Save bleibt erlaubt). Wird in die async-Load-Closure gemoved, kein
    // Context-Zugriff im async-Body.
    let script_env_for_load: Option<ScriptRenderEnv> = use_context::<ScriptRenderEnv>();
```

- [ ] **Step 3: Move the env into the closure and register after `import_required_from`**

Inside the `LocalResource::new(move || { … })` closure, add the clone alongside the other moves (after `let header = header_for_load.clone();`, ~`editor.rs:108`):

```rust
        let script_env = script_env_for_load.clone();
```

Then, inside the `async move` block, immediately after the existing `import_required_from` block (replace lines `editor.rs:111-114`):

```rust
            let meta = fetch_editor(&entity_type).await.ok().flatten();
            // Q0017: vor jeder (Re-)Registrierung die Tasks dieses Typs leeren,
            // damit erneutes Oeffnen desselben Editors nicht doppelt anhaengt.
            validation.update(|sys| sys.clear(&entity_type));
            if let Some(m) = meta.clone() {
                editor_meta.set(Some(m.clone()));
                validation.update(|sys| sys.import_required_from(&m));
            }
            // Q0017: Script-Validatoren aus den Columns registrieren (additiv
            // nach import_required_from). Fail-open: kein Env => kein Register.
            if let Some(env) = script_env.clone() {
                if let Ok(columns) = fetch_columns(&entity_type).await {
                    validation.update(|sys| {
                        register_script_validators(
                            sys,
                            &entity_type,
                            &columns,
                            env.registry.clone(),
                            env.host.clone(),
                            ScriptCtx::default(),
                        );
                    });
                }
            }
```

> The `clear(&entity_type)` runs before `import_required_from` so re-opening the same Editor in the same app-lifetime does not double-register required *or* script tasks (spec §8 re-load, option (a)). `ScriptCtx::default()` per resolved choice (1); `env.make_ctx()` is the trivial upgrade if a ctx-reading validator ever lands.

- [ ] **Step 4: fmt + clippy + build**

Run:
```
cargo fmt
cargo clippy -p client --target-dir target-test -- -D warnings
```
Expected: clean (no warnings). If E0786/STATUS_STACK_BUFFER_OVERRUN/rlib link errors appear, that's the Q0016 `target-test` cache issue — `cargo clean --target-dir target-test` and re-run.

- [ ] **Step 5: Run full client test suite**

Run: `cargo test -p client --target-dir target-test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add client/src/routes/editor.rs
git commit -m "feat(editor): register script: column-validators into ValidationSystem on load (Q0017)"
```

---

## Task 6: i18n — `validation-script` key (de/en/fr)

**Files:**
- Modify: `client/locales/de/main.ftl`
- Modify: `client/locales/en/main.ftl`
- Modify: `client/locales/fr/main.ftl`

The Editor renders the message via `t("validation.script")` → normalized Fluent id `validation-script` (`i18n/mod.rs:270`). The key is absent from all three locales today; without it the message renders as the raw key. Add it to the existing `## Validation` block in each file (after `validation-enum_value`).

- [ ] **Step 1: Add the German key**

In `client/locales/de/main.ftl`, after the `validation-enum_value = …` line (line 70):

```
validation-script = Validierung fehlgeschlagen.
```

- [ ] **Step 2: Add the English key**

In `client/locales/en/main.ftl`, after the `validation-enum_value = …` line (line 70):

```
validation-script = Validation failed.
```

- [ ] **Step 3: Add the French key**

In `client/locales/fr/main.ftl`, after the `validation-enum_value = …` line (line 70):

```
validation-script = Échec de la validation.
```

- [ ] **Step 4: Build to confirm `include_str!` still compiles the bundles**

Run: `cargo build -p client --target-dir target-test`
Expected: success (the `.ftl` files are `include_str!`-embedded; a syntax error in Fluent would not fail the Rust build but a missing file would — a successful build confirms the files are present and read).

- [ ] **Step 5: Commit**

```bash
git add client/locales/de/main.ftl client/locales/en/main.ftl client/locales/fr/main.ftl
git commit -m "i18n: add validation-script message key (de/en/fr) for script validators (Q0017)"
```

---

## Task 7: Full verification + manual smoke

**Files:** none (verification only)

- [ ] **Step 1: Full format/lint/test sweep**

Run:
```
cargo fmt --check
cargo clippy --workspace --all-targets --target-dir target-test -- -D warnings
cargo test -p shared --target-dir target-test
cargo test -p client --target-dir target-test
```
Expected: all green. `shared` tests confirm the wire contract is untouched. If Q0016 cache errors appear, `cargo clean --target-dir target-test` and re-run.

- [ ] **Step 2: Manual UI smoke (not CI — spec §6.2)**

1. `cargo run -p server -- --data-dir ./examples/d2v` (server on its config port).
2. `cd client && trunk serve` (client on 8080, proxies `/graphql`).
3. Open a `datev_entry` editor, enter SOLL=100 / HABEN=90 (unbalanced), click Save → save is blocked and the `validation.script` message ("Validierung fehlgeschlagen." / "Validation failed.") shows under the `value` field.
4. Balance it (HABEN=100) → save proceeds to the server.
5. **Kill the dev-server (and `trunk serve`) before handing back** — do not leave background dev-servers running (port-collision hazard).

- [ ] **Step 3: Final commit (if any straggler changes)**

```bash
git status   # expect clean; nothing to commit if Tasks 1-6 committed cleanly
```

---

## Self-Review (against spec)

- **DoD-1** (register columns with `validator_id` `script:` into the save-path `ValidationSystem`, additive after `import_required_from`): Task 5 — `register_script_validators` runs in the Load closure after `import_required_from`, on the same `validation` handle the on-save `run` queries.
- **DoD-2** (failing validation blocks save + fills `validation_messages`): pre-existing `editor.rs:136` `has_blocking()` → `return` path, now fed by the registered script task; proven by Task 4 (`has_blocking()` true, message key `validation.script`, target `value`).
- **DoD-3** (`d2v_balance_validator` end-to-end, column references validator): Task 3.
- **DoD-4** (`validation-script` key in de/en/fr): Task 6.
- **DoD-5** (native `#[test]` proves block-on-violation + pass-on-valid via the editor's registration fn): Task 4.
- **Out-of-scope invariants** (§2.1): no `shared` change, no server change, no new query, no `EditorPropertyMeta.validator_id` — honored (only `client/` + `examples/` + locales touched). Fail-open preserved (`script_validator_task` `_ => None`; env-Option; `clear` before register).
- **Re-load idempotency** (§8 option a): Task 1 `clear` + Task 5 call.
- **Plan-directive** (`use_context::<ScriptRenderEnv>()` Option, never panic): Task 5 Step 2.
- **Placeholder scan:** every code step shows full code; no TBD/TODO. The `ValidationMessage` accessor (`message_key`) is verified against `shared/src/validation.rs:44/53` — not a placeholder.
- **Type consistency:** `register_script_validators` signature is identical in Task 2 impl, the Task 2 inline test, the Task 4 file, and the Task 5 call site (`&mut ValidationSystem, &str, &[ColumnMeta], Arc<ScriptRegistry>, Arc<dyn HostApi>, ScriptCtx`). `clear(&str)` consistent across Task 1 and Task 5.
