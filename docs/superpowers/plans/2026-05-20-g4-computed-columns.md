# G4 — Computed / Generated Columns Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Express expression-based generated columns (`GENERATED ALWAYS AS (factor * value) STORED`) in the schema language. Today `ColumnGenerated` covers system-filled defaults only; this plan adds a `compute: Option<ComputeExpression>` plus `compute_stored: bool` to `DbColumn` and emits them as native `GENERATED` clauses for SQLite (and forward to Postgres/MSSQL when those Sources land).

**Architecture:**
- New `ComputeExpression(String)` mini-DSL in `shared/src/builder/compute.rs` modeled on `guard.rs` and `check.rs` (this plan's G2 sibling): transparent string wire form parsed on demand.
- Grammar: arithmetic (`+ - * /`), parentheses, column refs, numeric/string literals, and a fixed function set (`ABS`, `COALESCE`).
- DDL: render `GENERATED ALWAYS AS (...) STORED` (or `VIRTUAL` when `compute_stored = false`) — SQLite ≥ 3.31 supports both.
- Source-layer fallback (engines without GENERATED) is **out of scope** for this plan; document the hook point only.

**Tech Stack:** Rust (`shared`, `server`), serde, SeaORM for DDL.

**Source spec:** `docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md` §G4.

---

## File Structure

- Create: `shared/src/builder/compute.rs` — `ComputeExpression(String)` + AST + parser + evaluator.
- Modify: `shared/src/builder.rs` — `pub mod compute;` + re-export.
- Modify: `shared/src/lib.rs` — re-export `ComputeExpression`; add `compute` + `compute_stored` to `DbColumn`.
- Create: `shared/tests/compute_expression.rs` — parser/eval/wire tests.
- Modify: `shared/tests/db_schema_roundtrip.rs` — extend roundtrip with a computed column.
- Modify: `server/src/ddl.rs` — emit `GENERATED ALWAYS AS (...) STORED|VIRTUAL`.

---

## Task 1: `ComputeExpression` AST + parser

**Files:**
- Create: `shared/src/builder/compute.rs`
- Modify: `shared/src/builder.rs`, `shared/src/lib.rs`
- Test: `shared/tests/compute_expression.rs`

- [ ] **Step 1: Write the failing test**

Create `shared/tests/compute_expression.rs`:

```rust
use shared::ComputeExpression;
use shared::builder::compute::{ComputeAst, BinOp, Operand, Literal};

#[test]
fn parses_simple_arithmetic() {
    let ast = ComputeExpression::new("factor * value").parse().unwrap();
    assert!(matches!(ast, ComputeAst::Binary { op: BinOp::Mul, .. }));
}

#[test]
fn parses_parenthesized_expression() {
    let ast = ComputeExpression::new("(a + b) * 2").parse().unwrap();
    assert!(matches!(ast, ComputeAst::Binary { op: BinOp::Mul, .. }));
}

#[test]
fn parses_abs_function() {
    let ast = ComputeExpression::new("ABS(value)").parse().unwrap();
    assert!(matches!(ast, ComputeAst::Func { ref name, .. } if name == "ABS"));
}

#[test]
fn parses_coalesce_multiple_args() {
    let ast = ComputeExpression::new("COALESCE(a, b, 0)").parse().unwrap();
    match ast {
        ComputeAst::Func { name, args } => {
            assert_eq!(name, "COALESCE");
            assert_eq!(args.len(), 3);
        }
        _ => panic!("expected Func"),
    }
}

#[test]
fn precedence_mul_before_add() {
    // a + b * c → a + (b * c)
    let ast = ComputeExpression::new("a + b * c").parse().unwrap();
    match ast {
        ComputeAst::Binary { op: BinOp::Add, left, right } => {
            assert!(matches!(*left, ComputeAst::Operand(Operand::ColumnRef(ref s)) if s == "a"));
            assert!(matches!(*right, ComputeAst::Binary { op: BinOp::Mul, .. }));
        }
        _ => panic!("expected Add at root, got {:?}", ast),
    }
}

#[test]
fn rejects_unknown_function() {
    let err = ComputeExpression::new("FOO(x)").parse().unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("FOO") || msg.contains("unknown"), "got: {msg}");
}

#[test]
fn serializes_transparently_as_string() {
    let expr = ComputeExpression::new("factor * value");
    let v = serde_json::to_value(&expr).unwrap();
    assert_eq!(v, serde_json::json!("factor * value"));
}

#[test]
fn evaluates_arithmetic_against_fields() {
    let ast = ComputeExpression::new("factor * value").parse().unwrap();
    let mut fields = serde_json::Map::new();
    fields.insert("factor".into(), serde_json::json!(2));
    fields.insert("value".into(), serde_json::json!(5));
    assert_eq!(ast.evaluate(&fields), Some(serde_json::json!(10.0)));
}

#[test]
fn evaluates_coalesce_returns_first_non_null() {
    let ast = ComputeExpression::new("COALESCE(a, b, 0)").parse().unwrap();
    let mut fields = serde_json::Map::new();
    fields.insert("a".into(), serde_json::json!(null));
    fields.insert("b".into(), serde_json::json!(7));
    assert_eq!(ast.evaluate(&fields), Some(serde_json::json!(7)));
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p shared --test compute_expression`
Expected: compile error — `ComputeExpression` doesn't exist.

- [ ] **Step 3: Create the module**

Create `shared/src/builder/compute.rs`:

```rust
//! `ComputeExpression` — typed mini-DSL for generated/computed columns.
//!
//! Transparent String wire format (mirrors `guard.rs` and `check.rs`).
//! Grammar:
//!   expr     = term ( ("+" | "-") term )*
//!   term     = factor ( ("*" | "/") factor )*
//!   factor   = "(" expr ")" | function | operand
//!   function = ("ABS" | "COALESCE") "(" expr ( "," expr )* ")"
//!   operand  = number | string | column

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ComputeExpression(pub String);

impl ComputeExpression {
    pub fn new(src: impl Into<String>) -> Self {
        ComputeExpression(src.into())
    }
    pub fn source(&self) -> &str {
        &self.0
    }
    pub fn parse(&self) -> Result<ComputeAst, ComputeError> {
        parse(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComputeAst {
    Binary { op: BinOp, left: Box<ComputeAst>, right: Box<ComputeAst> },
    Func { name: String, args: Vec<ComputeAst> },
    Operand(Operand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp { Add, Sub, Mul, Div }

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    ColumnRef(String),
    Literal(Literal),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Num(f64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComputeError {
    UnexpectedChar { pos: usize, ch: char },
    UnexpectedEof,
    UnexpectedToken { pos: usize, found: String },
    UnknownFunction { pos: usize, name: String },
    InvalidNumber { pos: usize, src: String },
}

impl core::fmt::Display for ComputeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedChar { pos, ch } => write!(f, "unexpected character '{ch}' at pos {pos}"),
            Self::UnexpectedEof => write!(f, "unexpected end of input"),
            Self::UnexpectedToken { pos, found } => write!(f, "unexpected token '{found}' at pos {pos}"),
            Self::UnknownFunction { pos, name } => write!(f, "unknown function '{name}' at pos {pos}"),
            Self::InvalidNumber { pos, src } => write!(f, "invalid number '{src}' at pos {pos}"),
        }
    }
}
impl std::error::Error for ComputeError {}

pub fn parse(input: &str) -> Result<ComputeAst, ComputeError> {
    // Use a recursive-descent parser. Closely follow the structure of
    // `shared/src/builder/guard.rs` (tokenize → parse_expr → parse_term → parse_factor).
    // The supported function set is exactly { "ABS", "COALESCE" } — every
    // other identifier followed by `(` becomes an `UnknownFunction` error.
    todo!("see Task 1 Step 4 for the body")
}

impl ComputeAst {
    pub fn evaluate(&self, fields: &serde_json::Map<String, serde_json::Value>) -> Option<serde_json::Value> {
        // Returns `None` only for ABS-of-string or other type-incoherent cases.
        // Numeric ops always return `Number`; COALESCE returns the first
        // non-null arg as-is. See Task 1 Step 5 for the body.
        todo!()
    }
}
```

Update `shared/src/builder.rs`:

```rust
pub mod compute;
pub use compute::ComputeExpression;
```

Update `shared/src/lib.rs` `pub use builder::{...}` line:

```rust
pub use builder::{CheckExpression, ComputeExpression, EventKind, EventTrigger, GuardExpr, TriggerTarget};
```

- [ ] **Step 4: Implement `parse`**

Replace the `todo!()` in `parse`. Borrow `tokenize` from `shared/src/builder/guard.rs` (drop the `Dot` token, drop the `&&`/`||`/`==` set, add `+ - * /` and `,`). The parser is straight Pratt/recursive-descent:

```rust
struct Parser { tokens: Vec<Token>, pos: usize }

impl Parser {
    fn parse_expr(&mut self) -> Result<ComputeAst, ComputeError> {
        let mut left = self.parse_term()?;
        while let Some(op) = self.peek_addsub() {
            self.bump();
            let right = self.parse_term()?;
            left = ComputeAst::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }
    fn parse_term(&mut self) -> Result<ComputeAst, ComputeError> {
        let mut left = self.parse_factor()?;
        while let Some(op) = self.peek_muldiv() {
            self.bump();
            let right = self.parse_factor()?;
            left = ComputeAst::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }
    fn parse_factor(&mut self) -> Result<ComputeAst, ComputeError> {
        if self.consume_if(TokKind::LParen) {
            let inner = self.parse_expr()?;
            self.expect(TokKind::RParen)?;
            return Ok(inner);
        }
        // function call?
        if let Some(TokKind::Ident(name)) = self.peek_kind() {
            // peek-ahead for `(`
            if self.peek_kind_at(1) == Some(TokKind::LParen) {
                let name_pos = self.peek().unwrap().start;
                self.bump(); // ident
                self.bump(); // (
                if !["ABS", "COALESCE"].contains(&name.to_ascii_uppercase().as_str()) {
                    return Err(ComputeError::UnknownFunction { pos: name_pos, name });
                }
                let mut args = vec![self.parse_expr()?];
                while self.consume_if(TokKind::Comma) {
                    args.push(self.parse_expr()?);
                }
                self.expect(TokKind::RParen)?;
                return Ok(ComputeAst::Func { name: name.to_ascii_uppercase(), args });
            }
        }
        Ok(ComputeAst::Operand(self.parse_operand()?))
    }
    // ...
}
```

(Helpers like `peek_addsub`, `peek_muldiv`, `consume_if`, `expect`, `peek_kind`, `peek_kind_at(1)` are short; write them inline.)

- [ ] **Step 5: Implement `evaluate`**

```rust
impl ComputeAst {
    pub fn evaluate(&self, fields: &serde_json::Map<String, serde_json::Value>) -> Option<serde_json::Value> {
        use ComputeAst::*;
        match self {
            Operand(op) => Some(match op {
                self::Operand::ColumnRef(name) => fields.get(name).cloned().unwrap_or(serde_json::Value::Null),
                self::Operand::Literal(Literal::Num(n)) => serde_json::json!(*n),
                self::Operand::Literal(Literal::Str(s)) => serde_json::Value::String(s.clone()),
            }),
            Binary { op, left, right } => {
                let l = left.evaluate(fields)?.as_f64()?;
                let r = right.evaluate(fields)?.as_f64()?;
                let out = match op {
                    BinOp::Add => l + r,
                    BinOp::Sub => l - r,
                    BinOp::Mul => l * r,
                    BinOp::Div if r == 0.0 => return None,
                    BinOp::Div => l / r,
                };
                Some(serde_json::json!(out))
            }
            Func { name, args } => match name.as_str() {
                "ABS" => {
                    let v = args.first()?.evaluate(fields)?.as_f64()?;
                    Some(serde_json::json!(v.abs()))
                }
                "COALESCE" => {
                    for a in args {
                        if let Some(v) = a.evaluate(fields) {
                            if !v.is_null() { return Some(v); }
                        }
                    }
                    Some(serde_json::Value::Null)
                }
                _ => None,
            },
        }
    }
}
```

- [ ] **Step 6: Run the tests and confirm they pass**

Run: `cargo test -p shared --test compute_expression`
Expected: PASS (all 9 tests).

- [ ] **Step 7: Commit**

```bash
git add shared/src/builder/compute.rs shared/src/builder.rs shared/src/lib.rs shared/tests/compute_expression.rs
git commit -m "feat(shared): ComputeExpression mini-DSL for generated columns"
```

---

## Task 2: Add `compute` + `compute_stored` to `DbColumn`

**Files:**
- Modify: `shared/src/lib.rs` (DbColumn around line 403)
- Test: `shared/tests/db_schema_roundtrip.rs`

- [ ] **Step 1: Write the failing roundtrip extension**

Append to `shared/tests/db_schema_roundtrip.rs`:

```rust
use shared::ComputeExpression;

#[test]
fn db_column_carries_compute_expression() {
    let mut schema = sample_schema();
    schema.tables[0].columns[0].compute = Some(ComputeExpression::new("factor * value"));
    schema.tables[0].columns[0].compute_stored = false; // VIRTUAL

    let json = serde_json::to_string(&schema).unwrap();
    let back: shared::DbSchema = serde_json::from_str(&json).unwrap();
    assert_eq!(schema, back);

    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["tables"][0]["columns"][0]["compute"], serde_json::json!("factor * value"));
    assert_eq!(value["tables"][0]["columns"][0]["computeStored"], serde_json::json!(false));
}

#[test]
fn db_column_compute_stored_defaults_to_true() {
    let legacy = r#"{
        "id":"s","name":"S","tables":[{"id":"t","name":"T","position":{"x":0.0,"y":0.0},
            "columns":[{"id":"c","name":"c","dataType":{"kind":"integer"},
                        "nullable":false,"primaryKey":false,"unique":false,
                        "compute":"a + b"}]}],
        "relations":[]
    }"#;
    let parsed: shared::DbSchema = serde_json::from_str(legacy).unwrap();
    assert!(parsed.tables[0].columns[0].compute_stored, "default = stored");
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p shared --test db_schema_roundtrip`
Expected: compile error.

- [ ] **Step 3: Extend `DbColumn`**

In `shared/src/lib.rs`, add to `DbColumn`:

```rust
/// Phase 0.7-G4: expression-based generated column (e.g. `factor * value`).
#[serde(default, skip_serializing_if = "Option::is_none")]
pub compute: Option<ComputeExpression>,
/// Storage policy when `compute` is set. `true` = STORED (default),
/// `false` = VIRTUAL (recomputed on every SELECT). Ignored when `compute` is None.
#[serde(default = "default_compute_stored")]
pub compute_stored: bool,
```

Add the default fn near the bottom of the file:

```rust
fn default_compute_stored() -> bool { true }
```

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p shared --test db_schema_roundtrip`
Expected: PASS.

- [ ] **Step 5: Run the full shared suite**

Run: `cargo test -p shared`
Expected: PASS. Fix any `DbColumn { ... }` constructor that doesn't use struct-update syntax.

- [ ] **Step 6: Commit**

```bash
git add shared/src/lib.rs shared/tests/db_schema_roundtrip.rs
git commit -m "feat(shared): DbColumn.compute + compute_stored"
```

---

## Task 3: DDL emits `GENERATED ALWAYS AS (...)` for SQLite

**Files:**
- Modify: `server/src/ddl.rs`

- [ ] **Step 1: Write the failing DDL tests**

Extend the `tests` module in `server/src/ddl.rs`:

```rust
#[test]
fn renders_stored_generated_column() {
    let mut t = DbTable {
        id: "t".into(),
        name: "journal_line".into(),
        position: Position::default(),
        columns: vec![
            col("factor", DbColumnType::Decimal { precision: 18, scale: 8 }, false, false),
            col("value", DbColumnType::Decimal { precision: 18, scale: 4 }, false, false),
            col("debit_value", DbColumnType::Decimal { precision: 18, scale: 4 }, false, true),
        ],
        checks: Vec::new(), // assumes G2 landed; otherwise drop this line and skip the field
    };
    t.columns[2].compute = Some(shared::ComputeExpression::new("factor * value"));
    t.columns[2].compute_stored = true;

    let sql = render_create_table(&t);
    assert!(
        sql.contains("GENERATED ALWAYS AS (factor * value) STORED"),
        "missing GENERATED clause:\n{sql}"
    );
}

#[test]
fn renders_virtual_generated_column() {
    let mut t = DbTable {
        id: "t".into(),
        name: "x".into(),
        position: Position::default(),
        columns: vec![col("v", DbColumnType::Integer, false, true)],
        checks: Vec::new(),
    };
    t.columns[0].compute = Some(shared::ComputeExpression::new("1 + 1"));
    t.columns[0].compute_stored = false;

    let sql = render_create_table(&t);
    assert!(
        sql.contains("GENERATED ALWAYS AS (1 + 1) VIRTUAL"),
        "missing VIRTUAL clause:\n{sql}"
    );
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p server --lib ddl::tests --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implement rendering**

In `server/src/ddl.rs::render_create_table`, after the existing column DDL emission, branch on `col.compute`:

```rust
if let Some(expr) = &col.compute {
    let kind = if col.compute_stored { "STORED" } else { "VIRTUAL" };
    line.push_str(&format!(" GENERATED ALWAYS AS ({}) {}", expr.source(), kind));
}
```

Place it after the `DEFAULT` clause and before the (G2) `CHECK` clause.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p server --lib ddl::tests --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Run the server suite**

Run: `cargo test -p server --target-dir target-test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add server/src/ddl.rs
git commit -m "feat(server): DDL emits GENERATED ALWAYS AS for computed columns"
```

---

## Notes for the implementer

- SQLite has supported `GENERATED ALWAYS AS` since 3.31 (Jan 2020). The workspace's `rusqlite` / SeaORM driver bundles a recent SQLite, so STORED/VIRTUAL just work. Postgres + MSSQL syntax is identical, MySQL ≥ 8 close enough — when those Sources land they can share `render_create_table`.
- Fallback path for engines without GENERATED: hook into the `pre_insert`/`pre_update` step of the Source trait and run `ComputeAst::evaluate` to fill the column. Out of scope for this plan; document the hook point in the source-architecture spec when the time comes.
- The plan deliberately **omits** an evaluator-based runtime pre-fill — `ComputeAst::evaluate` exists already (Task 1), but wiring it into mutations is the fallback story we deferred.
