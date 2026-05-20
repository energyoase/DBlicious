# F2 — Single-Entity-Excel-Template Spec

Date: 2026-05-20
Status: Draft — awaiting user review
Trigger: T19 (Voucher — XLSX-Beleg pro DATEV-Buchung).
Quelle: Gap-Analyse §4.3 (F2), ROADMAP §1.8 F2.
Visual: `.superpowers/brainstorm/.../f2-voucher.html` — Voucher-Layout-Beispiel.
Abgrenzung zu Phase 1.7.14: 1.7.14 ist Listen-Export (n Rows → eine Tabelle). F2 ist **eine Row → ein gestaltetes Sheet** mit Sections, Spannings, Styles. Kein Overlap.

## 1. Ziel

Pro Entity-Row optional ein XLSX-Sheet erzeugen, das

- frei deklarativ aufgebaut ist (Sections, Tabellen, Summen-Boxen, Header/Footer),
- Felder der Row + abgeleitete Rows (z.B. AccountEntries einer Buchung) bindet,
- Style-Tokens nutzt (Schriften, Farben, Borders) für konsistentes Look-and-Feel,
- per Row-Action "📄 Beleg drucken" (oder Button im Editor) heruntergeladen wird.

Mehrere Templates pro Entity-Typ möglich (z.B. "standard-voucher", "kompakt-voucher").

## 2. Nicht-Ziele

- WYSIWYG-Template-Editor (Templates werden per Hand geschrieben).
- PDF-Generation aus dem gleichen Template — Phase 1.7.9 hat eigene PDF-Engine (T18 Jahresabschluss). Cross-Engine-Templates lohnen sich nicht für eine Spec.
- Mehrere Rows pro Template (Bulk-Voucher) — kann per for-Schleife in der Aufruf-Site geschehen.
- Beliebige Formel-Engine (Excel-Formeln werden statisch eingefügt, keine dynamische Auswertung).

## 3. Architektur

### 3.1 `shared::ExcelTemplate`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExcelTemplate {
    pub id: String,                       // "datev.standard-voucher"
    pub title_key: String,
    pub entity_type: String,              // "DatevEntry"
    pub filename_template: String,        // "Voucher-{id}-{posted_at:date}.xlsx"
    pub page: PageSetup,
    pub sections: Vec<TemplateSection>,
    /// Optionaler derive_handler (siehe U2) — liefert abgeleitete Rows
    /// (z.B. AccountEntries) für die Template-Bindings.
    #[serde(default)]
    pub derive_handler: Option<String>,
}

pub struct PageSetup {
    pub orientation: Orientation,         // Portrait | Landscape
    pub paper_size: PaperSize,
    pub margins: Margins,                  // in mm
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TemplateSection {
    /// Free-form Header mit Titel + Meta.
    Header {
        title: TemplateText,
        meta_rows: Vec<TemplateText>,
        style: Option<SectionStyle>,
    },
    /// Key-Value-Tabelle (zwei Spalten).
    KeyValue {
        rows: Vec<KeyValueRow>,
        style: Option<SectionStyle>,
    },
    /// Summen-Box mit Label/Value-Paaren, optional Trennlinie vor letztem.
    SummaryBox {
        rows: Vec<KeyValueRow>,
        emphasize_last: bool,
        style: Option<SectionStyle>,
    },
    /// Tabelle aus abgeleiteten Rows (z.B. AccountEntries der Buchung).
    DerivedTable {
        source: String,                   // Name der derived-Result-Liste, z.B. "accountEntries"
        columns: Vec<TableColumn>,
        footer: Vec<FooterAggregate>,
        style: Option<SectionStyle>,
    },
    /// Konstanter Text oder Spacer.
    Text { text: TemplateText },
    Spacer { lines: u32 },
}

pub struct TemplateText {
    /// Mustache-ähnliches Template: "Buchung #{id} · {posted_at:date}"
    /// Spalten-Platzhalter: {field_key} oder {field_key:format-hint}
    /// Format-Hints: :date | :datetime | :money | :percent | :int | :raw
    pub template: String,
    pub style: Option<TextStyle>,
}

pub struct KeyValueRow {
    pub label: TemplateText,
    pub value: TemplateText,
}

pub struct TableColumn {
    pub key: String,
    pub label_key: String,
    pub width_chars: Option<f32>,
    pub align: ColumnAlign,
    pub format_hint: Option<String>,       // "money", "date", "raw"
}

pub struct SectionStyle {
    pub background: Option<HexColor>,
    pub border: Option<BorderStyle>,
    pub padding_lines: Option<u32>,
}

pub struct TextStyle {
    pub font_size_pt: Option<u8>,
    pub bold: bool,
    pub italic: bool,
    pub color: Option<HexColor>,
    pub background: Option<HexColor>,
    pub border: Option<BorderStyle>,
    pub align: Option<ColumnAlign>,
}
```

### 3.2 Renderer-Crate-Wahl

`rust_xlsxwriter` (Apache-2.0, kein C-Dependency, gut gepflegt). Alternative `umya-spreadsheet` (mehr Excel-Features wie Charts, aber schwerere Dependency). Wir starten mit `rust_xlsxwriter`; falls Charts/Pivots in Templates später dazukommen, ist `umya` der Upgrade-Pfad.

Neue Server-Dependency: `rust_xlsxwriter = "..."` in `server/Cargo.toml`.

### 3.3 Renderer

`server/src/excel/renderer.rs`:

```rust
pub struct TemplateRenderer<'a> {
    template: &'a ExcelTemplate,
    entity: &'a Entity,
    derived: BTreeMap<String, Vec<Entity>>,
    column_meta: &'a [ColumnMeta],         // für Format-Defaults
}

impl TemplateRenderer<'_> {
    pub fn render_to_bytes(&self) -> Result<Vec<u8>, ExcelError>;
}
```

Section-Dispatch: pro `TemplateSection` ein Writer-Module (`header.rs`, `keyvalue.rs`, `summary_box.rs`, `derived_table.rs`, `text.rs`, `spacer.rs`). Jedes Modul nimmt einen mutablen `&mut Worksheet`-Cursor und einen `&RenderContext` (Entity + Derived + ColumnMeta + Style-Defaults).

### 3.4 Loader

Templates werden aus `--data-dir/excel-templates/<id>.{toml,json}` geladen. Loader-Branch in `server/src/example/loader.rs`, dieselbe `read_typed`-Mechanik wie für Entities/Wizards/Reports.

Templates pro Entity-Typ können in `EntitySettings` referenziert werden:

```rust
#[serde(default)]
pub excel_templates: Vec<String>,         // ["datev.standard-voucher", "datev.kompakt-voucher"]
```

### 3.5 GraphQL / HTTP

Download als HTTP-Endpoint, nicht GraphQL (GraphQL ist nicht für Binary-Responses gedacht):

```
GET /export/excel/:template_id/:entity_id
  → application/vnd.openxmlformats-officedocument.spreadsheetml.sheet
  → Content-Disposition: attachment; filename="..."
```

Server holt:
1. `ExcelTemplate` aus Loader-Registry.
2. `Entity` aus Source-Layer.
3. Falls `derive_handler` gesetzt: ruft U2-Derive-Handler-Registry (same Mechanik wie Save-Preview-Derived-Rows).
4. `TemplateRenderer::render_to_bytes` → Response.

Auth: gleiches Permission-Modell wie `entities` (`can_read` auf der Entity).

### 3.6 Client-Trigger

- **Row-Action**: erweitert `row_actions`. `EntitySettings.excel_templates` produziert pro Template eine Row-Action `{ id: "excel:<template_id>", label_key: <template.title_key>, kind: "download" }`. Klick → öffnet `/export/excel/<id>/<entity_id>` (download).
- **Editor-Toolbar**: gleicher Trigger, anderer Einstiegspunkt.

Kein Modal — direkter Download.

## 4. Beispiel: `datev.standard-voucher.toml`

```toml
id = "datev.standard-voucher"
title_key = "voucher-standard-title"
entity_type = "DatevEntry"
filename_template = "Voucher-{id}-{posted_at:date}.xlsx"
derive_handler = "datev-account-entries"

[page]
orientation = "portrait"
paper_size = "A4"

[page.margins]
top = 18
bottom = 18
left = 15
right = 15

# --- Sections ---

[[sections]]
kind = "header"
[sections.title]
template = "Buchungsbeleg · DATEV Eintrag #{id}"
[sections.title.style]
font_size_pt = 14
bold = true

[[sections.meta_rows]]
template = "Belegdatum: {document_date:date}"
[[sections.meta_rows]]
template = "Buchungstag: {posted_at:date}"
[[sections.meta_rows]]
template = "Stack: {stack_name}"

[[sections]]
kind = "keyValue"
[[sections.rows]]
label = { template = "Verwendungszweck" }
value = { template = "{purpose}" }
[[sections.rows]]
label = { template = "Externe Belegnr." }
value = { template = "{external_id}", style = { font_name = "Cascadia Mono" } }
[[sections.rows]]
label = { template = "USt-Schlüssel" }
value = { template = "{tax_key_label}" }

[[sections]]
kind = "summaryBox"
emphasize_last = true
[[sections.rows]]
label = { template = "Bruttobetrag" }
value = { template = "{amount:money}" }
[[sections.rows]]
label = { template = "davon Vorsteuer" }
value = { template = "{vat_amount:money}" }
[[sections.rows]]
label = { template = "Nettobetrag" }
value = { template = "{net_amount:money}" }

[[sections]]
kind = "derivedTable"
source = "accountEntries"
[[sections.columns]]
key = "account_number"
label_key = "account-number"
[[sections.columns]]
key = "account_name"
label_key = "account-name"
[[sections.columns]]
key = "soll"
label_key = "soll"
format_hint = "money"
align = "right"
[[sections.columns]]
key = "haben"
label_key = "haben"
format_hint = "money"
align = "right"

[[sections.footer]]
column = "soll"
op = "sum"
[[sections.footer]]
column = "haben"
op = "sum"
```

## 5. Fehler-/Edge-Cases

- **Template fehlt**: Endpoint 404 mit i18n-Message.
- **Entity nicht gefunden**: 404.
- **Permission fehlt**: 403.
- **`derive_handler` schlägt fehl**: das Sheet wird ohne Derived-Tabelle gerendert; ein Warn-Sheet "Sheet 2" listet Fehler — Voucher ist trotzdem nutzbar.
- **Template-Placeholder unbekannt** (z.B. `{foo}` aber kein Feld `foo`): wird zu leerem String, logged warn auf Server-Seite.
- **Sehr lange `DerivedTable`**: kein Cap; aber Doku-Hinweis, dass F2 für *single*-Entity-Sheets gedacht ist, nicht für n=1000-Rows-Listen (das ist 1.7.14).

## 6. Komponenten-Inventar

**Neu**:
- `shared/src/excel_template.rs` — alle Template-Typen.
- `server/src/excel/mod.rs`, `renderer.rs`, je ein Modul pro Section-Kind.
- `server/src/example/loader.rs` — `excel-templates/` Branch.
- `server/src/main.rs` — Route `/export/excel/...` registrieren.
- `client/src/components/table/row_actions.rs` — Branch auf `kind=download` mit URL.

**Erweitert**:
- `server/Cargo.toml` — `rust_xlsxwriter` Dep.
- `shared/src/settings.rs::EntitySettings` — `excel_templates: Vec<String>`.

## 7. Tests

- `shared/tests/excel_template_wire.rs` — Roundtrip.
- `server/tests/excel_render.rs` — gegebene Template + Entity-Stub → erzeugte XLSX-Bytes wieder parsen (mit `calamine` als Test-Reader) → erwartete Zellinhalte.
- Edge-Cases: fehlende Placeholder, fehlende derived-Source, leere DerivedTable.

## 8. Backwards-Compat

- Vollständig additiv. Ohne registrierte Templates erscheint kein neuer UI-Trigger.

## 9. Größe + Risiken

**Größe**: S — Renderer ist gradlinig, da Sections statisch.

**Risiken**:
- **Style-Komplexität**: Excel ist anspruchsvoll bei Borders, Spacings, Page-Breaks. Erste Version unterstützt einen festen Style-Vocabulary; vollständige Excel-Feature-Liste ist explizit nicht das Ziel.
- **`derive_handler` doppelt**: U2 verwendet denselben Begriff. Klärung: U2's Handler liefern Diff/Preview-Rows; F2 verwendet dieselbe Registry, fragt aber im Read-Modus ab (kein Schreib-Pfad). Beide stützen sich auf eine gemeinsame Funktion `compute_derived(entity, handler_id) -> BTreeMap<String, Vec<Entity>>`.
- **Filename-Template-Injection**: `{}`-Platzhalter werden vor dem `Content-Disposition`-Header sanitized (Filesystem-sichere Zeichen).

## 10. Spätere Erweiterungen

- Optionale Multi-Sheet-Templates (Header-Sheet + Detail-Sheet).
- Charts/Sparklines pro Section.
- Template-Vererbung (`extends = "datev.standard-voucher"`).
- WYSIWYG-Editor.
- PDF-Output aus demselben Template (über Phase 1.7.9 PDF-Engine).

## 11. Decisions

1. **`rust_xlsxwriter`** als Render-Engine — pure Rust, leichte Dep.
2. **TOML-/JSON-Templates** in `--data-dir/excel-templates/` — gleiches Pattern wie Entities/Reports/Wizards.
3. **HTTP-Endpoint** für Download statt GraphQL — passend für Binary.
4. **`derive_handler`-Wiederverwendung mit U2** — eine Registry, zwei Use-Cases.
5. **Single-Entity per Template** — Bulk-Voucher wird durch wiederholten Aufruf gelöst, nicht durch Template-Erweiterung.

## 12. Referenzen

- ROADMAP §1.8 F2, Gap-Analyse §4.3.
- D2V-Inspiration: Voucher-Excel-Vorlage (kein direktes UI-Pendant in D2V's UI/-Ordner; Beleg-Druck in D2V ist Lib/Helper-Code).
- `server/src/example/loader.rs` — Loader-Pattern.
- U2 Spec — `derive_handler`-Registry.
