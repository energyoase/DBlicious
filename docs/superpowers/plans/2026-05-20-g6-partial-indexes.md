# G6 — Partial Indexes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Filtered/partial-index support. Extend `DbIndex` with `where_clause: Option<CheckExpression>` so the schema can express things like `UNIQUE(number) WHERE deleted_at IS NULL`. The DDL pass emits the predicate as native `WHERE ...` on SQLite (and Postgres/MSSQL when those Sources land); MySQL falls back to the existing full-index behavior.

**Architecture:**
- Reuse the `CheckExpression` AST introduced in G2. (If G2 hasn't shipped, ship the `CheckExpression` skeleton first; the rest of this plan is unchanged.)
- One new optional field on `DbIndex`. No new types.
- DDL: render `CREATE [UNIQUE] INDEX ... ON ... (cols) WHERE (predicate)` when `where_clause` is set; otherwise emit the existing form.

**Tech Stack:** Rust (`shared`, `server`), serde, SeaORM for DDL.

**Source spec:** `docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md` §G6.

**Hard dependency:** [G2 plan](2026-05-20-g2-check-constraints.md) — `CheckExpression` must exist. If G2 isn't merged yet, do Tasks 1–3 of the G2 plan first (parser is enough; you don't need the `DbColumn.check` wiring).

---

## File Structure

- Modify: `shared/src/lib.rs` — extend `DbIndex` (around line 509).
- Modify: `shared/tests/db_schema_roundtrip.rs` — extend roundtrip with a partial index.
- Modify: `server/src/ddl.rs` — render `WHERE (...)` on `CREATE INDEX`.
- Create: indexing DDL output is currently inline in `try_apply_schema`. If no `CREATE INDEX` emitter exists yet, this plan adds `render_create_index`.

---

## Task 1: `DbIndex.where_clause` field

**Files:**
- Modify: `shared/src/lib.rs:509-518`
- Test: `shared/tests/db_schema_roundtrip.rs`

- [ ] **Step 1: Write the failing roundtrip test**

Append to `shared/tests/db_schema_roundtrip.rs`:

```rust
use shared::CheckExpression;

#[test]
fn db_index_carries_where_clause() {
    let mut schema = sample_schema();
    schema.indices.push(shared::DbIndex {
        id: "ux_partial".into(),
        name: "UX_account_number_live".into(),
        table_id: "t-1".into(),
        unique: true,
        column_ids: vec!["c-1".into()],
        where_clause: Some(CheckExpression::new("deleted_at IS NULL")),
    });

    let json = serde_json::to_string(&schema).unwrap();
    let back: shared::DbSchema = serde_json::from_str(&json).unwrap();
    assert_eq!(schema, back);

    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let idx = &value["indices"][1];
    assert_eq!(idx["whereClause"], serde_json::json!("deleted_at IS NULL"));
}

#[test]
fn db_index_without_where_clause_still_parses() {
    let legacy = r#"{
        "id":"i","name":"IX","tableId":"t","unique":false,"columnIds":["c"]
    }"#;
    let parsed: shared::DbIndex = serde_json::from_str(legacy).unwrap();
    assert!(parsed.where_clause.is_none());
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p shared --test db_schema_roundtrip`
Expected: compile error — `where_clause` doesn't exist on `DbIndex`.

- [ ] **Step 3: Extend `DbIndex`**

Edit `shared/src/lib.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DbIndex {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub table_id: String,
    pub unique: bool,
    pub column_ids: Vec<String>,
    /// Phase 0.7-G6: optional partial-index predicate (`WHERE …`).
    /// Re-uses the [`CheckExpression`] AST so columns can be referenced by bare name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub where_clause: Option<CheckExpression>,
}
```

Note: `DbIndex` currently derives `Eq`. `CheckExpression` is `String`-backed and is `Eq`-able, so this should still compile. If a derive complains, drop `Eq` from `DbIndex` (the existing test only relies on `PartialEq`).

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cargo test -p shared --test db_schema_roundtrip`
Expected: PASS.

- [ ] **Step 5: Run the full shared suite**

Run: `cargo test -p shared`
Expected: PASS. Any explicit `DbIndex { ... }` constructor must add `where_clause: None`.

- [ ] **Step 6: Commit**

```bash
git add shared/src/lib.rs shared/tests/db_schema_roundtrip.rs
git commit -m "feat(shared): DbIndex.where_clause for partial/filtered indexes"
```

---

## Task 2: DDL emits `CREATE INDEX ... WHERE (...)`

**Files:**
- Modify: `server/src/ddl.rs`

- [ ] **Step 1: Find or write `render_create_index`**

If `render_create_index` already exists, jump to Step 2. Otherwise add it. Grep:

```bash
# Use Grep tool: pattern "CREATE INDEX" in path server/src/ddl.rs
```

If no emitter exists, add one:

```rust
use shared::{DbIndex, DbTable};

pub fn render_create_index(table: &DbTable, idx: &DbIndex) -> String {
    let unique = if idx.unique { "UNIQUE " } else { "" };
    // column_ids stores DbColumn.id, not DbColumn.name. Resolve.
    let cols: Vec<String> = idx
        .column_ids
        .iter()
        .filter_map(|cid| table.columns.iter().find(|c| &c.id == cid))
        .map(|c| format!("\"{}\"", c.name))
        .collect();
    let mut sql = format!(
        "CREATE {unique}INDEX IF NOT EXISTS \"{}\" ON \"{}\" ({})",
        idx.name,
        table.name,
        cols.join(", ")
    );
    if let Some(w) = &idx.where_clause {
        sql.push_str(&format!(" WHERE ({})", w.source()));
    }
    sql.push(';');
    sql
}
```

And in `try_apply_schema`, after the per-table `CREATE TABLE` loop:

```rust
for idx in &schema.indices {
    let Some(table) = schema.tables.iter().find(|t| t.id == idx.table_id) else {
        tracing::warn!(target: "server::ddl", "index {} references unknown table {}", idx.name, idx.table_id);
        continue;
    };
    let sql = render_create_index(table, idx);
    tracing::debug!(target: "server::ddl", "{}", sql);
    if let Err(e) = crate::db::execute_raw(&sql).await {
        tracing::warn!(target: "server::ddl", "index DDL failed: {e}\n{sql}");
    } else {
        applied += 1;
    }
}
```

- [ ] **Step 2: Write the failing test**

Extend the `tests` module in `server/src/ddl.rs`:

```rust
#[test]
fn renders_partial_index() {
    let t = DbTable {
        id: "t1".into(),
        name: "account".into(),
        position: Position::default(),
        columns: vec![
            col("number", DbColumnType::Integer, false, false),
            col("deleted_at", DbColumnType::DateTime, false, true),
        ],
        checks: Vec::new(), // assumes G2 landed; drop if it hasn't
    };
    let idx = DbIndex {
        id: "ux_partial".into(),
        name: "ux_account_number_live".into(),
        table_id: "t1".into(),
        unique: true,
        column_ids: vec!["number".into()],
        where_clause: Some(shared::CheckExpression::new("deleted_at IS NULL")),
    };
    let sql = render_create_index(&t, &idx);
    assert!(
        sql.contains("CREATE UNIQUE INDEX IF NOT EXISTS \"ux_account_number_live\""),
        "missing CREATE UNIQUE INDEX: {sql}"
    );
    assert!(
        sql.contains("WHERE (deleted_at IS NULL)"),
        "missing WHERE clause: {sql}"
    );
}

#[test]
fn renders_full_index_when_no_where_clause() {
    let t = DbTable {
        id: "t1".into(),
        name: "account".into(),
        position: Position::default(),
        columns: vec![col("number", DbColumnType::Integer, false, false)],
        checks: Vec::new(),
    };
    let idx = DbIndex {
        id: "ix".into(),
        name: "ix_account_number".into(),
        table_id: "t1".into(),
        unique: false,
        column_ids: vec!["number".into()],
        where_clause: None,
    };
    let sql = render_create_index(&t, &idx);
    assert!(sql.contains("CREATE INDEX IF NOT EXISTS"));
    assert!(!sql.contains("WHERE"));
}
```

- [ ] **Step 3: Run the tests and confirm they pass**

Run: `cargo test -p server --lib ddl::tests --target-dir target-test`
Expected: PASS.

- [ ] **Step 4: Run the server suite**

Run: `cargo test -p server --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/ddl.rs
git commit -m "feat(server): DDL emits partial indexes (CREATE INDEX ... WHERE ...)"
```

---

## Notes for the implementer

- **MySQL fallback:** MySQL doesn't support partial indexes. When a MySQL Source lands, `Capabilities::supports_partial_index = false`, and the renderer should emit the full index plus a tracing warning. Not in scope for this plan (managed SQLite supports partial indexes natively).
- **MSSQL "filtered indexes":** syntactically identical (`WHERE` clause); the same emitter works.
- **Predicate safety:** `CheckExpression.source()` is rendered verbatim — same risk and same mitigation as in the G2 plan (the parser must accept only the documented grammar; anything else fails `parse()` and produces a tracing warning + skipped DDL).
