# G5 — Materialized Views / Schema-Level Caches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** First-class view definitions in `DbSchema`: a new top-level `DbView { id, name, definition: ViewDefinition, materialized: bool, refresh: RefreshStrategy }` collection plus a typed query DSL (`ViewDefinition`) covering `SELECT` list, `FROM`/`JOIN`, `WHERE`, and `GROUP BY`. Render as `CREATE VIEW` / `CREATE MATERIALIZED VIEW` on engines that support it; defer the refresh-scheduler fallback to its own future spec.

**Architecture:**
- `DbView` lives in a new module `shared/src/view.rs`. Wire form is a regular tagged struct (no transparent string DSL — the structure is rich enough that JSON-of-AST is friendlier than a string parser).
- `ViewDefinition` is a typed AST: `SelectItem`, `FromClause`, `JoinClause`, `Predicate` (reuses `CheckExpression` from G2 if available; otherwise inlines the same shape), `GroupBy`.
- `RefreshStrategy`: `OnDemand`, `Scheduled { cron: String }`, `OnDependencyChange`. The strategy is metadata only in this plan; the actual job scheduler integration lives in a Phase 1.7 follow-up spec.
- DDL: render `CREATE [MATERIALIZED] VIEW <name> AS SELECT ...` for Postgres; SQLite supports `CREATE VIEW` only — `materialized: true` on SQLite falls back to `CREATE VIEW` with a warning unless an explicit `cache_table_name` is provided (out of scope; produces a warning).

**Tech Stack:** Rust (`shared`, `server`), serde, SeaORM for DDL execution.

**Source spec:** `docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md` §G5.

---

## File Structure

- Create: `shared/src/view.rs` — `DbView`, `ViewDefinition`, `RefreshStrategy`, `SelectItem`, `FromClause`, `JoinClause`, `JoinKind`, `GroupBy`. Reuse `CheckExpression` from G2 for `WHERE`.
- Modify: `shared/src/lib.rs` — `pub mod view;` + re-exports; `DbSchema.views: Vec<DbView>`.
- Create: `shared/tests/db_view_roundtrip.rs` — wire-format pin + roundtrip.
- Create: `server/src/ddl/view.rs` — `render_create_view` (split into its own file to keep `ddl.rs` digestible).
- Modify: `server/src/ddl.rs` — re-export `render_create_view`; call it from `try_apply_schema` after tables/indexes.

This plan **assumes G2 has landed** (`CheckExpression` for predicates). If G2 hasn't shipped, prefix Task 1 with a copy of `CheckExpression` from the G2 plan; the rest of the work is identical.

---

## Task 1: View types (`DbView`, `ViewDefinition`, …)

**Files:**
- Create: `shared/src/view.rs`
- Modify: `shared/src/lib.rs`
- Test: `shared/tests/db_view_roundtrip.rs`

- [ ] **Step 1: Write the failing test**

Create `shared/tests/db_view_roundtrip.rs`:

```rust
use shared::view::{
    DbView, FromClause, GroupBy, JoinClause, JoinKind, RefreshStrategy, SelectItem,
    ViewDefinition,
};
use shared::{CheckExpression, DbSchema};

fn sample_view() -> DbView {
    DbView {
        id: "v_balance".into(),
        name: "account_balance_summary".into(),
        materialized: true,
        refresh: RefreshStrategy::Scheduled { cron: "0 */6 * * *".into() },
        definition: ViewDefinition {
            select: vec![
                SelectItem {
                    expression: "account_id".into(),
                    alias: None,
                },
                SelectItem {
                    expression: "SUM(debit_value - credit_value)".into(),
                    alias: Some("balance".into()),
                },
            ],
            from: FromClause {
                table: "journal_line".into(),
                alias: None,
            },
            joins: vec![JoinClause {
                kind: JoinKind::Inner,
                table: "journal_entry".into(),
                alias: Some("e".into()),
                on: CheckExpression::new("journal_line.journal_entry_id = e.id"),
            }],
            where_clause: Some(CheckExpression::new("e.deleted_at IS NULL")),
            group_by: Some(GroupBy {
                expressions: vec!["account_id".into()],
            }),
        },
    }
}

#[test]
fn db_view_roundtrips_through_json() {
    let original = sample_view();
    let json = serde_json::to_string(&original).unwrap();
    let back: DbView = serde_json::from_str(&json).unwrap();
    assert_eq!(original, back);
}

#[test]
fn db_view_camel_case_field_names() {
    let v = serde_json::to_value(sample_view()).unwrap();
    assert!(v.get("materialized").is_some());
    assert!(v.get("refresh").is_some());
    assert!(v["definition"].get("groupBy").is_some());
    assert!(v["definition"].get("whereClause").is_some());
    assert!(v["definition"]["joins"][0].get("on").is_some());
}

#[test]
fn refresh_strategy_tagged_camel_case() {
    let on_demand = serde_json::to_value(RefreshStrategy::OnDemand).unwrap();
    assert_eq!(on_demand, serde_json::json!({"kind": "onDemand"}));

    let scheduled = serde_json::to_value(RefreshStrategy::Scheduled {
        cron: "* * * * *".into(),
    })
    .unwrap();
    assert_eq!(
        scheduled,
        serde_json::json!({"kind": "scheduled", "cron": "* * * * *"})
    );
}

#[test]
fn db_schema_carries_views() {
    let schema = DbSchema {
        id: "s".into(),
        name: "S".into(),
        tables: vec![],
        relations: vec![],
        keys: vec![],
        indices: vec![],
        views: vec![sample_view()],
    };
    let json = serde_json::to_string(&schema).unwrap();
    let back: DbSchema = serde_json::from_str(&json).unwrap();
    assert_eq!(schema, back);
}

#[test]
fn legacy_schema_without_views_still_parses() {
    let legacy = r#"{"id":"s","name":"S","tables":[],"relations":[]}"#;
    let parsed: DbSchema = serde_json::from_str(legacy).unwrap();
    assert!(parsed.views.is_empty());
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p shared --test db_view_roundtrip`
Expected: compile error — `view` module doesn't exist; `DbSchema.views` doesn't exist.

- [ ] **Step 3: Create the view module**

Create `shared/src/view.rs`:

```rust
//! `DbView` and supporting query-DSL types (Phase 0.7-G5).

use serde::{Deserialize, Serialize};

use crate::CheckExpression;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbView {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub materialized: bool,
    #[serde(default)]
    pub refresh: RefreshStrategy,
    pub definition: ViewDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RefreshStrategy {
    #[default]
    OnDemand,
    Scheduled { cron: String },
    OnDependencyChange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ViewDefinition {
    pub select: Vec<SelectItem>,
    pub from: FromClause,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joins: Vec<JoinClause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub where_clause: Option<CheckExpression>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<GroupBy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SelectItem {
    /// SQL expression (column reference or aggregate). Validated at DDL time.
    pub expression: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FromClause {
    pub table: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JoinClause {
    pub kind: JoinKind,
    pub table: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub on: CheckExpression,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GroupBy {
    pub expressions: Vec<String>,
}
```

Update `shared/src/lib.rs`:

```rust
pub mod view;
pub use view::{DbView, ViewDefinition, RefreshStrategy};

// In DbSchema:
#[serde(default)]
pub views: Vec<DbView>,
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cargo test -p shared --test db_view_roundtrip`
Expected: PASS (5 tests).

- [ ] **Step 5: Run the full shared suite**

Run: `cargo test -p shared`
Expected: PASS. Add `views: Vec::new()` to any `DbSchema { ... }` constructor that doesn't use struct-update syntax.

- [ ] **Step 6: Commit**

```bash
git add shared/src/view.rs shared/src/lib.rs shared/tests/db_view_roundtrip.rs
git commit -m "feat(shared): DbView + ViewDefinition typed query DSL"
```

---

## Task 2: DDL renderer for views

**Files:**
- Create: `server/src/ddl/view.rs`
- Modify: `server/src/ddl.rs` — convert into a module if it isn't already, or just import the new file.

- [ ] **Step 1: Promote `ddl` to a directory module if needed**

If `server/src/ddl.rs` is a single file, convert it: rename to `server/src/ddl/mod.rs` and add `pub mod view;` at its top. (`Glob server/src/ddl/*.rs` to check first; if the dir already exists, just add the new file.)

- [ ] **Step 2: Write the failing test**

Create `server/src/ddl/view.rs`:

```rust
//! View DDL emission for `DbView`. SQLite supports plain `CREATE VIEW`;
//! materialized views fall through to a `CREATE VIEW` with a tracing warning
//! (Postgres path lands when a Postgres Source is wired into the Source trait).

use shared::view::{DbView, JoinKind};

pub fn render_create_view(view: &DbView) -> String {
    let mut sql = String::new();
    let kind = if view.materialized {
        // SQLite has no MATERIALIZED VIEW. Tracing emits the warning in
        // try_apply_schema; here we render the closest-supported form.
        "VIEW"
    } else {
        "VIEW"
    };
    sql.push_str(&format!("CREATE {kind} IF NOT EXISTS \"{}\" AS SELECT ", view.name));

    let select_parts: Vec<String> = view
        .definition
        .select
        .iter()
        .map(|i| match &i.alias {
            Some(a) => format!("{} AS {}", i.expression, a),
            None => i.expression.clone(),
        })
        .collect();
    sql.push_str(&select_parts.join(", "));

    sql.push_str(&format!(" FROM \"{}\"", view.definition.from.table));
    if let Some(a) = &view.definition.from.alias {
        sql.push_str(&format!(" AS {a}"));
    }

    for j in &view.definition.joins {
        let kw = match j.kind {
            JoinKind::Inner => "INNER JOIN",
            JoinKind::Left => "LEFT JOIN",
            JoinKind::Right => "RIGHT JOIN",
            JoinKind::Full => "FULL JOIN",
        };
        sql.push_str(&format!(" {kw} \"{}\"", j.table));
        if let Some(a) = &j.alias {
            sql.push_str(&format!(" AS {a}"));
        }
        sql.push_str(&format!(" ON ({})", j.on.source()));
    }

    if let Some(w) = &view.definition.where_clause {
        sql.push_str(&format!(" WHERE ({})", w.source()));
    }
    if let Some(g) = &view.definition.group_by {
        sql.push_str(&format!(" GROUP BY {}", g.expressions.join(", ")));
    }
    sql.push(';');
    sql
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::view::*;
    use shared::CheckExpression;

    fn sample() -> DbView {
        DbView {
            id: "v".into(),
            name: "balance".into(),
            materialized: false,
            refresh: RefreshStrategy::OnDemand,
            definition: ViewDefinition {
                select: vec![
                    SelectItem { expression: "account_id".into(), alias: None },
                    SelectItem {
                        expression: "SUM(debit_value - credit_value)".into(),
                        alias: Some("balance".into()),
                    },
                ],
                from: FromClause { table: "journal_line".into(), alias: None },
                joins: vec![JoinClause {
                    kind: JoinKind::Inner,
                    table: "journal_entry".into(),
                    alias: Some("e".into()),
                    on: CheckExpression::new("journal_line.journal_entry_id = e.id"),
                }],
                where_clause: Some(CheckExpression::new("e.deleted_at IS NULL")),
                group_by: Some(GroupBy { expressions: vec!["account_id".into()] }),
            },
        }
    }

    #[test]
    fn renders_select_from_join_where_group() {
        let sql = render_create_view(&sample());
        assert!(sql.contains("CREATE VIEW IF NOT EXISTS \"balance\""));
        assert!(sql.contains("SUM(debit_value - credit_value) AS balance"));
        assert!(sql.contains("FROM \"journal_line\""));
        assert!(sql.contains("INNER JOIN \"journal_entry\" AS e"));
        assert!(sql.contains("ON (journal_line.journal_entry_id = e.id)"));
        assert!(sql.contains("WHERE (e.deleted_at IS NULL)"));
        assert!(sql.contains("GROUP BY account_id"));
    }
}
```

- [ ] **Step 3: Run the test and confirm it passes**

Run: `cargo test -p server --lib ddl::view::tests --target-dir target-test`
Expected: PASS.

- [ ] **Step 4: Wire into `try_apply_schema`**

In `server/src/ddl.rs` (or `server/src/ddl/mod.rs`):

```rust
pub mod view;

pub async fn try_apply_schema(schema: &DbSchema) -> usize {
    let mut applied = 0usize;
    for table in &schema.tables {
        // existing logic …
    }
    for v in &schema.views {
        if v.materialized {
            tracing::warn!(
                target: "server::ddl",
                "view {}: materialized=true requested but SQLite has no MATERIALIZED VIEW; \
                 emitting plain CREATE VIEW. A scheduler-backed cache table is a follow-up spec.",
                v.name
            );
        }
        let sql = view::render_create_view(v);
        tracing::debug!(target: "server::ddl", "{}", sql);
        match crate::db::execute_raw(&sql).await {
            Ok(_) => applied += 1,
            Err(e) => tracing::warn!(target: "server::ddl", "view DDL failed: {e}\n{sql}"),
        }
    }
    applied
}
```

- [ ] **Step 5: Run the server suite**

Run: `cargo test -p server --target-dir target-test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add server/src/ddl server/src/ddl.rs
git commit -m "feat(server): DDL emits CREATE VIEW for DbView definitions"
```

---

## Notes for the implementer

- **`RefreshStrategy` is metadata only in this plan.** No scheduler runs cron jobs; no dependency-tracking refreshes the view. Document the integration hook (`server/src/jobs/` or whatever lands during Phase 1.7) and surface it as a separate spec.
- **`SelectItem.expression` and `GroupBy.expressions` are raw SQL strings.** They flow into DDL untouched. If a Source-layer plan introduces engine-specific dialect translation, those become AST citizens — for now, the caller is responsible for engine-compatible SQL inside those fields.
- **Postgres MATERIALIZED VIEW path:** when the Postgres Source lands, swap the keyword inside `render_create_view` based on `Capabilities::supports_materialized_view`. No new wire-format work needed; just dispatch.
- **Out of scope for this plan:**
  - Job scheduler for `RefreshStrategy::Scheduled` / `OnDependencyChange`.
  - SQLite cache-table fallback (an extra table + trigger array) — its own future spec.
  - Server-side query result caching independent of DB views.
