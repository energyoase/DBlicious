# FieldType::DirectionalEnum — Design

Date: 2026-05-25
Status: Draft — awaiting user review
Welle: 1 (Framework-Primitives) der D2V-script-first-Umsetzung
(`2026-05-24-d2v-script-first-gap-analysis.md` §4b, Schicht 1).

## 1. Ziel

Ein neues `FieldType`-Primitive für **vorzeichenbehaftete Aufzählungen**: ein Enum-Feld, dessen Werte ein Vorzeichen (+1/−1) tragen und einem **Betrags-Feld** derselben Entität eine Richtung geben. Erster konkreter Anwendungsfall: D2V `ValueType` (SOLL/HABEN) gewichtet den Buchungs-`value` für den Konto-Saldo (Saldo = Σ SOLL − Σ HABEN).

**Generalisierung (Vier-Schichten-Modell, Schicht 1):** Das Primitive ist nicht D2V-spezifisch — jede doppelte Buchführung / jedes Soll-Haben-artige Modell nutzt es. D2V liefert nur die konkreten Werte + Labels als Config.

## 2. Scope

**Welle 1 (diese Spec):**
- Wire-Typ `FieldType::DirectionalEnum` in `shared`.
- Server: `RawColumnMeta`-Roundtrip + int↔`wire_name`-Konvertierung an der Quelle-Grenze (wie IntEnum/G7).
- Client: Darstellung (Formatter zeigt `label_key`/`wire_name` wie `Enum`).
- Manifest-/Schema-Validierung (`sign ∈ {−1, +1}`, `amount_field` referenziert existierende Spalte).

**Bewusst NICHT in dieser Welle:**
- Die Vorzeichen-**Anwendung** in Summen/Saldo. Das ist **Aggregation (1.7.12, Welle 2)**. Das Wire-Modell trägt `sign` + `amount_field` bereits, damit die Aggregation es direkt konsumiert, ohne erneute Schema-Änderung.
- Editor-Komponente (Auswahl-Widget) — die generische Enum-Editor-Resolution deckt die Eingabe vorerst; ein dedizierter Direction-Editor ist optional später.

## 3. Architektur

### Wire-Typ (`shared/src/lib.rs`)

```rust
/// Vorzeichenbehafteter Aufzählungstyp: jeder Wert trägt ein Vorzeichen,
/// das `amount_field` (ein Betrags-Feld derselben Entität) in Aggregationen
/// gewichtet. Beispiel: ValueType SOLL(+1)/HABEN(−1) gewichtet `value`.
DirectionalEnum {
    values: Vec<DirectionalEnumValue>,
    /// Schlüssel des Betrags-Felds derselben Entität, das dieses Feld
    /// vorzeichnet (z.B. "value"). Analog zu `Money.currency_code_field`.
    amount_field: String,
},

/// Ein Wert eines `DirectionalEnum`. Wie `IntEnumValue` (DB-int ↔ wire_name
/// ↔ label) plus `sign`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirectionalEnumValue {
    pub value: i32,
    pub label_key: String,
    pub wire_name: String,
    /// +1 oder −1. Bestimmt das Vorzeichen von `amount_field` in
    /// Aggregationen.
    pub sign: i8,
}
```

`FieldType` bleibt `#[serde(tag = "kind", rename_all = "camelCase")]` → `{"kind":"directionalEnum","values":[…],"amountField":"value"}`. Innere snake_case-Felder bleiben (`label_key`/`wire_name` → camelCase über `rename_all` auf der Struct; `amount_field` → `amountField`). Pinnen via `shared/tests/field_type_wire_format.rs`.

### Eigene Variante, nicht IntEnum-Erweiterung

`amount_field` ist feldweit, `sign` ist pro Wert — das passt nicht in `IntEnumValue`. Daher eigene Variante. **Geteilt** mit IntEnum wird aber die int↔`wire_name`-Konvertierung an der Server-Grenze: ein gemeinsamer Helper (z.B. `server/src/...`-Funktion `int_to_wire`/`wire_to_int` über eine `&[(i32, &str)]`-Liste), den beide Varianten nutzen — keine Duplikation der Konvertierungslogik.

### Server (`server/src/schema.rs`)

- `RawColumnMeta` → `ColumnMeta`-Parsing erweitern: `directionalEnum` wird wie `intEnum` behandelt (JSON-Blob → `FieldType`-Enum; Fallback auf `Text` bei Parse-Fehler bleibt).
- Boundary-Konvertierung: beim Lesen aus der Source wird der DB-`i32` → `wire_name` gemappt; beim Schreiben `wire_name` → `i32`. Identischer Pfad wie IntEnum — Helper teilen.

### Client (`client/src/components/...`)

- Formatter: `DirectionalEnum` rendert den `wire_name`/`label_key` des aktiven Werts (wie `Enum`). Das Vorzeichen ist in Welle 1 **nicht** visuell hervorgehoben (kommt mit der Saldo-Aggregation).
- Default-Resolution (`registries/resolve.rs`): `DirectionalEnum` fällt auf denselben Enum-Formatter-Default; kein neuer Client-Default zwingend nötig.

## 4. Validierung

Beim Manifest-/Schema-Laden (analog bestehender FieldType-Validierung):
- `sign` jedes Werts ∈ {−1, +1} — sonst Lade-Fehler.
- `amount_field` nicht leer; **soll** auf eine existierende Spalte derselben Entität zeigen (Money/Decimal/Integer). Verletzung → Warnung/Fehler (Konsistenz mit der Money.currency_code_field-Prüfung, falls vorhanden — sonst neue, milde Validierung mit `tracing::warn`).
- `wire_name`/`value` eindeutig innerhalb der `values`-Liste.

## 5. Tests

- `shared/tests/field_type_wire_format.rs`: Roundtrip von `DirectionalEnum` (camelCase-Tag `directionalEnum`, `amountField`, `sign`-Erhalt; serde-Roundtrip-Gleichheit).
- Server: int↔wire-Konvertierung (DB-`i32` ↔ `wire_name`) inkl. unbekannter Wert → Fallback-Verhalten; `RawColumnMeta`-Parse-Roundtrip.
- Client: Formatter zeigt den korrekten `wire_name`/`label` für einen DB-Wert; unbekannter Wert → leer/Fallback.
- Validierung: `sign=0`/`sign=2` → abgelehnt; fehlendes `amount_field` → abgelehnt.

## 6. Verhältnis zu bestehenden FieldTypes

- **IntEnum (G7)**: Strukturelle Vorlage; teilt die int↔wire-Konvertierung. DirectionalEnum = IntEnum + `sign` pro Wert + feldweites `amount_field`.
- **Enum**: nur String-Werte, keine int-Speicherung, keine Richtung. DirectionalEnum ist nicht dessen Erweiterung.
- **Money**: liefert das `amount_field`-Verweis-Muster (`currency_code_field`) als Vorlage für `amount_field`.

## 7. Offene Punkte (nicht blockierend, für D2V-Config = Schicht 3)

- **Echter DB-Typ von `ValueType`**: heute im d2v-Port als `{kind:"text"}` gemappt. Ob die SQLite-Spalte int oder string ist, ist eine **Port-/Mapping-Frage** (Schicht 3), nicht des Primitives. Das Primitive ist int-basiert (wie IntEnum); falls die Spalte string ist, mappt die foreign-sqlite-Binding bzw. ein Port-Schritt sie. In dieser Welle wird `datev_entry` **noch nicht** umgestellt — das Primitive entsteht zuerst, die D2V-Config-Umstellung ist ein separater kleiner Schritt.

## 8. Decisions

1. Eigene `FieldType`-Variante (nicht IntEnum-Erweiterung) wegen feldweitem `amount_field` + Wert-`sign`.
2. Int-basiert wie IntEnum (DB-`i32` ↔ `wire_name`), gemeinsamer Konvertierungs-Helper.
3. Welle 1 baut **Daten + Wire + Darstellung + Validierung**; die Saldo-**Anwendung** ist Aggregation (Welle 2), das Modell ist dafür vorbereitet.
4. Ein `amount_field` (Mehrfach-Vorzeichnung ist späteres Add-on).
5. D2V-`datev_entry`-Umstellung auf `directionalEnum` ist ein **separater** Config-Schritt (Schicht 3), nicht Teil dieser Welle.

## 9. Referenzen

- `2026-05-24-d2v-script-first-gap-analysis.md` §4b — Schichten-Klassifikation (ValueType → Schicht 1).
- `shared/src/lib.rs` — `FieldType`, `IntEnumValue` (G7-Vorlage).
- `examples/d2v/entities/datev_entry/{columns,binding}.json` — ValueType + value heute.
- Memory [[generalisierung-vier-schichten]].
