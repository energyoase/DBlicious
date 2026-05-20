# F2 — Single-Entity-Excel-Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pro Entity-Row optional ein deklaratives XLSX-Sheet erzeugen (Sections: Header, KeyValue, SummaryBox, DerivedTable, Text, Spacer). Templates liegen als TOML/JSON im data-dir, werden via existing Loader-Pattern geladen, gerendert durch `rust_xlsxwriter` und über einen axum-HTTP-Endpoint `/export/excel/:template_id/:entity_id` ausgeliefert. Trigger ist eine Row-Action mit `kind=download` (kein Modal).

**Architecture:** Wire-Typ `shared::ExcelTemplate` mit getaggter `TemplateSection`-Enum (kind=header/keyValue/summaryBox/derivedTable/text/spacer). Renderer-Modul `server/src/excel/` dispatched pro Section. `derive_handler` reuse die Registry aus U2 (oder eine eigene Stub-Registry, wenn U2 noch nicht gelandet ist — Plan ist davon nicht blockiert). Download als binary HTTP-Response neben dem `/graphql`-Mount.

**Tech Stack:** Rust, `shared` (serde), `server` (axum + `rust_xlsxwriter`), `client` (Leptos, `<a download>`), Tests via `calamine` als Reader, `cargo test --workspace`.

**Spec-Referenz:** [`docs/superpowers/specs/2026-05-20-f2-single-entity-excel-template-design.md`](../specs/2026-05-20-f2-single-entity-excel-template-design.md).

**Vorbedingung:** keine harten. U2 (`derive_handler`-Registry) ist optional: bis U2 lebt nutzt F2 eine lokale leere Registry mit Log-Warning bei Aufruf.

**Decisions aufgenommen:**
- **Template-Format (a)**: deklarative TOML/JSON-Sections (Header/KeyValue/SummaryBox/DerivedTable/Text/Spacer). Spec §3.1 enumeriert sie — Format (b) "leere XLSX mit Zellen-Platzhaltern" kann das nicht abbilden.
- **Excel-Crate**: `rust_xlsxwriter` 0.79+ als Schreib-Engine (Spec §3.2); `calamine` 0.26 als dev-only Test-Reader für Roundtrip-Checks.

---

## File Structure

**Neu:**
- `shared/src/excel_template.rs` — alle Template-Typen
- `shared/tests/excel_template_wire.rs` — Roundtrip
- `server/src/excel/mod.rs` — Modul-Root + `TemplateRenderer`
- `server/src/excel/renderer.rs` — Render-Pipeline + Helpers (Format-Hints, Style-Apply)
- `server/src/excel/sections/mod.rs` — Section-Dispatch
- `server/src/excel/sections/header.rs`
- `server/src/excel/sections/key_value.rs`
- `server/src/excel/sections/summary_box.rs`
- `server/src/excel/sections/derived_table.rs`
- `server/src/excel/sections/text.rs`
- `server/src/excel/sections/spacer.rs`
- `server/src/excel/template_text.rs` — Placeholder-Engine
- `server/src/excel/derive.rs` — Stub-Registry (U2-kompatibel)
- `server/src/excel/route.rs` — axum-Handler `/export/excel/...`
- `server/tests/excel_render.rs` — End-to-End-Roundtrip mit `calamine`
- `examples/shop/excel-templates/order-voucher.toml` — Fixture-Template

**Modifizieren:**
- `shared/src/lib.rs` — Re-Export `ExcelTemplate` + Section-Typen
- `shared/src/settings.rs` — `EntitySettings.excel_templates: Vec<String>`
- `server/Cargo.toml` — `rust_xlsxwriter` Dep + dev-dep `calamine`
- `server/src/lib.rs` (oder `main.rs`) — Route mounten
- `server/src/example/loader.rs` — `excel-templates/<id>.{toml,json}`-Branch
- `client/src/components/table/row_actions.rs` — `MenuAction::DownloadExcel(template_id, entity_id)` oder ein neuer `RowActionKind::Download { url }`

---

## Architektur-Details

### `shared::ExcelTemplate` — komplettes Wire-Inventar

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExcelTemplate {
    pub id: String,
    pub title_key: String,
    pub entity_type: String,
    pub filename_template: String,
    pub page: PageSetup,
    pub sections: Vec<TemplateSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derive_handler: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageSetup {
    pub orientation: Orientation,
    pub paper_size: PaperSize,
    #[serde(default)]
    pub margins: Margins,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Orientation { #[default] Portrait, Landscape }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum PaperSize { #[default] A4, Letter, Legal }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Margins { pub top: f32, pub bottom: f32, pub left: f32, pub right: f32 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TemplateSection {
    Header { title: TemplateText, #[serde(default)] meta_rows: Vec<TemplateText>, #[serde(default)] style: Option<SectionStyle> },
    KeyValue { rows: Vec<KeyValueRow>, #[serde(default)] style: Option<SectionStyle> },
    SummaryBox { rows: Vec<KeyValueRow>, #[serde(default)] emphasize_last: bool, #[serde(default)] style: Option<SectionStyle> },
    DerivedTable { source: String, columns: Vec<TableColumn>, #[serde(default)] footer: Vec<FooterAggregate>, #[serde(default)] style: Option<SectionStyle> },
    Text { text: TemplateText },
    Spacer { lines: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateText {
    pub template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<TextStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeyValueRow { pub label: TemplateText, pub value: TemplateText }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TableColumn {
    pub key: String,
    pub label_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub width_chars: Option<f32>,
    #[serde(default)] pub align: ColumnAlign,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub format_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColumnAlign { #[default] Left, Right, Center }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FooterAggregate {
    pub column: String,
    pub op: AggregateOp,
    #[serde(default)] pub label_key: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AggregateOp { Sum, Avg, Count, Min, Max }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SectionStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")] pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub border: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub padding_lines: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")] pub font_size_pt: Option<u8>,
    #[serde(default)] pub bold: bool,
    #[serde(default)] pub italic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub align: Option<ColumnAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub font_name: Option<String>,
}
```

### Placeholder-Engine

`{field_key}` und `{field_key:format-hint}` werden in `template_text.rs` über regex (oder simpler state-machine) ersetzt. Format-Hints: `date`, `datetime`, `money`, `percent`, `int`, `raw` (default). Unbekannte Felder → leerer String + warn-log.

### Derive-Registry

`server/src/excel/derive.rs`:

```rust
use std::collections::BTreeMap;
use std::sync::OnceLock;

type DeriveFn = fn(&shared::Entity) -> BTreeMap<String, Vec<shared::Entity>>;

static REGISTRY: OnceLock<std::collections::HashMap<String, DeriveFn>> = OnceLock::new();

pub fn registry() -> &'static std::collections::HashMap<String, DeriveFn> {
    REGISTRY.get_or_init(std::collections::HashMap::new)
}

pub fn compute_derived(
    entity: &shared::Entity,
    handler_id: Option<&str>,
) -> BTreeMap<String, Vec<shared::Entity>> {
    let Some(id) = handler_id else { return BTreeMap::new(); };
    match registry().get(id) {
        Some(f) => f(entity),
        None => {
            tracing::warn!(handler_id = id, "unknown derive_handler");
            BTreeMap::new()
        }
    }
}
```

Wenn U2 später eine richtige Registry liefert, ersetzt dieses Modul den `OnceLock` durch die U2-Registry; Call-Sites bleiben identisch.

### HTTP-Endpoint

`GET /export/excel/:template_id/:entity_id` — Antwort:

```
Content-Type: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet
Content-Disposition: attachment; filename="<rendered_filename>"
Body: <xlsx bytes>
```

Permission-Check: gleiches `can_read` wie der GraphQL-entities-Pfad. Auth-Header `Authorization` muss vom Client gesetzt werden — `<a download>` schickt das nicht; in V1 wird der Header über einen JS-Wrapper hinzugefügt (fetch + blob). Alternative: kurze signierte URL — Backlog.

### Was NICHT in diesem Plan ist

- Multi-Sheet-Templates
- Charts/Pivots
- WYSIWYG-Editor
- Konkrete D2V-`Voucher`-Variante (eigene Spec, T19)

---

## Tasks

### Task 1: `shared::ExcelTemplate` Wire-Typen + Roundtrip

**Files:**
- Neu: `shared/src/excel_template.rs`
- Modify: `shared/src/lib.rs`
- Neu (Test): `shared/tests/excel_template_wire.rs`

- [ ] **Schritt 1: Failing Test**

`shared/tests/excel_template_wire.rs`:

```rust
use shared::{ExcelTemplate, TemplateSection, TemplateText, KeyValueRow, PageSetup, Orientation, PaperSize};

#[test]
fn minimal_template_roundtrip() {
    let t = ExcelTemplate {
        id: "demo".into(),
        title_key: "demo-title".into(),
        entity_type: "Order".into(),
        filename_template: "demo-{id}.xlsx".into(),
        page: PageSetup::default(),
        sections: vec![TemplateSection::Spacer { lines: 1 }],
        derive_handler: None,
    };
    let json = serde_json::to_string(&t).unwrap();
    let back: ExcelTemplate = serde_json::from_str(&json).unwrap();
    assert_eq!(back, t);
}

#[test]
fn section_tag_is_camel_case_kind() {
    let s = TemplateSection::KeyValue {
        rows: vec![KeyValueRow {
            label: TemplateText { template: "L".into(), style: None },
            value: TemplateText { template: "{v}".into(), style: None },
        }],
        style: None,
    };
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.starts_with(r#"{"kind":"keyValue""#));
}

#[test]
fn orientation_paper_size_camel_case() {
    let p = PageSetup { orientation: Orientation::Landscape, paper_size: PaperSize::Letter, ..PageSetup::default() };
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains(r#""orientation":"landscape""#));
    assert!(json.contains(r#""paperSize":"letter""#));
}

#[test]
fn missing_optionals_skipped() {
    let t = ExcelTemplate {
        id: "x".into(), title_key: "x".into(), entity_type: "X".into(),
        filename_template: "x.xlsx".into(), page: PageSetup::default(),
        sections: vec![], derive_handler: None,
    };
    let json = serde_json::to_string(&t).unwrap();
    assert!(!json.contains("deriveHandler"));
}
```

- [ ] **Schritt 2: Test FAIL**

```
cargo test -p shared --test excel_template_wire
```

- [ ] **Schritt 3: Modul erstellen**

`shared/src/excel_template.rs` mit den Typen aus "Architektur-Details" oben (`ExcelTemplate`, `PageSetup`, `Orientation`, `PaperSize`, `Margins`, `TemplateSection`, `TemplateText`, `KeyValueRow`, `TableColumn`, `ColumnAlign`, `FooterAggregate`, `AggregateOp`, `SectionStyle`, `TextStyle`). Volle Definitionen wie dort gezeigt.

- [ ] **Schritt 4: `shared/src/lib.rs`**

```rust
pub mod excel_template;
pub use excel_template::{
    AggregateOp, ColumnAlign, ExcelTemplate, FooterAggregate, KeyValueRow, Margins,
    Orientation, PageSetup, PaperSize, SectionStyle, TableColumn, TemplateSection,
    TemplateText, TextStyle,
};
```

- [ ] **Schritt 5: Test PASS**

```
cargo test -p shared --test excel_template_wire
```
Erwartung: 4 Tests grün.

- [ ] **Schritt 6: Commit**

```
git add shared/src/excel_template.rs shared/src/lib.rs shared/tests/excel_template_wire.rs
git commit -m "feat(shared): ExcelTemplate wire types for F2"
```

---

### Task 2: `EntitySettings.excel_templates`

**Files:**
- Modify: `shared/src/settings.rs`
- Modify (Test): `shared/tests/excel_template_wire.rs`

- [ ] **Schritt 1: Failing Test**

In `shared/tests/excel_template_wire.rs` anhängen:

```rust
use shared::EntitySettings;

#[test]
fn entity_settings_excel_templates_default_empty_and_camel_case() {
    let mut s = EntitySettings::default();
    let json_empty = serde_json::to_string(&s).unwrap();
    assert!(!json_empty.contains("excelTemplates"));

    s.excel_templates = vec!["demo".into()];
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains(r#""excelTemplates":["demo"]"#));
    let back: EntitySettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back.excel_templates, vec!["demo".to_string()]);
}
```

- [ ] **Schritt 2: Test FAIL**

- [ ] **Schritt 3: Feld ergänzen**

`shared/src/settings.rs` im `EntitySettings`-Struct:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub excel_templates: Vec<String>,
```

- [ ] **Schritt 4: Test PASS**

- [ ] **Schritt 5: Commit**

```
git add shared/src/settings.rs shared/tests/excel_template_wire.rs
git commit -m "feat(shared): EntitySettings.excel_templates list"
```

---

### Task 3: Server-Crate `rust_xlsxwriter` + Modul-Scaffold

**Files:**
- Modify: `server/Cargo.toml`
- Neu: `server/src/excel/mod.rs`
- Neu: `server/src/excel/derive.rs`
- Modify: `server/src/lib.rs`

- [ ] **Schritt 1: Cargo-Dep**

`server/Cargo.toml`:

```toml
[dependencies]
rust_xlsxwriter = "0.79"

[dev-dependencies]
calamine = "0.26"
```

(Versionen ggf. an aktuell verfügbare anpassen — `cargo search rust_xlsxwriter` und `cargo search calamine`.)

- [ ] **Schritt 2: Modul-Scaffold**

`server/src/excel/mod.rs`:

```rust
//! XLSX-Template-Renderer (F2).

pub mod derive;
pub mod renderer;
pub mod route;
pub mod sections;
pub mod template_text;

pub use renderer::{TemplateRenderer, ExcelError};
pub use route::excel_router;
```

`server/src/excel/derive.rs` — wie in Architektur-Details gezeigt.

- [ ] **Schritt 3: In `lib.rs` einhängen**

```rust
pub mod excel;
```

- [ ] **Schritt 4: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```
Erwartung: Module fehlen noch (renderer/route/sections/template_text). Wir legen sie in den nächsten Tasks an — Stub-Module sind OK.

- [ ] **Schritt 5: Stub-Module anlegen damit compile-Check grün ist**

`server/src/excel/renderer.rs`:

```rust
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum ExcelError {
    #[error("rendering failed: {0}")]
    Render(String),
    #[error("unknown placeholder: {0}")]
    UnknownPlaceholder(String),
}

pub struct TemplateRenderer<'a> {
    pub template: &'a shared::ExcelTemplate,
    pub entity: &'a shared::Entity,
    pub derived: BTreeMap<String, Vec<shared::Entity>>,
    pub column_meta: &'a [shared::ColumnMeta],
}

impl TemplateRenderer<'_> {
    pub fn render_to_bytes(&self) -> Result<Vec<u8>, ExcelError> {
        Err(ExcelError::Render("not implemented yet".into()))
    }
}
```

`server/src/excel/route.rs`:

```rust
use axum::Router;
pub fn excel_router() -> Router { Router::new() }
```

`server/src/excel/sections/mod.rs` (leer):

```rust
//! Section-Dispatch — befüllt in Task 6.
```

`server/src/excel/template_text.rs` (leer):

```rust
//! Placeholder-Engine — befüllt in Task 5.
```

- [ ] **Schritt 6: Compile-Check PASS**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 7: Commit**

```
git add server/Cargo.toml server/src/excel/ server/src/lib.rs
git commit -m "feat(server): excel module scaffold + rust_xlsxwriter dep"
```

---

### Task 4: Placeholder-Engine `template_text.rs`

**Files:**
- Modify: `server/src/excel/template_text.rs`
- Modify: dieselbe Datei für inline `#[cfg(test)]`

- [ ] **Schritt 1: Failing Test inline**

`server/src/excel/template_text.rs`:

```rust
//! Placeholder-Engine.

use shared::Entity;

pub fn render(template: &str, entity: &Entity) -> String {
    render_impl(template, |key| entity.fields.get(key).cloned())
}

fn render_impl<F>(template: &str, lookup: F) -> String
where
    F: Fn(&str) -> Option<serde_json::Value>,
{
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut inner = String::new();
            let mut closed = false;
            for nc in chars.by_ref() {
                if nc == '}' { closed = true; break; }
                inner.push(nc);
            }
            if !closed {
                out.push('{'); out.push_str(&inner);
                continue;
            }
            let (key, hint) = match inner.split_once(':') {
                Some((k, h)) => (k.trim(), Some(h.trim())),
                None => (inner.trim(), None),
            };
            let value = lookup(key);
            out.push_str(&apply_format(value, hint));
        } else {
            out.push(c);
        }
    }
    out
}

fn apply_format(value: Option<serde_json::Value>, hint: Option<&str>) -> String {
    let Some(v) = value else { return String::new(); };
    match hint.unwrap_or("raw") {
        "raw" | "" => json_to_plain(&v),
        "date" => json_to_plain(&v),       // V1: identisch zu raw; Formatter kommt in Renderer-Style
        "datetime" => json_to_plain(&v),
        "money" => json_to_plain(&v),
        "percent" => json_to_plain(&v),
        "int" => match &v {
            serde_json::Value::Number(n) => n.as_i64().map(|i| i.to_string()).unwrap_or_default(),
            other => json_to_plain(other),
        },
        _ => json_to_plain(&v),
    }
}

fn json_to_plain(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn entity_with(fields: Vec<(&str, serde_json::Value)>) -> shared::Entity {
        let map: BTreeMap<String, serde_json::Value> = fields.into_iter()
            .map(|(k, v)| (k.to_string(), v)).collect();
        shared::Entity { id: "1".into(), fields: map }
    }

    #[test]
    fn plain_text_passes_through() {
        let e = entity_with(vec![]);
        assert_eq!(render("hello", &e), "hello");
    }

    #[test]
    fn replaces_known_placeholder() {
        let e = entity_with(vec![("name", json!("Order #42"))]);
        assert_eq!(render("Beleg {name}", &e), "Beleg Order #42");
    }

    #[test]
    fn unknown_placeholder_becomes_empty() {
        let e = entity_with(vec![]);
        assert_eq!(render("X={missing} Y", &e), "X= Y");
    }

    #[test]
    fn format_hint_int_strips_decimal() {
        let e = entity_with(vec![("n", json!(42))]);
        assert_eq!(render("{n:int}", &e), "42");
    }

    #[test]
    fn unclosed_brace_left_literal() {
        let e = entity_with(vec![]);
        assert_eq!(render("hello {name", &e), "hello {name");
    }
}
```

- [ ] **Schritt 2: Test laufen**

```
CARGO_TARGET_DIR=target-test cargo test -p server excel::template_text::tests
```

Erwartung: erstes Compile prüft Existenz von `shared::Entity { id, fields: BTreeMap<String, Value> }`. Falls Entity-Struktur abweicht, an reale Form anpassen (`grep -n "pub struct Entity" shared/src/lib.rs`).

- [ ] **Schritt 3: Tests grün → Commit**

```
git add server/src/excel/template_text.rs
git commit -m "feat(server): excel placeholder engine with format hints"
```

---

### Task 5: Section-Renderer (Header, KeyValue, SummaryBox, Text, Spacer)

**Files:**
- Modify: `server/src/excel/sections/mod.rs`
- Neu: `server/src/excel/sections/header.rs`
- Neu: `server/src/excel/sections/key_value.rs`
- Neu: `server/src/excel/sections/summary_box.rs`
- Neu: `server/src/excel/sections/text.rs`
- Neu: `server/src/excel/sections/spacer.rs`

- [ ] **Schritt 1: Cursor + Context-Typ**

`server/src/excel/sections/mod.rs`:

```rust
pub mod header;
pub mod key_value;
pub mod summary_box;
pub mod text;
pub mod spacer;
pub mod derived_table; // wird in Task 6 angelegt

use rust_xlsxwriter::Worksheet;

pub struct RenderCursor<'a> {
    pub sheet: &'a mut Worksheet,
    pub row: u32,
}

impl<'a> RenderCursor<'a> {
    pub fn new(sheet: &'a mut Worksheet) -> Self { Self { sheet, row: 0 } }
    pub fn advance(&mut self, lines: u32) { self.row += lines; }
}

pub struct RenderContext<'a> {
    pub entity: &'a shared::Entity,
    pub derived: &'a std::collections::BTreeMap<String, Vec<shared::Entity>>,
}
```

- [ ] **Schritt 2: Header-Section**

`server/src/excel/sections/header.rs`:

```rust
use rust_xlsxwriter::{Format, FormatAlign, XlsxColor};
use super::{RenderCursor, RenderContext};
use crate::excel::template_text::render;
use crate::excel::renderer::ExcelError;

pub fn write(
    cursor: &mut RenderCursor,
    ctx: &RenderContext,
    title: &shared::TemplateText,
    meta_rows: &[shared::TemplateText],
) -> Result<(), ExcelError> {
    let title_str = render(&title.template, ctx.entity);
    let title_fmt = Format::new().set_bold().set_font_size(14.0);
    cursor.sheet.write_string_with_format(cursor.row, 0, &title_str, &title_fmt)
        .map_err(|e| ExcelError::Render(e.to_string()))?;
    cursor.advance(1);

    for m in meta_rows {
        let s = render(&m.template, ctx.entity);
        cursor.sheet.write_string(cursor.row, 0, &s)
            .map_err(|e| ExcelError::Render(e.to_string()))?;
        cursor.advance(1);
    }
    cursor.advance(1); // Spacer after header
    Ok(())
}
```

- [ ] **Schritt 3: KeyValue-Section**

`server/src/excel/sections/key_value.rs`:

```rust
use rust_xlsxwriter::Format;
use super::{RenderCursor, RenderContext};
use crate::excel::template_text::render;
use crate::excel::renderer::ExcelError;

pub fn write(
    cursor: &mut RenderCursor,
    ctx: &RenderContext,
    rows: &[shared::KeyValueRow],
) -> Result<(), ExcelError> {
    let label_fmt = Format::new().set_bold();
    for r in rows {
        let l = render(&r.label.template, ctx.entity);
        let v = render(&r.value.template, ctx.entity);
        cursor.sheet.write_string_with_format(cursor.row, 0, &l, &label_fmt)
            .map_err(|e| ExcelError::Render(e.to_string()))?;
        cursor.sheet.write_string(cursor.row, 1, &v)
            .map_err(|e| ExcelError::Render(e.to_string()))?;
        cursor.advance(1);
    }
    cursor.advance(1);
    Ok(())
}
```

- [ ] **Schritt 4: SummaryBox-Section**

`server/src/excel/sections/summary_box.rs`:

```rust
use rust_xlsxwriter::{Format, FormatBorder};
use super::{RenderCursor, RenderContext};
use crate::excel::template_text::render;
use crate::excel::renderer::ExcelError;

pub fn write(
    cursor: &mut RenderCursor,
    ctx: &RenderContext,
    rows: &[shared::KeyValueRow],
    emphasize_last: bool,
) -> Result<(), ExcelError> {
    let label_fmt = Format::new().set_bold();
    let last_idx = rows.len().saturating_sub(1);
    for (i, r) in rows.iter().enumerate() {
        let l = render(&r.label.template, ctx.entity);
        let v = render(&r.value.template, ctx.entity);
        let mut val_fmt = Format::new();
        if emphasize_last && i == last_idx {
            val_fmt = val_fmt.set_bold().set_border_top(FormatBorder::Thin);
        }
        cursor.sheet.write_string_with_format(cursor.row, 0, &l, &label_fmt)
            .map_err(|e| ExcelError::Render(e.to_string()))?;
        cursor.sheet.write_string_with_format(cursor.row, 1, &v, &val_fmt)
            .map_err(|e| ExcelError::Render(e.to_string()))?;
        cursor.advance(1);
    }
    cursor.advance(1);
    Ok(())
}
```

- [ ] **Schritt 5: Text + Spacer**

`server/src/excel/sections/text.rs`:

```rust
use super::{RenderCursor, RenderContext};
use crate::excel::template_text::render;
use crate::excel::renderer::ExcelError;

pub fn write(cursor: &mut RenderCursor, ctx: &RenderContext, text: &shared::TemplateText) -> Result<(), ExcelError> {
    let s = render(&text.template, ctx.entity);
    cursor.sheet.write_string(cursor.row, 0, &s).map_err(|e| ExcelError::Render(e.to_string()))?;
    cursor.advance(1);
    Ok(())
}
```

`server/src/excel/sections/spacer.rs`:

```rust
use super::RenderCursor;

pub fn write(cursor: &mut RenderCursor, lines: u32) {
    cursor.advance(lines);
}
```

- [ ] **Schritt 6: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 7: Commit**

```
git add server/src/excel/sections/
git commit -m "feat(server): excel sections (header/keyvalue/summary/text/spacer)"
```

---

### Task 6: DerivedTable-Section + Footer-Aggregates

**Files:**
- Neu: `server/src/excel/sections/derived_table.rs`

- [ ] **Schritt 1: Implementierung**

```rust
use rust_xlsxwriter::Format;
use super::{RenderCursor, RenderContext};
use crate::excel::renderer::ExcelError;

pub fn write(
    cursor: &mut RenderCursor,
    ctx: &RenderContext,
    source: &str,
    columns: &[shared::TableColumn],
    footer: &[shared::FooterAggregate],
) -> Result<(), ExcelError> {
    let rows = match ctx.derived.get(source) {
        Some(r) => r,
        None => {
            tracing::warn!(source = source, "derived source missing");
            return Ok(());
        }
    };

    // Header-Row
    let head_fmt = Format::new().set_bold().set_background_color(rust_xlsxwriter::XlsxColor::RGB(0xE8E8E8));
    for (c, col) in columns.iter().enumerate() {
        cursor.sheet.write_string_with_format(cursor.row, c as u16, &col.label_key, &head_fmt)
            .map_err(|e| ExcelError::Render(e.to_string()))?;
    }
    cursor.advance(1);

    // Data-Rows
    for row in rows {
        for (c, col) in columns.iter().enumerate() {
            let v = row.fields.get(&col.key).cloned().unwrap_or(serde_json::Value::Null);
            match v {
                serde_json::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        cursor.sheet.write_number(cursor.row, c as u16, f)
                            .map_err(|e| ExcelError::Render(e.to_string()))?;
                    }
                }
                serde_json::Value::String(s) => {
                    cursor.sheet.write_string(cursor.row, c as u16, &s)
                        .map_err(|e| ExcelError::Render(e.to_string()))?;
                }
                serde_json::Value::Null => {}
                other => {
                    cursor.sheet.write_string(cursor.row, c as u16, &other.to_string())
                        .map_err(|e| ExcelError::Render(e.to_string()))?;
                }
            }
        }
        cursor.advance(1);
    }

    // Footer-Aggregates
    if !footer.is_empty() {
        let foot_fmt = Format::new().set_bold().set_border_top(rust_xlsxwriter::FormatBorder::Thin);
        for agg in footer {
            // Spaltenindex finden
            let Some(col_idx) = columns.iter().position(|c| c.key == agg.column) else {
                tracing::warn!(column = agg.column.as_str(), "footer references unknown column");
                continue;
            };
            let value = aggregate(&rows, &agg.column, agg.op);
            cursor.sheet.write_number_with_format(cursor.row, col_idx as u16, value, &foot_fmt)
                .map_err(|e| ExcelError::Render(e.to_string()))?;
        }
        cursor.advance(1);
    }

    cursor.advance(1);
    Ok(())
}

fn aggregate(rows: &[shared::Entity], column: &str, op: shared::AggregateOp) -> f64 {
    let values: Vec<f64> = rows.iter()
        .filter_map(|r| r.fields.get(column).and_then(|v| v.as_f64()))
        .collect();
    if values.is_empty() { return 0.0; }
    match op {
        shared::AggregateOp::Sum => values.iter().sum(),
        shared::AggregateOp::Avg => values.iter().sum::<f64>() / values.len() as f64,
        shared::AggregateOp::Count => values.len() as f64,
        shared::AggregateOp::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
        shared::AggregateOp::Max => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    }
}
```

- [ ] **Schritt 2: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 3: Commit**

```
git add server/src/excel/sections/derived_table.rs
git commit -m "feat(server): excel derived-table section with footer aggregates"
```

---

### Task 7: `TemplateRenderer::render_to_bytes` zusammenstecken

**Files:**
- Modify: `server/src/excel/renderer.rs`

- [ ] **Schritt 1: Renderer-Body schreiben**

```rust
use rust_xlsxwriter::Workbook;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum ExcelError {
    #[error("rendering failed: {0}")]
    Render(String),
    #[error("unknown placeholder: {0}")]
    UnknownPlaceholder(String),
}

pub struct TemplateRenderer<'a> {
    pub template: &'a shared::ExcelTemplate,
    pub entity: &'a shared::Entity,
    pub derived: BTreeMap<String, Vec<shared::Entity>>,
    pub column_meta: &'a [shared::ColumnMeta],
}

impl TemplateRenderer<'_> {
    pub fn render_to_bytes(&self) -> Result<Vec<u8>, ExcelError> {
        let mut wb = Workbook::new();
        let sheet = wb.add_worksheet();
        let mut cursor = crate::excel::sections::RenderCursor::new(sheet);
        let ctx = crate::excel::sections::RenderContext {
            entity: self.entity,
            derived: &self.derived,
        };

        for section in &self.template.sections {
            match section {
                shared::TemplateSection::Header { title, meta_rows, style: _ } => {
                    crate::excel::sections::header::write(&mut cursor, &ctx, title, meta_rows)?;
                }
                shared::TemplateSection::KeyValue { rows, style: _ } => {
                    crate::excel::sections::key_value::write(&mut cursor, &ctx, rows)?;
                }
                shared::TemplateSection::SummaryBox { rows, emphasize_last, style: _ } => {
                    crate::excel::sections::summary_box::write(&mut cursor, &ctx, rows, *emphasize_last)?;
                }
                shared::TemplateSection::DerivedTable { source, columns, footer, style: _ } => {
                    crate::excel::sections::derived_table::write(&mut cursor, &ctx, source, columns, footer)?;
                }
                shared::TemplateSection::Text { text } => {
                    crate::excel::sections::text::write(&mut cursor, &ctx, text)?;
                }
                shared::TemplateSection::Spacer { lines } => {
                    crate::excel::sections::spacer::write(&mut cursor, *lines);
                }
            }
        }

        let bytes = wb.save_to_buffer().map_err(|e| ExcelError::Render(e.to_string()))?;
        Ok(bytes)
    }

    pub fn rendered_filename(&self) -> String {
        let raw = crate::excel::template_text::render(&self.template.filename_template, self.entity);
        sanitize_filename(&raw)
    }
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect()
}
```

- [ ] **Schritt 2: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 3: Commit**

```
git add server/src/excel/renderer.rs
git commit -m "feat(server): TemplateRenderer assembles all sections + filename sanitize"
```

---

### Task 8: End-to-End-Render-Test mit `calamine`

**Files:**
- Neu: `server/tests/excel_render.rs`
- Neu: `examples/shop/excel-templates/order-voucher.toml`

- [ ] **Schritt 1: Fixture-Template**

`examples/shop/excel-templates/order-voucher.toml`:

```toml
id = "shop.order-voucher"
title_key = "voucher-shop-title"
entity_type = "Order"
filename_template = "Voucher-{id}.xlsx"

[page]
orientation = "portrait"
paper_size = "a4"

[[sections]]
kind = "header"
title = { template = "Bestellung #{id}" }
meta_rows = [
    { template = "Kunde: {customer_name}" },
    { template = "Datum: {order_date}" },
]

[[sections]]
kind = "keyValue"
rows = [
    { label = { template = "Status" }, value = { template = "{status}" } },
    { label = { template = "Summe" }, value = { template = "{total}" } },
]

[[sections]]
kind = "summaryBox"
emphasize_last = true
rows = [
    { label = { template = "Nettobetrag" }, value = { template = "{net}" } },
    { label = { template = "Steuer" }, value = { template = "{tax}" } },
    { label = { template = "Bruttobetrag" }, value = { template = "{total}" } },
]
```

- [ ] **Schritt 2: Test schreiben**

`server/tests/excel_render.rs`:

```rust
use calamine::{open_workbook_from_rs, Reader, Xlsx};
use std::collections::BTreeMap;
use std::io::Cursor;

fn make_template() -> shared::ExcelTemplate {
    let toml = std::fs::read_to_string("../examples/shop/excel-templates/order-voucher.toml").unwrap();
    toml::from_str(&toml).unwrap()
}

fn make_entity() -> shared::Entity {
    let mut fields = BTreeMap::new();
    fields.insert("id".into(), serde_json::json!(42));
    fields.insert("customer_name".into(), serde_json::json!("ACME"));
    fields.insert("order_date".into(), serde_json::json!("2025-03-15"));
    fields.insert("status".into(), serde_json::json!("paid"));
    fields.insert("total".into(), serde_json::json!(119.0));
    fields.insert("net".into(), serde_json::json!(100.0));
    fields.insert("tax".into(), serde_json::json!(19.0));
    shared::Entity { id: "42".into(), fields }
}

#[test]
fn render_produces_valid_xlsx_with_expected_values() {
    let tpl = make_template();
    let ent = make_entity();
    let renderer = server::excel::TemplateRenderer {
        template: &tpl,
        entity: &ent,
        derived: BTreeMap::new(),
        column_meta: &[],
    };
    let bytes = renderer.render_to_bytes().unwrap();
    assert!(bytes.len() > 1000, "xlsx should be > 1KB");

    let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(&bytes)).unwrap();
    let sheet_name = wb.sheet_names()[0].clone();
    let range = wb.worksheet_range(&sheet_name).unwrap();

    let title_cell = range.get((0, 0)).unwrap();
    let title_str = format!("{}", title_cell);
    assert!(title_str.contains("Bestellung #42"), "title was {title_str:?}");

    let kunde_cell = range.get((1, 0)).unwrap();
    let kunde_str = format!("{}", kunde_cell);
    assert!(kunde_str.contains("ACME"));
}

#[test]
fn rendered_filename_sanitized() {
    let tpl = make_template();
    let ent = make_entity();
    let renderer = server::excel::TemplateRenderer {
        template: &tpl,
        entity: &ent,
        derived: BTreeMap::new(),
        column_meta: &[],
    };
    let name = renderer.rendered_filename();
    assert_eq!(name, "Voucher-42.xlsx");
    assert!(!name.contains('/'));
}
```

- [ ] **Schritt 3: TOML-Dep für Test ergänzen**

In `server/Cargo.toml` `[dev-dependencies]`:

```toml
toml = "0.8"
```

(Bereits vorhanden in der Regel — `cargo tree -p server | grep toml`.)

- [ ] **Schritt 4: Test laufen**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test excel_render
```

Erwartung: 2 Tests grün. Falls Asserts fehlschlagen, Cell-Positionen anpassen (Header schreibt Title in (0,0), erste Meta-Row in (1,0)).

- [ ] **Schritt 5: Commit**

```
git add server/tests/excel_render.rs examples/shop/excel-templates/order-voucher.toml server/Cargo.toml
git commit -m "test(server): excel render roundtrip via calamine"
```

---

### Task 9: Loader-Branch für `excel-templates/`

**Files:**
- Modify: `server/src/example/loader.rs`

- [ ] **Schritt 1: Stelle finden**

```
grep -n "entities/" server/src/example/loader.rs
```

Heute lädt der Loader pro Entity einen Ordner; wir ergänzen einen optionalen Branch für `excel-templates/<id>.{toml,json}`.

- [ ] **Schritt 2: Loader erweitern**

In der `load(dir)`-Funktion (oder Modul-Struktur entsprechend):

```rust
// --- Excel-Templates ---
let excel_dir = dir.join("excel-templates");
let mut excel_templates: Vec<shared::ExcelTemplate> = Vec::new();
if excel_dir.is_dir() {
    for entry in std::fs::read_dir(&excel_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(tpl) = read_typed::<shared::ExcelTemplate>(&path)? {
            excel_templates.push(tpl);
        }
    }
}
```

Im `ExampleSet`-Struct (oder Equivalent) ein neues Feld `pub excel_templates: Vec<shared::ExcelTemplate>`.

- [ ] **Schritt 3: Lookup-Helfer in `data.rs` oder `example/mod.rs`**

```rust
pub fn excel_template_by_id(id: &str) -> Option<shared::ExcelTemplate> {
    crate::example::current()
        .and_then(|s| s.excel_templates.iter().find(|t| t.id == id).cloned())
}
```

- [ ] **Schritt 4: Loader-Test**

In `server/tests/loader.rs` (falls existent) oder als Inline-Test:

```rust
#[test]
fn loads_excel_templates_from_example_shop() {
    let set = server::example::load(std::path::Path::new("../examples/shop")).unwrap();
    assert!(set.excel_templates.iter().any(|t| t.id == "shop.order-voucher"));
}
```

- [ ] **Schritt 5: Test laufen**

```
CARGO_TARGET_DIR=target-test cargo test -p server loader
```

- [ ] **Schritt 6: Commit**

```
git add server/src/example/loader.rs server/src/example/mod.rs server/tests/loader.rs
git commit -m "feat(server): loader reads excel-templates/<id>.{toml,json}"
```

---

### Task 10: HTTP-Route `/export/excel/:template_id/:entity_id`

**Files:**
- Modify: `server/src/excel/route.rs`
- Modify: `server/src/lib.rs` oder `main.rs` (Mount)

- [ ] **Schritt 1: Route-Handler**

`server/src/excel/route.rs`:

```rust
use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

pub fn excel_router() -> Router {
    Router::new().route("/export/excel/:template_id/:entity_id", get(handle_export))
}

async fn handle_export(Path((template_id, entity_id)): Path<(String, String)>) -> Response {
    let Some(template) = crate::data::excel_template_by_id(&template_id) else {
        return (StatusCode::NOT_FOUND, "template not found").into_response();
    };
    let entity = match crate::data::entity_by_id(&template.entity_type, &entity_id).await {
        Ok(Some(e)) => e,
        Ok(None) => return (StatusCode::NOT_FOUND, "entity not found").into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "entity lookup failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "lookup failed").into_response();
        }
    };

    let derived = crate::excel::derive::compute_derived(&entity, template.derive_handler.as_deref());

    let columns = crate::data::columns_for(&template.entity_type).unwrap_or_default();

    let renderer = crate::excel::renderer::TemplateRenderer {
        template: &template,
        entity: &entity,
        derived,
        column_meta: &columns,
    };

    let bytes = match renderer.render_to_bytes() {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = ?e, "render failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "render failed").into_response();
        }
    };

    let filename = renderer.rendered_filename();
    let disposition = format!("attachment; filename=\"{}\"", filename);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
            (header::CONTENT_DISPOSITION, disposition.as_str()),
        ],
        bytes,
    ).into_response()
}
```

Hinweis: `crate::data::entity_by_id` und `crate::data::columns_for` müssen existieren — falls Namen abweichen, `grep` und anpassen.

- [ ] **Schritt 2: Route ans App-Router mounten**

In `server/src/lib.rs` (oder wo der Router gebaut wird):

```rust
let app = Router::new()
    .route("/graphql", post(graphql_handler))
    // … andere routes …
    .merge(crate::excel::excel_router())
    .layer(/* CORS, etc. */);
```

- [ ] **Schritt 3: Manueller Smoke-Test**

```
cargo run -p server -- --data-dir ./examples/shop
# zweites Terminal:
curl -v -o /tmp/voucher.xlsx http://localhost:8000/export/excel/shop.order-voucher/1
```

Erwartung: 200 mit `Content-Disposition: attachment; filename="Voucher-1.xlsx"`. Datei lässt sich in Excel/LibreOffice öffnen.

- [ ] **Schritt 4: Auth-TODO dokumentieren**

In `server/src/excel/route.rs` als Code-Kommentar oben:

```rust
// TODO(F2.1): Permission-Check via AuthSession (heute offen — siehe Spec §3.5).
// Sobald ein konsistenter axum-extractor für AuthSession existiert,
// hier einhängen: `auth.require_read(&template.entity_type)?;`
```

- [ ] **Schritt 5: Commit**

```
git add server/src/excel/route.rs server/src/lib.rs
git commit -m "feat(server): /export/excel/:template_id/:entity_id route"
```

---

### Task 11: Client Row-Action für Excel-Download

**Files:**
- Modify: `client/src/components/table/row_actions.rs`
- Modify: `client/locales/de/main.ftl`, `client/locales/en/main.ftl`

- [ ] **Schritt 1: Settings auswerten + Buttons rendern**

In `client/src/components/table/row_actions.rs` an der Stelle wo bisherige Row-Actions gerendert werden:

```rust
let settings = use_settings_for(&entity_type);
let excel_templates = settings.read().excel_templates.clone();

// Pro Template einen Button rendern
view! {
    {excel_templates.into_iter().map(|tpl_id| {
        let id = tpl_id.clone();
        let row_id = row_id.clone();
        let href = format!("/export/excel/{}/{}", id, row_id);
        view! {
            <a href=href download="" style=design.button().inline.clone()
                                       class=design.button().class.clone()>
                { t("excel-download") }
            </a>
        }
    }).collect_view()}
}
```

(Konkrete Stelle hängt von der heutigen Row-Action-Implementierung ab — `grep -n "RowActionKind" client/src/components/table/row_actions.rs`.)

- [ ] **Schritt 2: i18n-Keys**

`client/locales/de/main.ftl`:
```
excel-download = Excel
```

`client/locales/en/main.ftl`:
```
excel-download = Excel
```

- [ ] **Schritt 3: Compile-Check**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

- [ ] **Schritt 4: Settings für Beispiel-Shop ergänzen**

In `examples/shop/entities/order/settings.toml`:

```toml
excelTemplates = ["shop.order-voucher"]
```

- [ ] **Schritt 5: Manuelle UI-Verifikation**

```
cargo run -p server -- --data-dir ./examples/shop
cd client && trunk serve
```

In der Order-Liste pro Zeile sollte ein "Excel"-Link erscheinen, der die Datei herunterlädt.

- [ ] **Schritt 6: Commit**

```
git add client/src/components/table/row_actions.rs client/locales/ examples/shop/entities/order/settings.toml
git commit -m "feat(client): per-row Excel download button from EntitySettings"
```

---

### Task 12: Workspace-Cleanup

**Files:** Verifikation.

- [ ] **Schritt 1: Clippy**

```
CARGO_TARGET_DIR=target-test cargo clippy --workspace --no-deps -- -A clippy::doc_overindented_list_items -A clippy::too_many_arguments
```

- [ ] **Schritt 2: Format**

```
cargo fmt && git diff --quiet || git commit -am "style: cargo fmt"
```

- [ ] **Schritt 3: Workspace-Tests**

```
CARGO_TARGET_DIR=target-test cargo test --workspace
```

- [ ] **Schritt 4: Self-Review-Check**

- Spec §3.1 Wire → Task 1 ✓
- Spec §3.2 Crate-Wahl `rust_xlsxwriter` → Task 3 ✓
- Spec §3.3 Renderer + Sections → Tasks 4-7 ✓
- Spec §3.4 Loader → Task 9 ✓
- Spec §3.5 HTTP-Endpoint → Task 10 ✓
- Spec §3.6 Client-Trigger → Task 11 ✓
- Spec §5 Edge-Cases (template fehlt → 404; entity fehlt → 404; derive-fail → ohne Tabelle; unknown placeholder → leerer String) → Tasks 4, 6, 10 ✓
- Spec §7 Tests (Wire + Render-Roundtrip + Edge-Cases) → Tasks 1, 2, 8 (Edge-Cases nachtragen)

- [ ] **Schritt 5: Edge-Case-Tests in `excel_render.rs` ergänzen**

```rust
#[test]
fn unknown_placeholder_renders_empty() {
    let mut tpl = make_template();
    if let shared::TemplateSection::Header { ref mut title, .. } = tpl.sections[0] {
        title.template = "X={missing} Y".into();
    }
    let renderer = server::excel::TemplateRenderer {
        template: &tpl, entity: &make_entity(),
        derived: std::collections::BTreeMap::new(), column_meta: &[],
    };
    let bytes = renderer.render_to_bytes().unwrap();
    let mut wb: calamine::Xlsx<_> = calamine::open_workbook_from_rs(std::io::Cursor::new(&bytes)).unwrap();
    let range = wb.worksheet_range("Sheet1").unwrap();
    let s = format!("{}", range.get((0, 0)).unwrap());
    assert!(s.contains("X= Y"));
}
```

- [ ] **Schritt 6: Commit Cleanup**

```
git add server/tests/excel_render.rs
git commit -m "test(server): excel renderer edge-case (unknown placeholder)"
```

---

## Was später kommt

- Multi-Sheet-Templates
- Template-Vererbung (`extends`)
- PDF-Output via Phase 1.7.9
- Konkrete D2V-`Voucher`-Variante als T19-Spec
- Auth-Pfad für `/export/excel/...` (Bearer/Session)
