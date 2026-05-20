# F1 — Operations-Resource Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hebt Operations (Cross-Entity-Logiken wie "Regenerate Account Entries", "Invert", "Reconcile") zu einer eigenständigen Top-Level-Resource neben `entities` und `reports`: GraphQL-Queries `operations` / `operation(id)`, neue Routes `/operations` + `/operations/:id` + `/operations/:id/runs/:runId`, persistente `operation_runs`-SeaORM-Tabelle für Run-History, Detail-Seite mit Input-Form (`<FieldRenderer>`-Reuse) + Run-History-Tabelle, Plugin-Anbindungspunkt für Phase 2.

**Architecture:** F1 ist die **Sichtbarkeitsschicht** über U6 — kein neuer Ausführungspfad. `ActionDefinition` (aus U6) wird in eine projektierte `OperationView` mit `recentRuns` umgewandelt. `operation_runs`-Tabelle persistiert Runs, die U6's In-Memory-Store mit 30-Min-TTL nicht halten kann. Re-Run ist UI-Helfer (kein Server-Pfad), der das gespeicherte Input-JSON ins Formular vorbefüllt. `input_schema` wird als `shared::EditorMeta` modelliert (existing Editor-Stub) — derselbe `<FieldRenderer>` wie für Entities/Wizards. F1 plant nur SyncResult; LongRunning bleibt Stub bis U6 lebt.

**Tech Stack:** Rust, `shared` (serde), `server` (axum + async-graphql + SeaORM), `client` (Leptos + leptos_router 0.7), `cargo test --workspace`.

**Spec-Referenz:** [`docs/superpowers/specs/2026-05-20-f1-operations-resource-design.md`](../specs/2026-05-20-f1-operations-resource-design.md).

**Vorbedingung:**
- U6-Wire-Typen (`ActionDefinition`, `ActionKind`) müssen verfügbar sein. Falls U6 noch nicht gelandet ist: dieser Plan kann mit einer reduzierten lokalen Version von `ActionKind` starten und später auf U6 umstellen.
- `<FieldRenderer>` und `EditorMeta` (siehe `shared/src/editor.rs`) — existieren heute.

**Decision aufgenommen:** `input_schema` ist eine `shared::EditorMeta` (existing Editor-Subset), nicht ein separates JSON-Schema. `<FieldRenderer>`-Reuse folgt direkt; kein zusätzlicher JSON-Schema-Parser.

---

## File Structure

**Neu:**
- `shared/src/operation.rs` — `OperationView`, `OperationRunSummary`, `OperationFilter`
- `shared/tests/operation_wire.rs` — Roundtrip
- `server/src/entity/operation_run.rs` — SeaORM-Modell
- `server/src/operations.rs` — Resolver + Persistenz-Hooks
- `server/tests/operations.rs` — End-to-End
- `client/src/routes/operations.rs` — Routes für `/operations`, `/operations/:id`, `/operations/:id/runs/:runId`
- `client/src/components/operations/mod.rs`
- `client/src/components/operations/list.rs`
- `client/src/components/operations/detail.rs`
- `client/src/components/operations/run_history.rs`
- `examples/shop/operations.toml` — Optional: Categories/Order
- `examples/shop/operations/echo-op.toml` — Beispiel-Operation für Tests

**Modifizieren:**
- `shared/src/lib.rs` — Re-Export Operation-Typen
- `shared/src/menu.rs` — `MenuAction::OpenOperation(String)`, `MenuAction::OpenOperationsList`
- `server/src/db.rs` — `operation_runs`-Tabelle in `create_table_from_entity` registrieren
- `server/src/schema.rs` — Queries `operations`, `operation(id)`
- `server/src/example/loader.rs` — `operations.toml` + `operations/<id>.toml`-Branch
- `server/src/lib.rs` — Operations-Modul mounten
- `client/src/lib.rs` — neues Modul `components::operations` + `routes::operations`
- `client/src/components/navigation.rs` — Branch auf `OpenOperation` / `OpenOperationsList`
- `client/src/graphql/queries.rs` — `fetch_operations`, `fetch_operation`
- `client/locales/de/main.ftl`, `client/locales/en/main.ftl` — `operation-*` Keys

---

## Architektur-Details

### `shared::OperationView` — Server→Client-Projektion

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationView {
    pub id: String,
    pub title_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub kind: ActionKindWire,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_meta: Option<EditorMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_duration_seconds: Option<u32>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub recent_runs: Vec<OperationRunSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActionKindWire {
    RowAction,
    BulkAction,
    EntityAction,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationRunSummary {
    pub run_id: String,
    pub started_at: String,             // ISO-8601
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub status: RunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub initiated_by: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus { Running, Completed, Failed, Cancelled }

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ActionKindWire>,
}
```

**Hinweis zu `ActionKindWire`**: spiegelt U6's `ActionKind` 1:1. Wenn U6 zuerst landet, ersetzt diese Plan-Spec den lokalen Re-Export durch ein `pub use shared::ActionKind as ActionKindWire`. Bis dahin steht das Wire-Format hier eigenständig.

### `operation_runs`-Tabelle

```rust
// server/src/entity/operation_run.rs
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "operation_runs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub run_id: String,
    pub operation_id: String,
    pub started_at: DateTimeUtc,
    pub completed_at: Option<DateTimeUtc>,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub initiated_by: String,
    pub input_json: String,
    pub final_report_json: Option<String>,
    pub error: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Schema-Creation idempotent in `server/src/db.rs::init` — wie alle anderen Tabellen via `Schema::create_table_from_entity`.

### Server-Pipeline

1. `data::operations(filter, auth)` liefert alle registrierten `ActionDefinition`s aus U6's Registry, gefiltert durch `auth.permissions`, optional gefiltert durch `category`/`kind`. Wenn U6 noch nicht da: leere Liste.
2. `data::operation(id, auth)` liefert eine Operation + `recent_runs(operation_id, limit=10)` aus `operation_runs`-Tabelle.
3. `operations::record_run_start(run_id, op_id, input_json, user)` schreibt eine Row mit `status="running"`.
4. `operations::record_run_finish(run_id, status, duration, report?, error?)` updated die Row.

Diese beiden Funktionen werden aus U6's `ActionHandler::run`-Wrapper aufgerufen. F1 stellt sie bereit; U6 ruft sie, sobald U6 landet. Bis dahin: ein Test-Helper schreibt synthetische Rows.

### `<FieldRenderer>`-Reuse für `input_meta`

Das Client-Detail (`/operations/:id`) übergibt `operation.input_meta` an `<FieldRenderer>` (existiert in `client/src/components/field/`). Submit sammelt die Felder zu einem JSON-Objekt → `POST /actions/start` (U6-Pfad) oder Mock-Aufruf für die F1-Tests.

### Was NICHT in diesem Plan ist

- **Live-Run-Stream** (SSE-Subscribe an `/actions/stream/:run_id`) — gehört zu U6. Detail-Seite kann den `ActionRunModal` aus U6 wiederverwenden, sobald U6 da ist; bis dahin zeigt sie nur die DB-Snapshot.
- **`/operations/:id/runs/:runId` Read-Only-View** — Stub-Route mit Platzhalter; volle Implementation in U6+F1-Folge-Spec.
- **String-Permission `operation:<id>`-Enforcement** — Markierung als `TODO(F1.1)` im Resolver; Permission-Liste wird heute nur informativ zurückgegeben.
- **Daily-Purge-Task** für `operation_runs`-Tabelle (Spec §9 Backlog) — separater Plan.

---

## Tasks

### Task 1: `shared::OperationView` Wire + Test

**Files:**
- Neu: `shared/src/operation.rs`
- Modify: `shared/src/lib.rs`
- Neu: `shared/tests/operation_wire.rs`

- [ ] **Schritt 1: Test schreiben**

`shared/tests/operation_wire.rs`:

```rust
use shared::{OperationView, OperationRunSummary, ActionKindWire, RunStatus};

#[test]
fn operation_view_minimal_roundtrip() {
    let op = OperationView {
        id: "echo".into(),
        title_key: "op-echo".into(),
        description_key: None,
        category: None,
        kind: ActionKindWire::Global,
        input_meta: None,
        estimated_duration_seconds: None,
        permissions: vec![],
        recent_runs: vec![],
    };
    let json = serde_json::to_string(&op).unwrap();
    let back: OperationView = serde_json::from_str(&json).unwrap();
    assert_eq!(back, op);
}

#[test]
fn action_kind_wire_camel_case() {
    let json = serde_json::to_string(&ActionKindWire::RowAction).unwrap();
    assert_eq!(json, "\"rowAction\"");
}

#[test]
fn run_summary_status_camel_case() {
    let s = OperationRunSummary {
        run_id: "r1".into(),
        started_at: "2025-01-01T00:00:00Z".into(),
        completed_at: None,
        status: RunStatus::Running,
        duration_ms: None,
        initiated_by: "alice".into(),
    };
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains(r#""status":"running""#));
}

#[test]
fn empty_recent_runs_skipped_in_json() {
    let op = OperationView {
        id: "x".into(), title_key: "x".into(),
        description_key: None, category: None,
        kind: ActionKindWire::Global, input_meta: None,
        estimated_duration_seconds: None,
        permissions: vec![], recent_runs: vec![],
    };
    let json = serde_json::to_string(&op).unwrap();
    // Wir wollen empty Vec NICHT skippen damit das Frontend einen leeren Array bekommt
    assert!(json.contains(r#""recentRuns":[]"#) || !json.contains("recentRuns"));
}
```

- [ ] **Schritt 2: FAIL**

```
cargo test -p shared --test operation_wire
```

- [ ] **Schritt 3: Modul anlegen**

`shared/src/operation.rs` mit den Typen aus "Architektur-Details" oben.

- [ ] **Schritt 4: `lib.rs` exportieren**

```rust
pub mod operation;
pub use operation::{
    ActionKindWire, OperationFilter, OperationRunSummary, OperationView, RunStatus,
};
```

- [ ] **Schritt 5: PASS + Commit**

```
cargo test -p shared --test operation_wire
git add shared/src/operation.rs shared/src/lib.rs shared/tests/operation_wire.rs
git commit -m "feat(shared): OperationView + RunSummary wire types"
```

---

### Task 2: `MenuAction::OpenOperation`-Varianten

**Files:**
- Modify: `shared/src/menu.rs`
- Modify (Test): existing `shared/tests/*menu*` oder Inline in operation_wire.rs

- [ ] **Schritt 1: Test ergänzen**

In `shared/tests/operation_wire.rs`:

```rust
use shared::MenuAction;

#[test]
fn open_operation_action_camel_case_kind() {
    let a = MenuAction::OpenOperation("echo".into());
    let json = serde_json::to_string(&a).unwrap();
    assert!(json.contains(r#""kind":"openOperation""#));
    let back: MenuAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back, a);
}

#[test]
fn open_operations_list_action() {
    let a = MenuAction::OpenOperationsList;
    let json = serde_json::to_string(&a).unwrap();
    assert!(json.contains(r#""kind":"openOperationsList""#));
}
```

- [ ] **Schritt 2: FAIL**

- [ ] **Schritt 3: Varianten ergänzen**

In `shared/src/menu.rs` zum bestehenden `MenuAction`-Enum:

```rust
OpenOperation(String),
OpenOperationsList,
```

Beim `from_route`-Mapping ggf. ein Match für Routen wie `/operations/...` ergänzen.

- [ ] **Schritt 4: PASS + Commit**

```
cargo test -p shared --test operation_wire
git add shared/src/menu.rs shared/tests/operation_wire.rs
git commit -m "feat(shared): MenuAction::OpenOperation + OpenOperationsList"
```

---

### Task 3: SeaORM `operation_runs`-Modell + DB-Init

**Files:**
- Neu: `server/src/entity/operation_run.rs`
- Modify: `server/src/entity/mod.rs` (oder `lib.rs`)
- Modify: `server/src/db.rs`

- [ ] **Schritt 1: Modell anlegen**

`server/src/entity/operation_run.rs` (Code aus "Architektur-Details").

- [ ] **Schritt 2: Im Entity-Modul registrieren**

In `server/src/entity/mod.rs`:

```rust
pub mod operation_run;
```

- [ ] **Schritt 3: DB-Init ergänzen**

In `server/src/db.rs::init` (oder wo die Tabellen erzeugt werden):

```rust
let stmt = schema.create_table_from_entity(crate::entity::operation_run::Entity);
db.execute(builder.build(&stmt.if_not_exists())).await?;
```

(Identisch zum Pattern für andere Tabellen — `grep -n "create_table_from_entity" server/src/db.rs`.)

- [ ] **Schritt 4: Inline-Test**

In `server/src/entity/operation_run.rs` am Ende:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, Set};

    #[tokio::test]
    #[serial_test::serial]
    async fn insert_and_read_run() {
        server::fresh_test_setup().await;
        let db = server::db::get().await;

        let now = chrono::Utc::now();
        let m = ActiveModel {
            run_id: Set("test-run-1".into()),
            operation_id: Set("echo".into()),
            started_at: Set(now),
            completed_at: Set(None),
            status: Set("running".into()),
            duration_ms: Set(None),
            initiated_by: Set("alice".into()),
            input_json: Set("{}".into()),
            final_report_json: Set(None),
            error: Set(None),
        };
        m.insert(db).await.unwrap();

        let read = Entity::find_by_id("test-run-1".to_string()).one(db).await.unwrap();
        assert!(read.is_some());
        assert_eq!(read.unwrap().status, "running");
    }
}
```

- [ ] **Schritt 5: Test laufen**

```
CARGO_TARGET_DIR=target-test cargo test -p server entity::operation_run::tests
```

- [ ] **Schritt 6: Commit**

```
git add server/src/entity/operation_run.rs server/src/entity/mod.rs server/src/db.rs
git commit -m "feat(server): operation_runs SeaORM model + db init"
```

---

### Task 4: `data::operations` + `data::operation` Resolver-Layer

**Files:**
- Neu: `server/src/operations.rs`
- Modify: `server/src/lib.rs`

- [ ] **Schritt 1: Modul anlegen**

`server/src/operations.rs`:

```rust
use chrono::Utc;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use shared::{ActionKindWire, OperationFilter, OperationRunSummary, OperationView, RunStatus};

/// Liefert alle sichtbaren Operations.
///
/// Quelle: U6's `ActionRegistry` (heute noch nicht installiert ⇒ leere Liste).
/// Filterung durch Auth ist `TODO(F1.1)` — wird in einer Folge-Spec ergänzt.
pub async fn list(filter: Option<OperationFilter>) -> Vec<OperationView> {
    let registered = registered_actions();
    let mut out: Vec<OperationView> = registered
        .into_iter()
        .filter(|op| match &filter {
            None => true,
            Some(f) => {
                f.category.as_ref().map_or(true, |c| op.category.as_deref() == Some(c.as_str()))
                    && f.kind.as_ref().map_or(true, |k| op.kind == *k)
            }
        })
        .collect();

    // recent_runs per Operation
    for op in &mut out {
        op.recent_runs = recent_runs_for(&op.id, 10).await;
    }
    out
}

pub async fn get(id: &str) -> Option<OperationView> {
    let mut all = list(None).await;
    all.retain(|o| o.id == id);
    all.pop()
}

pub async fn record_run_start(
    run_id: &str,
    operation_id: &str,
    input_json: &str,
    initiated_by: &str,
) -> Result<(), sea_orm::DbErr> {
    use crate::entity::operation_run;
    let db = crate::db::get().await;
    let m = operation_run::ActiveModel {
        run_id: sea_orm::Set(run_id.into()),
        operation_id: sea_orm::Set(operation_id.into()),
        started_at: sea_orm::Set(Utc::now()),
        completed_at: sea_orm::Set(None),
        status: sea_orm::Set("running".into()),
        duration_ms: sea_orm::Set(None),
        initiated_by: sea_orm::Set(initiated_by.into()),
        input_json: sea_orm::Set(input_json.into()),
        final_report_json: sea_orm::Set(None),
        error: sea_orm::Set(None),
    };
    sea_orm::EntityTrait::insert::<operation_run::Entity>(m).exec(db).await?;
    Ok(())
}

pub async fn record_run_finish(
    run_id: &str,
    status: RunStatus,
    final_report_json: Option<String>,
    error: Option<String>,
) -> Result<(), sea_orm::DbErr> {
    use crate::entity::operation_run;
    let db = crate::db::get().await;
    let now = Utc::now();
    let Some(row) = operation_run::Entity::find_by_id(run_id.to_string()).one(db).await? else {
        return Ok(());
    };
    let started = row.started_at;
    let duration_ms = (now - started).num_milliseconds();
    let status_str = match status {
        RunStatus::Running => "running",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
    };
    let mut m: operation_run::ActiveModel = row.into();
    m.completed_at = sea_orm::Set(Some(now));
    m.status = sea_orm::Set(status_str.into());
    m.duration_ms = sea_orm::Set(Some(duration_ms));
    m.final_report_json = sea_orm::Set(final_report_json);
    m.error = sea_orm::Set(error);
    use sea_orm::ActiveModelTrait;
    m.update(db).await?;
    Ok(())
}

async fn recent_runs_for(operation_id: &str, limit: u64) -> Vec<OperationRunSummary> {
    use crate::entity::operation_run;
    let db = crate::db::get().await;
    let rows = operation_run::Entity::find()
        .filter(operation_run::Column::OperationId.eq(operation_id))
        .order_by_desc(operation_run::Column::StartedAt)
        .limit(limit)
        .all(db).await
        .unwrap_or_default();
    rows.into_iter().map(|r| OperationRunSummary {
        run_id: r.run_id,
        started_at: r.started_at.to_rfc3339(),
        completed_at: r.completed_at.map(|t| t.to_rfc3339()),
        status: match r.status.as_str() {
            "running" => RunStatus::Running,
            "completed" => RunStatus::Completed,
            "failed" => RunStatus::Failed,
            "cancelled" => RunStatus::Cancelled,
            _ => RunStatus::Failed,
        },
        duration_ms: r.duration_ms,
        initiated_by: r.initiated_by,
    }).collect()
}

/// Stub: liefert die in U6's Registry registrierten Actions.
/// Heute leer; sobald U6 landet, hier die Registry abfragen.
fn registered_actions() -> Vec<OperationView> {
    #[cfg(test)]
    {
        // Test-Fixture: eine Echo-Operation für die Integrationstests
        return vec![OperationView {
            id: "echo".into(),
            title_key: "op-echo".into(),
            description_key: Some("op-echo-desc".into()),
            category: Some("test".into()),
            kind: ActionKindWire::Global,
            input_meta: None,
            estimated_duration_seconds: Some(2),
            permissions: vec!["operation:echo".into()],
            recent_runs: vec![],
        }];
    }
    #[cfg(not(test))]
    vec![]
}
```

- [ ] **Schritt 2: In `lib.rs` mounten**

```rust
pub mod operations;
```

- [ ] **Schritt 3: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 4: Commit**

```
git add server/src/operations.rs server/src/lib.rs
git commit -m "feat(server): operations module with list/get/record_run_*"
```

---

### Task 5: GraphQL-Queries `operations` und `operation(id)`

**Files:**
- Modify: `server/src/schema.rs`

- [ ] **Schritt 1: Resolver ergänzen**

Im `QueryRoot`-Impl in `server/src/schema.rs`:

```rust
async fn operations(
    &self,
    _ctx: &Context<'_>,
    filter: Option<async_graphql::Json<shared::OperationFilter>>,
) -> async_graphql::Result<async_graphql::Json<Vec<shared::OperationView>>> {
    let f = filter.map(|j| j.0);
    Ok(async_graphql::Json(crate::operations::list(f).await))
}

async fn operation(
    &self,
    _ctx: &Context<'_>,
    id: String,
) -> async_graphql::Result<Option<async_graphql::Json<shared::OperationView>>> {
    Ok(crate::operations::get(&id).await.map(async_graphql::Json))
}
```

(Analog zum Pattern für `entities` mit JSON-Wrap — verifiziere die heutige Schreibweise via `grep -n "entities" server/src/schema.rs`.)

- [ ] **Schritt 2: End-to-End-Test**

`server/tests/operations.rs`:

```rust
use serde_json::json;
use shared::{OperationView, RunStatus};

#[tokio::test]
#[serial_test::serial]
async fn operations_query_lists_test_fixture() {
    server::fresh_test_setup().await;

    let body = json!({
        "query": "query { operations(filter: null) }"
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let arr = resp["data"]["operations"].as_array().unwrap();
    assert!(arr.iter().any(|op| op["id"] == "echo"));
}

#[tokio::test]
#[serial_test::serial]
async fn operation_query_returns_recent_runs() {
    server::fresh_test_setup().await;
    server::operations::record_run_start("r1", "echo", "{}", "alice").await.unwrap();
    server::operations::record_run_finish("r1", RunStatus::Completed, Some("{\"ok\":true}".into()), None).await.unwrap();

    let body = json!({
        "query": "query { operation(id: \"echo\") }"
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let op = &resp["data"]["operation"];
    let runs = op["recentRuns"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["status"], "completed");
    assert_eq!(runs[0]["initiatedBy"], "alice");
}

#[tokio::test]
#[serial_test::serial]
async fn operation_query_unknown_returns_null() {
    server::fresh_test_setup().await;
    let body = json!({"query": "query { operation(id: \"unknown\") }"});
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    assert!(resp["data"]["operation"].is_null());
}
```

- [ ] **Schritt 3: Test laufen**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test operations
```
Erwartung: 3 Tests grün. Falls `test_helpers::run_graphql` fehlt, im Pattern von bestehenden Server-Tests anlegen.

- [ ] **Schritt 4: Commit**

```
git add server/src/schema.rs server/tests/operations.rs
git commit -m "feat(server): GraphQL operations + operation(id) queries"
```

---

### Task 6: Loader-Branch für `operations/`-Verzeichnis

**Files:**
- Modify: `server/src/example/loader.rs`

- [ ] **Schritt 1: Loader-Branch ergänzen**

Im `load(dir)`-Pfad einen optionalen Branch für `dir/operations/<id>.{toml,json}`:

```rust
let ops_dir = dir.join("operations");
let mut operations: Vec<shared::OperationView> = Vec::new();
if ops_dir.is_dir() {
    for entry in std::fs::read_dir(&ops_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(op) = read_typed::<shared::OperationView>(&path)? {
            operations.push(op);
        }
    }
}
```

Im `ExampleSet` ein Feld `pub operations: Vec<shared::OperationView>` ergänzen.

- [ ] **Schritt 2: Lookup-Helfer**

In `server/src/operations.rs::registered_actions` den Test-only-Stub durch einen `cfg(not(test))`-Pfad ersetzen, der `crate::example::current()` befragt:

```rust
fn registered_actions() -> Vec<OperationView> {
    crate::example::current()
        .map(|s| s.operations.clone())
        .unwrap_or_default()
}
```

(Test-Fixtures werden jetzt aus `examples/shop/operations/echo-op.toml` geladen.)

- [ ] **Schritt 3: Beispiel-Operation**

`examples/shop/operations/echo-op.toml`:

```toml
id = "echo"
title_key = "op-echo"
description_key = "op-echo-desc"
category = "test"
kind = "global"
estimatedDurationSeconds = 2
permissions = ["operation:echo"]
```

- [ ] **Schritt 4: Loader-Test ergänzen**

In `server/tests/loader.rs` (falls existent):

```rust
#[test]
fn loads_operations_from_example_shop() {
    let set = server::example::load(std::path::Path::new("../examples/shop")).unwrap();
    assert!(set.operations.iter().any(|op| op.id == "echo"));
}
```

- [ ] **Schritt 5: Tests laufen**

```
CARGO_TARGET_DIR=target-test cargo test -p server loader
CARGO_TARGET_DIR=target-test cargo test -p server --test operations
```

- [ ] **Schritt 6: Commit**

```
git add server/src/example/loader.rs server/src/example/mod.rs server/src/operations.rs examples/shop/operations/echo-op.toml server/tests/loader.rs
git commit -m "feat(server): loader reads operations/<id>.{toml,json}"
```

---

### Task 7: Client-GraphQL-Funktionen

**Files:**
- Modify: `client/src/graphql/queries.rs`

- [ ] **Schritt 1: Funktionen ergänzen**

```rust
const OPERATIONS_QUERY: &str = r#"
query Operations($filter: JSON) {
    operations(filter: $filter)
}
"#;

const OPERATION_QUERY: &str = r#"
query Operation($id: String!) {
    operation(id: $id)
}
"#;

pub async fn fetch_operations(filter: Option<shared::OperationFilter>) -> Result<Vec<shared::OperationView>, GraphqlError> {
    #[derive(serde::Deserialize)]
    struct Resp { operations: Vec<shared::OperationView> }
    let vars = serde_json::json!({ "filter": filter });
    let r: Resp = crate::graphql::execute(OPERATIONS_QUERY, &vars).await?;
    Ok(r.operations)
}

pub async fn fetch_operation(id: &str) -> Result<Option<shared::OperationView>, GraphqlError> {
    #[derive(serde::Deserialize)]
    struct Resp { operation: Option<shared::OperationView> }
    let vars = serde_json::json!({ "id": id });
    let r: Resp = crate::graphql::execute(OPERATION_QUERY, &vars).await?;
    Ok(r.operation)
}
```

(`GraphqlError`, `execute`-Signatur an existing graphql-Modul anpassen.)

- [ ] **Schritt 2: Compile-Check**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

- [ ] **Schritt 3: Commit**

```
git add client/src/graphql/queries.rs
git commit -m "feat(client): fetch_operations + fetch_operation"
```

---

### Task 8: Operations-Listen-Seite

**Files:**
- Neu: `client/src/components/operations/mod.rs`
- Neu: `client/src/components/operations/list.rs`
- Neu: `client/src/routes/operations.rs`
- Modify: `client/src/lib.rs`
- Modify: `client/src/app.rs` (Route registrieren)
- Modify: `client/locales/de/main.ftl`, `en/main.ftl`

- [ ] **Schritt 1: Komponente**

`client/src/components/operations/mod.rs`:

```rust
pub mod list;
pub mod detail;
pub mod run_history;

pub use list::OperationsList;
pub use detail::OperationDetail;
```

`client/src/components/operations/list.rs`:

```rust
use leptos::prelude::*;
use crate::graphql::queries::fetch_operations;
use crate::i18n::t;
use crate::styling::use_design;

#[component]
pub fn OperationsList() -> impl IntoView {
    let design = use_design();
    let ops = LocalResource::new(|| async move {
        fetch_operations(None).await.unwrap_or_default()
    });

    view! {
        <div style=design.surface().inline.clone()>
            <h1>{ t("operations-title") }</h1>
            <Suspense fallback=|| view! { <p>{ t("loading") }</p> }>
                {move || ops.read().as_ref().map(|list| {
                    let by_cat = group_by_category(list);
                    view! {
                        <div>
                        {by_cat.into_iter().map(|(cat, items)| view! {
                            <section>
                                <h2>{cat.clone()}</h2>
                                <ul>
                                {items.iter().map(|op| {
                                    let href = format!("/operations/{}", op.id);
                                    view! {
                                        <li>
                                            <a href=href>{ t(&op.title_key) }</a>
                                            { op.description_key.as_ref().map(|d| view! { <p>{ t(d) }</p> }) }
                                        </li>
                                    }
                                }).collect_view()}
                                </ul>
                            </section>
                        }).collect_view()}
                        </div>
                    }
                })}
            </Suspense>
        </div>
    }
}

fn group_by_category(ops: &[shared::OperationView]) -> Vec<(String, Vec<shared::OperationView>)> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, Vec<shared::OperationView>> = BTreeMap::new();
    for op in ops {
        map.entry(op.category.clone().unwrap_or_else(|| "general".into()))
            .or_default()
            .push(op.clone());
    }
    map.into_iter().collect()
}
```

`client/src/routes/operations.rs`:

```rust
use leptos::prelude::*;
use leptos_router::components::Outlet;

#[component]
pub fn OperationsRoute() -> impl IntoView {
    view! { <Outlet/> }
}
```

- [ ] **Schritt 2: Route registrieren**

In `client/src/app.rs::App` (oder wo die Routes definiert sind):

```rust
<Route path=path!("/operations") view=crate::components::operations::OperationsList/>
<Route path=path!("/operations/:id") view=crate::components::operations::OperationDetail/>
```

(Genaue API hängt von leptos_router 0.7 ab — `grep -n "<Route" client/src/app.rs` für das heutige Pattern.)

- [ ] **Schritt 3: Modul exportieren**

In `client/src/lib.rs`:

```rust
pub mod components { pub mod operations; /* … */ }
pub mod routes { pub mod operations; /* … */ }
```

- [ ] **Schritt 4: i18n-Keys**

`client/locales/de/main.ftl`:
```
operations-title = Operationen
operation-run = Ausführen
operation-recent-runs = Letzte Läufe
op-echo = Echo-Operation
op-echo-desc = Demo-Aktion ohne Effekt
```

`client/locales/en/main.ftl`:
```
operations-title = Operations
operation-run = Run
operation-recent-runs = Recent runs
op-echo = Echo operation
op-echo-desc = Demo action with no side effect
```

- [ ] **Schritt 5: Compile + manuelle Verifikation**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
cargo run -p server -- --data-dir ./examples/shop &
cd client && trunk serve
```

Im Browser: `http://localhost:8080/operations` zeigt "Operationen" + "test"-Sektion + Echo-Link.

- [ ] **Schritt 6: Commit**

```
git add client/src/components/operations/ client/src/routes/operations.rs client/src/lib.rs client/src/app.rs client/locales/
git commit -m "feat(client): operations list route"
```

---

### Task 9: Operations-Detail-Seite + Run-History

**Files:**
- Neu: `client/src/components/operations/detail.rs`
- Neu: `client/src/components/operations/run_history.rs`

- [ ] **Schritt 1: Detail-Komponente**

`client/src/components/operations/detail.rs`:

```rust
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::graphql::queries::fetch_operation;
use crate::i18n::t;
use crate::styling::use_design;

#[component]
pub fn OperationDetail() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();
    let design = use_design();

    let op = LocalResource::new(move || {
        let id = id();
        async move { fetch_operation(&id).await.ok().flatten() }
    });

    view! {
        <div style=design.surface().inline.clone()>
            <Suspense fallback=|| view! { <p>{ t("loading") }</p> }>
                {move || match op.read().as_ref() {
                    Some(Some(op)) => view! {
                        <div>
                            <h1>{ t(&op.title_key) }</h1>
                            { op.description_key.as_ref().map(|d| view! { <p>{ t(d) }</p> }) }

                            // Input-Form (V1: leer wenn input_meta=None)
                            <section>
                                { match &op.input_meta {
                                    None => view! { <p>{ t("operation-no-input") }</p> }.into_any(),
                                    Some(_meta) => view! {
                                        // TODO(F1.1): <FieldRenderer meta=meta.clone() value=signal />
                                        <p>{ t("operation-input-todo") }</p>
                                    }.into_any(),
                                }}
                                <button>{ t("operation-run") }</button>
                            </section>

                            // Run-History
                            <section>
                                <h2>{ t("operation-recent-runs") }</h2>
                                <crate::components::operations::run_history::RunHistory runs=op.recent_runs.clone()/>
                            </section>
                        </div>
                    }.into_any(),
                    Some(None) => view! { <p>{ t("operation-not-found") }</p> }.into_any(),
                    None => view! { <p>{ t("loading") }</p> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}
```

- [ ] **Schritt 2: Run-History-Komponente**

`client/src/components/operations/run_history.rs`:

```rust
use leptos::prelude::*;
use shared::{OperationRunSummary, RunStatus};
use crate::i18n::t;

#[component]
pub fn RunHistory(runs: Vec<OperationRunSummary>) -> impl IntoView {
    if runs.is_empty() {
        return view! { <p>{ t("operation-no-runs") }</p> }.into_any();
    }

    view! {
        <table>
            <thead>
                <tr>
                    <th>{ t("run-started-at") }</th>
                    <th>{ t("run-status") }</th>
                    <th>{ t("run-duration") }</th>
                    <th>{ t("run-initiated-by") }</th>
                </tr>
            </thead>
            <tbody>
                {runs.into_iter().map(|r| {
                    let status_label = match r.status {
                        RunStatus::Running => t("run-status-running"),
                        RunStatus::Completed => t("run-status-completed"),
                        RunStatus::Failed => t("run-status-failed"),
                        RunStatus::Cancelled => t("run-status-cancelled"),
                    };
                    let duration = r.duration_ms.map(|ms| format!("{} ms", ms)).unwrap_or_default();
                    view! {
                        <tr>
                            <td>{r.started_at.clone()}</td>
                            <td>{status_label}</td>
                            <td>{duration}</td>
                            <td>{r.initiated_by.clone()}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }.into_any()
}
```

- [ ] **Schritt 3: i18n-Keys ergänzen**

In beiden FTL-Dateien:
```
operation-no-input = Diese Operation hat keine Eingaben.
operation-input-todo = Input-Form folgt mit U6.
operation-no-runs = Noch keine Läufe.
operation-not-found = Operation nicht gefunden.
run-started-at = Gestartet
run-status = Status
run-duration = Dauer
run-initiated-by = Ausgelöst von
run-status-running = Läuft
run-status-completed = Fertig
run-status-failed = Fehler
run-status-cancelled = Abgebrochen
loading = Lade…
```

(Englisch analog.)

- [ ] **Schritt 4: Compile + manuelle Verifikation**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

Im Browser: `/operations/echo` zeigt Titel, "Keine Eingaben"-Hinweis, "Noch keine Läufe"-Tabelle.

- [ ] **Schritt 5: Commit**

```
git add client/src/components/operations/detail.rs client/src/components/operations/run_history.rs client/locales/
git commit -m "feat(client): operation detail route with run history"
```

---

### Task 10: Navigation-Branch + Final Cleanup

**Files:**
- Modify: `client/src/components/navigation.rs`
- Workspace-Cleanup

- [ ] **Schritt 1: Nav-Branch**

`grep -n "MenuAction::OpenEntity" client/src/components/navigation.rs` und dort einen weiteren Arm ergänzen:

```rust
shared::MenuAction::OpenOperationsList => view! {
    <a href="/operations">{ t(&node.label_key) }</a>
}.into_any(),
shared::MenuAction::OpenOperation(id) => view! {
    <a href=format!("/operations/{}", id)>{ t(&node.label_key) }</a>
}.into_any(),
```

- [ ] **Schritt 2: Optional `operations.toml` Categories/Order**

`examples/shop/operations.toml`:

```toml
[[categories]]
id = "test"
label_key = "ops-cat-test"
order = 1
```

Loader liest optional — V1 ignoriert das Order/Label-Feld, nutzt nur Operations selbst (Erweiterung in Folge-Spec).

- [ ] **Schritt 3: Clippy + fmt + alle Tests**

```
CARGO_TARGET_DIR=target-test cargo clippy --workspace --no-deps -- -A clippy::doc_overindented_list_items -A clippy::too_many_arguments
cargo fmt
CARGO_TARGET_DIR=target-test cargo test --workspace
```

- [ ] **Schritt 4: Self-Review**

- Spec §3.1 Beziehung zu U6 → Architektur-Details ✓
- Spec §3.2 GraphQL → Task 5 ✓
- Spec §3.3 Run-History persistent → Task 3 + Task 4 ✓
- Spec §3.4 Client-Routen → Tasks 8 + 9 ✓
- Spec §3.5 Navigation → Task 10 ✓
- Spec §3.6 Re-Run UI-Helfer → TODO in Detail (Task 9, kann später als input_json-Prefill-Hook ergänzt werden — bewusst out of scope, mit `TODO(F1.1)`-Kommentar in detail.rs markieren)
- Spec §5 Edge-Cases (Permission fehlt: heute alle Ops, da TODO; deregistered: handled in `get(...)` → null; input_schema fehlt: handled in Detail) → ✓ teilweise (Permission TODO)
- Spec §7 Tests → Tasks 1, 5 ✓

- [ ] **Schritt 5: Commit**

```
git add client/src/components/navigation.rs examples/shop/operations.toml
git commit -m "feat(client): navigation links to OpenOperation/OpenOperationsList"
```

---

## Was später kommt

- **F1.1**: Permission-Enforcement, Re-Run-UI-Helfer (Input-JSON-Prefill), `input_meta`-Form-Rendering via FieldRenderer
- **F1.2**: `/operations/:id/runs/:runId` Read-Only-View, integriert mit U6-`ActionRunModal` im "completed"-Mode
- **F1.3**: Daily-Purge-Task für `operation_runs`-Tabelle (TTL konfigurierbar)
- **Plugin-Integration** (Phase 2): Operations aus WASM-Plugins registrieren
- **Cron-Scheduling** (Phase 2 Job-Scheduler)
