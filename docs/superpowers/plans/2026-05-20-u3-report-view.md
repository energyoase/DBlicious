# U3 — Report-View Component Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eine deklarative Read-Only-Komponentenfamilie, die aus einer Aggregation-Query (Phase 1.7.12 — bis dahin: Stub-Adapter über `entities`) eine Cross-Table-Auswertung lädt und in drei austauschbaren Layouts rendert: **Linear** (Tabelle + Footer-Aggregates), **Tree** (rekursive Gruppen + Expand/Collapse), **Pivot** (Zeilen-Dim × Spalten-Dim mit Aggregate-Zellen). Reports werden via `report.{toml,json}` deklariert. Damit sind SuSa (Linear), Bilanz (Tree), KontoAuszug (Pivot) drei Konfigurationen einer einzigen Komponente.

**Architecture:** Wire-Typ `shared::ReportDefinition` mit getaggter `ReportLayout`-Enum. Server lädt Reports analog zu Entities via `server/src/example/loader.rs`-Erweiterung. Zwei neue GraphQL-Queries: `report(id)` und `reportResult(id, filter)`. `reportResult` fragt aus der Aggregations-Source (V1: Stub-Adapter, der `entities`-Pfad nutzt) eine flache Reihen-Liste; Tree/Pivot-Materialisierung passiert client-seitig. Client: `<ReportView>` als Sibling zu `EntityTable` (kein Code-Reuse — andere Verantwortlichkeit), mit `LinearBody` / `TreeBody` / `PivotBody` als ausgetauschten Strategien. Drei Layout-Module unter `client/src/components/report/`.

**Tech Stack:** Rust, `shared` (serde), `server` (axum + async-graphql), `client` (Leptos + DesignSystem), `cargo test --workspace`.

**Spec-Referenz:** [`docs/superpowers/specs/2026-05-20-u3-report-view-design.md`](../specs/2026-05-20-u3-report-view-design.md).

**Vorbedingung:** keine harten. Phase 1.7.12 (Aggregation-Queries) ist parallel; V1 nutzt einen `aggregation_id` → `entities`-Adapter, der eine simple gefilterte Liste zurückgibt.

**Frontend-design Entscheidungen** ("Ledger-modern"-Aesthetic):
1. **Tabular-Nums-Spalten** non-negotiable: jede Zahl-Zelle bekommt `font-variant-numeric: tabular-nums` via neuem `numeric_cell()`-Accessor. Höchster-Hebel-Detail für die Ledger-Anmutung.
2. **Aggregate-Rows differenzieren via *weight*, *border*, *type-treatment*** — nicht via Background-Tinting. Grand-Total bekommt eine `border-top: 3px double` (direkte Referenz an Print-Buchhaltungs-Konventionen). Subtotal hairline-Border. Backgrounds bleiben dem `:hover`-Layer vorbehalten.
3. **Expand-Affordance: `▸` / `▾`** (U+25B8/U+25BE) — Excel-/SAP-/Finder-Konvention, vermeidet `+`/`−` (Vorzeichen-Verwechslung) und `▶`/`▼` (Play-Button-Anmutung).
4. **Indentation 1.25rem pro Level**, nur in der Label-Spalte, niemals in den Zahl-Spalten. Connector-Lines optional/off-by-default (`report.layout.show_connectors`-Flag); bei ≤3 Levels nur Rauschen.
5. **Sticky-Pivot**: erste Spalte (Row-Dim) + 1-2 Header-Rows `position: sticky`. Non-negotiable für Pivots ≥ 6 Spalten — ohne Sticky kollabiert das Pattern bei horizontalem Scroll.

**Zusätzliche Decisions**:
6. Loading-State: kein Spinner. Statt-dessen muted-italic-centered "Aggregation wird berechnet…". Spinners sind nervous-energy für Multi-Sekunden-Aggregationen; ruhiger Text passt zur Ledger-Anmutung.
7. Empty-State: zweizeilig (Report-Titel muted + Caption mit Action-Hinweis). Nicht generisches "Keine Daten."
8. Threshold-Banner (>N Rows): 4px solid Amber-Left-Border, ersetzt den Body komplett, plus "Filter öffnen"-Button. Großdataset-Schutz vor UI-Freeze.

---

## File Structure

**Neu:**
- `shared/src/report.rs` — `ReportDefinition`, `ReportLayout`, `ReportColumn`, `FooterAggregate`, `PivotMeasure`, `AggregateOp`, `ColumnAlign`, `ColumnWidth`
- `shared/tests/report_definition_wire.rs` — Roundtrip
- `server/tests/reports.rs` — Loader + reportResult
- `client/src/components/report/mod.rs`
- `client/src/components/report/view.rs` — `<ReportView>` Shell mit Loading/Empty/Threshold
- `client/src/components/report/linear.rs` — `<LinearBody>` + Footer
- `client/src/components/report/tree.rs` — `<TreeBody>` mit group_by + expand/collapse
- `client/src/components/report/pivot.rs` — `<PivotBody>` mit Stacked-Headers + Sticky
- `client/src/components/report/aggregate.rs` — pure-function `sum/avg/count/min/max`
- `client/src/routes/report.rs` — Route `/reports/:id`
- `client/src/graphql/reports.rs`
- `examples/shop/reports/demo-linear.toml`
- `examples/shop/reports/demo-tree.toml`
- `examples/shop/reports/demo-pivot.toml`

**Modifizieren:**
- `shared/src/lib.rs` — Re-Export + `MenuAction::OpenReport`
- `shared/src/menu.rs` — MenuAction-Variante
- `server/src/example/loader.rs` — `reports/<id>.{toml,json}`-Branch
- `server/src/data.rs` — `list_reports`, `report_by_id`, `report_result_adapter`
- `server/src/schema.rs` — Queries `report(id)`, `reportResult(id, filter)`
- `server/src/lib.rs`
- `client/src/lib.rs` — Module exportieren
- `client/src/app.rs` — Route mounten
- `client/src/components/navigation.rs` — Branch auf `OpenReport`
- `client/src/styling/mod.rs` + `inline.rs` — 16 neue Accessoren (siehe Architektur-Details)
- `client/locales/de/main.ftl`, `en/main.ftl`

---

## Architektur-Details

### `shared::report` Wire-Inventar

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportDefinition {
    pub id: String,
    pub title_key: String,
    pub aggregation_id: String,
    pub layout: ReportLayout,
    pub columns: Vec<ReportColumn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_filter: Option<crate::FilterCriteria>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_threshold: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReportLayout {
    Linear {
        #[serde(default)] footer: Vec<FooterAggregate>,
    },
    Tree {
        group_by: Vec<String>,
        #[serde(default)] initial_expand_depth: u32,
        #[serde(default)] show_connectors: bool,
    },
    Pivot {
        row_dim: String,
        col_dims: Vec<String>,
        measures: Vec<PivotMeasure>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportColumn {
    pub key: String,
    pub label_key: String,
    pub field_type: crate::FieldType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<ColumnAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<ColumnWidth>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColumnAlign { #[default] Left, Right, Center }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ColumnWidth { Auto, Px { px: u32 }, Star { weight: u8 } }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FooterAggregate {
    pub column: String,
    pub op: AggregateOp,
    pub label_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PivotMeasure {
    pub column: String,
    pub op: AggregateOp,
    pub label_key: String,
    pub field_type: crate::FieldType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AggregateOp { Sum, Avg, Count, Min, Max }
```

### Server: `reportResult`-Adapter

V1-Strategie: `aggregation_id` ist ein String, der entweder
- `"entities:<entity_type>"` → adapter holt eine paginierte Liste via existing `data::entities_page`, flacht zu Result-Rows ab; oder
- `"stub:<name>"` → für Tests fest hinterlegte Rows-Sets.

Sobald Phase 1.7.12 landet, kommt ein dritter Match-Arm `"aggregation:<id>"`, der die echte Aggregation-Query feuert.

```rust
pub async fn report_result(id: &str, filter: Option<shared::FilterCriteria>)
    -> Result<Vec<shared::Entity>, AppError>
{
    let def = report_by_id(id).ok_or(AppError::NotFound)?;
    let agg = def.aggregation_id.clone();
    let combined_filter = match (def.default_filter, filter) {
        (Some(d), Some(f)) => Some(merge_filters(d, f)),
        (Some(d), None) => Some(d),
        (None, Some(f)) => Some(f),
        (None, None) => None,
    };

    if let Some(entity_type) = agg.strip_prefix("entities:") {
        return crate::data::entities_page(
            entity_type, None, None, combined_filter,
            /* page: */ 0, /* limit: */ 10_000,
        ).await.map(|p| p.rows);
    }
    if let Some(stub_name) = agg.strip_prefix("stub:") {
        return Ok(stub_result(stub_name));
    }

    Err(AppError::NotImplemented(format!("aggregation source {agg}")))
}
```

### Client-Komponentenfamilie

```
client/src/components/report/
├── mod.rs              — Re-Exports
├── view.rs             — <ReportView> Shell (loading/empty/threshold/dispatch)
├── linear.rs           — <LinearBody> + Footer
├── tree.rs             — <TreeBody> + group_by + expand-state (RwSignal<HashSet<RowKey>>)
├── pivot.rs            — <PivotBody> + Matrix-Materialisierung
└── aggregate.rs        — pure-function sum/avg/count/min/max
```

### 16 neue DesignSystem-Accessoren

In **einem** Schritt vor allen UI-Tasks zur `DesignSystem`-Trait ergänzen (siehe Task 6):

```rust
fn numeric_cell(&self) -> Style;
fn report_row_leaf(&self) -> Style;
fn report_row_subtotal(&self, depth: usize) -> Style;
fn report_row_grandtotal(&self) -> Style;
fn report_section_header(&self, depth: usize) -> Style;
fn tree_label_cell(&self, depth: usize) -> Style;
fn tree_expand_icon(&self, expanded: bool) -> Style;
fn tree_connector(&self, depth: usize) -> Style;
fn pivot_header_outer(&self) -> Style;
fn pivot_header_inner(&self) -> Style;
fn pivot_header_group_separator(&self) -> Style;
fn report_sticky_col(&self) -> Style;
fn report_sticky_header(&self) -> Style;
fn report_empty_state(&self) -> Style;
fn report_threshold_banner(&self) -> Style;
fn report_footer_row(&self) -> Style;
```

### Was NICHT in diesem Plan ist

- Drill-Down (Klick auf Pivot-Zelle → Liste der Source-Rows) — Backlog
- Charts pro Report-Definition — Backlog
- Live-Update via Subscriptions — Backlog
- Multi-Measure-Pivot in Tree — Backlog
- Echte Phase-1.7.12 Aggregations-Source — Folge-Task, wenn 1.7.12 verfügbar
- Excel/CSV-Export pro Report — Phase 1.7.14

---

## Tasks

### Task 1: `shared::report` Wire + Roundtrip

**Files:**
- Neu: `shared/src/report.rs`
- Modify: `shared/src/lib.rs`
- Neu: `shared/tests/report_definition_wire.rs`

- [ ] **Schritt 1: Failing Test**

`shared/tests/report_definition_wire.rs`:

```rust
use shared::{
    ReportDefinition, ReportLayout, ReportColumn, FooterAggregate,
    PivotMeasure, AggregateOp, ColumnAlign, FieldType,
};

#[test]
fn linear_layout_roundtrip() {
    let d = ReportDefinition {
        id: "susa".into(),
        title_key: "susa-title".into(),
        aggregation_id: "stub:susa".into(),
        layout: ReportLayout::Linear { footer: vec![
            FooterAggregate { column: "soll".into(), op: AggregateOp::Sum, label_key: "total-soll".into() }
        ]},
        columns: vec![ReportColumn {
            key: "account_number".into(), label_key: "account-number".into(),
            field_type: FieldType::Text,
            align: Some(ColumnAlign::Left), width: None,
        }],
        default_filter: None,
        row_threshold: Some(10_000),
    };
    let json = serde_json::to_string(&d).unwrap();
    let back: ReportDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
}

#[test]
fn tree_layout_camel_case_kind() {
    let layout = ReportLayout::Tree {
        group_by: vec!["category".into(), "subcategory".into()],
        initial_expand_depth: 1,
        show_connectors: false,
    };
    let json = serde_json::to_string(&layout).unwrap();
    assert!(json.contains(r#""kind":"tree""#));
    assert!(json.contains(r#""groupBy":["category","subcategory"]"#));
    assert!(json.contains(r#""initialExpandDepth":1"#));
}

#[test]
fn pivot_layout_with_measures() {
    let layout = ReportLayout::Pivot {
        row_dim: "period".into(),
        col_dims: vec!["movement".into()],
        measures: vec![PivotMeasure {
            column: "amount".into(), op: AggregateOp::Sum,
            label_key: "sum".into(),
            field_type: FieldType::Decimal { precision: 18, scale: 4 },
        }],
    };
    let json = serde_json::to_string(&layout).unwrap();
    assert!(json.contains(r#""kind":"pivot""#));
    assert!(json.contains(r#""rowDim":"period""#));
}

#[test]
fn aggregate_op_camel_case() {
    assert_eq!(serde_json::to_string(&AggregateOp::Sum).unwrap(), "\"sum\"");
    assert_eq!(serde_json::to_string(&AggregateOp::Avg).unwrap(), "\"avg\"");
}
```

- [ ] **Schritt 2: FAIL**

```
cargo test -p shared --test report_definition_wire
```

- [ ] **Schritt 3: Modul + Re-Export**

`shared/src/report.rs` mit Code aus "Architektur-Details".

`shared/src/lib.rs`:
```rust
pub mod report;
pub use report::{
    AggregateOp, ColumnAlign, ColumnWidth, FooterAggregate, PivotMeasure,
    ReportColumn, ReportDefinition, ReportLayout,
};
```

- [ ] **Schritt 4: PASS + Commit**

```
cargo test -p shared --test report_definition_wire
git add shared/src/report.rs shared/src/lib.rs shared/tests/report_definition_wire.rs
git commit -m "feat(shared): ReportDefinition wire types"
```

---

### Task 2: `MenuAction::OpenReport`

**Files:**
- Modify: `shared/src/menu.rs`
- Modify (Test): `shared/tests/report_definition_wire.rs`

- [ ] **Schritt 1: Test**

```rust
use shared::MenuAction;

#[test]
fn open_report_action_camel_case() {
    let a = MenuAction::OpenReport("susa".into());
    let json = serde_json::to_string(&a).unwrap();
    assert!(json.contains(r#""kind":"openReport""#));
}
```

- [ ] **Schritt 2: Variante**

```rust
OpenReport(String),
```

- [ ] **Schritt 3: PASS + Commit**

```
cargo test -p shared
git add shared/src/menu.rs shared/tests/report_definition_wire.rs
git commit -m "feat(shared): MenuAction::OpenReport"
```

---

### Task 3: Loader-Branch + Demo-Reports

**Files:**
- Modify: `server/src/example/loader.rs`
- Modify: ExampleSet erweitern
- Neu: `examples/shop/reports/demo-linear.toml`
- Neu: `examples/shop/reports/demo-tree.toml`
- Neu: `examples/shop/reports/demo-pivot.toml`

- [ ] **Schritt 1: Loader-Branch**

```rust
let rep_dir = dir.join("reports");
let mut reports: Vec<shared::ReportDefinition> = Vec::new();
if rep_dir.is_dir() {
    for entry in std::fs::read_dir(&rep_dir)? {
        let path = entry?.path();
        if let Some(r) = read_typed::<shared::ReportDefinition>(&path)? {
            reports.push(r);
        }
    }
}
```

ExampleSet bekommt `pub reports: Vec<shared::ReportDefinition>`.

- [ ] **Schritt 2: Demo-Linear**

`examples/shop/reports/demo-linear.toml`:

```toml
id = "demo-linear"
titleKey = "report-demo-linear-title"
aggregationId = "entities:Order"
rowThreshold = 10000

[layout]
kind = "linear"

[[layout.footer]]
column = "total"
op = "sum"
labelKey = "report-total"

[[columns]]
key = "id"
labelKey = "id"
fieldType = { kind = "text" }

[[columns]]
key = "customer_name"
labelKey = "customer"
fieldType = { kind = "text" }

[[columns]]
key = "total"
labelKey = "total"
fieldType = { kind = "decimal", precision = 18, scale = 2 }
align = "right"
```

- [ ] **Schritt 3: Demo-Tree**

`examples/shop/reports/demo-tree.toml`:

```toml
id = "demo-tree"
titleKey = "report-demo-tree-title"
aggregationId = "entities:Order"

[layout]
kind = "tree"
groupBy = ["status"]
initialExpandDepth = 1
showConnectors = false

[[columns]]
key = "id"
labelKey = "id"
fieldType = { kind = "text" }

[[columns]]
key = "customer_name"
labelKey = "customer"
fieldType = { kind = "text" }

[[columns]]
key = "total"
labelKey = "total"
fieldType = { kind = "decimal", precision = 18, scale = 2 }
align = "right"
```

- [ ] **Schritt 4: Demo-Pivot**

`examples/shop/reports/demo-pivot.toml`:

```toml
id = "demo-pivot"
titleKey = "report-demo-pivot-title"
aggregationId = "entities:Order"

[layout]
kind = "pivot"
rowDim = "customer_name"
colDims = ["status"]

[[layout.measures]]
column = "total"
op = "sum"
labelKey = "total"
fieldType = { kind = "decimal", precision = 18, scale = 2 }

[[columns]]
key = "customer_name"
labelKey = "customer"
fieldType = { kind = "text" }

[[columns]]
key = "status"
labelKey = "status"
fieldType = { kind = "text" }

[[columns]]
key = "total"
labelKey = "total"
fieldType = { kind = "decimal", precision = 18, scale = 2 }
```

- [ ] **Schritt 5: Helpers + Loader-Test**

`server/src/data.rs`:
```rust
pub fn list_reports() -> Vec<shared::ReportDefinition> {
    crate::example::current().map(|s| s.reports.clone()).unwrap_or_default()
}
pub fn report_by_id(id: &str) -> Option<shared::ReportDefinition> {
    crate::example::current().and_then(|s| s.reports.iter().find(|r| r.id == id).cloned())
}
```

`server/tests/loader.rs`:
```rust
#[test]
fn loads_reports_from_example_shop() {
    let set = server::example::load(std::path::Path::new("../examples/shop")).unwrap();
    assert!(set.reports.iter().any(|r| r.id == "demo-linear"));
    assert!(set.reports.iter().any(|r| r.id == "demo-tree"));
    assert!(set.reports.iter().any(|r| r.id == "demo-pivot"));
}
```

- [ ] **Schritt 6: Tests + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server loader
git add server/src/example/ examples/shop/reports/ server/tests/loader.rs server/src/data.rs
git commit -m "feat(server): loader reads reports/<id>.{toml,json} + demo reports"
```

---

### Task 4: GraphQL `report` + `reportResult`

**Files:**
- Modify: `server/src/schema.rs`
- Modify: `server/src/data.rs` (für `report_result`)

- [ ] **Schritt 1: Adapter-Funktion in `data.rs`**

```rust
pub async fn report_result_adapter(id: &str, filter: Option<shared::FilterCriteria>)
    -> Result<Vec<shared::Entity>, AppError>
{
    let def = report_by_id(id).ok_or_else(|| AppError::NotFound(format!("report {id}")))?;
    if let Some(entity_type) = def.aggregation_id.strip_prefix("entities:") {
        let page = entities_page(entity_type, None, None, filter, 0, 10_000).await?;
        return Ok(page.rows);
    }
    if let Some(_stub) = def.aggregation_id.strip_prefix("stub:") {
        return Ok(stub_result_rows(&def));
    }
    Err(AppError::NotImplemented(format!("aggregation {}", def.aggregation_id)))
}

fn stub_result_rows(_def: &shared::ReportDefinition) -> Vec<shared::Entity> {
    use std::collections::BTreeMap;
    let mut rows = Vec::new();
    for i in 1..=5 {
        let mut fields = BTreeMap::new();
        fields.insert("id".into(), serde_json::json!(i));
        fields.insert("customer_name".into(), serde_json::json!(format!("Cust{i}")));
        fields.insert("total".into(), serde_json::json!(i as f64 * 10.0));
        rows.push(shared::Entity { id: i.to_string(), fields });
    }
    rows
}
```

- [ ] **Schritt 2: Resolver**

```rust
async fn report(&self, _ctx: &Context<'_>, id: String)
    -> async_graphql::Result<Option<async_graphql::Json<shared::ReportDefinition>>>
{
    Ok(crate::data::report_by_id(&id).map(async_graphql::Json))
}

async fn report_result(
    &self, _ctx: &Context<'_>, id: String,
    filter: Option<async_graphql::Json<shared::FilterCriteria>>,
) -> async_graphql::Result<async_graphql::Json<Vec<shared::Entity>>> {
    let rows = crate::data::report_result_adapter(&id, filter.map(|j| j.0)).await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;
    Ok(async_graphql::Json(rows))
}
```

- [ ] **Schritt 3: Test**

`server/tests/reports.rs`:

```rust
use serde_json::json;

#[tokio::test]
#[serial_test::serial]
async fn report_query_returns_definition() {
    server::fresh_test_setup().await;
    let body = json!({
        "query": "query R($id: String!) { report(id: $id) }",
        "variables": {"id": "demo-linear"}
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let def = &resp["data"]["report"];
    assert_eq!(def["id"], "demo-linear");
    assert_eq!(def["layout"]["kind"], "linear");
}

#[tokio::test]
#[serial_test::serial]
async fn report_result_returns_rows() {
    server::fresh_test_setup().await;
    let body = json!({
        "query": "query R($id: String!) { reportResult(id: $id, filter: null) }",
        "variables": {"id": "demo-linear"}
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let rows = resp["data"]["reportResult"].as_array().unwrap();
    assert!(!rows.is_empty());
}

#[tokio::test]
#[serial_test::serial]
async fn report_unknown_returns_null() {
    server::fresh_test_setup().await;
    let body = json!({
        "query": "query R($id: String!) { report(id: $id) }",
        "variables": {"id": "does-not-exist"}
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    assert!(resp["data"]["report"].is_null());
}
```

- [ ] **Schritt 4: Tests + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test reports
git add server/src/schema.rs server/src/data.rs server/tests/reports.rs
git commit -m "feat(server): report + reportResult graphql + adapter"
```

---

### Task 5: Client GraphQL-Adapter

**Files:**
- Neu: `client/src/graphql/reports.rs`
- Modify: `client/src/graphql/mod.rs`

- [ ] **Schritt 1: Adapter**

```rust
use shared::{Entity, FilterCriteria, ReportDefinition};

pub async fn fetch_report(id: &str) -> Result<Option<ReportDefinition>, crate::graphql::GraphqlError> {
    #[derive(serde::Deserialize)]
    struct Resp { report: Option<ReportDefinition> }
    let vars = serde_json::json!({"id": id});
    let r: Resp = crate::graphql::execute("query R($id: String!) { report(id: $id) }", &vars).await?;
    Ok(r.report)
}

pub async fn fetch_report_result(id: &str, filter: Option<FilterCriteria>)
    -> Result<Vec<Entity>, crate::graphql::GraphqlError>
{
    #[derive(serde::Deserialize)]
    struct Resp { #[serde(rename = "reportResult")] result: Vec<Entity> }
    let vars = serde_json::json!({"id": id, "filter": filter});
    let r: Resp = crate::graphql::execute(
        "query R($id: String!, $filter: JSON) { reportResult(id: $id, filter: $filter) }", &vars,
    ).await?;
    Ok(r.result)
}
```

- [ ] **Schritt 2: Commit**

```
git add client/src/graphql/reports.rs client/src/graphql/mod.rs
git commit -m "feat(client): report graphql adapter"
```

---

### Task 6: 16 DesignSystem-Accessoren

**Files:**
- Modify: `client/src/styling/mod.rs` (Trait)
- Modify: `client/src/styling/inline.rs` (InlineDesign)

- [ ] **Schritt 1: Trait erweitern**

In `DesignSystem`-Trait die 16 Methoden aus "Architektur-Details" hinzufügen.

- [ ] **Schritt 2: InlineDesign-Impl**

```rust
impl DesignSystem for InlineDesign {
    fn numeric_cell(&self) -> Style {
        Style { inline: "font-variant-numeric:tabular-nums;text-align:right;".into(), class: String::new() }
    }
    fn report_row_leaf(&self) -> Style {
        Style { inline: "".into(), class: String::new() }
    }
    fn report_row_subtotal(&self, depth: usize) -> Style {
        let modifier = if depth == 0 {
            "text-transform:uppercase;letter-spacing:0.04em;font-size:0.85em;"
        } else { "" };
        Style {
            inline: format!("font-weight:500;border-top:1px solid #d6d3d1;{modifier}"),
            class: String::new(),
        }
    }
    fn report_row_grandtotal(&self) -> Style {
        Style {
            inline: "font-weight:600;border-top:3px double #44403c;padding-top:2px;".into(),
            class: String::new(),
        }
    }
    fn report_section_header(&self, depth: usize) -> Style {
        let size = if depth == 0 { "1em" } else { "0.95em" };
        Style {
            inline: format!("font-weight:600;color:#1c1917;font-size:{size};padding:6px 0 2px 0;"),
            class: String::new(),
        }
    }
    fn tree_label_cell(&self, depth: usize) -> Style {
        let padding = 0.5 + (depth as f32) * 1.25;
        Style {
            inline: format!("padding-left:{padding}rem;text-align:left;"),
            class: String::new(),
        }
    }
    fn tree_expand_icon(&self, _expanded: bool) -> Style {
        Style {
            inline: "display:inline-block;width:0.9em;color:#78716c;cursor:pointer;".into(),
            class: String::new(),
        }
    }
    fn tree_connector(&self, _depth: usize) -> Style {
        // Default: off — kein border. Wird vom Caller nur appliziert wenn show_connectors=true.
        Style { inline: "border-left:1px dotted #d6d3d1;".into(), class: String::new() }
    }
    fn pivot_header_outer(&self) -> Style {
        Style {
            inline: "font-weight:600;text-align:center;border-bottom:1px solid #d6d3d1;padding:4px 8px;".into(),
            class: String::new(),
        }
    }
    fn pivot_header_inner(&self) -> Style {
        Style {
            inline: "font-weight:400;font-size:0.85em;text-align:right;padding:2px 6px;font-variant-numeric:tabular-nums;".into(),
            class: String::new(),
        }
    }
    fn pivot_header_group_separator(&self) -> Style {
        Style { inline: "border-left:1px solid #d6d3d1;".into(), class: String::new() }
    }
    fn report_sticky_col(&self) -> Style {
        Style { inline: "position:sticky;left:0;background:#fff;z-index:1;".into(), class: String::new() }
    }
    fn report_sticky_header(&self) -> Style {
        Style { inline: "position:sticky;top:0;background:#fff;z-index:2;".into(), class: String::new() }
    }
    fn report_empty_state(&self) -> Style {
        Style {
            inline: "text-align:center;padding:4rem 1rem;color:#78716c;font-style:italic;".into(),
            class: String::new(),
        }
    }
    fn report_threshold_banner(&self) -> Style {
        Style {
            inline: "border-left:4px solid #d97706;background:#fffbeb;padding:12px 16px;color:#78350f;margin:1rem 0;".into(),
            class: String::new(),
        }
    }
    fn report_footer_row(&self) -> Style {
        Style {
            inline: "font-weight:600;border-top:1px solid #44403c;font-variant-numeric:tabular-nums;".into(),
            class: String::new(),
        }
    }
}
```

- [ ] **Schritt 3: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/styling/
git commit -m "feat(client): 16 DesignSystem accessors for report layouts"
```

---

### Task 7: `aggregate.rs` — pure aggregation functions

**Files:**
- Neu: `client/src/components/report/aggregate.rs`
- Neu: `client/src/components/report/mod.rs`

- [ ] **Schritt 1: Module + pure functions**

`client/src/components/report/mod.rs`:

```rust
pub mod aggregate;
pub mod linear;
pub mod pivot;
pub mod tree;
pub mod view;

pub use view::ReportView;
```

`client/src/components/report/aggregate.rs`:

```rust
use shared::{AggregateOp, Entity};

pub fn aggregate(rows: &[Entity], column: &str, op: AggregateOp) -> f64 {
    let values: Vec<f64> = rows.iter()
        .filter_map(|r| r.fields.get(column).and_then(|v| v.as_f64()))
        .collect();
    if values.is_empty() && !matches!(op, AggregateOp::Count) { return 0.0; }
    match op {
        AggregateOp::Sum => values.iter().sum(),
        AggregateOp::Avg => if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 },
        AggregateOp::Count => rows.iter().filter(|r| r.fields.contains_key(column)).count() as f64,
        AggregateOp::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
        AggregateOp::Max => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use serde_json::json;

    fn entity(fields: Vec<(&str, serde_json::Value)>) -> Entity {
        let m: BTreeMap<String, serde_json::Value> = fields.into_iter().map(|(k, v)| (k.into(), v)).collect();
        Entity { id: "1".into(), fields: m }
    }

    #[test]
    fn sum_simple() {
        let rows = vec![
            entity(vec![("v", json!(10.0))]),
            entity(vec![("v", json!(20.0))]),
            entity(vec![("v", json!(30.0))]),
        ];
        assert_eq!(aggregate(&rows, "v", AggregateOp::Sum), 60.0);
    }

    #[test]
    fn avg_with_missing_values() {
        let rows = vec![
            entity(vec![("v", json!(10.0))]),
            entity(vec![("v", json!(20.0))]),
            entity(vec![]),
        ];
        assert_eq!(aggregate(&rows, "v", AggregateOp::Avg), 15.0);
    }

    #[test]
    fn count_includes_present_key_only() {
        let rows = vec![
            entity(vec![("v", json!(1.0))]),
            entity(vec![]),
        ];
        assert_eq!(aggregate(&rows, "v", AggregateOp::Count), 1.0);
    }

    #[test]
    fn min_max() {
        let rows = vec![
            entity(vec![("v", json!(5.0))]),
            entity(vec![("v", json!(1.0))]),
            entity(vec![("v", json!(9.0))]),
        ];
        assert_eq!(aggregate(&rows, "v", AggregateOp::Min), 1.0);
        assert_eq!(aggregate(&rows, "v", AggregateOp::Max), 9.0);
    }
}
```

- [ ] **Schritt 2: Tests + Commit**

```
cargo test -p client --lib components::report::aggregate::tests
git add client/src/components/report/
git commit -m "feat(client): report aggregate pure functions + tests"
```

---

### Task 8: `<ReportView>` Shell (loading / empty / threshold / dispatch)

**Files:**
- Neu: `client/src/components/report/view.rs`

- [ ] **Schritt 1: Shell**

```rust
use leptos::prelude::*;
use shared::{Entity, ReportDefinition, ReportLayout};
use crate::components::report::{linear::LinearBody, tree::TreeBody, pivot::PivotBody};
use crate::graphql::reports;
use crate::i18n::t;
use crate::styling::use_design;

#[component]
pub fn ReportView(report_id: String) -> impl IntoView {
    let definition = LocalResource::new({
        let id = report_id.clone();
        move || {
            let id = id.clone();
            async move { reports::fetch_report(&id).await.ok().flatten() }
        }
    });

    let result = LocalResource::new({
        let id = report_id.clone();
        move || {
            let id = id.clone();
            async move { reports::fetch_report_result(&id, None).await.unwrap_or_default() }
        }
    });

    let design = use_design();

    view! {
        <Suspense fallback=move || view! {
            <p style=design.report_empty_state().inline.clone()>
                { t("report-loading") }
            </p>
        }>
            {move || {
                let def = definition.read();
                let rows_opt = result.read();
                let (Some(Some(def)), Some(rows)) = (def.as_deref().cloned(), rows_opt.as_deref().cloned())
                    else { return view! { <p>{ t("report-loading") }</p> }.into_any(); };

                // Threshold-Check
                let threshold = def.row_threshold.unwrap_or(10_000);
                if rows.len() as u32 > threshold {
                    return view! {
                        <div style=design.report_threshold_banner().inline.clone()>
                            <p>{format!("{} > {}: {}",
                                t("report-threshold-rows"),
                                threshold,
                                t("report-threshold-message"))}</p>
                            <button>{ t("report-threshold-open-filter") }</button>
                        </div>
                    }.into_any();
                }

                // Empty-State
                if rows.is_empty() {
                    return view! {
                        <div style=design.report_empty_state().inline.clone()>
                            <p>{ t(&def.title_key) }</p>
                            <p>{ t("report-empty-hint") }</p>
                        </div>
                    }.into_any();
                }

                // Dispatch nach Layout
                match def.layout.clone() {
                    ReportLayout::Linear { footer } => view! {
                        <LinearBody columns=def.columns.clone() rows=rows.clone() footer=footer/>
                    }.into_any(),
                    ReportLayout::Tree { group_by, initial_expand_depth, show_connectors } => view! {
                        <TreeBody columns=def.columns.clone() rows=rows.clone()
                                  group_by=group_by initial_expand_depth=initial_expand_depth
                                  show_connectors=show_connectors/>
                    }.into_any(),
                    ReportLayout::Pivot { row_dim, col_dims, measures } => view! {
                        <PivotBody columns=def.columns.clone() rows=rows.clone()
                                   row_dim=row_dim col_dims=col_dims measures=measures/>
                    }.into_any(),
                }
            }}
        </Suspense>
    }
}
```

- [ ] **Schritt 2: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/report/view.rs
git commit -m "feat(client): ReportView shell with loading/empty/threshold/dispatch"
```

---

### Task 9: `<LinearBody>` + Footer-Aggregates

**Files:**
- Neu: `client/src/components/report/linear.rs`

- [ ] **Schritt 1: Komponente**

```rust
use leptos::prelude::*;
use shared::{Entity, FooterAggregate, ReportColumn, ColumnAlign};
use crate::components::report::aggregate::aggregate;
use crate::styling::use_design;
use crate::i18n::t;

#[component]
pub fn LinearBody(
    columns: Vec<ReportColumn>,
    rows: Vec<Entity>,
    footer: Vec<FooterAggregate>,
) -> impl IntoView {
    let design = use_design();
    let columns_for_footer = columns.clone();
    let rows_for_footer = rows.clone();

    view! {
        <table style="width:100%;border-collapse:collapse;">
            <thead>
                <tr>
                    {columns.iter().map(|c| {
                        let style = if matches!(c.align, Some(ColumnAlign::Right)) {
                            "text-align:right;"
                        } else { "text-align:left;" };
                        view! {
                            <th style=format!("{style}border-bottom:1px solid #44403c;padding:4px 8px;")>
                                { t(&c.label_key) }
                            </th>
                        }
                    }).collect_view()}
                </tr>
            </thead>
            <tbody>
                {rows.into_iter().map(|row| {
                    let cols = columns.clone();
                    view! {
                        <tr style=design.report_row_leaf().inline.clone()>
                            {cols.iter().map(|c| {
                                let v = row.fields.get(&c.key).cloned().unwrap_or(serde_json::Value::Null);
                                let is_num = matches!(v, serde_json::Value::Number(_));
                                let cell_style = if is_num {
                                    design.numeric_cell().inline.clone()
                                } else { "text-align:left;padding:2px 8px;".into() };
                                view! {
                                    <td style=cell_style>{render_cell(&v)}</td>
                                }
                            }).collect_view()}
                        </tr>
                    }
                }).collect_view()}
            </tbody>
            {(!footer.is_empty()).then(|| view! {
                <tfoot>
                    <tr style=design.report_footer_row().inline.clone()>
                        {columns_for_footer.iter().map(|c| {
                            let agg = footer.iter().find(|f| f.column == c.key);
                            match agg {
                                Some(f) => {
                                    let v = aggregate(&rows_for_footer, &c.key, f.op);
                                    let style = design.numeric_cell().inline.clone();
                                    view! { <td style=style>{format!("{:.2}", v)}</td> }
                                }
                                None => view! { <td>" "</td> },
                            }
                        }).collect_view()}
                    </tr>
                </tfoot>
            })}
        </table>
    }
}

fn render_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "—".into(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => format!("{}", n),
        other => other.to_string(),
    }
}
```

- [ ] **Schritt 2: Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/report/linear.rs
git commit -m "feat(client): LinearBody with footer aggregates"
```

---

### Task 10: `<TreeBody>` mit group_by + Expand/Collapse

**Files:**
- Neu: `client/src/components/report/tree.rs`

- [ ] **Schritt 1: Komponente**

```rust
use leptos::prelude::*;
use shared::{Entity, ReportColumn};
use std::collections::{BTreeMap, HashSet};
use crate::styling::use_design;
use crate::i18n::t;

#[component]
pub fn TreeBody(
    columns: Vec<ReportColumn>,
    rows: Vec<Entity>,
    group_by: Vec<String>,
    initial_expand_depth: u32,
    show_connectors: bool,
) -> impl IntoView {
    let design = use_design();
    let (expanded, set_expanded) = create_signal::<HashSet<String>>({
        // Pre-expand bis initial_expand_depth
        let mut s = HashSet::new();
        let tree = build_tree(&rows, &group_by);
        prefill_expand(&tree, &mut s, "", 0, initial_expand_depth as usize);
        s
    });

    let tree = build_tree(&rows, &group_by);

    view! {
        <table style="width:100%;border-collapse:collapse;">
            <thead>
                <tr>
                    {columns.iter().map(|c| view! {
                        <th style="text-align:left;padding:4px 8px;border-bottom:1px solid #44403c;">{ t(&c.label_key) }</th>
                    }).collect_view()}
                </tr>
            </thead>
            <tbody>
                {render_tree_nodes(tree, columns.clone(), &group_by, "", 0, expanded, set_expanded, show_connectors)}
            </tbody>
        </table>
    }
}

#[derive(Clone)]
struct TreeNode {
    label: String,
    children: BTreeMap<String, TreeNode>,
    leaves: Vec<Entity>,
}

fn build_tree(rows: &[Entity], group_by: &[String]) -> BTreeMap<String, TreeNode> {
    let mut root: BTreeMap<String, TreeNode> = BTreeMap::new();
    for row in rows {
        let mut cursor = &mut root;
        for (i, key) in group_by.iter().enumerate() {
            let val = row.fields.get(key)
                .and_then(|v| v.as_str().map(String::from).or_else(|| Some(v.to_string())))
                .unwrap_or_else(|| "—".into());
            let node = cursor.entry(val.clone()).or_insert(TreeNode {
                label: val.clone(), children: BTreeMap::new(), leaves: vec![],
            });
            if i == group_by.len() - 1 {
                node.leaves.push(row.clone());
            } else {
                cursor = &mut node.children;
            }
        }
    }
    root
}

fn prefill_expand(tree: &BTreeMap<String, TreeNode>, set: &mut HashSet<String>, prefix: &str, depth: usize, max: usize) {
    if depth >= max { return; }
    for (k, v) in tree {
        let path = format!("{prefix}/{k}");
        set.insert(path.clone());
        prefill_expand(&v.children, set, &path, depth + 1, max);
    }
}

fn render_tree_nodes(
    tree: BTreeMap<String, TreeNode>,
    columns: Vec<ReportColumn>,
    group_by: &[String],
    prefix: &str,
    depth: usize,
    expanded: ReadSignal<HashSet<String>>,
    set_expanded: WriteSignal<HashSet<String>>,
    show_connectors: bool,
) -> View<Vec<View<()>>> {
    let cols_for_inner = columns.clone();
    tree.into_iter().map(|(key, node)| {
        let path = format!("{prefix}/{key}");
        let is_expanded = move || expanded.with(|s| s.contains(&path));
        let toggle = {
            let path = path.clone();
            let set = set_expanded.clone();
            move |_| set.update(|s| {
                if s.contains(&path) { s.remove(&path); } else { s.insert(path.clone()); }
            })
        };
        let icon = move || if is_expanded() { "▾" } else { "▸" };
        let label = node.label.clone();
        let leaves = node.leaves.clone();
        let children = node.children.clone();

        let group_row = view! {
            <tr>
                <td style=format!("text-align:left;padding:2px 8px;padding-left:{}rem;",
                                  0.5 + (depth as f32) * 1.25)>
                    <span style="cursor:pointer;color:#78716c;" on:click=toggle.clone()>{icon}</span>
                    " "
                    <strong>{label}</strong>
                </td>
                {(0..cols_for_inner.len() - 1).map(|_| view! { <td/> }).collect_view()}
            </tr>
        };

        view! {
            {group_row}
            {move || is_expanded().then(|| {
                let leaves = leaves.clone();
                let cols = cols_for_inner.clone();
                let children_inner = children.clone();
                let nested = if !children_inner.is_empty() {
                    render_tree_nodes(children_inner, cols.clone(), group_by, &path,
                                      depth + 1, expanded, set_expanded, show_connectors).into_any()
                } else {
                    view! { <></> }.into_any()
                };
                view! {
                    {leaves.into_iter().map(|row| {
                        let cols = cols.clone();
                        view! {
                            <tr>
                                {cols.iter().enumerate().map(|(i, c)| {
                                    let v = row.fields.get(&c.key).cloned().unwrap_or(serde_json::Value::Null);
                                    let is_num = matches!(v, serde_json::Value::Number(_));
                                    let style = if i == 0 {
                                        format!("text-align:left;padding:2px 8px;padding-left:{}rem;",
                                                0.5 + ((depth + 1) as f32) * 1.25)
                                    } else if is_num {
                                        "font-variant-numeric:tabular-nums;text-align:right;padding:2px 8px;".into()
                                    } else { "padding:2px 8px;".into() };
                                    view! { <td style=style>{render_cell(&v)}</td> }
                                }).collect_view()}
                            </tr>
                        }
                    }).collect_view()}
                    {nested}
                }
            })}
        }
    }).collect_view()
}

fn render_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "—".into(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => format!("{}", n),
        other => other.to_string(),
    }
}
```

Hinweis zur Leptos-View-Komposition: oben ist konzeptionell — Leptos 0.7 verlangt typkonsistente Returns. Bei Compile-Fehlern via `.into_any()` an View-Boundaries normalisieren oder die View-Funktion umstrukturieren (z.B. mit `move ||` und `Suspense`).

- [ ] **Schritt 2: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/report/tree.rs
git commit -m "feat(client): TreeBody with group_by + expand/collapse + ▸/▾"
```

---

### Task 11: `<PivotBody>` mit Stacked-Headers + Sticky

**Files:**
- Neu: `client/src/components/report/pivot.rs`

- [ ] **Schritt 1: Komponente**

```rust
use leptos::prelude::*;
use shared::{Entity, PivotMeasure, ReportColumn};
use std::collections::BTreeMap;
use crate::components::report::aggregate::aggregate;
use crate::styling::use_design;
use crate::i18n::t;

#[component]
pub fn PivotBody(
    columns: Vec<ReportColumn>,
    rows: Vec<Entity>,
    row_dim: String,
    col_dims: Vec<String>,
    measures: Vec<PivotMeasure>,
) -> impl IntoView {
    let design = use_design();

    // V1: max 2 col-dims (Spec §9). Hard-Cap.
    let col_dims_capped = if col_dims.len() > 2 { col_dims[..2].to_vec() } else { col_dims };

    // Eindeutige Row- und Col-Keys sammeln
    let row_keys: Vec<String> = unique_values(&rows, &row_dim);
    let col_keys: Vec<Vec<String>> = col_dims_capped.iter()
        .map(|d| unique_values(&rows, d))
        .collect();

    // Pro Col-Tupel × Row eine Aggregate-Zelle
    let col_tuples: Vec<Vec<String>> = cartesian(&col_keys);

    view! {
        <div style="overflow:auto;max-height:80vh;">
            <table style="width:100%;border-collapse:collapse;">
                <thead>
                    // Erste Header-Row: Outer Col-Dim labels
                    <tr>
                        <th style=format!("{}{}", design.report_sticky_col().inline,
                                                  design.report_sticky_header().inline)>" "</th>
                        {col_keys.first().map(|outer| {
                            outer.iter().map(|v| view! {
                                <th colspan=measures.len()
                                    style=format!("{}{}", design.pivot_header_outer().inline,
                                                  design.report_sticky_header().inline)>
                                    {v.clone()}
                                </th>
                            }).collect_view()
                        })}
                    </tr>
                    // Zweite Header-Row: Measure-Namen pro Outer-Group
                    <tr>
                        <th style=design.report_sticky_col().inline.clone()>{ t(&row_dim) }</th>
                        {col_tuples.iter().map(|_| {
                            measures.iter().map(|m| view! {
                                <th style=design.pivot_header_inner().inline.clone()>{ t(&m.label_key) }</th>
                            }).collect_view()
                        }).collect_view()}
                    </tr>
                </thead>
                <tbody>
                    {row_keys.into_iter().map(|rk| {
                        let rk_for_cell = rk.clone();
                        view! {
                            <tr>
                                <td style=format!("{}{}", design.report_sticky_col().inline,
                                                          "padding:2px 8px;")>{rk.clone()}</td>
                                {col_tuples.iter().map(|ct| {
                                    let cells = measures.iter().map(|m| {
                                        let matching: Vec<Entity> = rows.iter()
                                            .filter(|r| {
                                                let row_match = r.fields.get(&row_dim)
                                                    .map(|v| v.to_string().trim_matches('"').to_string()) == Some(rk_for_cell.clone());
                                                let col_match = col_dims_capped.iter().enumerate().all(|(i, d)| {
                                                    r.fields.get(d).map(|v| v.to_string().trim_matches('"').to_string()).as_deref() == ct.get(i).map(String::as_str)
                                                });
                                                row_match && col_match
                                            })
                                            .cloned().collect();
                                        let v = aggregate(&matching, &m.column, m.op);
                                        let style = design.numeric_cell().inline.clone();
                                        view! { <td style=style>{format!("{:.2}", v)}</td> }
                                    }).collect_view();
                                    cells
                                }).collect_view()}
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

fn unique_values(rows: &[Entity], key: &str) -> Vec<String> {
    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for r in rows {
        if let Some(v) = r.fields.get(key) {
            set.insert(v.to_string().trim_matches('"').to_string());
        }
    }
    set.into_iter().collect()
}

fn cartesian(groups: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut result: Vec<Vec<String>> = vec![vec![]];
    for g in groups {
        let mut next: Vec<Vec<String>> = Vec::new();
        for prefix in &result {
            for v in g {
                let mut new_prefix = prefix.clone();
                new_prefix.push(v.clone());
                next.push(new_prefix);
            }
        }
        result = next;
    }
    result
}
```

- [ ] **Schritt 2: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/report/pivot.rs
git commit -m "feat(client): PivotBody with stacked headers + sticky col/header"
```

---

### Task 12: Route + Navigation

**Files:**
- Neu: `client/src/routes/report.rs`
- Modify: `client/src/app.rs`
- Modify: `client/src/components/navigation.rs`
- Modify: `client/src/lib.rs`

- [ ] **Schritt 1: Route**

`client/src/routes/report.rs`:

```rust
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::components::report::ReportView;

#[component]
pub fn ReportRoute() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();
    view! { <ReportView report_id=id()/> }
}
```

In `app.rs`:

```rust
<Route path=path!("/reports/:id") view=crate::routes::report::ReportRoute/>
```

- [ ] **Schritt 2: Navigation**

```rust
shared::MenuAction::OpenReport(id) => view! {
    <a href=format!("/reports/{}", id)>{ t(&node.label_key) }</a>
}.into_any(),
```

- [ ] **Schritt 3: Module-Registrierung in `lib.rs`**

```rust
pub mod components { pub mod report; /* ... */ }
pub mod routes { pub mod report; /* ... */ }
```

- [ ] **Schritt 4: Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/routes/report.rs client/src/app.rs client/src/components/navigation.rs client/src/lib.rs
git commit -m "feat(client): /reports/:id route + navigation"
```

---

### Task 13: i18n + manuelle Verifikation

**Files:**
- Modify: `client/locales/de/main.ftl`, `client/locales/en/main.ftl`

- [ ] **Schritt 1: Keys**

`client/locales/de/main.ftl`:
```
report-loading = Aggregation wird berechnet…
report-empty-hint = Filter prüfen oder Buchungsperiode öffnen.
report-threshold-rows = Zu viele Zeilen
report-threshold-message = Filter setzen, um den Report zu rendern.
report-threshold-open-filter = Filter öffnen
report-demo-linear-title = Demo (Linear)
report-demo-tree-title = Demo (Tree)
report-demo-pivot-title = Demo (Pivot)
report-total = Summe
customer = Kunde
total = Summe
status = Status
id = ID
```

`client/locales/en/main.ftl`:
```
report-loading = Computing aggregation…
report-empty-hint = Check filters or open the booking period.
report-threshold-rows = Too many rows
report-threshold-message = Apply filters to render the report.
report-threshold-open-filter = Open filter
report-demo-linear-title = Demo (Linear)
report-demo-tree-title = Demo (Tree)
report-demo-pivot-title = Demo (Pivot)
report-total = Total
customer = Customer
total = Total
status = Status
id = ID
```

- [ ] **Schritt 2: Manuelle Verifikation**

```
cargo run -p server -- --data-dir ./examples/shop &
cd client && trunk serve
```

- `/reports/demo-linear` → Linear-Tabelle mit Footer "Summe".
- `/reports/demo-tree` → Tree gruppiert nach `status`, Expand/Collapse via ▸/▾.
- `/reports/demo-pivot` → Pivot mit Kunden in Zeilen, Status in Spalten.

- [ ] **Schritt 3: Commit**

```
git add client/locales/
git commit -m "feat(client): i18n keys for report layouts"
```

---

### Task 14: Workspace-Cleanup + Self-Review

- [ ] **Schritt 1: Clippy + fmt + Tests**

```
CARGO_TARGET_DIR=target-test cargo clippy --workspace --no-deps -- -A clippy::doc_overindented_list_items -A clippy::too_many_arguments
cargo fmt
CARGO_TARGET_DIR=target-test cargo test --workspace
```

- [ ] **Schritt 2: Self-Review**

- Spec §3.1 ReportDefinition Wire → Task 1 ✓
- Spec §3.2 Datenfluss → Task 4 (Adapter über `entities:` Prefix; spätere Phase 1.7.12-Integration als Folge-Task dokumentiert) ✓
- Spec §3.3 Drei Strategien → Tasks 9, 10, 11 ✓
- Spec §3.4 Formatter-Wiederverwendung → V1: einfache Stringify in `render_cell` (Folge-Task ersetzt durch `registries::formatter`)
- Spec §4 Drei Beispiel-Configs → Task 3 ✓
- Spec §5 Edge-Cases: leere Daten (Empty-State Task 8), fehlende `group_by`-Spalte (Tree fällt graceful auf "—"-Label zurück — in build_tree); Pivot-Sparse (aggregate liefert 0.0 für leere Set) ✓
- Spec §6 Komponenten-Inventar → vollständig
- Spec §7 Tests: Wire (1), Server-Loader+reportResult (3, 4), Client-Unit (aggregate.rs Task 7) — Tree/Pivot-Layout-Unit-Tests fehlen, ergänzt in Schritt 3 falls Zeit
- Spec §9 Risks: Aggregations-Layer (Adapter-Strategie dokumentiert), Performance (Threshold-Banner Task 8), Tree-Sort-Stabilität (BTreeMap sorted alphabetisch), Pivot kombinatorische Explosion (Cap auf 2 col_dims in Task 11), Footer in Tree/Pivot (V1 nur Linear) ✓

- [ ] **Schritt 3: Optionale Unit-Tests für Tree/Pivot**

In `tree.rs` und `pivot.rs` jeweils einen Test, der `build_tree` / `cartesian` / `unique_values` als pure functions verifiziert:

```rust
// in tree.rs
#[cfg(test)]
mod tests {
    use super::*;
    // build_tree-Test mit 2 Levels, prüfen dass nested-keys korrekt sind
}

// in pivot.rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cartesian_empty_input_returns_one_empty() {
        let r = super::cartesian(&[]);
        assert_eq!(r, vec![vec![] as Vec<String>]);
    }
    #[test]
    fn cartesian_single_dim() {
        let r = super::cartesian(&[vec!["A".into(), "B".into()]]);
        assert_eq!(r.len(), 2);
    }
    #[test]
    fn cartesian_two_dims() {
        let r = super::cartesian(&[vec!["A".into(), "B".into()], vec!["X".into(), "Y".into()]]);
        assert_eq!(r.len(), 4);
    }
}
```

- [ ] **Schritt 4: Final-Commit**

```
git diff --quiet || git commit -am "style: cargo fmt"
```

---

## Was später kommt

- **V1.1**: `registries::formatter`-Reuse in `render_cell` (Geld, Datum, Prozent)
- **V1.2**: Tree/Pivot-Footer-Aggregates
- **V2**: Drill-Down (Klick auf Pivot-Zelle → Source-Rows-Liste)
- **V2**: Charts per Report-Definition
- **V2**: Phase 1.7.12 Aggregations-Source-Integration (echte `aggregation:<id>`)
- **V2**: Live-Update via Subscriptions
- **V3**: Multi-Measure-Pivot in Tree
