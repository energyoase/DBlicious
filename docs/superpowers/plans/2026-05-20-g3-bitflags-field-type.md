# G3 — Bitflags as a Typed Field-Type Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `FieldType::Bitflags { values: Vec<BitflagValue> }` so the schema can express a multi-bit integer column (e.g. D2V `account.account_type`) as a labeled set, plus a default client-side `bitflags-checkbox-group` editor that renders one checkbox per bit.

**Architecture:**
- The storage type stays `DbColumnType::Integer` — the bitflags semantics live at the `FieldType` (field) layer, not the schema layer. Mirrors the spec's recommendation ("Schema-Layer = wie speichern, Field-Layer = wie darstellen").
- Wire form: tagged enum variant `{"kind":"bitflags","values":[…]}`, each value `{bit:u8,labelKey:String,default:bool}`.
- Client default editor implementation: register `bitflags-checkbox-group` so any column whose `field_type.kind == "bitflags"` falls back to it via Phase 1.5's resolution chain.

**Tech Stack:** Rust (`shared`), Leptos (`client`), serde.

**Source spec:** `docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md` §G3.

---

## File Structure

- Modify: `shared/src/lib.rs` — extend `FieldType` (around line 97) and add `BitflagValue`. Update `is_scalar` and `kind_str`.
- Modify: `shared/tests/field_type_wire_format.rs` — pin wire form for the new variant.
- Modify: `client/src/components/editor/mod.rs` (or wherever editors are registered) — register `bitflags-checkbox-group` as default for `field_type.kind == "bitflags"`.
- Create: `client/src/components/editor/bitflags.rs` — checkbox-group component bound to an integer cell.

---

## Task 1: `FieldType::Bitflags` wire form

**Files:**
- Modify: `shared/src/lib.rs:97-150`
- Test: `shared/tests/field_type_wire_format.rs`

- [ ] **Step 1: Write the failing test**

Append to `shared/tests/field_type_wire_format.rs`:

```rust
use shared::BitflagValue;

#[test]
fn bitflags_serializes_with_kind_and_values() {
    let v = serde_json::to_value(FieldType::Bitflags {
        values: vec![
            BitflagValue {
                bit: 0,
                label_key: "account-type.balance-sheet".into(),
                default: false,
            },
            BitflagValue {
                bit: 1,
                label_key: "account-type.profit".into(),
                default: true,
            },
        ],
    })
    .unwrap();
    assert_eq!(
        v,
        json!({
            "kind": "bitflags",
            "values": [
                {"bit": 0, "labelKey": "account-type.balance-sheet", "default": false},
                {"bit": 1, "labelKey": "account-type.profit", "default": true}
            ]
        })
    );
}

#[test]
fn bitflags_roundtrips() {
    let original = FieldType::Bitflags {
        values: vec![BitflagValue {
            bit: 7,
            label_key: "x".into(),
            default: false,
        }],
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: FieldType = serde_json::from_str(&json).unwrap();
    assert_eq!(original, back);
}

#[test]
fn bitflags_is_scalar() {
    let ft = FieldType::Bitflags { values: vec![] };
    assert!(ft.is_scalar());
    assert_eq!(ft.kind_str(), "bitflags");
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p shared --test field_type_wire_format`
Expected: compile error — `FieldType::Bitflags` and `BitflagValue` don't exist.

- [ ] **Step 3: Extend `FieldType`**

Edit `shared/src/lib.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BitflagValue {
    pub bit: u8,
    pub label_key: String,
    #[serde(default)]
    pub default: bool,
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
    /// Phase 0.7-G3: Multi-bit integer column rendered as a set of labeled checkboxes.
    Bitflags { values: Vec<BitflagValue> },
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
                | FieldType::Bitflags { .. }
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
            FieldType::Bitflags { .. } => "bitflags",
        }
    }
}
```

Re-export `BitflagValue` from the top of `shared/src/lib.rs` (in the `pub use` block).

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p shared --test field_type_wire_format`
Expected: PASS.

- [ ] **Step 5: Run the full shared suite**

Run: `cargo test -p shared`
Expected: PASS. Watch for any non-exhaustive `match FieldType { ... }` callsites — add a `FieldType::Bitflags { .. } => …` arm.

- [ ] **Step 6: Commit**

```bash
git add shared/src/lib.rs shared/tests/field_type_wire_format.rs
git commit -m "feat(shared): FieldType::Bitflags + BitflagValue"
```

---

## Task 2: Client default editor `bitflags-checkbox-group`

**Files:**
- Create: `client/src/components/editor/bitflags.rs`
- Modify: `client/src/components/editor/mod.rs` (or wherever editors are registered)

- [ ] **Step 1: Locate the editor registry**

Grep the client for how the existing editors register:

```
cargo run --quiet -p ... is not the right call; instead grep the source.
```

Use Grep tool: search for `"text-input"` or other editor IDs in `client/src/components/`. The registry is the file you'll modify in Step 3.

- [ ] **Step 2: Write the bitflags editor**

Create `client/src/components/editor/bitflags.rs`:

```rust
//! `bitflags-checkbox-group` — default editor for `FieldType::Bitflags`.
//!
//! Reads the current integer value, renders one checkbox per `BitflagValue`,
//! and writes the recombined integer on every toggle.

use leptos::*;
use shared::{BitflagValue, FieldType};

#[component]
pub fn BitflagsEditor(
    value: Signal<i64>,
    on_change: Callback<i64>,
    field_type: FieldType,
) -> impl IntoView {
    let values: Vec<BitflagValue> = match field_type {
        FieldType::Bitflags { values } => values,
        _ => Vec::new(), // editor only meaningful for Bitflags
    };

    view! {
        <div class="bitflags-checkbox-group">
            <For
                each=move || values.clone()
                key=|v| v.bit
                children=move |v: BitflagValue| {
                    let bit = v.bit;
                    let mask: i64 = 1i64 << bit;
                    let checked = Signal::derive(move || value.get() & mask != 0);
                    view! {
                        <label>
                            <input
                                type="checkbox"
                                prop:checked=checked
                                on:change=move |_| {
                                    let next = if checked.get() {
                                        value.get() & !mask
                                    } else {
                                        value.get() | mask
                                    };
                                    on_change.call(next);
                                }
                            />
                            <span>{v.label_key.clone()}</span>
                        </label>
                    }
                }
            />
        </div>
    }
}
```

- [ ] **Step 3: Register the editor**

In `client/src/components/editor/mod.rs` (or the registry module), add the bitflags editor and a `default_editor_id_for` arm mapping `"bitflags"` → `"bitflags-checkbox-group"`. Mirror the pattern already used for `"text"`, `"integer"`, …:

```rust
pub mod bitflags;

pub fn default_editor_for(kind: &str) -> &'static str {
    match kind {
        "bitflags" => "bitflags-checkbox-group",
        // ... existing arms
        _ => "text-input",
    }
}
```

If the registry uses a `HashMap<&str, Box<dyn EditorImpl>>` or a `match` on the kind string, extend whatever idiom is already in place; do not introduce a new abstraction.

- [ ] **Step 4: Build the client**

Run: `cd client && trunk build`
Expected: PASS.

- [ ] **Step 5: Smoke-test manually**

Add a column with `FieldType::Bitflags` in `examples/shop/entities/<some-entity>/columns.toml` (one bit will do), start server + client, open the entity editor in the browser, and confirm checkboxes appear and toggling them updates the underlying integer.

```bash
cargo run -p server -- --data-dir ./examples/shop
# in another shell:
cd client && trunk serve
```

- [ ] **Step 6: Commit**

```bash
git add client/src/components/editor/bitflags.rs client/src/components/editor/mod.rs
git commit -m "feat(client): bitflags-checkbox-group default editor for FieldType::Bitflags"
```

---

## Notes for the implementer

- This plan **does not** add a server-side conversion — bitflags columns serialize as plain integers on the wire. Decoding the bits is the client/UI's job. If/when a server-side `bitflags!`-style domain layer wants typed access, that's a separate concern.
- The `default` field on `BitflagValue` is **advisory only**: the client uses it as the initial value when creating a new row, but it does **not** override an existing column default coming from `DbColumn.default_value`.
- The `bit` field is `u8` to accommodate up to 64 bits (i64 storage). If a use case needs >64 bits, switch storage to BigInt; the editor will need rework anyway.
