# U3 — Report-View-Component Spec

Date: 2026-05-20
Status: Draft — awaiting user review
Trigger: T6 (Account-View), T7 (DatevCalculation-Bilanzbaum), T17 (SuSa).
Quelle: Gap-Analyse §4.1 U3, ROADMAP §1.8 U3.
Visual: `.superpowers/brainstorm/.../u3-report-views.html` — drei Layouts (Linear, Tree, Pivot).
Abhängigkeit: **Phase 1.7.12 Aggregation-Queries** (Datenquelle).

## 1. Ziel

Eine deklarative Read-Only-Komponente, die

- aus einer Aggregation-Query (Phase 1.7.12) eine Cross-Table-Auswertung lädt,
- diese in einem von drei Layouts rendert: **Linear** (klassische Tabelle mit Footer-Summen), **Tree** (rekursive Gruppen, Expand/Collapse, Spalten als Dimensionen), **Pivot** (Zeilen-Dim × Spalten-Dim mit Aggregate-Zellen),
- via `report.json` deklariert wird — kein Per-Report-Rust-Code.

Damit sind SuSa (Linear), Bilanz (Tree), KontoAuszug (Pivot) drei Konfigurationen einer einzigen Komponente.

## 2. Nicht-Ziele

- Mutierbare Reports / "Editor" — Report-View ist read-only.
- Charts/Grafiken — Phase 1.7.20 Tree-FieldType + spätere Visual-Erweiterung.
- Live-Aggregation in der Komponente — die Aggregation passiert in 1.7.12, U3 rendert nur das Result.
- Export (CSV/Excel) — Phase 1.7.14 deckt das; U3 nutzt es bei "Export ▾".

## 3. Architektur

### 3.1 `shared::ReportDefinition`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportDefinition {
    pub id: String,                          // "datev.susa"
    pub title_key: String,
    pub aggregation_id: String,              // Verweis auf 1.7.12 Query
    pub layout: ReportLayout,
    pub columns: Vec<ReportColumn>,
    /// Optionaler Standard-Filter (z.B. context active_year).
    #[serde(default)]
    pub default_filter: Option<FilterCriteria>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReportLayout {
    Linear {
        /// Optionale Footer-Aggregates.
        #[serde(default)]
        footer: Vec<FooterAggregate>,
    },
    Tree {
        /// Hierarchie-Pfad als Spalten-Folge: ["category", "subcategory"].
        /// Letzte Ebene = Blatt-Rows.
        group_by: Vec<String>,
        /// Initial expanded bis Tiefe N.
        #[serde(default)]
        initial_expand_depth: u32,
    },
    Pivot {
        row_dim: String,
        col_dims: Vec<String>,
        /// Aggregations-Spalte(n) (z.B. "amount") und Funktion(en).
        measures: Vec<PivotMeasure>,
    },
}

pub struct ReportColumn {
    pub key: String,
    pub label_key: String,
    pub field_type: FieldType,
    pub align: Option<ColumnAlign>,       // Left | Right | Center
    pub width: Option<ColumnWidth>,        // Auto | Px(u32) | Star(u8)
}

pub struct FooterAggregate {
    pub column: String,
    pub op: AggregateOp,                  // Sum | Avg | Count | Min | Max | Custom(String)
    pub label_key: String,
}

pub struct PivotMeasure {
    pub column: String,
    pub op: AggregateOp,
    pub label_key: String,
    pub field_type: FieldType,
}
```

### 3.2 Datenfluss

1. Server lädt Report-Definitions aus `--data-dir/reports/<id>.{toml,json}` (wie `loader.rs` heute Entities lädt).
2. GraphQL: `report(id: String!): ReportDefinition!` und `reportResult(id: String!, filter: JSON): JSON!`.
3. `reportResult` ruft die in `aggregation_id` referenzierte 1.7.12-Query, mit Filter-Merge analog zu `entities`.
4. Resultat-Form ist **flach** (Reihen mit den deklarierten `columns` als Keys) — das Layout-Rendering ist Client-only. Vorteil: Server bleibt simpel; Tree/Pivot-Logik ist serialisierbar.

### 3.3 Client-Komponente `<ReportView>`

```
ReportView (component)
├── ReportHeader (Titel, Filter-Chips, Export-Button)
├── ReportBody (Strategy-Dispatch nach layout.kind)
│   ├── LinearBody
│   ├── TreeBody
│   └── PivotBody
└── ReportFooter (Footer-Aggregates oder leer)
```

Strategien sind separate Module unter `client/src/components/report/`:
- `linear.rs` — naive Tabelle + Footer-Row mit gefolderter Sum/Avg/…
- `tree.rs` — Client-side Group-By + Expand-State (`HashSet<RowKey>` für expandierte Knoten).
- `pivot.rs` — Client-side Pivot-Materialisierung (`PivotMatrix`) + Tabelle.

**Begründung für Client-Side Tree/Pivot**: heutige Reports (SuSa, Bilanz) bewegen sich im Bereich ≤ 1000 Rows. Server-Side-Pivoting wäre Premature Optimization. Falls eine Aggregation > 10 k Rows liefert, wird stattdessen die zugrundeliegende 1.7.12-Query getuned (mehr Pre-Aggregation).

### 3.4 Wiederverwendete Bausteine

- **Formatter-Registry** (`registries::formatter`) — pro Spalte je nach `field_type`. Geld, Datum, Prozent kommen automatisch.
- **DesignSystem** — `surface`, `table_cell`, `text` für konsistentes Look-and-Feel.
- **Routing**: `/reports/:report_id` neu in `routes/mod.rs`. Navigation-Tree kann Report-Knoten als `MenuAction::OpenReport(id)` haben.
- **AppContext (U5)**: `default_filter` kann `$context.active_year` referenzieren; gleiches Merge-Verhalten wie U5 §3.3.
- **Aggregation-Quelle**: in dieser Spec offen — `aggregation_id` ist Forward-Reference auf Phase 1.7.12. Bis dahin kann ein **Adapter** den `entities`-Resolver mit "fake" Aggregation füttern (Test-Stub `aggregations.json`).

## 4. Konkretisierung: drei Beispiel-Configs

### 4.1 SuSa (Linear)

```toml
# examples/d2v/reports/susa.toml
id = "datev.susa"
title_key = "report-susa-title"
aggregation_id = "datev.susa-by-account"

[layout]
kind = "linear"

[[layout.footer]]
column = "soll"
op = "sum"
label_key = "report-susa-total-soll"
[[layout.footer]]
column = "haben"
op = "sum"
label_key = "report-susa-total-haben"
[[layout.footer]]
column = "saldo"
op = "sum"
label_key = "report-susa-total-saldo"

[[columns]]
key = "account_number"
label_key = "account-number"
field_type = { kind = "text" }
align = "left"
[[columns]]
key = "account_name"
label_key = "account-name"
field_type = { kind = "text" }
[[columns]]
key = "soll"
label_key = "soll"
field_type = { kind = "money", currency_code_field = "currency" }
align = "right"
# … haben, saldo analog
```

### 4.2 Bilanz (Tree)

```toml
id = "datev.bilanz"
aggregation_id = "datev.calc-tree-by-year"

[layout]
kind = "tree"
group_by = ["category", "subcategory"]
initial_expand_depth = 1

[[columns]]
key = "position_name"
label_key = "position"
field_type = { kind = "text" }
[[columns]]
key = "y2025"
label_key = "year-2025"
field_type = { kind = "money" }
align = "right"
[[columns]]
key = "y2024"
label_key = "year-2024"
field_type = { kind = "money" }
align = "right"
[[columns]]
key = "delta_pct"
label_key = "delta"
field_type = { kind = "percent" }
align = "right"
```

### 4.3 Kontoauszug (Pivot)

```toml
id = "datev.account-pivot"
aggregation_id = "datev.entries-by-period-account"

[layout]
kind = "pivot"
row_dim = "period"
col_dims = ["movement_kind"]      # "soll" | "haben"

[[layout.measures]]
column = "amount"
op = "sum"
label_key = "movement-sum"
field_type = { kind = "money" }
[[layout.measures]]
column = "id"
op = "count"
label_key = "movement-count"
field_type = { kind = "integer" }
```

## 5. Fehler-/Edge-Cases

- **Aggregation liefert leere Daten** → Layout zeigt leeren State mit i18n-Hinweis.
- **`group_by`-Spalte fehlt** → Tree-Layout zeigt nur Flat-Rows + Warn-Banner (graceful).
- **Pivot-Sparse-Matrix**: fehlende Zellen werden als "—" oder `0` gerendert (Formatter-Hook).
- **Performance bei >10 k Rows**: Banner "Aggregation liefert zu viele Rows; bitte Filter setzen" + Skipping des Rendering — schützt vor UI-Freeze. Konfigurierbarer Threshold pro Report.

## 6. Komponenten-Inventar

**Neu**:
- `shared/src/report.rs` — `ReportDefinition`, `ReportLayout`, `ReportColumn`, …
- `server/src/example/loader.rs` — Branch `reports/`
- `server/src/schema.rs` — Queries `report(id)`, `reportResult(id, filter)`
- `server/src/data.rs` — `reports_definitions()`, `report_result(id, filter)`
- `client/src/components/report/mod.rs`
- `client/src/components/report/linear.rs`
- `client/src/components/report/tree.rs`
- `client/src/components/report/pivot.rs`
- `client/src/routes/report.rs` — Route-Handler `/reports/:id`
- `client/src/graphql/queries.rs` — `fetch_report`, `fetch_report_result`

**Erweitert**:
- `shared/src/menu.rs` — `MenuAction::OpenReport(String)`
- `client/src/components/navigation.rs` — Branch auf neue MenuAction

## 7. Tests

- `shared/tests/report_definition_wire.rs` — Roundtrip aller Layout-Varianten.
- `server/tests/reports.rs` — Loader liest TOML/JSON; `reportResult` mit Test-Aggregation.
- `client` Unit-Tests pro Layout-Modul: gegebene Result-Rows + Definition → erwartete Render-Struktur. Tree-Expand/Collapse-State. Pivot-Materialisierung.

## 8. Backwards-Compat

- Vollständig additiv. Keine Änderung am `entities`-Pfad.
- Wenn `reports/` Verzeichnis fehlt: `data.reports_definitions()` liefert leer.

## 9. Größe + Risiken

**Größe**: L. Drei Layout-Strategien + Loader + Routes + Wire-Typen. Tree und Pivot sind nicht-trivial.

**Risiken**:
- **Aggregation-Layer noch nicht da (1.7.12)**: Mitigation: zunächst Adapter über `entities` mit fake-Aggregation; produktive Variante wenn 1.7.12 landet. Spec klärt das.
- **Client-Side Performance**: bei großen Datasets droht UI-Freeze. Threshold-Banner als Sicherheitsnetz.
- **Tree-Group-By-Stabilität**: bei mehreren Group-By-Spalten muss Sort-Reihenfolge deterministisch sein (Aggregation muss sortiert liefern; sonst Client sortiert).
- **Pivot mit n Col-Dims**: kombinatorische Explosion. Mitigation: max. 2 Col-Dims initial, harter Cap.
- **Footer-Aggregates für Tree/Pivot**: in der ersten Iteration nur in Linear. Tree-/Pivot-Footer ist spätere Erweiterung.

## 10. Spätere Erweiterungen

- Drill-Down (Klick auf Pivot-Zelle → Liste der Buchungen).
- Charts pro Report-Definition (`chart = { kind = "bar", x = "period", y = "amount" }`).
- Live-Update bei Mutationen (Subscriptions).
- Multi-Measure Pivot in Tree.
- Aggregation-Cache (Phase 1.7.12 wird das vermutlich liefern).

## 11. Decisions

1. **Eine Komponente, drei Strategien** statt drei separate Komponenten — gleicher Header/Footer, gleiche Export- und Filter-Handhabung.
2. **Client-Side Tree/Pivot** — heutige Datenvolumina rechtfertigen keinen Server-Side-Pivot.
3. **Declarative `report.json`-Definition** statt Per-Report-Code — entspricht der Architektur von Entity-Settings.
4. **Drill-Down + Charts in 10/Spätere Erweiterungen** — YAGNI für heute.

## 12. Referenzen

- `server/src/example/loader.rs` (Loader-Pattern)
- `shared/src/settings.rs::EntitySettings` (Configuration-Pattern)
- `client/src/components/table/` (Renderer-Inspiration)
- ROADMAP §1.7.12 (Aggregation-Queries), §1.8 U3
- Gap-Analyse §4.1 U3
- D2V-Inspiration: `DatevAccountEntryControls/DatevAccountView.xaml`, `SuSaControls/SuSaView.xaml`, `DatevCalculationControls/View.xaml`
