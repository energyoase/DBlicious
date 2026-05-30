# Q0014 — `validator_id`-Slot + `script:`-Prefix im Filter-Pfad Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mach zwei in Q0013 dormant ausgelieferte d2v-Metadaten live: ein `validator_id`-Slot in `ColumnMeta`/`FieldTypeDefaults` (Lücke A), der einen `script:`-Validator durch das echte `ValidationSystem::run` treibt, und ein `script:`-Filter-Pfad in `LocalSource::passes` (Lücke B), der Zeilen per `stackId` ein-/ausschließt.

**Architecture:** `validator_id` spiegelt `formatter_id` 1:1 additiv durch die Wire-Kette (shared → server schema/data → client GraphQL-Deser). Der Live-Beweis läuft jeweils über die **echte** `lookup_provider(ProviderSlot::Validator|Filter)`-Engine, nicht über Mocks der Prädikatlogik. Beide Felder sind additiv → `shared::DATA_DIR_FORMAT` bleibt `1`. Editor-UI-Hookup und Page-Level-DataSource-Routing sind explizit **Out of Scope** (Follow-up-Items).

**Tech Stack:** Rust-Workspace (`shared`/`server`/`client`), `serde` (camelCase Wire), `async-graphql` (`SimpleObject` Re-Wrap), Leptos CSR (client `rlib` native `#[test]`-Harness), Rhai-Skript-Engine über `shared::script::engine`.

---

## Wichtige Vorab-Fakten (gegen aktuellen Code verifiziert, 2026-05-30)

- **Wire-Form:** `ColumnMeta`/`FieldTypeDefaults` sind `#[serde(rename_all = "camelCase")]` → `validator_id` serialisiert als `"validatorId"`.
- **`formatter_id`-Muster** an jeder Touch-Stelle: `shared/src/lib.rs:300-302`, `shared/src/settings.rs:32-33` + `allowed_formatter_ids:45`, `server/src/schema.rs:39`, `server/src/data.rs:84`, `client/src/graphql/queries.rs` COLUMNS_QUERY `formatterId:67` + `RawColumnMeta:88` + Mapping `:122`.
- **Konstruktor-Call-Sites** ohne `..Default::default()`, die das neue Feld setzen müssen: `server/src/reference.rs:76` (Test-Helfer `col`), `client/src/components/registries/resolve.rs:95` (Test-Helfer `col`). `convert_column` (server) und `fetch_columns` (client) bauen `ColumnMeta`-Literale → ebenfalls setzen.
- **Client-Test-Harness:** native `#[test]` (kein `wasm_bindgen_test`) — die Skript-/Validation-Logik ist DOM-frei. Vorbild: `client/tests/script_provider_lookup.rs` (Integrationstest gegen `rlib`, baut `Script` inline, nutzt `ScriptRegistry::new()/insert()`, `MockHostApi::new()`, `ScriptCtx::default()`).
- **`ValidationSystem::run`** (`client/src/validation/mod.rs:62`) iteriert `tasks` pro `entity_type`, ruft jeden `TaskFn(&target, &value, &fields)`, sammelt `Some(ValidationMessage)` in `ValidationResult`. `ValidationResult::is_empty()` / `has_blocking()` (`shared/src/validation.rs:95/99`). `ValidationMessage::error(target, key)` (`:53`).
- **`lookup_provider`** (`client/src/script/provider_lookup.rs:67`) Signatur: `(raw_id, expected_slot, &ScriptRegistry, Arc<dyn HostApi>, ScriptCtx, ScriptInputs) -> LookupResult`. `ScriptInputs { value, fields }` (`shared/src/script/engine.rs:22`). `ScriptValue::Bool(b)` (`:50`). `ProviderSlot::Validator`/`Filter` (`provider_lookup.rs:158/156`).
- **`LocalSource::passes`** (`client/src/components/table/data_source.rs:217`) iteriert `filter.predicates`, holt `ColumnLookup` (`:184`, hält schon `filter_id`), wertet via `ops_for_named` aus. `ColumnFilter { key, predicate }` / `FilterPredicate` (`shared/src/lib.rs:395/361`). Der vom UI gewählte Stack steckt für eine `integer`-Spalte in `FilterPredicate::NumberEquals { value }`.
- **`resolve_filter_id`** (`client/src/components/table/filters/mod.rs:35`) — der `_ => default_id_for(...)`-Fallback (`:50`) rendert `script:`-IDs heute fälschlich als Range-Widget. `SCRIPT_PREFIX` = `"script:"` (`provider_lookup.rs:34`).
- **`d2v_stack_filter.rhai`** liest `fields.selectedStackId` + `fields.stackId`; `sel == () || sel == -1` → `true` (alle). **`d2v_balance_validator.rhai`** liest `fields.{value,valueType,partnerValue,partnerValueType}`, gibt `true` = valide. Manifeste: `provider`-Kind, Slot `validator`/`filter`, `reader`-Tier, `computeOnly`(+`readI18n`).
- **`shared::DATA_DIR_FORMAT` = 1** (`shared/src/lib.rs:20`). Bleibt `1`.

## File Structure

**shared:**
- `shared/src/lib.rs` — `ColumnMeta.validator_id` (additiv neben `formatter_id`).
- `shared/src/settings.rs` — `FieldTypeDefaults.validator_id` + `allowed_validator_ids`.
- `shared/tests/column_meta_wire_format.rs` — **neu**: ColumnMeta-Roundtrip-Pin (Verantwortung: Wire-Stabilität + Abwärtskompatibilität des additiven Felds).

**server:**
- `server/src/schema.rs` — `SimpleObject`-`ColumnMeta.validator_id`.
- `server/src/data.rs` — `convert_column` reicht `validator_id` durch.
- `server/src/reference.rs` — Test-Helfer-Call-Site (`validator_id: None`).

**client:**
- `client/src/graphql/queries.rs` — `COLUMNS_QUERY` Selection + `RawColumnMeta` + `fetch_columns`-Mapping.
- `client/src/components/registries/resolve.rs` — `"validator"`-Arm in beiden Resolution-Stufen + Test-Helfer-Call-Site.
- `client/src/validation/script_task.rs` — **neu**: `script_validator_task(validator_id, registry, host, ctx) -> Option<ValidationTask>` (Verantwortung: `script:`-Validator → `TaskFn` über echte Engine).
- `client/src/validation/mod.rs` — `pub mod script_task;` registrieren.
- `client/src/components/table/filters/mod.rs` — `script:`-Früh-Return in `resolve_filter_id`.
- `client/src/components/table/data_source.rs` — `LocalSource::passes` `script:`-Zweig + Skript-Prädikat-Helper.

**Tests (neu, gegen `rlib`):**
- `client/tests/validation_script_task.rs` — **neu**: ValidationSystem-System-Test (Lücke A live).
- `client/tests/local_source_script_filter.rs` — **neu**: End-to-End-LocalSource-Prädikat (Lücke B live).

**Fixtures (existieren, nur Testeingang):** `examples/d2v/scripts/d2v_balance_validator.rhai`, `examples/d2v/scripts/d2v_stack_filter.rhai`.

---

## Verifikations-Sequenz (Windows-Caveat beachten)

Alle Test-Runs mit `--target-dir target-test` (gitignored, vermeidet `server.exe`-File-Locks). Bei `E0786` / `STATUS_STACK_BUFFER_OVERRUN` / "required in rlib format": bekannte transiente target-test-Cache-Korruption aus Q0016 — `cargo clean --target-dir target-test` bzw. frisches Target-Dir, **kein** echter Bug, nicht neu diagnostizieren. `cargo test --workspace` kann OOMen → **per Crate** scopen.

- `cargo fmt`
- `cargo clippy -p shared` / `cargo clippy -p server` / `cargo clippy -p client` (per Crate, nicht `--workspace`, um OOM zu vermeiden)
- `cargo test -p shared --target-dir target-test` (Wire-Pin)
- `cargo test -p server --target-dir target-test` (Schema-Re-Wrap kompiliert + Test-Helfer)
- `cargo test -p client --target-dir target-test` (Resolver, Validation-System-Test, Filter-End-to-End — native `#[test]`)

---

### Task 1: ColumnMeta-Wire-Pin (failing first, weil `validator_id` noch nicht existiert)

**Files:**
- Test: `shared/tests/column_meta_wire_format.rs` (neu)
- Modify: `shared/src/lib.rs:302` (Feld hinzufügen — erst in Schritt 3)

- [ ] **Step 1: Failing-Test schreiben**

Erstelle `shared/tests/column_meta_wire_format.rs`:

```rust
//! Wire-Pin fuer `ColumnMeta` — sichert die camelCase-Form und die
//! Abwaertskompatibilitaet des additiven `validator_id`-Felds (Q0014).
//! Muster gespiegelt aus `reference_wire_format.rs`.

use shared::{ColumnMeta, FieldType};

fn base() -> ColumnMeta {
    ColumnMeta {
        key: "stackId".into(),
        label_key: "field.datev_entry.stack_id".into(),
        field_type: FieldType::Integer,
        sortable: true,
        filterable: true,
        comparator_id: None,
        filter_id: None,
        editor_id: None,
        formatter_id: None,
        validator_id: None,
        action_ids: vec![],
    }
}

#[test]
fn validator_id_roundtrips_as_camel_case() {
    let mut c = base();
    c.validator_id = Some("script:d2v_balance_validator".into());
    let json = serde_json::to_string(&c).unwrap();
    assert!(
        json.contains("\"validatorId\":\"script:d2v_balance_validator\""),
        "serialized JSON should carry validatorId, got: {json}"
    );
    let back: ColumnMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c);
}

#[test]
fn validator_id_none_is_skipped() {
    let c = base(); // validator_id: None
    let json = serde_json::to_string(&c).unwrap();
    assert!(
        !json.contains("validatorId"),
        "None validator_id must be skipped (skip_serializing_if), got: {json}"
    );
}

#[test]
fn missing_validator_id_deserializes_to_none() {
    // Alte data-dir-JSON ohne validatorId -> None (Beweis: additiv).
    let json = r#"{
        "key": "stackId",
        "labelKey": "field.datev_entry.stack_id",
        "fieldType": { "kind": "integer" },
        "sortable": true,
        "filterable": true
    }"#;
    let c: ColumnMeta = serde_json::from_str(json).unwrap();
    assert_eq!(c.validator_id, None);
}
```

- [ ] **Step 2: Test laufen lassen, Fehlschlag verifizieren**

Run: `cargo test -p shared --target-dir target-test --test column_meta_wire_format`
Expected: FAIL beim Kompilieren — `ColumnMeta` hat kein Feld `validator_id` (E0560 "struct `ColumnMeta` has no field named `validator_id`").

- [ ] **Step 3: `validator_id` zu `shared::ColumnMeta` hinzufügen**

In `shared/src/lib.rs`, direkt nach `formatter_id` (`:302`), einfügen:

```rust
    /// Phase 1.5: erzwungene Formatter-ID fuer diese Spalte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatter_id: Option<String>,
    /// Q0014: erzwungene Validator-ID fuer diese Spalte. Mit `script:`-Prefix
    /// referenziert sie einen Provider-Skript-Validator (siehe
    /// `client::validation::script_task`). Spiegelt `formatter_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator_id: Option<String>,
```

(Das `formatter_id`-Feld bleibt — nur das `validator_id`-Block dahinter ist neu.)

- [ ] **Step 4: Test laufen lassen, Erfolg verifizieren**

Run: `cargo test -p shared --target-dir target-test --test column_meta_wire_format`
Expected: PASS (3 Tests grün).

- [ ] **Step 5: Commit**

```bash
git add shared/tests/column_meta_wire_format.rs shared/src/lib.rs
git commit -m "feat(shared): add validator_id slot to ColumnMeta with wire-pin (Q0014)"
```

---

### Task 2: `FieldTypeDefaults.validator_id` + `allowed_validator_ids`

**Files:**
- Modify: `shared/src/settings.rs:33` (+`:45`)
- Test: `shared/tests/column_meta_wire_format.rs` (erweitern)

- [ ] **Step 1: Failing-Test ergänzen**

Hänge an `shared/tests/column_meta_wire_format.rs` an:

```rust
#[test]
fn field_type_defaults_validator_id_roundtrips() {
    use shared::FieldTypeDefaults;
    let d = FieldTypeDefaults {
        validator_id: Some("script:d2v_balance_validator".into()),
        allowed_validator_ids: vec!["script:d2v_balance_validator".into()],
        ..Default::default()
    };
    let json = serde_json::to_string(&d).unwrap();
    assert!(json.contains("\"validatorId\""), "got: {json}");
    assert!(json.contains("\"allowedValidatorIds\""), "got: {json}");
    let back: FieldTypeDefaults = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
}

#[test]
fn field_type_defaults_without_validator_fields_default_empty() {
    use shared::FieldTypeDefaults;
    let d: FieldTypeDefaults = serde_json::from_str("{}").unwrap();
    assert_eq!(d.validator_id, None);
    assert!(d.allowed_validator_ids.is_empty());
}
```

- [ ] **Step 2: Test laufen lassen, Fehlschlag verifizieren**

Run: `cargo test -p shared --target-dir target-test --test column_meta_wire_format field_type_defaults`
Expected: FAIL beim Kompilieren — `FieldTypeDefaults` hat keine Felder `validator_id` / `allowed_validator_ids`.

- [ ] **Step 3: Felder zu `FieldTypeDefaults` hinzufügen**

In `shared/src/settings.rs`: nach `formatter_id` (`:33`) einfügen:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatter_id: Option<String>,
    /// Q0014: Vorgewaehlter Validator (analog `formatter_id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator_id: Option<String>,
```

und nach `allowed_formatter_ids` (`:45`) einfügen:

```rust
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_formatter_ids: Vec<String>,
    /// Q0014: weitere zulaessige Validator-Alternativen (analog
    /// `allowed_formatter_ids`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_validator_ids: Vec<String>,
```

(Die bestehenden `formatter_id`/`allowed_formatter_ids`-Zeilen bleiben.)

- [ ] **Step 4: Test laufen lassen, Erfolg verifizieren**

Run: `cargo test -p shared --target-dir target-test --test column_meta_wire_format`
Expected: PASS (5 Tests grün).

- [ ] **Step 5: Commit**

```bash
git add shared/src/settings.rs shared/tests/column_meta_wire_format.rs
git commit -m "feat(shared): add validator_id + allowed_validator_ids to FieldTypeDefaults (Q0014)"
```

---

### Task 3: Server-Durchreichung (`schema.rs` Re-Wrap + `data.rs` + Test-Helfer)

**Files:**
- Modify: `server/src/schema.rs:39`, `server/src/data.rs:84`, `server/src/reference.rs:76`

- [ ] **Step 1: `validator_id` ins `SimpleObject`-`ColumnMeta` aufnehmen**

In `server/src/schema.rs`, nach `formatter_id` (`:39`):

```rust
    pub formatter_id: Option<String>,
    /// Q0014: Provider-Skript-Validator-ID (`script:<id>`) o.ae.
    pub validator_id: Option<String>,
    pub action_ids: Vec<String>,
```

(`action_ids` bleibt die letzte Zeile.)

- [ ] **Step 2: `convert_column` durchreichen**

In `server/src/data.rs`, in `convert_column` nach `formatter_id: c.formatter_id,` (`:84`):

```rust
        formatter_id: c.formatter_id,
        validator_id: c.validator_id,
        action_ids: c.action_ids,
```

- [ ] **Step 3: Test-Helfer-Call-Site fixen**

In `server/src/reference.rs`, Test-Helfer `col` (`:85`), nach `formatter_id: None,`:

```rust
            formatter_id: None,
            validator_id: None,
            action_ids: vec![],
```

- [ ] **Step 4: Server kompiliert + Tests grün**

Run: `cargo test -p server --target-dir target-test`
Expected: PASS — Server kompiliert (alle `ColumnMeta`-Literale vollständig), bestehende `reference`-Tests grün.

- [ ] **Step 5: Commit**

```bash
git add server/src/schema.rs server/src/data.rs server/src/reference.rs
git commit -m "feat(server): thread validator_id through GraphQL ColumnMeta re-wrap (Q0014)"
```

---

### Task 4: Client-GraphQL-Deser (`COLUMNS_QUERY` + `RawColumnMeta` + `fetch_columns`)

**Files:**
- Modify: `client/src/graphql/queries.rs:67-68`, `:88`, `:122`

- [ ] **Step 1: `validatorId` ins Selection-Set**

In `client/src/graphql/queries.rs`, `COLUMNS_QUERY`, nach `formatterId` (`:67`):

```graphql
            formatterId
            validatorId
            actionIds
```

- [ ] **Step 2: `RawColumnMeta` erweitern**

Nach `formatter_id` (`:88`):

```rust
    #[serde(default)]
    formatter_id: Option<String>,
    #[serde(default)]
    validator_id: Option<String>,
    #[serde(default)]
    action_ids: Option<Vec<String>>,
```

- [ ] **Step 3: Mapping in `fetch_columns`**

Nach `formatter_id: c.formatter_id,` (`:122`):

```rust
                formatter_id: c.formatter_id,
                validator_id: c.validator_id,
                action_ids: c.action_ids.unwrap_or_default(),
```

- [ ] **Step 4: Client kompiliert**

Run: `cargo build -p client --target-dir target-test`
Expected: Kompiliert. (Eigenständiger Test folgt in Task 6; hier nur Kompilierung, da die Mapping-Stelle ein vollständiges `ColumnMeta`-Literal baut.)

- [ ] **Step 5: Commit**

```bash
git add client/src/graphql/queries.rs
git commit -m "feat(client): deserialize validatorId from COLUMNS_QUERY (Q0014)"
```

---

### Task 5: Resolver-Arm `"validator"` in `resolve_implementation_id`

**Files:**
- Modify: `client/src/components/registries/resolve.rs:26-31`, `:40-45`, `:95`
- Test: `client/src/components/registries/resolve.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Failing-Test schreiben**

In `client/src/components/registries/resolve.rs`, im `mod tests`-Block, nach `client_fallback_text_field` einfügen:

```rust
    #[test]
    fn validator_column_override_wins() {
        let mut c = col(FieldType::Money {
            currency_code_field: None,
        });
        c.validator_id = Some("script:d2v_balance_validator".into());
        assert_eq!(
            resolve_implementation_id(&c, None, "validator"),
            Some("script:d2v_balance_validator".to_string())
        );
    }

    #[test]
    fn validator_field_type_default_used() {
        let c = col(FieldType::Money {
            currency_code_field: None,
        });
        let mut settings = EntitySettings::default();
        let defaults = shared::FieldTypeDefaults {
            validator_id: Some("script:d2v_balance_validator".into()),
            ..Default::default()
        };
        settings.field_type_defaults.insert("money".into(), defaults);
        assert_eq!(
            resolve_implementation_id(&c, Some(&settings), "validator"),
            Some("script:d2v_balance_validator".to_string())
        );
    }

    #[test]
    fn validator_has_no_client_fallback() {
        let c = col(FieldType::Text);
        assert!(resolve_implementation_id(&c, None, "validator").is_none());
    }
```

Außerdem den Test-Helfer `col` (`:95`) vervollständigen — nach `formatter_id: None,`:

```rust
            formatter_id: None,
            validator_id: None,
            action_ids: Vec::new(),
```

- [ ] **Step 2: Test laufen lassen, Fehlschlag verifizieren**

Run: `cargo test -p client --target-dir target-test --lib resolve`
Expected: FAIL — `validator_column_override_wins`/`validator_field_type_default_used` liefern `None`, weil der `"validator"`-Arm in beiden Stufen fehlt und auf `_ => None` fällt. (`validator_has_no_client_fallback` ist bereits grün — Konvention: kein Validator-Fallback.)

- [ ] **Step 3: `"validator"`-Arm in beide Resolution-Stufen einbauen**

Stufe 1 (`:26-31`):

```rust
    let column_override = match registry {
        "filter" => column.filter_id.clone(),
        "editor" => column.editor_id.clone(),
        "formatter" => column.formatter_id.clone(),
        "validator" => column.validator_id.clone(),
        _ => None,
    };
```

Stufe 2 (`:40-45`):

```rust
            let from_settings = match registry {
                "filter" => defaults.filter_id.clone(),
                "editor" => defaults.editor_id.clone(),
                "formatter" => defaults.formatter_id.clone(),
                "validator" => defaults.validator_id.clone(),
                _ => None,
            };
```

`client_fallback` bleibt unverändert (kein `"validator"`-Arm → `_ => return None`), das ist die gewollte „kein Client-Hardcoded-Validator-Default"-Konvention.

- [ ] **Step 4: Test laufen lassen, Erfolg verifizieren**

Run: `cargo test -p client --target-dir target-test --lib resolve`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add client/src/components/registries/resolve.rs
git commit -m "feat(client): resolve validator implementation id through resolution chain (Q0014)"
```

---

### Task 6: Script-Validator-`TaskFn` + ValidationSystem-System-Test (Lücke A live)

**Files:**
- Create: `client/src/validation/script_task.rs`
- Modify: `client/src/validation/mod.rs` (`pub mod script_task;`)
- Test: `client/tests/validation_script_task.rs` (neu)

- [ ] **Step 1: Failing-Test schreiben (System-Level, echte Engine)**

Erstelle `client/tests/validation_script_task.rs`:

```rust
//! System-Test (Q0014, Lücke A): beweist, dass `validator_id =
//! "script:d2v_balance_validator"` als TaskFn durch das echte
//! `ValidationSystem::run` laeuft (reale Rhai-Engine, kein Mock-Praedikat).

use std::sync::Arc;

use client::script::registry::ScriptRegistry;
use client::validation::script_task::script_validator_task;
use client::validation::{ValidationSystem, ValidationTask};
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

fn fields(v: f64, vt: &str, pv: f64, pt: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    m.insert("value".into(), serde_json::json!(v));
    m.insert("valueType".into(), serde_json::json!(vt));
    m.insert("partnerValue".into(), serde_json::json!(pv));
    m.insert("partnerValueType".into(), serde_json::json!(pt));
    m
}

fn build_system() -> ValidationSystem {
    let reg = ScriptRegistry::new();
    reg.insert(validator_script("d2v_balance_validator", BALANCE_SRC));
    let host = Arc::new(MockHostApi::new());
    let task = script_validator_task(
        "value",
        "script:d2v_balance_validator",
        Arc::new(reg),
        host,
        ScriptCtx::default(),
    )
    .expect("script validator task should be constructible");
    let mut sys = ValidationSystem::new();
    sys.register("datev_entry", ValidationTask { target: "value".into(), task });
    sys
}

#[test]
fn balanced_fields_produce_no_validation_error() {
    let sys = build_system();
    // SOLL 100 vs HABEN 100 -> balanciert -> true -> kein Fehler.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 100.0, "HABEN"));
    assert!(res.is_empty(), "balanced should be valid, got: {res:?}");
}

#[test]
fn unbalanced_fields_block_validation() {
    let sys = build_system();
    // SOLL 100 vs HABEN 90 -> unbalanciert -> false -> blockierender Fehler.
    let res = sys.run("datev_entry", &fields(100.0, "SOLL", 90.0, "HABEN"));
    assert!(!res.is_empty(), "unbalanced should produce an error");
    assert!(res.has_blocking(), "validator error must block");
}
```

> Hinweis Pfad: `client/tests/` liegt eine Ebene unter dem Crate-Root; `../../examples/...` zeigt von `client/tests/` auf das Workspace-Root `examples/`. Falls `include_str!` den Pfad nicht auflöst, ist er relativ zur Quelldatei — `client/tests/validation_script_task.rs` → `../../examples/...` = `dblicious/examples/...`. Stimmt.

- [ ] **Step 2: Test laufen lassen, Fehlschlag verifizieren**

Run: `cargo test -p client --target-dir target-test --test validation_script_task`
Expected: FAIL beim Kompilieren — `client::validation::script_task` existiert nicht (`unresolved import`).

- [ ] **Step 3: `script_task.rs` implementieren**

Erstelle `client/src/validation/script_task.rs`:

```rust
//! Q0014 (Lücke A): uebersetzt einen `script:`-Validator in einen
//! `ValidationTask`. Spiegelt den Formatter-Pfad aus
//! `components/table/formatters.rs` — synchron, ueber die echte Engine via
//! `lookup_provider(ProviderSlot::Validator)`.
//!
//! Konvention: der Validator-Slot gibt `true` = "valide" zurueck (so liest
//! `d2v_balance_validator.rhai`). `Bool(false)` => blockierender
//! `ValidationMessage::error`. Fallback/NotAScriptId/Nicht-Bool => kein Block
//! (Validator nicht aktiv; `lookup_provider` hat das Audit-Log bereits gefuellt).

use std::sync::Arc;

use shared::script::engine::{HostApi, ScriptCtx, ScriptInputs, ScriptValue};
use shared::script::model::ProviderSlot;
use shared::ValidationMessage;

use crate::script::provider_lookup::{lookup_provider, LookupResult, SCRIPT_PREFIX};
use crate::script::registry::ScriptRegistry;
use crate::validation::TaskFn;

/// Fluent-Schluessel-Prefix fuer Skript-Validator-Fehler.
pub const SCRIPT_VALIDATION_KEY: &str = "validation.script";

/// Baut aus einer `validator_id` mit `script:`-Prefix einen `TaskFn`.
/// `None`, wenn die ID keinen `script:`-Prefix traegt (statischer Pfad gilt
/// dann; Q0014 kennt keine statischen Validator-IDs).
pub fn script_validator_task(
    target: &str,
    validator_id: &str,
    registry: Arc<ScriptRegistry>,
    host: Arc<dyn HostApi>,
    ctx: ScriptCtx,
) -> Option<TaskFn> {
    if !validator_id.starts_with(SCRIPT_PREFIX) {
        return None;
    }
    let validator_id = validator_id.to_string();
    let target_owned = target.to_string();
    let task: TaskFn = Arc::new(move |_t, value, fields| {
        let inputs = ScriptInputs {
            value: value.clone(),
            fields: fields.clone(),
        };
        match lookup_provider(
            &validator_id,
            ProviderSlot::Validator,
            &registry,
            host.clone(),
            ctx.clone(),
            inputs,
        ) {
            // true = valide, false = blockierender Fehler.
            LookupResult::Ok {
                value: ScriptValue::Bool(false),
            } => Some(ValidationMessage::error(
                target_owned.clone(),
                SCRIPT_VALIDATION_KEY,
            )),
            // Bool(true) / andere Slot-Konventionen / Fallback / NotAScriptId
            // => nicht blockieren.
            _ => None,
        }
    });
    Some(task)
}
```

- [ ] **Step 4: Modul registrieren**

In `client/src/validation/mod.rs`, vor `pub mod builtin {` (oder oben bei den Modulen — direkt nach den `use`-Statements, vor `pub type TaskFn`), einfügen:

```rust
pub mod script_task;
```

(`ValidationSystem`, `ValidationTask`, `TaskFn` sind bereits `pub` im selben Modul → von `client/tests/` als `client::validation::{ValidationSystem, ValidationTask}` erreichbar.)

- [ ] **Step 5: Test laufen lassen, Erfolg verifizieren**

Run: `cargo test -p client --target-dir target-test --test validation_script_task`
Expected: PASS (`balanced_*` grün/leer, `unbalanced_*` blockiert). **Das ist der Live-Beweis für Lücke A.**

- [ ] **Step 6: Commit**

```bash
git add client/src/validation/script_task.rs client/src/validation/mod.rs client/tests/validation_script_task.rs
git commit -m "feat(client): live script: validator via ValidationSystem::run (Q0014 Lücke A)"
```

---

### Task 7: `script:`-Früh-Return in `resolve_filter_id` (kein Built-in-Widget)

**Files:**
- Modify: `client/src/components/table/filters/mod.rs:35`
- Test: `client/src/components/table/filters/mod.rs` (`#[cfg(test)]` neu)

- [ ] **Step 1: Failing-Test schreiben**

Am Ende von `client/src/components/table/filters/mod.rs` einen Test-Block anfügen:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::ColumnMeta;

    fn col(filter_id: Option<&str>) -> ColumnMeta {
        ColumnMeta {
            key: "stackId".into(),
            label_key: "k".into(),
            field_type: FieldType::Integer,
            sortable: true,
            filterable: true,
            comparator_id: None,
            filter_id: filter_id.map(|s| s.to_string()),
            editor_id: None,
            formatter_id: None,
            validator_id: None,
            action_ids: Vec::new(),
        }
    }

    #[test]
    fn script_filter_renders_no_builtin_widget() {
        let reg = default_registry();
        let c = col(Some("script:d2v_stack_filter"));
        assert_eq!(resolve_filter_id(&c, &reg), None);
    }

    #[test]
    fn plain_integer_still_gets_number_range() {
        let reg = default_registry();
        let c = col(None);
        assert_eq!(resolve_filter_id(&c, &reg), Some(number_range::ID));
    }
}
```

- [ ] **Step 2: Test laufen lassen, Fehlschlag verifizieren**

Run: `cargo test -p client --target-dir target-test --lib filters::tests`
Expected: FAIL — `script_filter_renders_no_builtin_widget` erwartet `None`, bekommt aber `Some(number_range::ID)` (der `_ => default_id_for(...)`-Fallback rendert fälschlich das Range-Widget). `plain_integer_*` ist bereits grün.

- [ ] **Step 3: Früh-Return einbauen**

In `client/src/components/table/filters/mod.rs`, am Anfang von `resolve_filter_id` (`:36`, vor dem `if let Some(id) = column.filter_id.as_deref()`-Block), einfügen:

```rust
pub fn resolve_filter_id(column: &ColumnMeta, registry: &FilterRegistry) -> Option<&'static str> {
    // Q0014: ein `script:`-Filter ist ein Per-Row-Praedikat (siehe
    // LocalSource::passes), kein Eingabe-Widget. Kein Built-in rendern.
    if let Some(id) = column.filter_id.as_deref() {
        if id.starts_with(crate::script::provider_lookup::SCRIPT_PREFIX) {
            return None;
        }
    }
    // 1) Server-Vorgabe gewinnt, wenn registriert.
    if let Some(id) = column.filter_id.as_deref() {
```

(Rest der Funktion unverändert.)

- [ ] **Step 4: Test laufen lassen, Erfolg verifizieren**

Run: `cargo test -p client --target-dir target-test --lib filters::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add client/src/components/table/filters/mod.rs
git commit -m "fix(client): script: filter renders no built-in widget in resolve_filter_id (Q0014)"
```

---

### Task 8: `script:`-Prädikat-Zweig in `LocalSource::passes` + End-to-End-Test (Lücke B live)

**Files:**
- Modify: `client/src/components/table/data_source.rs:217` (`passes`), neuer Helper
- Test: `client/tests/local_source_script_filter.rs` (neu)

- [ ] **Step 1: Failing-Test schreiben (echte Engine, kein Mock-Prädikat)**

Erstelle `client/tests/local_source_script_filter.rs`:

```rust
//! End-to-End (Q0014, Lücke B): `LocalSource` filtert Zeilen per `stackId`
//! durch das echte `d2v_stack_filter`-Skript (reale Rhai-Engine, realer
//! lookup_provider — kein Mock-Praedikat).

use client::components::table::data_source::{DataRequest, DataSource, LocalSource};
use client::script::registry::ScriptRegistry;
use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptTier};
use shared::{ColumnFilter, ColumnMeta, Entity, FieldType, FilterCriteria, FilterPredicate};

const STACK_SRC: &str = include_str!("../../examples/d2v/scripts/d2v_stack_filter.rhai");

fn stack_filter_script() -> Script {
    Script {
        id: ScriptId("d2v_stack_filter".into()),
        kind: ScriptKind::Provider {
            slot: ProviderSlot::Filter,
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly],
            ..Default::default()
        },
        source: STACK_SRC.into(),
        version: 1,
        state: ScriptState::Active,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    }
}

fn row(id: &str, stack: i64) -> Entity {
    let mut m = serde_json::Map::new();
    m.insert("stackId".into(), serde_json::json!(stack));
    Entity { id: id.into(), fields: m }
}

fn columns() -> Vec<ColumnMeta> {
    vec![ColumnMeta {
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
    }]
}

fn filter_for_stack(sel: f64) -> FilterCriteria {
    FilterCriteria {
        global_search: None,
        predicates: vec![ColumnFilter {
            key: "stackId".into(),
            predicate: FilterPredicate::NumberEquals { value: sel },
        }],
    }
}

fn run(src: &LocalSource, filter: FilterCriteria) -> Vec<String> {
    let req = DataRequest {
        page: 1,
        page_size: 100,
        sort: None,
        filter,
    };
    let resp = futures::executor::block_on(src.fetch(req)).unwrap();
    resp.items.into_iter().map(|e| e.id).collect()
}

fn source() -> LocalSource {
    let reg = ScriptRegistry::new();
    reg.insert(stack_filter_script());
    // Host wird injiziert: im Test der testing-MockHostApi (das d2v_stack_filter-
    // Skript ist computeOnly und ruft keine Host-Methode). Produktion injiziert
    // RenderHost. So bleibt `data_source.rs` frei von testing-gegateten Typen.
    let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
        std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    LocalSource::with_script_registry(
        vec![row("a", 1), row("b", 2), row("c", 3)],
        &columns(),
        std::sync::Arc::new(reg),
        host,
    )
}

#[test]
fn selected_stack_includes_only_matching_rows() {
    let src = source();
    let ids = run(&src, filter_for_stack(2.0));
    assert_eq!(ids, vec!["b".to_string()]);
}

#[test]
fn unselected_stack_passes_all_rows() {
    let src = source();
    let ids = run(&src, filter_for_stack(-1.0));
    assert_eq!(ids.len(), 3, "selectedStackId == -1 => alle Stacks");
}
```

> Hinweis: `futures::executor::block_on` braucht `futures` als dev-dependency im client-Crate. Falls noch nicht vorhanden, in `client/Cargo.toml` unter `[dev-dependencies]` `futures = "0.3"` ergänzen (Schritt 3 deckt das ab).

- [ ] **Step 2: Test laufen lassen, Fehlschlag verifizieren**

Run: `cargo test -p client --target-dir target-test --test local_source_script_filter`
Expected: FAIL beim Kompilieren — `LocalSource::with_script_registry` existiert nicht (`no function or associated item`).

- [ ] **Step 3: `futures` dev-dependency + Skript-Registry + Host in `LocalSource` einbauen**

> **Verifizierte Anker (gegen Code):** `ScriptRegistry` ist **NICHT `Clone`** (`client/src/script/registry.rs:25` — nur `#[derive(Debug, Default)]`, hält `cache: Mutex<…>`). Daher **`Arc<ScriptRegistry>` direkt halten**, kein Klon. `MockHostApi` ist **`testing`-feature-gegatet** (nur dev-dependency) → darf **nicht** in `data_source.rs`-Produktionscode auftauchen. Deshalb wird der `Arc<dyn HostApi>` **in den Konstruktor injiziert** (Test: `MockHostApi`; Produktion: `RenderHost`, `client/src/script/render_host.rs:9`).

Falls `client/Cargo.toml` `[dev-dependencies]` kein `futures` hat, ergänzen (aktuell: nur `wasm-bindgen-test`, `shared` mit `testing`):

```toml
[dev-dependencies]
wasm-bindgen-test = "0.3"
shared = { path = "../shared", features = ["testing"] }
futures = "0.3"
```

In `client/src/components/table/data_source.rs`:

(a) Imports oben ergänzen (`FilterPredicate` zum bestehenden `shared::{…}`-Use; Engine/Provider-Imports neu):

```rust
use shared::{
    ops_for_named, ColumnMeta, Entity, EntityChangeResult, FieldType, FilterCriteria,
    FilterPredicate, Sort, SortDirection,
};
use shared::script::engine::{HostApi, ScriptCtx, ScriptInputs, ScriptValue};
use shared::script::model::ProviderSlot;

use crate::script::provider_lookup::{lookup_provider, LookupResult, SCRIPT_PREFIX};
use crate::script::registry::ScriptRegistry;
```

(`HostApi`/`ScriptCtx`/`ScriptValue` sind in `shared::script::engine`; **kein** `MockHostApi`-Import in dieser Datei.)

(b) `LocalSource` um optionale Skript-Felder erweitern (`:191`). **`Arc`, nicht `Rc`**, weil `ScriptRegistry` nicht `Clone` ist und `Arc` das Handle teilt:

```rust
#[derive(Clone)]
pub struct LocalSource {
    items: Rc<Vec<Entity>>,
    columns: Rc<HashMap<String, ColumnLookup>>,
    /// Q0014: optionale Skript-Registry + Host fuer `script:`-Filter-
    /// Praedikate. `None` => reiner Built-in-Ops-Pfad (Bestandsverhalten).
    scripts: Option<std::sync::Arc<ScriptRegistry>>,
    host: Option<std::sync::Arc<dyn HostApi>>,
}
```

(c) Bestehenden `LocalSource::new` auf `build` umstellen + neuen Konstruktor ergänzen (`:197`):

```rust
impl LocalSource {
    pub fn new(items: Vec<Entity>, columns: &[ColumnMeta]) -> Self {
        Self::build(items, columns, None, None)
    }

    /// Q0014: wie `new`, aber mit Skript-Registry + Host fuer `script:`-Filter.
    /// `host` wird injiziert (Produktion: `RenderHost`; Test: `MockHostApi`),
    /// damit diese Datei frei von `testing`-gegateten Typen bleibt.
    pub fn with_script_registry(
        items: Vec<Entity>,
        columns: &[ColumnMeta],
        scripts: std::sync::Arc<ScriptRegistry>,
        host: std::sync::Arc<dyn HostApi>,
    ) -> Self {
        Self::build(items, columns, Some(scripts), Some(host))
    }

    fn build(
        items: Vec<Entity>,
        columns: &[ColumnMeta],
        scripts: Option<std::sync::Arc<ScriptRegistry>>,
        host: Option<std::sync::Arc<dyn HostApi>>,
    ) -> Self {
        let columns = columns
            .iter()
            .map(|c| {
                (
                    c.key.clone(),
                    ColumnLookup {
                        field_type: c.field_type.clone(),
                        comparator_id: c.comparator_id.clone(),
                        filter_id: c.filter_id.clone(),
                    },
                )
            })
            .collect();
        Self {
            items: Rc::new(items),
            columns: Rc::new(columns),
            scripts,
            host,
        }
    }
```

(d) `passes`-Signatur um `scripts` + `host` erweitern (`:217`) und im Predicate-Loop den Skript-Zweig einbauen:

```rust
    fn passes(
        entity: &Entity,
        filter: &FilterCriteria,
        columns: &HashMap<String, ColumnLookup>,
        scripts: Option<&std::sync::Arc<ScriptRegistry>>,
        host: Option<&std::sync::Arc<dyn HostApi>>,
    ) -> bool {
        for cf in &filter.predicates {
            let Some(col) = columns.get(&cf.key) else {
                return false;
            };
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
            let value = entity.fields.get(&cf.key).cloned().unwrap_or(Value::Null);
            let ops = ops_for_named(
                &col.field_type,
                col.comparator_id.as_deref(),
                col.filter_id.as_deref(),
            );
            if !ops.matches(&value, &cf.predicate) {
                return false;
            }
        }
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
        true
    }
```

(e) Neuen Helper ans Ende des `impl LocalSource` (oder als freie Funktion im Modul) — wertet das Skript-Prädikat über die echte Engine aus:

```rust
/// Q0014: wertet ein `script:`-Filter-Praedikat fuer **eine** Zeile aus.
/// `selected` ist der Vergleichswert aus dem Filter-State (fuer eine
/// integer-Spalte ein `NumberEquals`-Value); er wird als `selectedStackId`
/// in eine angereicherte `fields`-Kopie injiziert. `true` = Zeile behalten.
/// `host` wird von aussen injiziert (kein `testing`-gegateter Typ hier).
/// Fallback/NotAScriptId/Nicht-Bool => `true` (nicht ausschliessen).
fn script_predicate(
    entity: &Entity,
    predicate: &FilterPredicate,
    filter_id: &str,
    registry: &ScriptRegistry,
    host: std::sync::Arc<dyn HostApi>,
) -> bool {
    let selected = match predicate {
        FilterPredicate::NumberEquals { value } => Some(*value),
        _ => None,
    };
    let mut fields = entity.fields.clone();
    let sel_value = selected
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .unwrap_or(Value::Null);
    fields.insert("selectedStackId".into(), sel_value);

    let inputs = ScriptInputs {
        value: Value::Null,
        fields,
    };
    match lookup_provider(
        filter_id,
        ProviderSlot::Filter,
        registry,
        host,
        ScriptCtx::default(),
        inputs,
    ) {
        LookupResult::Ok {
            value: ScriptValue::Bool(b),
        } => b,
        // Fallback / NotAScriptId / Nicht-Bool => Zeile nicht ausschliessen.
        _ => true,
    }
}
```

(f) Die `Self::passes(...)`-Aufrufsstelle in `fetch` (`:288`) auf die neue Signatur heben:

```rust
    fn fetch(&self, req: DataRequest) -> BoxFuture<Result<DataResponse, GqlError>> {
        let items = self.items.clone();
        let columns = self.columns.clone();
        let scripts = self.scripts.clone();
        let host = self.host.clone();
        Box::pin(async move {
            let mut filtered: Vec<Entity> = items
                .iter()
                .filter(|e| Self::passes(e, &req.filter, &columns, scripts.as_ref(), host.as_ref()))
                .cloned()
                .collect();
```

(Rest von `fetch` — Sort, Pagination — unverändert.)

> Host-Wahl: das `d2v_stack_filter`-Skript ist `computeOnly`/`reader` und ruft **keine** Host-API → ein beliebiger `HostApi` genügt. Test injiziert `MockHostApi` (testing-Feature im Test-Crate); Produktion injiziert `RenderHost` (`client/src/script/render_host.rs`). Das hält den Live-Pfad im Test echt (reale Engine), ohne `testing`-Typen in `data_source.rs`.

- [ ] **Step 4: Test laufen lassen, Erfolg verifizieren**

Run: `cargo test -p client --target-dir target-test --test local_source_script_filter`
Expected: PASS — `selected_stack_includes_only_matching_rows` liefert `["b"]`, `unselected_stack_passes_all_rows` liefert 3. **Das ist der Live-Beweis für Lücke B.**

- [ ] **Step 5: Bestehende `LocalSource`-Aufrufer prüfen (keine Regression)**

Run: `cargo build -p client --target-dir target-test`
Expected: kompiliert — `ScriptSource::from_script_result` (`client/src/script/data_source.rs:41`) ruft `LocalSource::new(...)` → unverändert (delegiert intern auf `build(..., None)`). Falls weitere `LocalSource::new`-Aufrufer existieren, bleiben sie API-kompatibel.

- [ ] **Step 6: Commit**

```bash
git add client/src/components/table/data_source.rs client/Cargo.toml client/tests/local_source_script_filter.rs
git commit -m "feat(client): script: filter predicate in LocalSource::passes, end-to-end (Q0014 Lücke B)"
```

---

### Task 9: Gesamt-Verifikation + `DATA_DIR_FORMAT`-Check

**Files:** keine Änderung — nur Verifikation.

- [ ] **Step 1: `DATA_DIR_FORMAT` ist unverändert `1`**

Run: `grep -n "DATA_DIR_FORMAT" shared/src/lib.rs`
Expected: `pub const DATA_DIR_FORMAT: u32 = 1;` — **unverändert**. (Beide neuen Felder sind additiv mit `#[serde(default)]`; CLAUDE.md-Konvention: additive Loader-Änderungen erhöhen die Konstante nicht. Der ColumnMeta-Wire-Pin Test `missing_validator_id_deserializes_to_none` sichert die Abwärtskompatibilität ab.)

- [ ] **Step 2: Format**

Run: `cargo fmt`
Expected: keine Diffs (oder nur erwartete Formatierung der neuen Dateien — dann committen).

- [ ] **Step 3: Clippy per Crate (OOM-sicher, nicht `--workspace`)**

Run: `cargo clippy -p shared` , dann `cargo clippy -p server` , dann `cargo clippy -p client`
Expected: jeweils keine Warnings (`-D warnings`-Baseline aus `.githooks/pre-push`).

- [ ] **Step 4: Tests per Crate (Windows-Caveat)**

Run nacheinander:
- `cargo test -p shared --target-dir target-test`
- `cargo test -p server --target-dir target-test`
- `cargo test -p client --target-dir target-test`

Expected: alle grün. Bei `E0786`/`STATUS_STACK_BUFFER_OVERRUN`/rlib-Link-Fehler: `cargo clean --target-dir target-test` und Run wiederholen (bekannte Q0016-Cache-Korruption, kein Bug).

- [ ] **Step 5: Abschluss-Commit (falls `cargo fmt` Diffs erzeugte)**

```bash
git add -A
git commit -m "chore(Q0014): cargo fmt + verification"
```

---

## Out of Scope (NICHT in diesem Plan — explizite Follow-up-Items)

- **Editor-UI-Hookup für Validatoren:** `validator_id` in die `editor.rs`-Load-Closure (`client/src/routes/editor.rs:~110`) verdrahten, sodass beim Editor-Load neben `import_required_from` auch Skript-Validatoren pro Spalte registriert und beim Save (`editor.rs:134`) ausgewertet werden. Q0014 beweist nur den `ValidationSystem::run`-Pfad system-level (Task 6).
- **Page-Level-DataSource-Routing für Skript-Filter:** Umschaltung `RemoteSource → LocalSource` in `EntityListPage`, damit der Skript-Filter im echten Tabellen-UI greift. Q0014 treibt `LocalSource` direkt (Task 8).
- **Serverseitiges Filtern / allgemeines Filter-Pushdown:** Server ignoriert `filter`-Args weiterhin.
- **Strukturierte Validator-Error-Payloads** (JSON via `script_value_to_json`): nur boolescher Pfad in Q0014.

---

## Self-Review-Notiz (gegen Spec)

| Spec-Anforderung | Task |
|---|---|
| `validator_id` additiv in `ColumnMeta` | Task 1 |
| `validator_id` + `allowed_validator_ids` in `FieldTypeDefaults` | Task 2 |
| Server `SimpleObject`-Re-Wrap + `convert_column` + Call-Sites | Task 3 |
| Client `COLUMNS_QUERY` + `RawColumnMeta` + `fetch_columns` | Task 4 |
| Resolver-Helper `"validator"`-Arm (beide Stufen) | Task 5 |
| ColumnMeta-Wire-Pin **neu** (Roundtrip + skip + backcompat) | Task 1+2 |
| Script-Validator-`TaskFn` LIVE via echtes `ValidationSystem::run` | Task 6 |
| `resolve_filter_id` `script:`-Guard (kein Built-in-Widget) | Task 7 |
| `LocalSource::passes` `script:`-Zweig + `selectedStackId`-Injektion | Task 8 |
| End-to-End-Filter-Test (include/exclude per `stackId`) | Task 8 |
| `DATA_DIR_FORMAT` bleibt `1` (Verifikation) | Task 9 |
| **NICHT** in `editor.rs`-Load-Closure verdrahten | Out of Scope |
| **KEIN** serverseitiges Filtern / Pushdown | Out of Scope |
