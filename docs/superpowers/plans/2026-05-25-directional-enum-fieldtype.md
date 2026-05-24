# FieldType::DirectionalEnum Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ein neues `FieldType::DirectionalEnum`-Primitive — eine int-gespeicherte Aufzählung, deren Werte ein Vorzeichen (`sign` +1/−1) tragen und über `amount_field` auf ein Betrags-Feld derselben Entität verweisen (Vorlage: D2V `ValueType` SOLL/HABEN gewichtet `value`).

**Architecture:** Strukturell wie `FieldType::IntEnum` (G7): DB-`i32` ↔ `wire_name`-String an der Source-Grenze, String auf der Leitung. Zusätzlich `sign` pro Wert + feldweites `amount_field`. Die int↔wire-Konvertierung wird als generischer Paar-Helper extrahiert, den IntEnum und DirectionalEnum teilen. Die Vorzeichen-**Anwendung** (Saldo) ist NICHT Teil dieser Welle — nur Daten/Wire/Darstellung/Validierung.

**Tech Stack:** Rust — `shared` (serde Wire-Typen), `server` (boundary-Konvertierung + data.rs-Integration), `client` (Leptos, FieldType-`match`-Arme). Tests: `cargo test`.

**Spec:** `docs/superpowers/specs/2026-05-25-directional-enum-fieldtype-design.md`.

**Build-Hinweis (Windows/Multi-Session):** target-dir `target-test`, `-j 2`, Exit-Code direkt erfassen (kein `| tail`). Commits mit explizitem Pathspec (fremde uncommittete Dateien wie `server/src/data.rs` ggf. vorhanden — nur eigene Pfade stagen).

---

## File Structure

**Modify:**
- `shared/src/lib.rs` — neue Variante `DirectionalEnum` + `DirectionalEnumValue`-Struct; `is_scalar`-Match-Arm.
- `shared/tests/field_type_wire_format.rs` — Wire-Roundtrip-Tests.
- `server/src/int_enum.rs` — Kern-Logik in Paar-Helper extrahieren (`decode_pairs`/`encode_pairs`); bestehende `decode`/`encode` darauf umbauen.
- `server/src/directional_enum.rs` *(neu)* — `decode`/`encode`/`specs`-Pendant für DirectionalEnum, nutzt den Paar-Helper.
- `server/src/lib.rs` *(oder mod-Deklaration)* — `pub mod directional_enum;`.
- `server/src/data.rs` — `directional_enum_specs` + decode/encode an den 4 IntEnum-Aufruf-Stellen (424, 454, 562, 614) + Validierung (~1224).
- `client/src/components/table/filters/registry.rs:72` — `DirectionalEnum` in den `enum-in`-Match.
- `client/src/components/table/filters/mod.rs` — FieldType-Match-Arm (non-exhaustive sonst).
- `client/src/components/table/builder_preview.rs:134` — Placeholder-Wert-Match-Arm.
- `client/src/components/registries/resolve.rs` — Default-Formatter-Resolution (falls FieldType-Match vorhanden).

**Vorlage:** `server/src/int_enum.rs` (decode/encode) und `shared/src/lib.rs::IntEnum` sind 1:1 das Muster.

---

## Tasks

### Task 1: shared Wire-Typ `DirectionalEnum` + `DirectionalEnumValue`

**Files:**
- Modify: `shared/src/lib.rs`
- Test: `shared/tests/field_type_wire_format.rs`

- [ ] **Step 1: Failing test** — an `shared/tests/field_type_wire_format.rs` anhängen:

```rust
#[test]
fn directional_enum_serializes_with_kind_amount_field_and_sign() {
    use shared::DirectionalEnumValue;
    let v = serde_json::to_value(FieldType::DirectionalEnum {
        values: vec![
            DirectionalEnumValue { value: 0, label_key: "soll".into(), wire_name: "SOLL".into(), sign: 1 },
            DirectionalEnumValue { value: 1, label_key: "haben".into(), wire_name: "HABEN".into(), sign: -1 },
        ],
        amount_field: "value".into(),
    })
    .unwrap();
    assert_eq!(v["kind"], "directionalEnum");
    assert_eq!(v["amountField"], "value");
    assert_eq!(v["values"][0]["wireName"], "SOLL");
    assert_eq!(v["values"][0]["sign"], 1);
    assert_eq!(v["values"][1]["sign"], -1);
}

#[test]
fn directional_enum_roundtrips() {
    use shared::DirectionalEnumValue;
    let original = FieldType::DirectionalEnum {
        values: vec![DirectionalEnumValue {
            value: 1, label_key: "haben".into(), wire_name: "HABEN".into(), sign: -1,
        }],
        amount_field: "value".into(),
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: FieldType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, original);
}
```

- [ ] **Step 2: Run — expect FAIL** (Variante/Struct existieren nicht):

```
cargo test -p shared --test field_type_wire_format --target-dir target-test directional_enum
```

- [ ] **Step 3: Implementieren** — in `shared/src/lib.rs` nach der `IntEnum`-Variante (nach Zeile ~182) ergänzen:

```rust
    /// Vorzeichenbehafteter Aufzählungstyp: jeder Wert traegt `sign` (+1/−1),
    /// das `amount_field` (Betrags-Feld derselben Entitaet) in Aggregationen
    /// gewichtet. Beispiel: ValueType SOLL(+1)/HABEN(−1) gewichtet `value`.
    /// Wie `IntEnum` int-gespeichert (DB-i32 ↔ wire_name); die Vorzeichen-
    /// Anwendung (Saldo) ist Aggregation (Welle 2), hier nur das Modell.
    DirectionalEnum {
        values: Vec<DirectionalEnumValue>,
        amount_field: String,
    },
```

Und nach dem `IntEnumValue`-Struct (nach Zeile ~193):

```rust
/// Ein Wert eines [`FieldType::DirectionalEnum`]: wie [`IntEnumValue`]
/// (DB-Zahl + Label + wire_name) plus `sign` (+1/−1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectionalEnumValue {
    pub value: i32,
    pub label_key: String,
    pub wire_name: String,
    pub sign: i8,
}
```

- [ ] **Step 4: Run — expect PASS:**

```
cargo test -p shared --test field_type_wire_format --target-dir target-test directional_enum
```

- [ ] **Step 5: Commit:**

```
git add shared/src/lib.rs shared/tests/field_type_wire_format.rs
git commit -m "feat(shared): FieldType::DirectionalEnum wire type (Welle 1)" -- shared/src/lib.rs shared/tests/field_type_wire_format.rs
```

---

### Task 2: `is_scalar` + alle shared-internen FieldType-Matches

**Files:**
- Modify: `shared/src/lib.rs` (`is_scalar`), ggf. `shared/src/ops.rs`

- [ ] **Step 1: Failing test** — an `shared/tests/field_type_wire_format.rs` anhängen:

```rust
#[test]
fn directional_enum_is_scalar() {
    use shared::DirectionalEnumValue;
    let ft = FieldType::DirectionalEnum {
        values: vec![DirectionalEnumValue { value: 0, label_key: "s".into(), wire_name: "S".into(), sign: 1 }],
        amount_field: "value".into(),
    };
    assert!(ft.is_scalar());
}
```

- [ ] **Step 2: Run — expect FAIL** (non-exhaustive match in `is_scalar`, oder falsches Ergebnis):

```
cargo test -p shared --test field_type_wire_format --target-dir target-test directional_enum_is_scalar
```

- [ ] **Step 3: Implementieren** — `FieldType::is_scalar` (`shared/src/lib.rs` ~195) um `DirectionalEnum` ergänzen, **wie `IntEnum`** behandelt (scalar = true). Falls `shared/src/ops.rs` ein FieldType-`match` hat (grep `FieldType::IntEnum` in ops.rs), denselben Arm wie IntEnum ergänzen.

- [ ] **Step 4: Run — expect PASS** (+ `cargo build -p shared --target-dir target-test` muss ohne non-exhaustive-Fehler bauen).

- [ ] **Step 5: Commit:**

```
git add shared/src/lib.rs shared/src/ops.rs shared/tests/field_type_wire_format.rs
git commit -m "feat(shared): DirectionalEnum scalar + ops match arms" -- shared/src/lib.rs shared/src/ops.rs shared/tests/field_type_wire_format.rs
```

---

### Task 3: Boundary-Konvertierung — Paar-Helper teilen

**Files:**
- Modify: `server/src/int_enum.rs`
- Create: `server/src/directional_enum.rs`
- Modify: `server/src/lib.rs` (mod-Deklaration)

- [ ] **Step 1: Failing test** — neue Datei `server/src/directional_enum.rs`, vorerst nur mit Tests + leerem Modul, das die API erwartet:

```rust
//! Grenz-Konvertierung fuer [`shared::FieldType::DirectionalEnum`].
//! Teilt die int<->wire-Logik mit `int_enum` ueber `boundary_enum`-Paare;
//! `sign` ist fuer die Konvertierung irrelevant (erst Aggregation/Welle 2).

use serde_json::Value;
use shared::DirectionalEnumValue;

pub fn decode(stored: &Value, values: &[DirectionalEnumValue]) -> Value {
    let pairs: Vec<(i32, &str)> = values.iter().map(|v| (v.value, v.wire_name.as_str())).collect();
    crate::int_enum::decode_pairs(stored, &pairs)
}

pub fn encode(incoming: &Value, values: &[DirectionalEnumValue]) -> Result<Value, crate::int_enum::IntEnumError> {
    let pairs: Vec<(i32, &str)> = values.iter().map(|v| (v.value, v.wire_name.as_str())).collect();
    crate::int_enum::encode_pairs(incoming, &pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn values() -> Vec<DirectionalEnumValue> {
        vec![
            DirectionalEnumValue { value: 0, label_key: "soll".into(), wire_name: "SOLL".into(), sign: 1 },
            DirectionalEnumValue { value: 1, label_key: "haben".into(), wire_name: "HABEN".into(), sign: -1 },
        ]
    }
    #[test]
    fn decode_known_int_to_wire() {
        assert_eq!(decode(&serde_json::json!(1), &values()), Value::String("HABEN".into()));
    }
    #[test]
    fn decode_unknown_to_null() {
        assert_eq!(decode(&serde_json::json!(99), &values()), Value::Null);
    }
    #[test]
    fn encode_known_name_to_int() {
        assert_eq!(encode(&serde_json::json!("SOLL"), &values()).unwrap(), serde_json::json!(0));
    }
    #[test]
    fn encode_unknown_name_errors() {
        assert!(encode(&serde_json::json!("X"), &values()).is_err());
    }
}
```

- [ ] **Step 2: `int_enum.rs` um Paar-Helper erweitern + bestehende fns darauf umbauen.** In `server/src/int_enum.rs` hinzufügen + `decode`/`encode` refactoren:

```rust
/// Kern: DB-i32 -> wire_name. Unbekannt -> Null (defensiv). Geteilt von
/// IntEnum + DirectionalEnum.
pub fn decode_pairs(stored: &Value, pairs: &[(i32, &str)]) -> Value {
    let Some(n) = stored.as_i64() else { return Value::Null; };
    match pairs.iter().find(|(val, _)| *val as i64 == n) {
        Some((_, wire)) => Value::String((*wire).to_string()),
        None => Value::Null,
    }
}

/// Kern: wire_name -> DB-i32. Null passthrough; unbekannt -> Err.
pub fn encode_pairs(incoming: &Value, pairs: &[(i32, &str)]) -> Result<Value, IntEnumError> {
    if incoming.is_null() { return Ok(Value::Null); }
    let Some(name) = incoming.as_str() else { return Err(IntEnumError::WrongType(type_name(incoming))); };
    let (val, _) = pairs.iter().find(|(_, wire)| *wire == name)
        .ok_or_else(|| IntEnumError::UnknownWireName(name.into()))?;
    Ok(Value::Number(serde_json::Number::from(*val)))
}
```

Dann `decode`/`encode` (IntEnum) auf die Paar-Helper umstellen:

```rust
pub fn decode(stored: &Value, values: &[IntEnumValue]) -> Value {
    let pairs: Vec<(i32, &str)> = values.iter().map(|v| (v.value, v.wire_name.as_str())).collect();
    decode_pairs(stored, &pairs)
}
pub fn encode(incoming: &Value, values: &[IntEnumValue]) -> Result<Value, IntEnumError> {
    let pairs: Vec<(i32, &str)> = values.iter().map(|v| (v.value, v.wire_name.as_str())).collect();
    encode_pairs(incoming, &pairs)
}
```

(`type_name` bleibt; `IntEnumError` bleibt — wird von beiden Modulen genutzt.)

- [ ] **Step 3: Modul registrieren** — in `server/src/lib.rs` bei den `pub mod`-Deklarationen (neben `pub mod int_enum;`):

```rust
pub mod directional_enum;
```

- [ ] **Step 4: Run — expect PASS** (int_enum-Tests bleiben grün, directional_enum-Tests neu grün):

```
cargo test -p server --lib --target-dir target-test int_enum directional_enum
```

- [ ] **Step 5: Commit:**

```
git add server/src/int_enum.rs server/src/directional_enum.rs server/src/lib.rs
git commit -m "feat(server): DirectionalEnum boundary conversion (shared int<->wire helper)" -- server/src/int_enum.rs server/src/directional_enum.rs server/src/lib.rs
```

---

### Task 4: data.rs-Integration (Read/Write/Validierung)

**Files:**
- Modify: `server/src/data.rs`

- [ ] **Step 1: `directional_enum_specs` hinzufügen** — neben `int_enum_specs` (~494):

```rust
/// Liefert `(key, values)` fuer alle `FieldType::DirectionalEnum`-Spalten.
fn directional_enum_specs(entity_type: &str) -> Vec<(String, Vec<shared::DirectionalEnumValue>)> {
    let Some(set) = crate::example::current() else { return Vec::new(); };
    let Some(et) = set.entities.get(entity_type) else { return Vec::new(); };
    et.columns.iter().filter_map(|c| match &c.field_type {
        shared::FieldType::DirectionalEnum { values, .. } => Some((c.key.clone(), values.clone())),
        _ => None,
    }).collect()
}
```

- [ ] **Step 2: Decode/Encode-Anwendung** — in `apply_int_enum_decode` (~511) und `apply_int_enum_encode` (~528) jeweils nach der IntEnum-Schleife eine analoge DirectionalEnum-Schleife ergänzen (gleiche Struktur, `crate::directional_enum::decode`/`encode`, `directional_enum_specs`). Beispiel decode-Ergänzung:

```rust
    for (key, values) in directional_enum_specs(entity_type) {
        if let Some(stored) = fields.get(&key) {
            let wired = crate::directional_enum::decode(stored, &values);
            fields.insert(key, wired);
        }
    }
```

encode-Ergänzung analog zur IntEnum-encode-Schleife (nur String-Werte, Fehler → `tracing::warn`).

- [ ] **Step 3: Validierung** — in der Validierungs-Funktion (~1224, neben der IntEnum-wire_name-Prüfung) eine DirectionalEnum-Prüfung ergänzen (eingehender Wert muss bekannter wire_name sein):

```rust
    for (key, values) in directional_enum_specs(entity_type) {
        if let Some(serde_json::Value::String(s)) = fields.get(&key) {
            if !s.is_empty() && !values.iter().any(|v| &v.wire_name == s) {
                result.push(ValidationMessage::error(key.clone(), "validation.enum_value"));
            }
        }
    }
```

- [ ] **Step 4: Run** — server lib + bestehende data-Tests:

```
cargo test -p server --lib --target-dir target-test data
cargo check -p server --target-dir target-test
```
Expected: grün, keine non-exhaustive-Fehler.

- [ ] **Step 5: Commit:**

```
git add server/src/data.rs
git commit -m "feat(server): DirectionalEnum read/write/validation in data.rs" -- server/src/data.rs
```

---

### Task 5: Client FieldType-Match-Arme

**Files:**
- Modify: `client/src/components/table/filters/registry.rs`, `client/src/components/table/filters/mod.rs`, `client/src/components/table/builder_preview.rs`, ggf. `client/src/components/registries/resolve.rs`

- [ ] **Step 1: Compile-Check zeigt die Lücken** — eine neue Variante macht alle FieldType-`match` non-exhaustive:

```
cargo check -p client --target-dir target-test 2>&1 | grep -E "non-exhaustive|DirectionalEnum"
```
Expected: Liste der Match-Stellen (wie damals bei IntEnum/G7).

- [ ] **Step 2: `filters/registry.rs:72`** — `DirectionalEnum { .. }` in den `enum-in`-Arm aufnehmen (wie `IntEnum`):

```rust
Reference { .. } | Collection { .. } | Enum { .. } | IntEnum { .. } | DirectionalEnum { .. } => &["enum-in"],
```

- [ ] **Step 3: `builder_preview.rs:134`** — Placeholder-Arm ergänzen (wie `IntEnum`: ersten `wire_name` als Vorschauwert):

```rust
        FieldType::DirectionalEnum { values, .. } => {
            values.first().map(|v| Value::String(v.wire_name.clone())).unwrap_or(Value::Null)
        }
```

- [ ] **Step 4: `filters/mod.rs` + `registries/resolve.rs`** — die dort vorhandenen FieldType-`match`-Arme analog zu `IntEnum`/`Enum` um `DirectionalEnum { .. }` ergänzen (Default-Formatter = Enum-Formatter; kein neuer Client-Default nötig). Exakte Arme aus dem Compile-Fehler von Step 1.

- [ ] **Step 5: Run — expect PASS:**

```
cargo check -p client --target-dir target-test
cargo test -p client --lib --target-dir target-test
```
Expected: keine non-exhaustive-Fehler, bestehende client-Tests grün.

- [ ] **Step 6: Commit:**

```
git add client/src/components/table/filters/registry.rs client/src/components/table/filters/mod.rs client/src/components/table/builder_preview.rs client/src/components/registries/resolve.rs
git commit -m "feat(client): DirectionalEnum field type match arms" -- client/src/components/table/filters/registry.rs client/src/components/table/filters/mod.rs client/src/components/table/builder_preview.rs client/src/components/registries/resolve.rs
```

---

### Task 6: Workspace-Verifikation + Spec-Status

**Files:** Verifikation; `docs/superpowers/specs/2026-05-25-directional-enum-fieldtype-design.md` (Status).

- [ ] **Step 1: Workspace-Test** (isoliert, Exit-Code direkt, kein `| tail`):

```
cargo test --workspace --target-dir target-test -j 2 > /tmp/de_verify.log 2>&1; echo "EXIT=$?"; grep -aE "test result|FAILED" /tmp/de_verify.log | grep -v "0 failed"
```
Expected: keine Fehl-Zeilen (alle grün). Bei flaky Concurrency-Fails betroffene Tests einzeln re-runnen (Memory `parallel-claude-sessions`).

- [ ] **Step 2: Spec-Status** — in `2026-05-25-directional-enum-fieldtype-design.md` `Status: Draft` → `Status: Implemented (Welle 1)`.

- [ ] **Step 3: Commit:**

```
git add docs/superpowers/specs/2026-05-25-directional-enum-fieldtype-design.md
git commit -m "docs(spec): DirectionalEnum implemented (Welle 1)" -- docs/superpowers/specs/2026-05-25-directional-enum-fieldtype-design.md
```

---

## Self-Review (gegen Spec)

- §3 Wire-Typ (`DirectionalEnum` + `DirectionalEnumValue`, `amount_field`, `sign`) → Task 1 ✓
- §3 eigene Variante + geteilte int↔wire-Konvertierung → Task 3 (Paar-Helper) ✓
- §3 Server `RawColumnMeta`/data.rs-Integration → Task 4 ✓ (Hinweis: `RawColumnMeta`→`ColumnMeta` parst `field_type` als JSON-Blob in den `FieldType`-Enum — eine neue Variante wird automatisch via serde geparst, kein zusätzlicher schema.rs-Arm nötig; in Task 4 verifiziert der `cargo check`, dass nichts bricht)
- §3 Client-Darstellung → Task 5 ✓
- §4 Validierung (`sign`, `amount_field`, wire_name-Eindeutigkeit) → **teilweise**: Task 4 prüft wire_name-Gültigkeit eingehender Werte. Die **Schema-Struktur-Validierung** (`sign ∈ {−1,+1}`, `amount_field` existiert) ist in Welle 1 bewusst leichtgewichtig — `sign: i8` erlaubt jeden Wert; eine harte Lade-Validierung ist ein optionaler Folge-Schritt (die Aggregation in Welle 2 nutzt `sign` als Faktor, ein falscher Wert fällt dort auf). **Entscheidung im Plan:** Welle 1 verzichtet auf den Lade-Hook, um Scope klein zu halten; in der Spec §4 als „soll" formuliert, hier als Welle-2-Vorbedingung vermerkt.
- §5 Tests → Tasks 1, 3 (Wire + Konvertierung); Client-Format implizit über Compile + bestehende Tests.

**Bewusste Abweichung von Spec §4:** Die strukturelle `sign`/`amount_field`-Lade-Validierung wird auf Welle 2 (Aggregation) verschoben — dort wird `sign` erst angewendet und ein ungültiger Wert wirksam. Welle 1 validiert nur eingehende Daten-Werte (wie IntEnum). Falls der User die harte Lade-Validierung schon in Welle 1 will, ist das eine zusätzliche Task (FieldType-Selbstvalidierung in shared + Loader-Hook).

## Was NICHT in diesem Plan ist

- Vorzeichen-Anwendung / Saldo-Aggregation → Welle 2 (1.7.12).
- Umstellung von `examples/d2v/entities/datev_entry` auf `directionalEnum` → separater Config-Schritt (Schicht 3).
- Dedizierter Direction-Editor (Eingabe-Widget) → generische Enum-Editor-Resolution reicht.
