# G2 — CHECK Constraints / Column Invariants Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Native CHECK-constraint vocabulary in `DbSchema`. Column-level (`DbColumn.check`) and table-level (`DbTable.checks`) invariants expressed as a typed mini-DSL, emitted as DB-native `CHECK` clauses by the DDL generator and (for engines without CHECK) enforced by the Source layer before INSERT/UPDATE.

**Architecture:**
- Introduce `CheckExpression` in `shared/src/builder/check.rs`, modeled on the existing `GuardExpr` (`shared/src/builder/guard.rs`): transparent String wire format + parse-on-demand to a typed `CheckAst`.
- `CheckAst` covers `Compare`, `Between`, `In`, `IsNull`/`IsNotNull`, combined by `And`/`Or`/`Not`. Operand references are bare column names (no `fields.` prefix — these are DB-side constraints, not field guards).
- DDL pass renders `CHECK (...)` clauses for SQLite today; the same AST will feed Postgres/MSSQL later.
- Source-layer fallback (`pre_insert_check`/`pre_update_check`) evaluates the AST in Rust for engines where native CHECK is unavailable.

**Tech Stack:** Rust (`shared`, `server`), serde, SeaORM for DDL execution.

**Source spec:** `docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md` §G2.

---

## File Structure

- Create: `shared/src/builder/check.rs` — `CheckExpression(String)` + AST + parser + evaluator. Mirror layout of `shared/src/builder/guard.rs`.
- Modify: `shared/src/builder.rs` — `pub mod check;` + re-export.
- Modify: `shared/src/lib.rs` — `pub use builder::CheckExpression;` and extend `DbColumn` / `DbTable`.
- Create: `shared/tests/check_expression.rs` — parse/eval/wire-format tests.
- Modify: `shared/tests/db_schema_roundtrip.rs` — extend roundtrip with a column-check and a table-check.
- Modify: `server/src/ddl.rs` — emit CHECK clauses in `render_create_table` plus inline `CHECK (...)` per column.
- Modify: `server/src/data.rs` — pre-insert/pre-update evaluation when the engine lacks native CHECK (Source-aware fallback, gated by `Capabilities.supports_check`).

---

## Task 1: `CheckExpression` AST + parser scaffolding

**Files:**
- Create: `shared/src/builder/check.rs`
- Modify: `shared/src/builder.rs`
- Test: `shared/tests/check_expression.rs`

- [ ] **Step 1: Write the failing test**

Create `shared/tests/check_expression.rs`:

```rust
use shared::CheckExpression;
use shared::builder::check::{CheckAst, CmpOp, Operand, Literal};

#[test]
fn parses_simple_compare() {
    let ast = CheckExpression::new("value > 0").parse().unwrap();
    assert!(matches!(
        ast,
        CheckAst::Compare {
            op: CmpOp::Gt,
            ..
        }
    ));
}

#[test]
fn parses_between_form() {
    let ast = CheckExpression::new("month BETWEEN 1 AND 12").parse().unwrap();
    assert!(matches!(ast, CheckAst::Between { .. }));
}

#[test]
fn parses_in_form() {
    let ast = CheckExpression::new("status IN (\"new\", \"paid\")")
        .parse()
        .unwrap();
    assert!(matches!(ast, CheckAst::In { .. }));
}

#[test]
fn parses_is_null_and_not_null() {
    let null = CheckExpression::new("deleted_at IS NULL").parse().unwrap();
    let not_null = CheckExpression::new("name IS NOT NULL").parse().unwrap();
    assert!(matches!(null, CheckAst::IsNull(_)));
    assert!(matches!(not_null, CheckAst::IsNotNull(_)));
}

#[test]
fn parses_logical_combination() {
    let ast = CheckExpression::new("value > 0 AND month BETWEEN 1 AND 12")
        .parse()
        .unwrap();
    assert!(matches!(ast, CheckAst::And(_, _)));
}

#[test]
fn rejects_fields_prefix() {
    // CHECK constraints reference bare columns, not `fields.X`.
    let err = CheckExpression::new("fields.value > 0").parse().unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("unexpected"), "got: {msg}");
}

#[test]
fn check_expression_serializes_transparently_as_string() {
    let expr = CheckExpression::new("value > 0");
    let v = serde_json::to_value(&expr).unwrap();
    assert_eq!(v, serde_json::json!("value > 0"));
    let back: CheckExpression = serde_json::from_value(v).unwrap();
    assert_eq!(back, expr);
}

#[test]
fn evaluates_compare_against_fields() {
    let ast = CheckExpression::new("value > 0").parse().unwrap();
    let mut fields = serde_json::Map::new();
    fields.insert("value".into(), serde_json::json!(10));
    assert!(ast.evaluate(&fields));
    fields.insert("value".into(), serde_json::json!(0));
    assert!(!ast.evaluate(&fields));
}

#[test]
fn evaluates_between_inclusive() {
    let ast = CheckExpression::new("month BETWEEN 1 AND 12").parse().unwrap();
    for v in [1, 6, 12] {
        let mut f = serde_json::Map::new();
        f.insert("month".into(), serde_json::json!(v));
        assert!(ast.evaluate(&f), "boundary {v}");
    }
    for v in [0, 13] {
        let mut f = serde_json::Map::new();
        f.insert("month".into(), serde_json::json!(v));
        assert!(!ast.evaluate(&f));
    }
}

#[test]
fn evaluates_in_set() {
    let ast = CheckExpression::new("status IN (\"new\", \"paid\")")
        .parse()
        .unwrap();
    let mut f = serde_json::Map::new();
    f.insert("status".into(), serde_json::json!("paid"));
    assert!(ast.evaluate(&f));
    f.insert("status".into(), serde_json::json!("draft"));
    assert!(!ast.evaluate(&f));
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p shared --test check_expression`
Expected: compile error — `CheckExpression` and supporting types don't exist.

- [ ] **Step 3: Create the module skeleton**

Create `shared/src/builder/check.rs`:

```rust
//! `CheckExpression` — typed mini-DSL for column / table CHECK constraints.
//!
//! Modeled on `super::guard`: the wire form is the raw source string,
//! parsed on demand into a typed AST that can be emitted as native SQL or
//! evaluated in Rust as a Source-layer fallback.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct CheckExpression(pub String);

impl CheckExpression {
    pub fn new(src: impl Into<String>) -> Self {
        CheckExpression(src.into())
    }
    pub fn source(&self) -> &str {
        &self.0
    }
    pub fn parse(&self) -> Result<CheckAst, CheckError> {
        parse(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckAst {
    And(Box<CheckAst>, Box<CheckAst>),
    Or(Box<CheckAst>, Box<CheckAst>),
    Not(Box<CheckAst>),
    Compare { left: Operand, op: CmpOp, right: Operand },
    Between { value: Operand, low: Operand, high: Operand },
    In { value: Operand, set: Vec<Literal> },
    IsNull(Operand),
    IsNotNull(Operand),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    ColumnRef(String),
    Literal(Literal),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp { Eq, Ne, Lt, Le, Gt, Ge }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    UnexpectedChar { pos: usize, ch: char },
    UnexpectedEof,
    UnexpectedToken { pos: usize, found: String },
    UnterminatedString { pos: usize },
    InvalidNumber { pos: usize, src: String },
}

impl core::fmt::Display for CheckError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CheckError::UnexpectedChar { pos, ch } => write!(f, "unexpected character '{ch}' at pos {pos}"),
            CheckError::UnexpectedEof => write!(f, "unexpected end of input"),
            CheckError::UnexpectedToken { pos, found } => write!(f, "unexpected token '{found}' at pos {pos}"),
            CheckError::UnterminatedString { pos } => write!(f, "unterminated string at pos {pos}"),
            CheckError::InvalidNumber { pos, src } => write!(f, "invalid number '{src}' at pos {pos}"),
        }
    }
}
impl std::error::Error for CheckError {}

pub fn parse(input: &str) -> Result<CheckAst, CheckError> {
    // TODO: see Task 2.
    let _ = input;
    Err(CheckError::UnexpectedEof)
}

impl CheckAst {
    pub fn evaluate(&self, _fields: &serde_json::Map<String, serde_json::Value>) -> bool {
        // TODO: see Task 3.
        false
    }
}
```

Then wire it up. Edit `shared/src/builder.rs` to add `pub mod check;` and re-export `CheckExpression`:

```rust
pub mod check;
pub use check::CheckExpression;
```

Edit `shared/src/lib.rs` near the existing `pub use builder::{...}` line to also re-export `CheckExpression`:

```rust
pub use builder::{CheckExpression, EventKind, EventTrigger, GuardExpr, TriggerTarget};
```

- [ ] **Step 4: Verify the skeleton compiles**

Run: `cargo build -p shared`
Expected: PASS (compiles; the `parse` and `evaluate` bodies are stubs).

- [ ] **Step 5: Commit**

```bash
git add shared/src/builder/check.rs shared/src/builder.rs shared/src/lib.rs shared/tests/check_expression.rs
git commit -m "feat(shared): CheckExpression skeleton (types + transparent wire form)"
```

---

## Task 2: Tokenizer + parser

**Files:**
- Modify: `shared/src/builder/check.rs`

- [ ] **Step 1: Implement tokenizer + parser**

Replace the `parse` stub and add helpers below the AST. The grammar:

```
or         = and ( "OR" and )*
and        = unary ( "AND" unary )*
unary      = "NOT" unary | comparison
comparison = "(" or ")" | operand suffix
suffix     = compare_op operand
           | "BETWEEN" operand "AND" operand
           | "IN" "(" literal ( "," literal )* ")"
           | "IS" "NULL"
           | "IS" "NOT" "NULL"
operand    = literal | column_ref
column_ref = IDENT  (* bare column name, no dots; no "fields." prefix *)
```

Reuse the structure of `guard.rs::tokenize`/`Parser` — copy the file as a starting point, then:

1. Drop the `Dot` token (we don't navigate paths).
2. Add keywords `AND`, `OR`, `NOT`, `BETWEEN`, `IN`, `IS`, `NULL` (case-insensitive — match `ident.to_ascii_uppercase()`).
3. Replace `Operand::FieldRef(Vec<String>)` with `Operand::ColumnRef(String)` and reject any `.` after an identifier with `CheckError::UnexpectedChar`.
4. Add `parse_suffix` after `parse_operand` to handle `BETWEEN`/`IN`/`IS [NOT] NULL`.

A reference shape for `parse_suffix`:

```rust
fn parse_suffix(&mut self, left: Operand) -> Result<CheckAst, CheckError> {
    match self.peek().map(|t| &t.kind) {
        Some(TokKind::Eq | TokKind::Ne | TokKind::Lt | TokKind::Le | TokKind::Gt | TokKind::Ge) => {
            let op = self.bump_cmp_op();
            let right = self.parse_operand()?;
            Ok(CheckAst::Compare { left, op, right })
        }
        Some(TokKind::Kw(kw)) if kw == "BETWEEN" => {
            self.bump();
            let low = self.parse_operand()?;
            self.expect_kw("AND")?;
            let high = self.parse_operand()?;
            Ok(CheckAst::Between { value: left, low, high })
        }
        Some(TokKind::Kw(kw)) if kw == "IN" => {
            self.bump();
            self.expect(TokKind::LParen)?;
            let mut set = Vec::new();
            loop {
                set.push(self.parse_literal()?);
                if !self.consume_if(TokKind::Comma) { break; }
            }
            self.expect(TokKind::RParen)?;
            Ok(CheckAst::In { value: left, set })
        }
        Some(TokKind::Kw(kw)) if kw == "IS" => {
            self.bump();
            if self.consume_if(TokKind::Kw("NOT".into())) {
                self.expect_kw("NULL")?;
                Ok(CheckAst::IsNotNull(left))
            } else {
                self.expect_kw("NULL")?;
                Ok(CheckAst::IsNull(left))
            }
        }
        _ => {
            let pos = self.peek().map(|t| t.start).unwrap_or(0);
            Err(CheckError::UnexpectedToken {
                pos,
                found: self.peek().map(|t| t.kind.describe()).unwrap_or_else(|| "<eof>".into()),
            })
        }
    }
}
```

(Adapt to whatever token-kind shape you end up with; this is shape, not a finished implementation.)

- [ ] **Step 2: Run the parser tests**

Run: `cargo test -p shared --test check_expression -- --skip evaluates`
Expected: all parser tests PASS; the three `evaluates_*` tests still fail (Task 3).

- [ ] **Step 3: Commit**

```bash
git add shared/src/builder/check.rs
git commit -m "feat(shared): CheckExpression parser (compare/between/in/is null/logical)"
```

---

## Task 3: Evaluator

**Files:**
- Modify: `shared/src/builder/check.rs`

- [ ] **Step 1: Implement `CheckAst::evaluate`**

Add (or replace the stub):

```rust
impl CheckAst {
    pub fn evaluate(&self, fields: &serde_json::Map<String, serde_json::Value>) -> bool {
        use CheckAst::*;
        match self {
            And(a, b) => a.evaluate(fields) && b.evaluate(fields),
            Or(a, b) => a.evaluate(fields) || b.evaluate(fields),
            Not(a) => !a.evaluate(fields),
            Compare { left, op, right } => compare(left, *op, right, fields),
            Between { value, low, high } => {
                let v = resolve(value, fields);
                compare_values(&v, CmpOp::Ge, &resolve(low, fields))
                    && compare_values(&v, CmpOp::Le, &resolve(high, fields))
            }
            In { value, set } => {
                let v = resolve(value, fields);
                set.iter().any(|lit| compare_values(&v, CmpOp::Eq, &literal_to_value(lit)))
            }
            IsNull(op) => resolve(op, fields).is_null(),
            IsNotNull(op) => !resolve(op, fields).is_null(),
        }
    }
}

fn resolve(op: &Operand, fields: &serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    match op {
        Operand::Literal(lit) => literal_to_value(lit),
        Operand::ColumnRef(name) => fields.get(name).cloned().unwrap_or(serde_json::Value::Null),
    }
}

fn literal_to_value(lit: &Literal) -> serde_json::Value {
    match lit {
        Literal::Str(s) => serde_json::Value::String(s.clone()),
        Literal::Num(n) => serde_json::json!(*n),
        Literal::Bool(b) => serde_json::Value::Bool(*b),
        Literal::Null => serde_json::Value::Null,
    }
}

fn compare(left: &Operand, op: CmpOp, right: &Operand, fields: &serde_json::Map<String, serde_json::Value>) -> bool {
    compare_values(&resolve(left, fields), op, &resolve(right, fields))
}

fn compare_values(l: &serde_json::Value, op: CmpOp, r: &serde_json::Value) -> bool {
    use serde_json::Value as V;
    match (l, r) {
        (V::Null, V::Null) => matches!(op, CmpOp::Eq | CmpOp::Le | CmpOp::Ge),
        (V::Bool(a), V::Bool(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            _ => false,
        },
        (V::Number(a), V::Number(b)) => {
            let (Some(af), Some(bf)) = (a.as_f64(), b.as_f64()) else {
                return matches!(op, CmpOp::Ne);
            };
            match op {
                CmpOp::Eq => af == bf,
                CmpOp::Ne => af != bf,
                CmpOp::Lt => af < bf,
                CmpOp::Le => af <= bf,
                CmpOp::Gt => af > bf,
                CmpOp::Ge => af >= bf,
            }
        }
        (V::String(a), V::String(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
        },
        _ => matches!(op, CmpOp::Ne),
    }
}
```

- [ ] **Step 2: Run the full test suite**

Run: `cargo test -p shared --test check_expression`
Expected: PASS (all 10 tests).

- [ ] **Step 3: Commit**

```bash
git add shared/src/builder/check.rs
git commit -m "feat(shared): CheckExpression evaluator"
```

---

## Task 4: Hook into `DbColumn` and `DbTable`

**Files:**
- Modify: `shared/src/lib.rs` (DbColumn around line 403, DbTable around line 426)
- Test: `shared/tests/db_schema_roundtrip.rs` (extend)

- [ ] **Step 1: Write the failing roundtrip extension**

Append to `shared/tests/db_schema_roundtrip.rs`:

```rust
use shared::CheckExpression;
use shared::NamedCheck;

#[test]
fn db_column_carries_check_expression() {
    let mut schema = sample_schema();
    schema.tables[0].columns[0].check = Some(CheckExpression::new("id IS NOT NULL"));
    schema.tables[0].checks.push(NamedCheck {
        name: "ck_kunde_demo".into(),
        expression: CheckExpression::new("id IS NOT NULL"),
    });

    let json = serde_json::to_string(&schema).unwrap();
    let back: shared::DbSchema = serde_json::from_str(&json).unwrap();
    assert_eq!(schema, back);

    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value["tables"][0]["columns"][0]["check"].is_string());
    assert!(value["tables"][0]["checks"][0]["expression"].is_string());
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p shared --test db_schema_roundtrip db_column_carries_check_expression`
Expected: compile error — `DbColumn.check` and `DbTable.checks` don't exist.

- [ ] **Step 3: Extend the types**

Edit `shared/src/lib.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NamedCheck {
    #[serde(default)]
    pub name: String,
    pub expression: CheckExpression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbColumn {
    pub id: String,
    pub name: String,
    pub data_type: DbColumnType,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    #[serde(default)]
    pub generated: ColumnGenerated,
    #[serde(default)]
    pub concurrency_token: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub audit_role: AuditRole,
    /// Phase 0.7-G2: optional column-level CHECK constraint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbTable {
    pub id: String,
    pub name: String,
    pub position: Position,
    pub columns: Vec<DbColumn>,
    /// Phase 0.7-G2: optional table-level (multi-column) CHECK constraints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<NamedCheck>,
}
```

Re-export `NamedCheck` from `lib.rs` top-level.

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cargo test -p shared --test db_schema_roundtrip`
Expected: PASS.

- [ ] **Step 5: Run the full shared suite**

Run: `cargo test -p shared`
Expected: PASS. Any existing `DbColumn { ... }` constructor that doesn't use struct-update syntax must add `check: None`. Any `DbTable { ... }` likewise needs `checks: Vec::new()`.

- [ ] **Step 6: Commit**

```bash
git add shared/src/lib.rs shared/tests/db_schema_roundtrip.rs
git commit -m "feat(shared): DbColumn.check + DbTable.checks for CHECK constraints"
```

---

## Task 5: DDL emits CHECK clauses

**Files:**
- Modify: `server/src/ddl.rs`

- [ ] **Step 1: Write the failing DDL test**

Extend the `tests` module in `server/src/ddl.rs`:

```rust
#[test]
fn renders_column_check() {
    let mut t = DbTable {
        id: "t".into(),
        name: "product".into(),
        position: Position::default(),
        columns: vec![col("id", DbColumnType::Text, true, false)],
        checks: Vec::new(),
    };
    t.columns[0].check = Some(shared::CheckExpression::new("id IS NOT NULL"));
    let sql = render_create_table(&t);
    assert!(
        sql.contains("CHECK (id IS NOT NULL)"),
        "missing inline CHECK in SQL:\n{sql}"
    );
}

#[test]
fn renders_table_check() {
    let t = DbTable {
        id: "t".into(),
        name: "journal_entry".into(),
        position: Position::default(),
        columns: vec![col("value", DbColumnType::Integer, false, false)],
        checks: vec![shared::NamedCheck {
            name: "ck_value_positive".into(),
            expression: shared::CheckExpression::new("value > 0"),
        }],
    };
    let sql = render_create_table(&t);
    assert!(
        sql.contains("CONSTRAINT ck_value_positive CHECK (value > 0)"),
        "missing table-level CHECK:\n{sql}"
    );
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p server --lib ddl::tests --target-dir target-test`
Expected: FAIL — the generated SQL contains no `CHECK`.

- [ ] **Step 3: Implement the rendering**

Edit `server/src/ddl.rs`:

```rust
pub fn render_create_table(table: &DbTable) -> String {
    let mut parts: Vec<String> = Vec::new();
    for col in &table.columns {
        let mut line = format!("  \"{}\" {}", col.name, sqlite_type(&col.data_type));
        if col.primary_key { line.push_str(" PRIMARY KEY"); }
        if !col.nullable { line.push_str(" NOT NULL"); }
        if col.unique && !col.primary_key { line.push_str(" UNIQUE"); }
        if let Some(def) = &col.default_value {
            line.push_str(&format!(" DEFAULT {}", def));
        }
        if let Some(check) = &col.check {
            line.push_str(&format!(" CHECK ({})", check.source()));
        }
        parts.push(line);
    }
    for chk in &table.checks {
        if chk.name.is_empty() {
            parts.push(format!("  CHECK ({})", chk.expression.source()));
        } else {
            parts.push(format!(
                "  CONSTRAINT {} CHECK ({})",
                chk.name,
                chk.expression.source()
            ));
        }
    }
    format!(
        "CREATE TABLE IF NOT EXISTS \"{}\" (\n{}\n);",
        table.name,
        parts.join(",\n")
    )
}
```

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p server --lib ddl::tests --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Run the server crate suite**

Run: `cargo test -p server --target-dir target-test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add server/src/ddl.rs
git commit -m "feat(server): DDL emits column-level + table-level CHECK"
```

---

## Notes for the implementer

- The Source-layer fallback for engines without CHECK (mentioned in the spec) is intentionally **not** in this plan. Today the only managed engine is SQLite, which supports CHECK. Add the fallback as a separate plan when a non-CHECK engine (e.g. older MySQL) enters the Source trait.
- `CheckExpression.source()` is rendered verbatim into SQL. The parser must reject any token that isn't SQL-safe (no semicolons, no parenthesis imbalance). The grammar above already does that — but if you extend the grammar, re-check that the source string can't become a SQL-injection vector.
- Default policy: an invalid `CheckExpression` (failed `parse()`) should be surfaced at `try_apply_schema` time as a warning and the statement skipped — mirror today's "warn and continue" pattern in `try_apply_schema`.
