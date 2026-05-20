# U7 — Filter-Builder + Saved-Filters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Erweitert `shared::FilterCriteria` um boolean-Kombinatoren (`FilterExpression::All/Any/Not/Predicate`) als getaggte Union, bietet einen Drag-and-drop-Builder für nested AND/OR-Gruppen, persistiert benannte Filter pro User in einer neuen `saved_filter`-SeaORM-Tabelle und integriert das alles als optionale Toolbar-Erweiterung neben den bestehenden Per-Spalten-Quickfiltern (die unverändert bleiben).

**Architecture:** `shared::FilterExpression` additiv zu `FilterCriteria.expression: Option<FilterExpression>`. Bestehender `predicates: Vec<ColumnFilter>`-Pfad bleibt; Server konsolidiert beide zu einem Tree (`predicates → All(predicates) ∧ expression`) bevor die Source ausgewertet wird. Neue Source-Capability `supports_expression: bool` lässt Sources, die das nicht können, vom Server post-fetch nachfiltern. SeaORM-Tabelle `saved_filter` hält `(id, user_id, entity_type, name, expression_json, is_default, created_at, updated_at)`. Client: rekursive `<GroupNode>`-Komponente, `<PredicateRow>` mit dynamischem Value-Input je nach FieldType, Saved-Filter-Chip-Bar in der EntityTable-Toolbar.

**Tech Stack:** Rust, `shared` (serde, FilterCriteria), `server` (axum + async-graphql + SeaORM), `client` (Leptos + HTML5-DnD), `cargo test --workspace`.

**Spec-Referenz:** [`docs/superpowers/specs/2026-05-20-u7-filter-builder-design.md`](../specs/2026-05-20-u7-filter-builder-design.md).

**Vorbedingung:** keine. U1 (Reference-Picker) ergänzt sich später für Reference-Value-Inputs — bis dahin nutzt U7 ein einfaches Text-Input.

**Frontend-design Entscheidungen** (für den Builder):
1. **Engineering-Tooling-Aesthetic** statt SaaS-Glanz: enge Spalten-Grid `[column-select | operator | value-input | × ]` mit gleichmäßigem 8px-Gap, monospace für SQL-Preview, neutrale Backgrounds (`#f8fafc`).
2. **Indent-lines für Group-Nesting**, nicht Background-Tinting: 2px solid `#cbd5e1` left-border pro Group-Depth-Level. Vermeidet "Stacked-Grey-Bands"-Look, bleibt scan-fähig auch bei 4+ Levels.
3. **AND/OR-Toggle** als segmentierter Switch direkt am Group-Header (`[ AND | OR ]`), Amber-Akzent (`#d97706`) auf der aktiven Seite. Klar und kompakt — kein verstecktes Dropdown.
4. **Sticky Footer** mit `[Anwenden] [Speichern als…] [Verwerfen]` — Builder kann scrollen, Actions bleiben erreichbar.
5. **Saved-Filter-Chips** mit aktiver Markierung durch 2px solid Amber-Border, hover zeigt ein `×` zum Entfernen-aus-aktivem-Filter (löscht den Filter nicht, deselected nur).

---

## File Structure

**Neu:**
- `shared/src/filter_expression.rs` — `FilterExpression`-Enum
- `shared/tests/filter_expression_wire.rs` — Roundtrip + Backwards-Compat-Tests
- `server/src/entity/saved_filter.rs` — SeaORM-Modell
- `server/tests/expression_filter.rs` — End-to-End nested AND/OR
- `server/tests/saved_filters.rs` — CRUD + Permission + Default-Toggle
- `client/src/components/table/filter_builder/mod.rs`
- `client/src/components/table/filter_builder/group.rs` — rekursive Group-Node
- `client/src/components/table/filter_builder/predicate_row.rs`
- `client/src/components/table/filter_builder/column_picker.rs`
- `client/src/components/table/filter_builder/operator_picker.rs`
- `client/src/components/table/filter_builder/value_input.rs`
- `client/src/components/table/filter_builder/dnd.rs` — HTML5-DnD-Helper
- `client/src/components/table/filter_builder/chip_bar.rs` — Saved-Filter-Chips
- `client/src/components/table/filter_builder/reducer.rs` — Unit-tested Tree-Mutation
- `client/src/graphql/saved_filters.rs`

**Modifizieren:**
- `shared/src/lib.rs` — Re-Export `FilterExpression`, Erweiterung `FilterCriteria.expression`
- `shared/src/source/mod.rs` (oder wo `Capabilities` lebt) — `supports_expression: bool`
- `server/src/db.rs` — `saved_filter`-Tabelle in `init`
- `server/src/data.rs` — `merge_expression(predicates, expression)`, Expression→Filter-Anwendung, `expression_to_sql_debug`
- `server/src/schema.rs` — `entities`-Resolver nimmt `expression` an; Saved-Filter Queries/Mutations
- `client/src/components/table/state.rs` — Filter-State um `expression` erweitern
- `client/src/components/table/shell.rs` — Chip-Bar + Builder-Toggle einbinden
- `client/src/lib.rs` — Modul exportieren
- `client/locales/de/main.ftl`, `client/locales/en/main.ftl`

---

## Architektur-Details

### `FilterExpression` Wire

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FilterExpression {
    Predicate(crate::ColumnFilter),
    All { children: Vec<FilterExpression> },
    Any { children: Vec<FilterExpression> },
    Not { child: Box<FilterExpression> },
}
```

In `FilterCriteria` ergänzen:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub expression: Option<FilterExpression>,
```

### Source-Capability

```rust
// in shared::source oder direkt in shared::lib
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapabilities {
    pub supports_expression: bool,
    // … evtl. bestehende Felder
}
```

Falls `Capabilities` schon existiert: nur das Feld dazu. managed-sqlite + foreign-sqlite setzen `true`; memory-/REST-Sources `false`.

### SeaORM `saved_filter`

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "saved_filters")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub entity_type: String,
    pub name: String,
    pub expression_json: String,
    pub is_default: bool,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

### Eval-Logik

`server/src/data.rs::apply_expression(rows, expression)` filtert post-fetch für Sources ohne SQL-Übersetzung. SQL-Sources bekommen `expression` an die Source-API weitergereicht; pro Source-Impl wird `expression_to_sql_where(expression) -> String` (oder eine SeaORM-`Condition`) gebaut.

In V1 reicht es, die Post-Fetch-Variante zu implementieren — auch managed-sqlite kann das. Eine spätere Optimierung zieht die WHERE-Clause direkt rein.

```rust
pub fn matches(row: &shared::Entity, expr: &shared::FilterExpression) -> bool {
    use shared::FilterExpression as E;
    match expr {
        E::Predicate(p) => crate::data::predicate_matches(row, p),
        E::All { children } => children.iter().all(|c| matches(row, c)),
        E::Any { children } => children.iter().any(|c| matches(row, c)),
        E::Not { child } => !matches(row, child),
    }
}

pub fn merge_into(criteria: &mut shared::FilterCriteria) -> Option<shared::FilterExpression> {
    use shared::FilterExpression as E;
    let pred_expr = (!criteria.predicates.is_empty()).then(|| {
        E::All {
            children: criteria.predicates.iter().cloned().map(E::Predicate).collect(),
        }
    });
    match (pred_expr, criteria.expression.take()) {
        (Some(p), Some(e)) => Some(E::All { children: vec![p, e] }),
        (Some(p), None) => Some(p),
        (None, Some(e)) => Some(e),
        (None, None) => None,
    }
}
```

### SQL-Debug-Renderer

`expression_to_sql_debug(expr) -> String` produziert ein menschen-lesbares Pseudo-SQL (kein echtes parameter-binding, nur Anzeige). Für Power-User-Trust.

### Client-Reducer

Der Builder-Tree wird als `RwSignal<FilterExpression>` gehalten. Mutationen über einen rein-funktionalen Reducer (`apply_action(expr, action)`) — leicht testbar.

```rust
pub enum BuilderAction {
    AddPredicate { path: Vec<usize> },
    RemoveNode { path: Vec<usize> },
    ToggleGroupKind { path: Vec<usize> }, // All ↔ Any
    SetPredicate { path: Vec<usize>, predicate: shared::ColumnFilter },
    MoveNode { from: Vec<usize>, to: Vec<usize> },
    WrapInNot { path: Vec<usize> },
}
```

`path` ist eine Kind-Index-Pfad-Liste durch das Tree.

### Was NICHT in diesem Plan ist

- SQL-Editor / "Schreib selber"-Modus (Spec §2)
- Cross-Entity-Filter (Joins)
- Geteilte Saved-Filters über User hinweg (Phase 0.7)
- Filter-Templates aus YAML (Spec §2)
- Echte SQL-WHERE-Übersetzung in den Source-Impls — post-fetch reicht für V1

---

## Tasks

### Task 1: `FilterExpression` Wire + Backwards-Compat

**Files:**
- Neu: `shared/src/filter_expression.rs`
- Modify: `shared/src/lib.rs` (FilterCriteria + Re-Export)
- Neu: `shared/tests/filter_expression_wire.rs`

- [ ] **Schritt 1: Failing Test**

`shared/tests/filter_expression_wire.rs`:

```rust
use shared::{FilterExpression, FilterCriteria, ColumnFilter, FilterPredicate};

#[test]
fn predicate_variant_camel_case() {
    let e = FilterExpression::Predicate(ColumnFilter {
        column: "name".into(),
        predicate: FilterPredicate::TextContains { value: "abc".into(), case_insensitive: true },
    });
    let json = serde_json::to_string(&e).unwrap();
    assert!(json.contains(r#""kind":"predicate""#));
}

#[test]
fn nested_all_any_roundtrip() {
    let e = FilterExpression::All {
        children: vec![
            FilterExpression::Predicate(ColumnFilter {
                column: "a".into(),
                predicate: FilterPredicate::IsNull,
            }),
            FilterExpression::Any {
                children: vec![
                    FilterExpression::Predicate(ColumnFilter {
                        column: "b".into(),
                        predicate: FilterPredicate::IsNotNull,
                    }),
                ],
            },
        ],
    };
    let json = serde_json::to_string(&e).unwrap();
    let back: FilterExpression = serde_json::from_str(&json).unwrap();
    assert_eq!(back, e);
}

#[test]
fn not_variant() {
    let inner = FilterExpression::Predicate(ColumnFilter {
        column: "x".into(),
        predicate: FilterPredicate::IsNull,
    });
    let e = FilterExpression::Not { child: Box::new(inner) };
    let json = serde_json::to_string(&e).unwrap();
    assert!(json.contains(r#""kind":"not""#));
    let back: FilterExpression = serde_json::from_str(&json).unwrap();
    assert_eq!(back, e);
}

#[test]
fn filter_criteria_backwards_compat_empty_expression() {
    let old_json = r#"{"predicates":[]}"#;
    let c: FilterCriteria = serde_json::from_str(old_json).unwrap();
    assert!(c.expression.is_none());
}

#[test]
fn filter_criteria_with_expression_camel_case() {
    let mut c = FilterCriteria::default();
    c.expression = Some(FilterExpression::Predicate(ColumnFilter {
        column: "x".into(), predicate: FilterPredicate::IsNull,
    }));
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("expression"));
}
```

(Konkrete `FilterPredicate`-Varianten an heutigen Stand anpassen; `grep -n "enum FilterPredicate" shared/src/lib.rs`.)

- [ ] **Schritt 2: FAIL**

- [ ] **Schritt 3: Modul anlegen**

`shared/src/filter_expression.rs` mit Code aus "Architektur-Details".

- [ ] **Schritt 4: `FilterCriteria.expression` ergänzen**

In `shared/src/lib.rs`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FilterCriteria {
    #[serde(default)] pub global_search: Option<String>,
    #[serde(default)] pub predicates: Vec<ColumnFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<FilterExpression>,
}
```

Re-Export:
```rust
pub mod filter_expression;
pub use filter_expression::FilterExpression;
```

- [ ] **Schritt 5: PASS + Commit**

```
cargo test -p shared --test filter_expression_wire
git add shared/src/filter_expression.rs shared/src/lib.rs shared/tests/filter_expression_wire.rs
git commit -m "feat(shared): FilterExpression tagged enum + FilterCriteria.expression"
```

---

### Task 2: `SourceCapabilities.supports_expression`

**Files:**
- Modify: `shared/src/source/mod.rs` (oder wo `Capabilities` heute lebt; `grep`)
- Modify (Test): `shared/tests/filter_expression_wire.rs`

- [ ] **Schritt 1: Test ergänzen**

```rust
use shared::source::SourceCapabilities; // oder real Pfad

#[test]
fn capabilities_default_no_expression_support() {
    let c = SourceCapabilities::default();
    assert!(!c.supports_expression);
}
```

- [ ] **Schritt 2: FAIL → Feld ergänzen**

In `Capabilities`-Struct:

```rust
#[serde(default)]
pub supports_expression: bool,
```

- [ ] **Schritt 3: PASS + Commit**

```
cargo test -p shared
git add shared/src/source/ shared/tests/filter_expression_wire.rs
git commit -m "feat(shared): SourceCapabilities.supports_expression flag"
```

---

### Task 3: Server `data::matches` + `data::merge_into` Helpers

**Files:**
- Modify: `server/src/data.rs`

- [ ] **Schritt 1: Inline-Tests**

In `server/src/data.rs` am Ende:

```rust
#[cfg(test)]
mod expression_tests {
    use super::*;
    use shared::{ColumnFilter, Entity, FilterCriteria, FilterExpression, FilterPredicate};
    use std::collections::BTreeMap;

    fn entity(fields: Vec<(&str, serde_json::Value)>) -> Entity {
        let map: BTreeMap<String, serde_json::Value> = fields.into_iter()
            .map(|(k, v)| (k.into(), v)).collect();
        Entity { id: "1".into(), fields: map }
    }

    #[test]
    fn predicate_matches_simple() {
        let e = entity(vec![("status", serde_json::json!("paid"))]);
        let expr = FilterExpression::Predicate(ColumnFilter {
            column: "status".into(),
            predicate: FilterPredicate::TextEquals { value: "paid".into(), case_insensitive: false },
        });
        assert!(crate::data::expr_matches(&e, &expr));
    }

    #[test]
    fn all_combines_with_and() {
        let e = entity(vec![("a", serde_json::json!("x")), ("b", serde_json::json!("y"))]);
        let expr = FilterExpression::All {
            children: vec![
                FilterExpression::Predicate(ColumnFilter {
                    column: "a".into(),
                    predicate: FilterPredicate::TextEquals { value: "x".into(), case_insensitive: false },
                }),
                FilterExpression::Predicate(ColumnFilter {
                    column: "b".into(),
                    predicate: FilterPredicate::TextEquals { value: "y".into(), case_insensitive: false },
                }),
            ],
        };
        assert!(crate::data::expr_matches(&e, &expr));
    }

    #[test]
    fn any_combines_with_or() {
        let e = entity(vec![("a", serde_json::json!("x"))]);
        let expr = FilterExpression::Any {
            children: vec![
                FilterExpression::Predicate(ColumnFilter {
                    column: "a".into(),
                    predicate: FilterPredicate::TextEquals { value: "x".into(), case_insensitive: false },
                }),
                FilterExpression::Predicate(ColumnFilter {
                    column: "a".into(),
                    predicate: FilterPredicate::TextEquals { value: "y".into(), case_insensitive: false },
                }),
            ],
        };
        assert!(crate::data::expr_matches(&e, &expr));
    }

    #[test]
    fn not_negates() {
        let e = entity(vec![("a", serde_json::json!("x"))]);
        let inner = FilterExpression::Predicate(ColumnFilter {
            column: "a".into(),
            predicate: FilterPredicate::IsNull,
        });
        let expr = FilterExpression::Not { child: Box::new(inner) };
        assert!(crate::data::expr_matches(&e, &expr));
    }

    #[test]
    fn merge_into_combines_predicates_and_expression() {
        let mut c = FilterCriteria::default();
        c.predicates.push(ColumnFilter {
            column: "a".into(),
            predicate: FilterPredicate::IsNotNull,
        });
        c.expression = Some(FilterExpression::Predicate(ColumnFilter {
            column: "b".into(), predicate: FilterPredicate::IsNotNull,
        }));
        let merged = crate::data::merge_into(&mut c).unwrap();
        match merged {
            FilterExpression::All { children } => assert_eq!(children.len(), 2),
            _ => panic!("expected All"),
        }
    }
}
```

- [ ] **Schritt 2: FAIL → Implementierung in `data.rs`**

```rust
pub fn expr_matches(row: &shared::Entity, expr: &shared::FilterExpression) -> bool {
    use shared::FilterExpression as E;
    match expr {
        E::Predicate(p) => predicate_matches(row, p),
        E::All { children } => children.iter().all(|c| expr_matches(row, c)),
        E::Any { children } => children.iter().any(|c| expr_matches(row, c)),
        E::Not { child } => !expr_matches(row, child),
    }
}

pub fn merge_into(criteria: &mut shared::FilterCriteria) -> Option<shared::FilterExpression> {
    use shared::FilterExpression as E;
    let preds = std::mem::take(&mut criteria.predicates);
    let pred_expr = if preds.is_empty() { None } else {
        Some(E::All { children: preds.into_iter().map(E::Predicate).collect() })
    };
    let mut expr = criteria.expression.take();
    match (pred_expr, expr.take()) {
        (Some(p), Some(e)) => Some(E::All { children: vec![p, e] }),
        (Some(p), None) => Some(p),
        (None, Some(e)) => Some(e),
        (None, None) => None,
    }
}

pub fn expression_to_sql_debug(expr: &shared::FilterExpression) -> String {
    use shared::FilterExpression as E;
    match expr {
        E::Predicate(p) => format!("{} <op>", p.column),  // erweiterbar pro FilterPredicate-Variante
        E::All { children } => children.iter().map(expression_to_sql_debug)
            .collect::<Vec<_>>().join(" AND "),
        E::Any { children } => {
            let parts: Vec<String> = children.iter().map(expression_to_sql_debug).collect();
            format!("({})", parts.join(" OR "))
        }
        E::Not { child } => format!("NOT ({})", expression_to_sql_debug(child)),
    }
}
```

`predicate_matches` muss existieren — falls nicht, schreiben (per Variante des heutigen `FilterPredicate`).

- [ ] **Schritt 3: PASS + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server data::expression_tests
git add server/src/data.rs
git commit -m "feat(server): expr_matches + merge_into helpers"
```

---

### Task 4: `entities`-Resolver nimmt `expression`

**Files:**
- Modify: `server/src/schema.rs`

- [ ] **Schritt 1: Resolver-Signatur erweitern**

`entities(...)` akzeptiert heute `filter: Option<Json<FilterCriteria>>`. Da `expression` jetzt Teil von `FilterCriteria` ist, ist die GraphQL-Signatur unverändert — aber im Resolver-Body muss der Merge stattfinden:

```rust
let mut criteria = filter.map(|j| j.0).unwrap_or_default();
let merged_expression = crate::data::merge_into(&mut criteria);
// Source-Aufruf bekommt entweder kombinierte expression oder die alte criteria
let page = crate::data::entities_page(
    entity_type, sort_by, sort_dir,
    /* expression: */ merged_expression,
    /* page/limit: */ page, limit,
).await?;
```

`entities_page`-Signatur anpassen, um optional `Option<FilterExpression>` zu nehmen.

- [ ] **Schritt 2: Source-Layer**

In `server/src/source/...` (oder dort, wo Sources `list_page` implementieren):

```rust
// neue Methode optional via Default-Impl, die zur alten delegiert:
async fn list_page_v2(
    &self,
    binding: &EntityBinding,
    sort: Sort,
    expression: Option<&FilterExpression>,
    page: u32, limit: u32,
) -> Result<EntityPage, SourceError> {
    // Default: alle laden + nachfiltern (post-fetch)
    let mut all = self.list_all(binding).await?;
    if let Some(e) = expression {
        all.retain(|row| crate::data::expr_matches(row, e));
    }
    let total = all.len() as u32;
    let start = (page * limit) as usize;
    let end = (start + limit as usize).min(all.len());
    let rows = all.get(start..end).map(|s| s.to_vec()).unwrap_or_default();
    Ok(EntityPage { total, rows })
}
```

(Pragmatisch — managed-sqlite Source kann eine optimierte SQL-Variante haben, aber V1 nutzt post-fetch.)

- [ ] **Schritt 3: Compile + bestehende Tests**

```
CARGO_TARGET_DIR=target-test cargo test -p server
```

Erwartung: alle bestehenden Tests grün; keine Regressionen.

- [ ] **Schritt 4: Commit**

```
git add server/src/schema.rs server/src/data.rs server/src/source/
git commit -m "feat(server): entities resolver applies merged FilterExpression"
```

---

### Task 5: End-to-End-Test für Boolean-Filter

**Files:**
- Neu: `server/tests/expression_filter.rs`

- [ ] **Schritt 1: Test**

```rust
use serde_json::json;

#[tokio::test]
#[serial_test::serial]
async fn nested_and_or_filter_works() {
    server::fresh_test_setup().await;

    // Annahme: Order-Fixture hat status=draft/paid und total=numeric
    // Filter: (status=paid) AND (total>=50 OR customer_name="VIP")
    let expression = json!({
        "kind": "all",
        "children": [
            {"kind": "predicate", "predicate": {
                "column": "status",
                "predicate": {"kind": "textEquals", "value": "paid", "caseInsensitive": false}
            }},
            {"kind": "any", "children": [
                {"kind": "predicate", "predicate": {
                    "column": "total",
                    "predicate": {"kind": "numberRange", "min": 50.0, "max": null}
                }},
                {"kind": "predicate", "predicate": {
                    "column": "customer_name",
                    "predicate": {"kind": "textEquals", "value": "VIP", "caseInsensitive": false}
                }}
            ]}
        ]
    });

    let body = json!({
        "query": "query Q($f: JSON) { entities(entityType: \"Order\", filter: $f) }",
        "variables": { "f": { "predicates": [], "expression": expression } }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let rows = resp["data"]["entities"].as_array().unwrap();
    // Konkrete Assertions hängen vom Fixture ab
    assert!(rows.iter().all(|r| {
        let status = r["fields"]["status"].as_str().unwrap_or("");
        status == "paid"
    }), "all rows should have status=paid");
}

#[tokio::test]
#[serial_test::serial]
async fn not_filter_negates() {
    server::fresh_test_setup().await;
    let expression = json!({
        "kind": "not",
        "child": {"kind": "predicate", "predicate": {
            "column": "status",
            "predicate": {"kind": "textEquals", "value": "draft", "caseInsensitive": false}
        }}
    });
    let body = json!({
        "query": "query Q($f: JSON) { entities(entityType: \"Order\", filter: $f) }",
        "variables": { "f": { "predicates": [], "expression": expression } }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let rows = resp["data"]["entities"].as_array().unwrap();
    assert!(rows.iter().all(|r| r["fields"]["status"] != "draft"));
}

#[tokio::test]
#[serial_test::serial]
async fn old_predicates_path_still_works() {
    server::fresh_test_setup().await;
    let body = json!({
        "query": "query Q($f: JSON) { entities(entityType: \"Order\", filter: $f) }",
        "variables": { "f": { "predicates": [
            {"column": "status", "predicate": {"kind": "textEquals", "value": "paid", "caseInsensitive": false}}
        ]} }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let rows = resp["data"]["entities"].as_array().unwrap();
    assert!(!rows.is_empty());
}
```

- [ ] **Schritt 2: Tests laufen + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test expression_filter
git add server/tests/expression_filter.rs
git commit -m "test(server): nested AND/OR/NOT filter end-to-end"
```

---

### Task 6: SeaORM `saved_filter`-Tabelle + DB-Init

**Files:**
- Neu: `server/src/entity/saved_filter.rs`
- Modify: `server/src/entity/mod.rs`
- Modify: `server/src/db.rs`

- [ ] **Schritt 1: Modell**

`server/src/entity/saved_filter.rs` — Code aus "Architektur-Details".

- [ ] **Schritt 2: Mod registrieren + DB-Init ergänzen**

`server/src/entity/mod.rs`:
```rust
pub mod saved_filter;
```

`server/src/db.rs::init`:
```rust
let stmt = schema.create_table_from_entity(crate::entity::saved_filter::Entity);
db.execute(builder.build(&stmt.if_not_exists())).await?;
```

- [ ] **Schritt 3: Inline-Unit-Test**

In `saved_filter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, Set};

    #[tokio::test]
    #[serial_test::serial]
    async fn insert_and_query_by_user() {
        server::fresh_test_setup().await;
        let db = server::db::get().await;
        let now = chrono::Utc::now();

        let m = ActiveModel {
            id: Set("f1".into()), user_id: Set("alice".into()),
            entity_type: Set("Order".into()), name: Set("Q1".into()),
            expression_json: Set("{\"kind\":\"all\",\"children\":[]}".into()),
            is_default: Set(false),
            created_at: Set(now), updated_at: Set(now),
        };
        m.insert(db).await.unwrap();

        use sea_orm::QueryFilter;
        let rows = Entity::find()
            .filter(Column::UserId.eq("alice"))
            .all(db).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Q1");
    }
}
```

- [ ] **Schritt 4: Tests + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server entity::saved_filter::tests
git add server/src/entity/saved_filter.rs server/src/entity/mod.rs server/src/db.rs
git commit -m "feat(server): saved_filter SeaORM model + db init"
```

---

### Task 7: Saved-Filter GraphQL CRUD

**Files:**
- Modify: `server/src/schema.rs`
- Neu: `server/tests/saved_filters.rs`

- [ ] **Schritt 1: Resolver**

```rust
async fn saved_filters(
    &self, ctx: &Context<'_>,
    entity_type: String,
) -> async_graphql::Result<Vec<async_graphql::Json<SavedFilterOut>>> {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let user_id = crate::auth::current_user_id(ctx)
        .map_err(|_| async_graphql::Error::new("auth required"))?;
    let db = crate::db::get().await;
    let rows = crate::entity::saved_filter::Entity::find()
        .filter(crate::entity::saved_filter::Column::UserId.eq(user_id))
        .filter(crate::entity::saved_filter::Column::EntityType.eq(entity_type))
        .all(db).await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;
    Ok(rows.into_iter().map(|r| async_graphql::Json(SavedFilterOut {
        id: r.id, name: r.name, entity_type: r.entity_type,
        expression: serde_json::from_str(&r.expression_json).unwrap_or(serde_json::json!(null)),
        is_default: r.is_default,
        updated_at: r.updated_at.to_rfc3339(),
    })).collect())
}

async fn create_saved_filter(
    &self, ctx: &Context<'_>,
    input: async_graphql::Json<SavedFilterCreateInput>,
) -> async_graphql::Result<async_graphql::Json<SavedFilterOut>> {
    use sea_orm::{ActiveModelTrait, Set};
    let user_id = crate::auth::current_user_id(ctx)
        .map_err(|_| async_graphql::Error::new("auth required"))?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let m = crate::entity::saved_filter::ActiveModel {
        id: Set(id.clone()), user_id: Set(user_id),
        entity_type: Set(input.0.entity_type.clone()),
        name: Set(input.0.name.clone()),
        expression_json: Set(serde_json::to_string(&input.0.expression)?),
        is_default: Set(input.0.is_default),
        created_at: Set(now), updated_at: Set(now),
    };
    let db = crate::db::get().await;
    let saved = m.insert(db).await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

    // Default-Toggle: wenn is_default=true, alle anderen für (user, entity_type) auf false
    if input.0.is_default {
        unset_other_defaults(&saved.user_id, &saved.entity_type, &saved.id).await?;
    }

    Ok(async_graphql::Json(SavedFilterOut {
        id: saved.id, name: saved.name, entity_type: saved.entity_type,
        expression: input.0.expression, is_default: saved.is_default,
        updated_at: saved.updated_at.to_rfc3339(),
    }))
}

async fn delete_saved_filter(
    &self, ctx: &Context<'_>, id: String,
) -> async_graphql::Result<bool> {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let user_id = crate::auth::current_user_id(ctx)
        .map_err(|_| async_graphql::Error::new("auth required"))?;
    let db = crate::db::get().await;
    let res = crate::entity::saved_filter::Entity::delete_many()
        .filter(crate::entity::saved_filter::Column::Id.eq(id))
        .filter(crate::entity::saved_filter::Column::UserId.eq(user_id))
        .exec(db).await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;
    Ok(res.rows_affected > 0)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFilterCreateInput {
    pub entity_type: String,
    pub name: String,
    pub expression: serde_json::Value,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFilterOut {
    pub id: String, pub name: String, pub entity_type: String,
    pub expression: serde_json::Value, pub is_default: bool,
    pub updated_at: String,
}

async fn unset_other_defaults(user_id: &str, entity_type: &str, except_id: &str) -> async_graphql::Result<()> {
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let db = crate::db::get().await;
    let rows = crate::entity::saved_filter::Entity::find()
        .filter(crate::entity::saved_filter::Column::UserId.eq(user_id))
        .filter(crate::entity::saved_filter::Column::EntityType.eq(entity_type))
        .filter(crate::entity::saved_filter::Column::IsDefault.eq(true))
        .all(db).await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;
    for r in rows {
        if r.id == except_id { continue; }
        let mut m: crate::entity::saved_filter::ActiveModel = r.into();
        m.is_default = Set(false);
        m.update(db).await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
    }
    Ok(())
}
```

- [ ] **Schritt 2: Test**

`server/tests/saved_filters.rs`:

```rust
use serde_json::json;

#[tokio::test]
#[serial_test::serial]
async fn create_then_list_saved_filter() {
    server::fresh_test_setup().await;
    server::test_helpers::login_as("alice").await;

    let body = json!({
        "query": "mutation C($i: JSON!) { createSavedFilter(input: $i) }",
        "variables": {"i": {
            "entityType": "Order", "name": "Q1",
            "expression": {"kind": "all", "children": []},
            "isDefault": false
        }}
    });
    let _: serde_json::Value = server::test_helpers::run_graphql(&body).await;

    let body = json!({
        "query": "query L($t: String!) { savedFilters(entityType: $t) }",
        "variables": {"t": "Order"}
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let arr = resp["data"]["savedFilters"].as_array().unwrap();
    assert!(arr.iter().any(|f| f["name"] == "Q1"));
}

#[tokio::test]
#[serial_test::serial]
async fn user_cannot_see_anothers_saved_filter() {
    server::fresh_test_setup().await;
    server::test_helpers::login_as("alice").await;
    let _: serde_json::Value = server::test_helpers::run_graphql(&json!({
        "query": "mutation C($i: JSON!) { createSavedFilter(input: $i) }",
        "variables": {"i": {"entityType": "Order", "name": "AliceQ",
                            "expression": {"kind": "all", "children": []}, "isDefault": false}}
    })).await;
    server::test_helpers::login_as("bob").await;
    let resp: serde_json::Value = server::test_helpers::run_graphql(&json!({
        "query": "query L($t: String!) { savedFilters(entityType: $t) }",
        "variables": {"t": "Order"}
    })).await;
    let arr = resp["data"]["savedFilters"].as_array().unwrap();
    assert!(!arr.iter().any(|f| f["name"] == "AliceQ"));
}

#[tokio::test]
#[serial_test::serial]
async fn default_toggle_unsets_others() {
    server::fresh_test_setup().await;
    server::test_helpers::login_as("alice").await;

    let mk = |name: &str, default: bool| json!({
        "query": "mutation C($i: JSON!) { createSavedFilter(input: $i) }",
        "variables": {"i": {"entityType": "Order", "name": name,
                            "expression": {"kind": "all", "children": []}, "isDefault": default}}
    });
    let _: serde_json::Value = server::test_helpers::run_graphql(&mk("A", true)).await;
    let _: serde_json::Value = server::test_helpers::run_graphql(&mk("B", true)).await;

    let resp: serde_json::Value = server::test_helpers::run_graphql(&json!({
        "query": "query L($t: String!) { savedFilters(entityType: $t) }",
        "variables": {"t": "Order"}
    })).await;
    let arr = resp["data"]["savedFilters"].as_array().unwrap();
    let defaults: Vec<_> = arr.iter().filter(|f| f["isDefault"] == true).collect();
    assert_eq!(defaults.len(), 1);
    assert_eq!(defaults[0]["name"], "B");
}
```

- [ ] **Schritt 3: Tests laufen + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test saved_filters
git add server/src/schema.rs server/tests/saved_filters.rs
git commit -m "feat(server): saved-filter GraphQL CRUD with user scoping"
```

---

### Task 8: Client-Reducer + Unit-Tests

**Files:**
- Neu: `client/src/components/table/filter_builder/reducer.rs`
- Neu: `client/src/components/table/filter_builder/mod.rs`

- [ ] **Schritt 1: Reducer**

```rust
use shared::{ColumnFilter, FilterExpression};

pub enum BuilderAction {
    AddPredicate { path: Vec<usize>, predicate: ColumnFilter },
    RemoveNode { path: Vec<usize> },
    SetGroupKind { path: Vec<usize>, kind: GroupKind },
    SetPredicate { path: Vec<usize>, predicate: ColumnFilter },
    WrapInNot { path: Vec<usize> },
    UnwrapNot { path: Vec<usize> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind { All, Any }

pub fn apply(expr: FilterExpression, action: BuilderAction) -> FilterExpression {
    match action {
        BuilderAction::AddPredicate { path, predicate } => {
            mutate_at(expr, &path, |node| {
                if let FilterExpression::All { children } | FilterExpression::Any { children } = node {
                    children.push(FilterExpression::Predicate(predicate));
                }
            })
        }
        BuilderAction::RemoveNode { path } => {
            if path.is_empty() {
                return FilterExpression::All { children: vec![] };
            }
            let (parent_path, idx) = path.split_at(path.len() - 1);
            let idx = idx[0];
            mutate_at(expr, parent_path, |parent| {
                if let FilterExpression::All { children } | FilterExpression::Any { children } = parent {
                    if idx < children.len() { children.remove(idx); }
                }
            })
        }
        BuilderAction::SetGroupKind { path, kind } => {
            mutate_at(expr, &path, |node| {
                if let FilterExpression::All { children } = node {
                    if kind == GroupKind::Any {
                        let cs = std::mem::take(children);
                        *node = FilterExpression::Any { children: cs };
                    }
                } else if let FilterExpression::Any { children } = node {
                    if kind == GroupKind::All {
                        let cs = std::mem::take(children);
                        *node = FilterExpression::All { children: cs };
                    }
                }
            })
        }
        BuilderAction::SetPredicate { path, predicate } => {
            mutate_at(expr, &path, |node| {
                *node = FilterExpression::Predicate(predicate);
            })
        }
        BuilderAction::WrapInNot { path } => {
            mutate_at(expr, &path, |node| {
                let inner = std::mem::replace(node, FilterExpression::All { children: vec![] });
                *node = FilterExpression::Not { child: Box::new(inner) };
            })
        }
        BuilderAction::UnwrapNot { path } => {
            mutate_at(expr, &path, |node| {
                if let FilterExpression::Not { child } = node {
                    let inner = std::mem::replace(child.as_mut(), FilterExpression::All { children: vec![] });
                    *node = inner;
                }
            })
        }
    }
}

fn mutate_at<F: FnOnce(&mut FilterExpression)>(mut root: FilterExpression, path: &[usize], f: F) -> FilterExpression {
    {
        let mut cursor: &mut FilterExpression = &mut root;
        for &idx in path {
            cursor = match cursor {
                FilterExpression::All { children } | FilterExpression::Any { children } => {
                    if idx < children.len() { &mut children[idx] } else { return root; }
                }
                FilterExpression::Not { child } => {
                    if idx == 0 { child.as_mut() } else { return root; }
                }
                FilterExpression::Predicate(_) => return root,
            };
        }
        f(cursor);
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{ColumnFilter, FilterPredicate};

    fn pred(col: &str) -> ColumnFilter {
        ColumnFilter {
            column: col.into(),
            predicate: FilterPredicate::IsNotNull,
        }
    }

    #[test]
    fn add_predicate_to_root() {
        let root = FilterExpression::All { children: vec![] };
        let r = apply(root, BuilderAction::AddPredicate { path: vec![], predicate: pred("a") });
        match r {
            FilterExpression::All { children } => assert_eq!(children.len(), 1),
            _ => panic!(),
        }
    }

    #[test]
    fn remove_first_child() {
        let root = FilterExpression::All { children: vec![
            FilterExpression::Predicate(pred("a")),
            FilterExpression::Predicate(pred("b")),
        ]};
        let r = apply(root, BuilderAction::RemoveNode { path: vec![0] });
        match r {
            FilterExpression::All { children } => {
                assert_eq!(children.len(), 1);
                if let FilterExpression::Predicate(p) = &children[0] {
                    assert_eq!(p.column, "b");
                }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn toggle_group_kind() {
        let root = FilterExpression::All { children: vec![FilterExpression::Predicate(pred("a"))] };
        let r = apply(root, BuilderAction::SetGroupKind { path: vec![], kind: GroupKind::Any });
        assert!(matches!(r, FilterExpression::Any { .. }));
    }

    #[test]
    fn wrap_in_not() {
        let root = FilterExpression::Predicate(pred("a"));
        let r = apply(root, BuilderAction::WrapInNot { path: vec![] });
        assert!(matches!(r, FilterExpression::Not { .. }));
    }

    #[test]
    fn nested_add() {
        let root = FilterExpression::All { children: vec![
            FilterExpression::Any { children: vec![] }
        ]};
        let r = apply(root, BuilderAction::AddPredicate { path: vec![0], predicate: pred("x") });
        match r {
            FilterExpression::All { children } => {
                match &children[0] {
                    FilterExpression::Any { children } => assert_eq!(children.len(), 1),
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
    }
}
```

- [ ] **Schritt 2: Modul-Scaffold**

`client/src/components/table/filter_builder/mod.rs`:

```rust
pub mod reducer;
pub mod predicate_row;
pub mod group;
pub mod column_picker;
pub mod operator_picker;
pub mod value_input;
pub mod dnd;
pub mod chip_bar;

pub use chip_bar::SavedFilterChipBar;
pub use group::FilterBuilderPanel;
```

- [ ] **Schritt 3: Tests laufen + Commit**

```
cargo test -p client --lib components::table::filter_builder::reducer::tests
git add client/src/components/table/filter_builder/
git commit -m "feat(client): filter builder reducer + unit tests"
```

---

### Task 9: `<PredicateRow>` + Pickers + Value-Input

**Files:**
- Neu: `client/src/components/table/filter_builder/column_picker.rs`
- Neu: `client/src/components/table/filter_builder/operator_picker.rs`
- Neu: `client/src/components/table/filter_builder/value_input.rs`
- Neu: `client/src/components/table/filter_builder/predicate_row.rs`

- [ ] **Schritt 1: Column-Picker**

```rust
use leptos::prelude::*;
use shared::ColumnMeta;

#[component]
pub fn ColumnPicker(
    columns: Vec<ColumnMeta>,
    value: ReadSignal<String>,
    on_change: Callback<String>,
) -> impl IntoView {
    let filterable: Vec<ColumnMeta> = columns.into_iter()
        .filter(|c| c.filterable)
        .collect();
    view! {
        <select on:change=move |ev| {
            let v = leptos::ev::target_value(&ev);
            on_change.call(v);
        }>
            <option value="">"–"</option>
            {filterable.iter().map(|c| {
                let key = c.key.clone();
                let label = c.label.clone();
                let selected = move || value.get() == key;
                view! { <option value=key.clone() selected=selected>{label}</option> }
            }).collect_view()}
        </select>
    }
}
```

- [ ] **Schritt 2: Operator-Picker**

```rust
use leptos::prelude::*;
use shared::{FieldType, FilterPredicate};

#[component]
pub fn OperatorPicker(
    field_type: ReadSignal<FieldType>,
    op: ReadSignal<String>, // OperatorId (z.B. "textEquals", "isNull")
    on_change: Callback<String>,
) -> impl IntoView {
    let ops_for_type = move || {
        match field_type.get() {
            FieldType::Text => vec!["textContains", "textEquals", "isNull", "isNotNull"],
            FieldType::Integer | FieldType::Decimal { .. } => vec!["numberEquals", "numberRange", "isNull"],
            FieldType::Boolean => vec!["boolEquals"],
            _ => vec!["isNull", "isNotNull"],
        }
    };
    view! {
        <select on:change=move |ev| {
            let v = leptos::ev::target_value(&ev);
            on_change.call(v);
        }>
            {move || ops_for_type().into_iter().map(|o| {
                let id = o.to_string();
                let label = match o {
                    "textContains" => "enthält",
                    "textEquals" => "ist gleich",
                    "isNull" => "ist leer",
                    "isNotNull" => "ist nicht leer",
                    "numberEquals" => "= ",
                    "numberRange" => "zwischen",
                    "boolEquals" => "ist",
                    _ => o,
                };
                let selected = move || op.get() == id;
                view! { <option value=id.clone() selected=selected>{label}</option> }
            }).collect_view()}
        </select>
    }
}
```

- [ ] **Schritt 3: Value-Input**

```rust
use leptos::prelude::*;
use shared::{FieldType, FilterPredicate};

#[component]
pub fn ValueInput(
    field_type: ReadSignal<FieldType>,
    op: ReadSignal<String>,
    value: ReadSignal<serde_json::Value>,
    on_change: Callback<serde_json::Value>,
) -> impl IntoView {
    let needs_input = move || !matches!(op.get().as_str(), "isNull" | "isNotNull");

    view! {
        {move || if !needs_input() {
            view! { <span/> }.into_any()
        } else {
            match field_type.get() {
                FieldType::Text => view! {
                    <input type="text" value=move || value.get().as_str().unwrap_or("").to_string()
                           on:input=move |ev| {
                               let v = leptos::ev::target_value(&ev);
                               on_change.call(serde_json::Value::String(v));
                           } />
                }.into_any(),
                FieldType::Integer => view! {
                    <input type="number" value=move || value.get().as_i64().map(|i| i.to_string()).unwrap_or_default()
                           on:input=move |ev| {
                               let v = leptos::ev::target_value(&ev);
                               if let Ok(n) = v.parse::<i64>() {
                                   on_change.call(serde_json::json!(n));
                               }
                           } />
                }.into_any(),
                FieldType::Boolean => view! {
                    <input type="checkbox" checked=move || value.get().as_bool().unwrap_or(false)
                           on:change=move |ev| {
                               let checked = event_target_checked(&ev);
                               on_change.call(serde_json::json!(checked));
                           } />
                }.into_any(),
                _ => view! {
                    <input type="text" value=move || value.get().to_string()
                           on:input=move |ev| {
                               let v = leptos::ev::target_value(&ev);
                               on_change.call(serde_json::Value::String(v));
                           } />
                }.into_any(),
            }
        }}
    }
}

fn event_target_checked(ev: &leptos::ev::Event) -> bool {
    use wasm_bindgen::JsCast;
    let target: web_sys::HtmlInputElement = ev.target().unwrap().dyn_into().unwrap();
    target.checked()
}
```

- [ ] **Schritt 4: PredicateRow stickt sie zusammen**

```rust
use leptos::prelude::*;
use shared::ColumnMeta;
use super::reducer::BuilderAction;
use super::column_picker::ColumnPicker;
use super::operator_picker::OperatorPicker;
use super::value_input::ValueInput;

#[component]
pub fn PredicateRow(
    path: Vec<usize>,
    predicate: shared::ColumnFilter,
    columns: Vec<ColumnMeta>,
    dispatch: Callback<BuilderAction>,
) -> impl IntoView {
    let (col, set_col) = create_signal(predicate.column.clone());
    let (op, set_op) = create_signal("textEquals".to_string()); // ableiten aus predicate.predicate
    let (value, set_value) = create_signal(serde_json::json!(""));
    let field_type = move || {
        columns.iter().find(|c| c.key == col.get())
            .map(|c| c.field_type.clone())
            .unwrap_or(shared::FieldType::Text)
    };
    let (ft, set_ft) = create_signal(field_type());

    // Auf col-Change FieldType + dispatch
    let dispatch_for_change = dispatch.clone();
    let path_for_remove = path.clone();
    let path_for_change = path.clone();

    let emit_predicate = move || {
        let cf = build_column_filter(&col.get(), &op.get(), &value.get());
        dispatch_for_change.call(BuilderAction::SetPredicate { path: path_for_change.clone(), predicate: cf });
    };

    view! {
        <div style="display:grid;grid-template-columns:200px 160px 1fr 32px;gap:8px;align-items:center;padding:4px;background:#f8fafc;border-radius:4px;">
            <ColumnPicker columns=columns.clone() value=col on_change=Callback::new(move |v| {
                set_col.set(v);
                emit_predicate();
            }) />
            <OperatorPicker field_type=ft op=op on_change=Callback::new(move |v| {
                set_op.set(v);
                emit_predicate();
            }) />
            <ValueInput field_type=ft op=op value=value on_change=Callback::new(move |v| {
                set_value.set(v);
                emit_predicate();
            }) />
            <button on:click=move |_| {
                dispatch.call(BuilderAction::RemoveNode { path: path_for_remove.clone() });
            }>"×"</button>
        </div>
    }
}

fn build_column_filter(column: &str, op: &str, value: &serde_json::Value) -> shared::ColumnFilter {
    shared::ColumnFilter {
        column: column.into(),
        predicate: match op {
            "isNull" => shared::FilterPredicate::IsNull,
            "isNotNull" => shared::FilterPredicate::IsNotNull,
            "textEquals" => shared::FilterPredicate::TextEquals {
                value: value.as_str().unwrap_or("").into(),
                case_insensitive: false,
            },
            "textContains" => shared::FilterPredicate::TextContains {
                value: value.as_str().unwrap_or("").into(),
                case_insensitive: false,
            },
            _ => shared::FilterPredicate::IsNull,
        },
    }
}
```

(Konkrete `FilterPredicate`-Varianten an heutigen Stand anpassen.)

- [ ] **Schritt 5: Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/table/filter_builder/
git commit -m "feat(client): predicate row + pickers + value input"
```

---

### Task 10: Rekursive `<GroupNode>` + Builder-Panel

**Files:**
- Neu: `client/src/components/table/filter_builder/group.rs`

- [ ] **Schritt 1: GroupNode**

```rust
use leptos::prelude::*;
use shared::{ColumnMeta, FilterExpression};
use super::reducer::{BuilderAction, GroupKind};
use super::predicate_row::PredicateRow;

#[component]
pub fn GroupNode(
    expr: FilterExpression,
    path: Vec<usize>,
    columns: Vec<ColumnMeta>,
    depth: usize,
    dispatch: Callback<BuilderAction>,
) -> impl IntoView {
    let indent_style = format!(
        "border-left:2px solid #cbd5e1;padding-left:{}px;margin-left:{}px;",
        12 + depth * 8,
        if depth == 0 { 0 } else { 8 },
    );

    match expr {
        FilterExpression::All { children } | FilterExpression::Any { children } => {
            let is_any = matches!(&FilterExpression::Any { children: vec![] }, _);
            // einfacher: kind aus dem Original-Pattern abfragen
            let kind = if matches!(expr.clone(), FilterExpression::Any { .. }) {
                GroupKind::Any
            } else {
                GroupKind::All
            };
            let toggle_path = path.clone();
            view! {
                <div style=indent_style>
                    <div style="display:flex;gap:8px;align-items:center;margin-bottom:4px;">
                        <strong>{ if kind == GroupKind::All { "AND" } else { "OR" } }</strong>
                        <button on:click={
                            let dispatch = dispatch.clone();
                            let toggle_path = toggle_path.clone();
                            move |_| {
                                let new_kind = if kind == GroupKind::All { GroupKind::Any } else { GroupKind::All };
                                dispatch.call(BuilderAction::SetGroupKind { path: toggle_path.clone(), kind: new_kind });
                            }
                        }>"toggle"</button>
                    </div>
                    {children.into_iter().enumerate().map(|(i, child)| {
                        let mut child_path = path.clone();
                        child_path.push(i);
                        view! {
                            <GroupNode expr=child path=child_path columns=columns.clone()
                                       depth=(depth+1) dispatch=dispatch.clone()/>
                        }
                    }).collect_view()}
                    <button on:click={
                        let dispatch = dispatch.clone();
                        let p = path.clone();
                        move |_| {
                            dispatch.call(BuilderAction::AddPredicate {
                                path: p.clone(),
                                predicate: shared::ColumnFilter {
                                    column: "".into(),
                                    predicate: shared::FilterPredicate::IsNull,
                                },
                            });
                        }
                    }>"+ Bedingung"</button>
                </div>
            }.into_any()
        }
        FilterExpression::Predicate(cf) => {
            view! {
                <PredicateRow path=path predicate=cf columns=columns dispatch=dispatch/>
            }.into_any()
        }
        FilterExpression::Not { child } => {
            view! {
                <div style=indent_style>
                    <strong>"NOT"</strong>
                    <GroupNode expr=*child path={ let mut p = path.clone(); p.push(0); p } columns=columns.clone()
                               depth=(depth+1) dispatch=dispatch.clone()/>
                </div>
            }.into_any()
        }
    }
}

#[component]
pub fn FilterBuilderPanel(
    expression: RwSignal<FilterExpression>,
    columns: Vec<ColumnMeta>,
    on_apply: Callback<FilterExpression>,
    on_save_as: Callback<FilterExpression>,
    on_discard: Callback<()>,
) -> impl IntoView {
    let dispatch = Callback::new({
        let expression = expression.clone();
        move |action: BuilderAction| {
            expression.update(|e| {
                let current = e.clone();
                *e = super::reducer::apply(current, action);
            });
        }
    });

    view! {
        <div style="border:1px solid #cbd5e1;padding:12px;background:#fff;">
            <GroupNode expr=expression.get_untracked() path=vec![] columns=columns
                       depth=0 dispatch=dispatch/>

            // SQL-Preview (read-only)
            <details style="margin-top:12px;">
                <summary>"SQL ansehen"</summary>
                <pre style="font-family:'JetBrains Mono',monospace;font-size:12px;background:#f1f5f9;padding:8px;">
                    {move || sql_debug(&expression.get())}
                </pre>
            </details>

            // Sticky Footer
            <footer style="position:sticky;bottom:0;background:#fff;padding-top:8px;display:flex;gap:8px;justify-content:flex-end;border-top:1px solid #e2e8f0;margin-top:12px;">
                <button on:click=move |_| on_discard.call(())>{ "Verwerfen" }</button>
                <button on:click=move |_| on_save_as.call(expression.get())>{ "Speichern als…" }</button>
                <button on:click=move |_| on_apply.call(expression.get())
                        style="background:#d97706;color:#fff;padding:4px 12px;border:0;border-radius:4px;">
                    { "Anwenden" }
                </button>
            </footer>
        </div>
    }
}

fn sql_debug(expr: &FilterExpression) -> String {
    use FilterExpression as E;
    match expr {
        E::Predicate(p) => format!("{} <op>", p.column),
        E::All { children } => children.iter().map(sql_debug).collect::<Vec<_>>().join(" AND "),
        E::Any { children } => {
            let parts: Vec<String> = children.iter().map(sql_debug).collect();
            format!("({})", parts.join(" OR "))
        }
        E::Not { child } => format!("NOT ({})", sql_debug(child)),
    }
}
```

(Hinweis: Pattern-Matching auf `expr` zweimal — `kind`-Detection ist etwas verbose; eine `clone()` + Match wäre sauberer. Refactor möglich, V1 reicht.)

- [ ] **Schritt 2: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/table/filter_builder/group.rs
git commit -m "feat(client): recursive GroupNode + FilterBuilderPanel"
```

---

### Task 11: Saved-Filter Client-Adapter

**Files:**
- Neu: `client/src/graphql/saved_filters.rs`
- Modify: `client/src/graphql/mod.rs`

- [ ] **Schritt 1: Adapter**

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SavedFilter {
    pub id: String, pub name: String, pub entity_type: String,
    pub expression: serde_json::Value, pub is_default: bool,
    pub updated_at: String,
}

pub async fn list(entity_type: &str) -> Result<Vec<SavedFilter>, crate::graphql::GraphqlError> {
    #[derive(Deserialize)]
    struct Resp { #[serde(rename = "savedFilters")] r: Vec<SavedFilter> }
    let vars = serde_json::json!({"t": entity_type});
    let r: Resp = crate::graphql::execute(
        "query L($t: String!) { savedFilters(entityType: $t) }", &vars,
    ).await?;
    Ok(r.r)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInput {
    pub entity_type: String, pub name: String,
    pub expression: serde_json::Value, pub is_default: bool,
}

pub async fn create(input: CreateInput) -> Result<SavedFilter, crate::graphql::GraphqlError> {
    #[derive(Deserialize)]
    struct Resp { #[serde(rename = "createSavedFilter")] r: SavedFilter }
    let vars = serde_json::json!({"i": input});
    let r: Resp = crate::graphql::execute(
        "mutation C($i: JSON!) { createSavedFilter(input: $i) }", &vars,
    ).await?;
    Ok(r.r)
}

pub async fn delete(id: &str) -> Result<bool, crate::graphql::GraphqlError> {
    #[derive(Deserialize)]
    struct Resp { #[serde(rename = "deleteSavedFilter")] r: bool }
    let vars = serde_json::json!({"id": id});
    let r: Resp = crate::graphql::execute(
        "mutation D($id: String!) { deleteSavedFilter(id: $id) }", &vars,
    ).await?;
    Ok(r.r)
}
```

- [ ] **Schritt 2: Commit**

```
git add client/src/graphql/saved_filters.rs client/src/graphql/mod.rs
git commit -m "feat(client): saved-filters GraphQL adapter"
```

---

### Task 12: Chip-Bar + Toolbar-Integration

**Files:**
- Neu: `client/src/components/table/filter_builder/chip_bar.rs`
- Modify: `client/src/components/table/shell.rs`

- [ ] **Schritt 1: Chip-Bar**

```rust
use leptos::prelude::*;
use crate::graphql::saved_filters::SavedFilter;

#[component]
pub fn SavedFilterChipBar(
    filters: Vec<SavedFilter>,
    active_id: ReadSignal<Option<String>>,
    on_select: Callback<SavedFilter>,
    on_deselect: Callback<()>,
) -> impl IntoView {
    view! {
        <div style="display:flex;gap:6px;flex-wrap:wrap;padding:4px;">
            {filters.into_iter().map(|f| {
                let f_for_select = f.clone();
                let f_for_id = f.id.clone();
                let active = move || active_id.get().as_deref() == Some(&f_for_id);
                let style = move || if active() {
                    "background:#fff;border:2px solid #d97706;color:#7c2d12;padding:2px 10px;border-radius:999px;cursor:pointer;"
                } else {
                    "background:#f8fafc;border:1px solid #cbd5e1;color:#0f172a;padding:2px 10px;border-radius:999px;cursor:pointer;"
                };
                view! {
                    <button style=style on:click={
                        let f = f_for_select.clone();
                        move |_| {
                            if active() { on_deselect.call(()); }
                            else { on_select.call(f.clone()); }
                        }
                    }>
                        {f.name.clone()}
                        {f.is_default.then(|| view! { <span style="font-size:10px;margin-left:4px;">"★"</span> })}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}
```

- [ ] **Schritt 2: Toolbar-Integration in `shell.rs`**

```rust
// Über der EntityTable-Toolbar:
{move || saved_filters.get().map(|list| view! {
    <SavedFilterChipBar
        filters=list
        active_id=active_filter_id
        on_select=Callback::new(move |f: SavedFilter| {
            // Expression aus f.expression in TableState.filter.expression schreiben
            let expr: Option<shared::FilterExpression> = serde_json::from_value(f.expression.clone()).ok();
            table_state.filter.update(|c| c.expression = expr);
            set_active_filter_id.set(Some(f.id.clone()));
        })
        on_deselect=Callback::new(move |_| {
            table_state.filter.update(|c| c.expression = None);
            set_active_filter_id.set(None);
        })
    />
})}

<button on:click=move |_| set_builder_open.set(!builder_open.get())>"Filter ▾"</button>

{move || builder_open.get().then(|| view! {
    <FilterBuilderPanel
        expression=builder_expression
        columns=columns.get()
        on_apply=Callback::new(move |expr| {
            table_state.filter.update(|c| c.expression = Some(expr));
            set_builder_open.set(false);
        })
        on_save_as=Callback::new(move |expr| {
            // Modal/Prompt für Namen
            let name = web_sys::window().unwrap().prompt_with_message("Filter-Name?").ok().flatten();
            if let Some(name) = name {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = crate::graphql::saved_filters::create(
                        crate::graphql::saved_filters::CreateInput {
                            entity_type: entity_type_for_save.clone(),
                            name, expression: serde_json::to_value(expr).unwrap(),
                            is_default: false,
                        }
                    ).await;
                });
            }
        })
        on_discard=Callback::new(move |_| set_builder_open.set(false))
    />
})}
```

- [ ] **Schritt 3: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/table/filter_builder/chip_bar.rs client/src/components/table/shell.rs
git commit -m "feat(client): chip bar + builder toggle integrated into table toolbar"
```

---

### Task 13: HTML5-DnD-Scaffold + Reorder

**Files:**
- Neu: `client/src/components/table/filter_builder/dnd.rs`

- [ ] **Schritt 1: V1: Up/Down-Buttons statt full DnD**

Pragmatisch: V1 lässt User Predicates per ↑/↓-Buttons re-ordern. Full HTML5-DnD ist eigene Folge-Iteration.

```rust
// dnd.rs als Stub mit Doc-Kommentar
//! Drag-and-drop reorder — V1 implementiert nur Up/Down-Buttons.
//! Volle DnD (HTML5 native events) folgt in U7.1.
```

In `predicate_row.rs` neben dem `×` zwei kleine Buttons:

```rust
<button on:click=move |_| dispatch.call(BuilderAction::MoveNode { ... up ... })>"↑"</button>
<button on:click=move |_| dispatch.call(BuilderAction::MoveNode { ... down ... })>"↓"</button>
```

(MoveNode-Action ist im Reducer als Backlog markiert — kann später ergänzt werden.)

- [ ] **Schritt 2: Commit**

```
git add client/src/components/table/filter_builder/dnd.rs
git commit -m "docs(client): defer full DnD to U7.1, document scaffold"
```

---

### Task 14: Workspace-Cleanup + Manuelle Verifikation

- [ ] **Schritt 1: Clippy + fmt + Tests**

```
CARGO_TARGET_DIR=target-test cargo clippy --workspace --no-deps -- -A clippy::doc_overindented_list_items -A clippy::too_many_arguments
cargo fmt
CARGO_TARGET_DIR=target-test cargo test --workspace
```

- [ ] **Schritt 2: Manuelle Verifikation**

```
cargo run -p server -- --data-dir ./examples/shop &
cd client && trunk serve
```

In Order-Liste: "Filter ▾" → Builder öffnet. "+ Bedingung" → Predicate-Row erscheint. Spalten-/Operator-/Wert-Auswahl. "Anwenden" → Tabelle filtert. "Speichern als…" → prompt nach Namen → erscheint als Chip. Klick auf Chip → re-applied. ↑/↓ ordern.

- [ ] **Schritt 3: Self-Review**

- Spec §3.1 FilterExpression Wire → Task 1 ✓
- Spec §3.2 Saved-Filters → Tasks 6, 7 ✓
- Spec §3.3 Komponente → Tasks 9, 10, 12 ✓
- Spec §3.4 Operator-Vokabular → Task 9 (OperatorPicker) ✓
- Spec §3.5 Toolbar → Task 12 ✓
- Spec §3.6 SQL-Preview → Task 10 (read-only `<details>`) ✓
- Spec §3.7 supports_expression → Task 2 ✓ (Post-Fetch in Task 4)
- Spec §5 Edge-Cases: empty expression (Task 5), gelöschte Spalte (Builder zeigt Warn-Banner — TODO/Backlog), Save unter existierendem Namen (Browser-prompt heute — Folge-UX), Default-Toggle (Task 7) ✓
- Spec §7 Tests: Wire (1), Server e2e (5), Saved-Filters CRUD (7), Reducer-Unit (8) ✓
- Spec §9 Risks: Komponente groß → Sub-Komponenten ✓; post-fetch langsam → Doku in Architektur-Details; Source-API-Break → list_page_v2 mit Default-Impl ✓

- [ ] **Schritt 4: Final Commit**

```
git diff --quiet || git commit -am "style: cargo fmt"
```

---

## Was später kommt

- **U7.1**: Volle HTML5-DnD (drag handle, drop zones, drag-over highlights)
- **U7.2**: Native SQL-WHERE-Übersetzung in managed-sqlite (statt Post-Fetch)
- **U7.3**: Sub-Query-Predicates (`EXISTS` für Cross-Entity)
- **Group-Sharing** (Phase 0.7)
- **Spalten-Validierung** (Warn-Banner bei gelöschten Spalten in gespeichertem Filter)
