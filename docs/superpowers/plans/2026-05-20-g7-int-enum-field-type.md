# G7 — Typed Integer Enum Field Type Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A new `FieldType::IntEnum { values: Vec<IntEnumValue> }` that pairs an integer storage column with named wire-form values. The server converts on the boundary (read: i32 → `wire_name`; write: `wire_name` → i32) so the client sees a string while the DB sees an integer for storage/indexing efficiency.

**Architecture:**
- Wire form: `{"kind":"intEnum","values":[{"value":0,"labelKey":"…","wireName":"SOLL"}, …]}`.
- Storage stays `DbColumnType::Integer` — the field-type carries the mapping.
- Server boundary: a thin `int_enum::encode`/`decode` helper invoked from the entity read/write path (`server/src/data.rs`) when a column's resolved `FieldType` is `IntEnum`.
- Client gets strings; existing `Enum` editor/formatter works unchanged.

**Tech Stack:** Rust (`shared`, `server`), serde.

**Source spec:** `docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md` §G7.

---

## File Structure

- Modify: `shared/src/lib.rs` — extend `FieldType` (around line 97); add `IntEnumValue`; update `is_scalar` and `kind_str`.
- Modify: `shared/tests/field_type_wire_format.rs` — pin wire form.
- Create: `server/src/int_enum.rs` — `encode`/`decode` helpers used by the entity path.
- Modify: `server/src/data.rs` — apply `decode` after reading and `encode` before writing whenever a column's resolved `FieldType` is `IntEnum`. Hook is per column key — keep it narrow.
- Create: `server/tests/int_enum.rs` — end-to-end roundtrip.

---

## Task 1: `FieldType::IntEnum` wire form

**Files:**
- Modify: `shared/src/lib.rs`
- Test: `shared/tests/field_type_wire_format.rs`

- [ ] **Step 1: Write the failing test**

Append to `shared/tests/field_type_wire_format.rs`:

```rust
use shared::IntEnumValue;

#[test]
fn int_enum_serializes_with_kind_and_values() {
    let v = serde_json::to_value(FieldType::IntEnum {
        values: vec![
            IntEnumValue { value: 0, label_key: "journal.soll".into(), wire_name: "SOLL".into() },
            IntEnumValue { value: 1, label_key: "journal.haben".into(), wire_name: "HABEN".into() },
        ],
    })
    .unwrap();
    assert_eq!(
        v,
        json!({
            "kind": "intEnum",
            "values": [
                {"value": 0, "labelKey": "journal.soll", "wireName": "SOLL"},
                {"value": 1, "labelKey": "journal.haben", "wireName": "HABEN"}
            ]
        })
    );
}

#[test]
fn int_enum_roundtrips() {
    let original = FieldType::IntEnum {
        values: vec![IntEnumValue { value: 7, label_key: "x".into(), wire_name: "X".into() }],
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: FieldType = serde_json::from_str(&json).unwrap();
    assert_eq!(original, back);
}

#[test]
fn int_enum_is_scalar() {
    let ft = FieldType::IntEnum { values: vec![] };
    assert!(ft.is_scalar());
    assert_eq!(ft.kind_str(), "intEnum");
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p shared --test field_type_wire_format`
Expected: compile error — `FieldType::IntEnum`, `IntEnumValue` don't exist.

- [ ] **Step 3: Extend `FieldType`**

Edit `shared/src/lib.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntEnumValue {
    pub value: i32,
    pub label_key: String,
    pub wire_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FieldType {
    Text,
    Integer,
    Decimal { precision: u8 },
    Boolean,
    Date,
    DateTime,
    Money { currency_code_field: Option<String> },
    Reference { entity: String },
    Collection { entity: String },
    Enum { values: Vec<String> },
    /// Phase 0.7-G7: integer-stored, name-on-the-wire enum.
    IntEnum { values: Vec<IntEnumValue> },
}

impl FieldType {
    pub fn is_scalar(&self) -> bool {
        matches!(
            self,
            FieldType::Text
                | FieldType::Integer
                | FieldType::Decimal { .. }
                | FieldType::Boolean
                | FieldType::Date
                | FieldType::DateTime
                | FieldType::Money { .. }
                | FieldType::Enum { .. }
                | FieldType::IntEnum { .. }
        )
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            FieldType::Text => "text",
            FieldType::Integer => "integer",
            FieldType::Decimal { .. } => "decimal",
            FieldType::Boolean => "boolean",
            FieldType::Date => "date",
            FieldType::DateTime => "dateTime",
            FieldType::Money { .. } => "money",
            FieldType::Reference { .. } => "reference",
            FieldType::Collection { .. } => "collection",
            FieldType::Enum { .. } => "enum",
            FieldType::IntEnum { .. } => "intEnum",
        }
    }
}
```

Re-export `IntEnumValue` from the top of `shared/src/lib.rs`.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p shared --test field_type_wire_format`
Expected: PASS.

- [ ] **Step 5: Run the full shared suite**

Run: `cargo test -p shared`
Expected: PASS. Add an `IntEnum { .. } => …` arm to any non-exhaustive `FieldType` match in callers.

- [ ] **Step 6: Commit**

```bash
git add shared/src/lib.rs shared/tests/field_type_wire_format.rs
git commit -m "feat(shared): FieldType::IntEnum + IntEnumValue"
```

---

## Task 2: Server-side encode/decode helpers

**Files:**
- Create: `server/src/int_enum.rs`
- Modify: `server/src/lib.rs` — `pub mod int_enum;`
- Test: unit tests inline in `int_enum.rs`.

- [ ] **Step 1: Write the failing helper tests**

Create `server/src/int_enum.rs`:

```rust
//! Boundary conversion for [`FieldType::IntEnum`].
//!
//! Read path (DB → wire): `decode(i32, &values)` → wire name `String`.
//! Write path (wire → DB): `encode(&str, &values)` → `i32`.
//! Unknown values are passed through as `Null` (read) or rejected with an
//! [`IntEnumError`] (write) — silently mapping a typo to a wrong integer
//! would be the worst possible outcome here.

use serde_json::Value;
use shared::IntEnumValue;

#[derive(Debug, thiserror::Error)]
pub enum IntEnumError {
    #[error("unknown intEnum wire name: {0}")]
    UnknownWireName(String),
    #[error("expected string for intEnum value, got {0}")]
    WrongType(&'static str),
}

pub fn decode(stored: &Value, values: &[IntEnumValue]) -> Value {
    let Some(n) = stored.as_i64() else { return Value::Null };
    match values.iter().find(|v| v.value as i64 == n) {
        Some(v) => Value::String(v.wire_name.clone()),
        None => Value::Null,
    }
}

pub fn encode(incoming: &Value, values: &[IntEnumValue]) -> Result<Value, IntEnumError> {
    if incoming.is_null() {
        return Ok(Value::Null);
    }
    let Some(name) = incoming.as_str() else {
        return Err(IntEnumError::WrongType(type_name(incoming)));
    };
    let v = values
        .iter()
        .find(|v| v.wire_name == name)
        .ok_or_else(|| IntEnumError::UnknownWireName(name.into()))?;
    Ok(Value::Number(serde_json::Number::from(v.value)))
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> Vec<IntEnumValue> {
        vec![
            IntEnumValue { value: 0, label_key: "soll".into(), wire_name: "SOLL".into() },
            IntEnumValue { value: 1, label_key: "haben".into(), wire_name: "HABEN".into() },
        ]
    }

    #[test]
    fn decode_known_int_to_wire_name() {
        assert_eq!(decode(&serde_json::json!(0), &values()), Value::String("SOLL".into()));
        assert_eq!(decode(&serde_json::json!(1), &values()), Value::String("HABEN".into()));
    }

    #[test]
    fn decode_unknown_int_to_null() {
        assert_eq!(decode(&serde_json::json!(99), &values()), Value::Null);
    }

    #[test]
    fn encode_known_name_to_int() {
        assert_eq!(
            encode(&serde_json::json!("SOLL"), &values()).unwrap(),
            serde_json::json!(0)
        );
    }

    #[test]
    fn encode_unknown_name_errors() {
        let err = encode(&serde_json::json!("MAYBE"), &values()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("MAYBE"));
    }

    #[test]
    fn encode_wrong_type_errors() {
        let err = encode(&serde_json::json!(7), &values()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("number"));
    }

    #[test]
    fn encode_null_passes_through() {
        assert_eq!(encode(&Value::Null, &values()).unwrap(), Value::Null);
    }
}
```

Edit `server/src/lib.rs` (or `server/src/main.rs` if there's no `lib.rs`) to declare the module:

```rust
pub mod int_enum;
```

- [ ] **Step 2: Run the tests and confirm they pass**

Run: `cargo test -p server --lib int_enum::tests --target-dir target-test`
Expected: PASS (6 tests).

- [ ] **Step 3: Commit**

```bash
git add server/src/int_enum.rs server/src/lib.rs
git commit -m "feat(server): int_enum encode/decode helpers for FieldType::IntEnum"
```

---

## Task 3: Apply encode/decode at the entity boundary

**Files:**
- Modify: `server/src/data.rs` — call `int_enum::decode` after reading entity fields; call `int_enum::encode` before persisting.
- Test: `server/tests/int_enum.rs` (new).

- [ ] **Step 1: Write the failing end-to-end test**

Create `server/tests/int_enum.rs`:

```rust
//! End-to-end: a column declared as `FieldType::IntEnum` round-trips through
//! create → fetch as the wire-name string, while the DB stores the integer.

use serial_test::serial;
use server::{data, example, fresh_test_setup};

const ENTITY: &str = "journal_entry";

async fn install_int_enum_column() {
    // The example loader normally installs columns from disk. For this test,
    // we synthesize a minimal `ColumnSet` with a single `value_type` column
    // typed as IntEnum {0 -> SOLL, 1 -> HABEN}.
    let mut bundle = shared::SettingsBundle::default();
    let _ = bundle.ensure(ENTITY);

    let columns = vec![shared::ColumnMeta {
        key: "value_type".into(),
        label_key: "journal.value-type".into(),
        field_type: shared::FieldType::IntEnum {
            values: vec![
                shared::IntEnumValue { value: 0, label_key: "soll".into(), wire_name: "SOLL".into() },
                shared::IntEnumValue { value: 1, label_key: "haben".into(), wire_name: "HABEN".into() },
            ],
        },
        sortable: true,
        filterable: false,
        comparator_id: None,
        filter_id: None,
        editor_id: None,
        formatter_id: None,
        action_ids: vec![],
    }];

    example::install_columns_for_tests(ENTITY, columns);
    example::install_settings_for_tests(bundle);
}

#[tokio::test]
#[serial]
async fn create_with_wire_name_persists_int_and_reads_back_as_name() {
    fresh_test_setup().await;
    install_int_enum_column().await;

    let created = data::create_entity(
        ENTITY,
        serde_json::json!({"value_type": "SOLL"}).as_object().unwrap().clone(),
    )
    .await
    .expect("create");

    // Reading back: server has translated 0 → "SOLL".
    let fetched = data::get_entity(ENTITY, &created.id).await.expect("get");
    assert_eq!(
        fetched.fields.get("value_type"),
        Some(&serde_json::Value::String("SOLL".into()))
    );
}

#[tokio::test]
#[serial]
async fn create_with_unknown_name_returns_validation_error() {
    fresh_test_setup().await;
    install_int_enum_column().await;

    let result = data::create_entity(
        ENTITY,
        serde_json::json!({"value_type": "MAYBE"}).as_object().unwrap().clone(),
    )
    .await;

    assert!(result.is_err(), "unknown wire name must fail validation");
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p server --test int_enum --target-dir target-test`
Expected: compile error / fail — `example::install_columns_for_tests` doesn't exist; encoding/decoding isn't applied.

- [ ] **Step 3: Add the test helper**

In `server/src/example/mod.rs` (next to `install_settings_for_tests` if you wrote it for G1, otherwise add both):

```rust
#[cfg(any(test, feature = "test-helpers"))]
pub fn install_columns_for_tests(entity_type: &str, columns: Vec<shared::ColumnMeta>) {
    // Splice the columns into the installed example set. Adapt to the actual
    // shape of the `INSTALLED` slot in this file.
    INSTALLED.write().expect("install lock").as_mut().map(|set| {
        set.columns.insert(entity_type.to_string(), columns);
    });
}
```

- [ ] **Step 4: Apply encode/decode in `data.rs`**

In `server/src/data.rs`, find the path that materializes an entity for return (around `pub async fn get_entity` and `list_entities`) and the path that ingests one for write (`create_entity`, `update_entity`). Wrap each with a column-aware transform:

```rust
fn apply_int_enum_decode(entity_type: &str, fields: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(cols) = columns_for(entity_type) else { return };
    for c in cols {
        if let shared::FieldType::IntEnum { values } = &c.field_type {
            if let Some(stored) = fields.get(&c.key) {
                let wired = crate::int_enum::decode(stored, values);
                fields.insert(c.key.clone(), wired);
            }
        }
    }
}

fn apply_int_enum_encode(
    entity_type: &str,
    fields: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), crate::int_enum::IntEnumError> {
    let Some(cols) = columns_for(entity_type) else { return Ok(()) };
    for c in cols {
        if let shared::FieldType::IntEnum { values } = &c.field_type {
            if let Some(incoming) = fields.get(&c.key) {
                let stored = crate::int_enum::encode(incoming, values)?;
                fields.insert(c.key.clone(), stored);
            }
        }
    }
    Ok(())
}
```

Call `apply_int_enum_decode` right before returning entities (in `get_entity` and `list_entities`). Call `apply_int_enum_encode` at the start of `create_entity`/`update_entity`; propagate its error as a validation message in the existing `EntityChangeResult` shape.

(`columns_for(entity_type)` is the existing accessor used to resolve `ColumnMeta` for an entity type; if it doesn't exist by that name, follow the access pattern already used in `schema.rs::columns` resolver.)

- [ ] **Step 5: Run the tests and confirm they pass**

Run: `cargo test -p server --test int_enum --target-dir target-test`
Expected: PASS (2 tests).

- [ ] **Step 6: Run the full server suite**

Run: `cargo test -p server --target-dir target-test`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add server/src/data.rs server/src/example/mod.rs server/tests/int_enum.rs
git commit -m "feat(server): apply int_enum encode/decode at entity read/write boundary"
```

---

## Notes for the implementer

- **Client side is unchanged.** The client sees a regular string; the existing `enum` editor (`FieldType::Enum`) can be reused by emitting a synthetic `FieldType::Enum { values: <wire_names> }` from the server's resolved-columns endpoint. If you want a dedicated `intEnum` editor, that's a follow-up — but for v1, treating it as a string-valued enum on the wire is enough.
- **Sorting/filtering:** since the wire form is a string but the DB stores an integer, naïve text sort yields surprising order. If sorting matters, the server should sort by the *underlying integer column* and return values in that order — already true if the columns layer pushes sort keys to SQL. Add a regression test the day a use case actually depends on order.
- **Out of scope:** changing the underlying column type. `IntEnum` ships only the field-type plus boundary conversion; the schema column stays `DbColumnType::Integer`.
