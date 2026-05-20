# U6 — Long-Running-Action Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Etabliert eine wiederverwendbare Pipeline für Operationen, die länger als ~2 s laufen, inkrementellen Fortschritt (Schritte, Counts, Logs) liefern, abbrechbar sind und am Ende einen strukturierten Bericht produzieren. Transport ist Server-Sent-Events (SSE) über klassische HTTP-Endpoints, nicht GraphQL-Subscriptions. Ein Echo-Test-Handler + `<ActionRunModal>` mit Progress-Bar + Live-Log + Final-Report machen das Ganze ab Tag 1 demonstrierbar.

**Architecture:** Wire-Typen `ActionDefinition`, `ActionEvent` (getaggte Union), `ActionReport` in `shared::action`. Server-Side: `ActionRunStore` (DashMap + `tokio::sync::broadcast`-Sender + CancellationToken pro Run) + `ActionHandler`-Trait + Registry. SSE-Endpoint `GET /actions/stream/:run_id` streamt JSON-Events, `POST /actions/start` startet einen Run als Tokio-Task, `POST /actions/cancel/:run_id` setzt das CancellationToken. Client: REST-Adapter im `graphql/`-Modul (Pfad-Wahl pragmatisch — auch `client/src/actions/`), SSE über `EventSource`/`gloo-net`, Reducer-State im Modal.

**Tech Stack:** Rust, `shared` (serde), `server` (axum + `tokio::sync::broadcast` + `tokio_util::sync::CancellationToken` + `tower-http`), `client` (Leptos CSR/WASM + `gloo-net` für SSE oder direkt `web-sys::EventSource`), `cargo test --workspace`.

**Spec-Referenz:** [`docs/superpowers/specs/2026-05-20-u6-long-running-action-design.md`](../specs/2026-05-20-u6-long-running-action-design.md).

**Transport-Decision:** **SSE** (gemäß Spec §3.3). Begründung: heutiger GraphQL-Client ist POST-only ohne Subscriptions; async-graphql-Schema benutzt `EmptySubscription`; nativ in axum verfügbar; Reverse-Proxy-tauglich; Client per `EventSource` einfach.

**Vorbedingung:** keine.

---

## File Structure

**Neu:**
- `shared/src/action.rs` — `ActionDefinition`, `ActionKind`, `ActionEvent`, `ActionReport`, `LogSeverity`, `ActionTarget`, `ActionWarning`, `ActionArtifact`
- `shared/tests/action_event_wire.rs` — Wire-Roundtrip
- `server/src/actions/mod.rs`
- `server/src/actions/store.rs` — `ActionRunStore` + `ActionRunHandle`
- `server/src/actions/handler.rs` — Trait + Registry + `ActionEmitter`
- `server/src/actions/sse.rs` — HTTP-Routes `/actions/start`, `/actions/stream/:run_id`, `/actions/cancel/:run_id`
- `server/src/actions/echo.rs` — Test-Handler `echo-progress`
- `server/tests/action_run.rs` — End-to-End
- `client/src/components/action/mod.rs`
- `client/src/components/action/action_run_modal.rs`
- `client/src/components/action/progress_view.rs`
- `client/src/components/action/reducer.rs` — Unit-tested Reducer
- `client/src/graphql/actions.rs` — REST-Adapter (Name aus Konsistenz im `graphql`-Modul; bedient REST + SSE)

**Modifizieren:**
- `shared/src/lib.rs` — Re-Export Action-Typen
- `server/Cargo.toml` — `tokio_util = { features = ["rt"] }`, `bytes`, `futures` (wenn nicht schon da)
- `server/src/lib.rs` (oder `main.rs`) — Router mit `/actions/...` mergen, Store in App-State legen
- `server/src/schema.rs` — neue Query `availableActions(target: ActionTarget): [ActionDefinition]`
- `client/src/lib.rs` — neues Modul `components::action` exportieren
- `client/src/components/table/row_actions.rs`, `top_menu.rs`, `entity_menu.rs` — Hooks für `ActionKind::RowAction`/`BulkAction`/`EntityAction`
- `client/locales/de/main.ftl`, `client/locales/en/main.ftl` — `action-*` keys

---

## Architektur-Details

### `shared::action` Wire-Inventar

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionDefinition {
    pub id: String,
    pub title_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_key: Option<String>,
    pub kind: ActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_duration_seconds: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActionKind { RowAction, BulkAction, EntityAction, Global }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ActionEvent {
    Started { run_id: String, total_steps: Option<u32> },
    Step { index: u32, title_key: String },
    Progress { current: u64, total: Option<u64>, message_key: Option<String> },
    Log { severity: LogSeverity, message: String },
    Warning { target: ActionTarget, message_key: String, message_args: serde_json::Value },
    Completed { report: ActionReport },
    Failed { error: String, partial_report: ActionReport },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogSeverity { Info, Warn, Error }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionTarget { pub entity_type: String, pub id: String }

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionReport {
    pub duration_ms: u64,
    #[serde(default)] pub counts: BTreeMap<String, i64>,
    #[serde(default)] pub warnings: Vec<ActionWarning>,
    #[serde(default)] pub artifacts: Vec<ActionArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionWarning {
    pub target: ActionTarget,
    pub message_key: String,
    #[serde(default)] pub message_args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionArtifact { pub label_key: String, pub url: String, pub mime: String }
```

### Server: `ActionRunStore`

```rust
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use std::sync::Mutex;

pub type RunId = String;

pub struct ActionRunHandle {
    pub started_at: Instant,
    pub cancel: CancellationToken,
    pub sender: broadcast::Sender<shared::ActionEvent>,
    pub snapshot: Mutex<RunSnapshot>,
}

#[derive(Default, Clone)]
pub struct RunSnapshot {
    pub started: Option<shared::ActionEvent>,
    pub current_step: Option<shared::ActionEvent>,
    pub last_progress: Option<shared::ActionEvent>,
    pub warnings: Vec<shared::ActionEvent>,
    pub terminal: Option<shared::ActionEvent>, // Completed | Failed
}

#[derive(Clone, Default)]
pub struct ActionRunStore {
    runs: Arc<DashMap<RunId, Arc<ActionRunHandle>>>,
}

impl ActionRunStore {
    pub fn new() -> Self { Self::default() }

    pub fn insert(&self, run_id: RunId, handle: Arc<ActionRunHandle>) {
        self.runs.insert(run_id, handle);
    }
    pub fn get(&self, run_id: &str) -> Option<Arc<ActionRunHandle>> {
        self.runs.get(run_id).map(|e| e.clone())
    }
    pub fn purge_older_than(&self, ttl: std::time::Duration) {
        let now = Instant::now();
        self.runs.retain(|_, h| now.duration_since(h.started_at) < ttl);
    }
}
```

`ArcSwapOption` (Spec §3.4) wird durch `Mutex<RunSnapshot>` ersetzt — eine Mutation pro Event ist genug, kein RW-Druck.

### `ActionHandler` + `ActionEmitter`

```rust
use async_trait::async_trait;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub struct ActionContext {
    pub run_id: String,
    pub user: String,
    pub cancel: CancellationToken,
}

#[derive(Clone)]
pub struct ActionEmitter {
    pub sender: broadcast::Sender<shared::ActionEvent>,
    pub snapshot: std::sync::Arc<std::sync::Mutex<crate::actions::store::RunSnapshot>>,
}

impl ActionEmitter {
    fn emit(&self, ev: shared::ActionEvent) {
        let _ = self.sender.send(ev.clone());
        if let Ok(mut snap) = self.snapshot.lock() {
            match &ev {
                shared::ActionEvent::Started { .. } => snap.started = Some(ev.clone()),
                shared::ActionEvent::Step { .. } => snap.current_step = Some(ev.clone()),
                shared::ActionEvent::Progress { .. } => snap.last_progress = Some(ev.clone()),
                shared::ActionEvent::Warning { .. } => snap.warnings.push(ev.clone()),
                shared::ActionEvent::Completed { .. } | shared::ActionEvent::Failed { .. } => {
                    snap.terminal = Some(ev.clone());
                }
                _ => {}
            }
        }
    }
    pub fn started(&self, run_id: &str, total_steps: Option<u32>) {
        self.emit(shared::ActionEvent::Started { run_id: run_id.into(), total_steps });
    }
    pub fn step(&self, index: u32, title_key: &str) {
        self.emit(shared::ActionEvent::Step { index, title_key: title_key.into() });
    }
    pub fn progress(&self, current: u64, total: Option<u64>, msg: Option<&str>) {
        self.emit(shared::ActionEvent::Progress {
            current, total, message_key: msg.map(|s| s.into())
        });
    }
    pub fn log(&self, severity: shared::LogSeverity, msg: impl Into<String>) {
        self.emit(shared::ActionEvent::Log { severity, message: msg.into() });
    }
    pub fn warning(&self, target: shared::ActionTarget, message_key: &str, args: serde_json::Value) {
        self.emit(shared::ActionEvent::Warning {
            target, message_key: message_key.into(), message_args: args
        });
    }
    pub fn completed(&self, report: shared::ActionReport) {
        self.emit(shared::ActionEvent::Completed { report });
    }
    pub fn failed(&self, error: impl Into<String>, partial: shared::ActionReport) {
        self.emit(shared::ActionEvent::Failed { error: error.into(), partial_report: partial });
    }
}

#[async_trait]
pub trait ActionHandler: Send + Sync + 'static {
    fn definition(&self) -> shared::ActionDefinition;
    async fn run(
        &self,
        ctx: ActionContext,
        input: serde_json::Value,
        emitter: ActionEmitter,
    ) -> Result<shared::ActionReport, ActionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

// Registry: lazy_static / OnceLock<HashMap<id, Arc<dyn ActionHandler>>>
```

### SSE-Routes

```
POST /actions/start
   Body: { actionId, input }
   → 200 { runId }

GET /actions/stream/:run_id
   → text/event-stream
   sendet erst Snapshot, dann broadcast::Receiver

POST /actions/cancel/:run_id
   → 200 { ok: true }
```

Replay-Strategie: vor dem Subscribe wird `RunSnapshot.started`, `current_step`, `last_progress`, alle `warnings` und (falls da) `terminal` einmal in-order rausgeschickt. Damit ist ein Re-Mount des Modals nicht "leer".

### Was NICHT in diesem Plan ist

- Persistente `action_runs`-Tabelle — gehört zu F1 (oder einer Folge-Spec).
- Plugin-getriebene Handler (Phase 2).
- "Im Hintergrund laufen lassen"-Toggle (Spec §2).
- Konkrete Domain-Handler T5/T10/T16 — eigene Specs.

---

## Tasks

### Task 1: `shared::action` Wire-Roundtrip

**Files:**
- Neu: `shared/src/action.rs`
- Modify: `shared/src/lib.rs`
- Neu: `shared/tests/action_event_wire.rs`

- [ ] **Schritt 1: Failing Test**

`shared/tests/action_event_wire.rs`:

```rust
use shared::{ActionEvent, ActionReport, ActionTarget, ActionDefinition, ActionKind, LogSeverity};

#[test]
fn action_event_started_camel_case_type_tag() {
    let ev = ActionEvent::Started { run_id: "r1".into(), total_steps: Some(3) };
    let json = serde_json::to_string(&ev).unwrap();
    assert!(json.contains(r#""type":"started""#));
    assert!(json.contains(r#""runId":"r1""#));
    let back: ActionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ev);
}

#[test]
fn action_event_progress_with_optional_message() {
    let ev = ActionEvent::Progress { current: 50, total: Some(100), message_key: None };
    let json = serde_json::to_string(&ev).unwrap();
    assert!(!json.contains("messageKey"));
    let back: ActionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ev);
}

#[test]
fn action_event_log_severity_lowercase() {
    let ev = ActionEvent::Log { severity: LogSeverity::Warn, message: "x".into() };
    let json = serde_json::to_string(&ev).unwrap();
    assert!(json.contains(r#""severity":"warn""#));
}

#[test]
fn action_event_completed_with_report() {
    let mut counts = std::collections::BTreeMap::new();
    counts.insert("processed".into(), 10);
    let report = ActionReport { duration_ms: 1500, counts, warnings: vec![], artifacts: vec![] };
    let ev = ActionEvent::Completed { report };
    let json = serde_json::to_string(&ev).unwrap();
    let back: ActionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ev);
}

#[test]
fn action_definition_kind_camel_case() {
    let d = ActionDefinition {
        id: "x".into(), title_key: "x".into(),
        description_key: None, kind: ActionKind::EntityAction,
        input_schema: None, permissions: vec![], estimated_duration_seconds: None,
    };
    let json = serde_json::to_string(&d).unwrap();
    assert!(json.contains(r#""kind":"entityAction""#));
}
```

- [ ] **Schritt 2: FAIL**

- [ ] **Schritt 3: Modul anlegen**

`shared/src/action.rs` mit den Typen aus "Architektur-Details".

- [ ] **Schritt 4: Re-export in `lib.rs`**

```rust
pub mod action;
pub use action::{
    ActionArtifact, ActionDefinition, ActionEvent, ActionKind, ActionReport, ActionTarget,
    ActionWarning, LogSeverity,
};
```

- [ ] **Schritt 5: PASS + Commit**

```
cargo test -p shared --test action_event_wire
git add shared/src/action.rs shared/src/lib.rs shared/tests/action_event_wire.rs
git commit -m "feat(shared): ActionEvent + ActionDefinition wire types"
```

---

### Task 2: Server `actions::store::ActionRunStore`

**Files:**
- Neu: `server/src/actions/mod.rs`
- Neu: `server/src/actions/store.rs`
- Modify: `server/src/lib.rs`
- Modify: `server/Cargo.toml`

- [ ] **Schritt 1: Cargo-Deps prüfen/ergänzen**

`server/Cargo.toml`:

```toml
[dependencies]
dashmap = "5"
tokio = { version = "1", features = ["sync", "rt-multi-thread", "macros", "time"] }
tokio-util = { version = "0.7", features = ["rt"] }
async-trait = "0.1"
futures = "0.3"
bytes = "1"
```

- [ ] **Schritt 2: Modul-Scaffold**

`server/src/actions/mod.rs`:

```rust
//! Long-Running Action Pipeline (U6).
pub mod store;
pub mod handler;
pub mod sse;
pub mod echo;

pub use store::{ActionRunStore, ActionRunHandle, RunSnapshot};
pub use handler::{ActionHandler, ActionContext, ActionEmitter, ActionError, Registry};
```

`server/src/actions/store.rs` mit Code aus "Architektur-Details".

- [ ] **Schritt 3: In `lib.rs` einhängen**

```rust
pub mod actions;
```

- [ ] **Schritt 4: Inline-Unit-Test**

Am Ende von `store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;

    fn handle() -> Arc<ActionRunHandle> {
        let (tx, _rx) = broadcast::channel(256);
        Arc::new(ActionRunHandle {
            started_at: Instant::now(),
            cancel: CancellationToken::new(),
            sender: tx,
            snapshot: Mutex::new(RunSnapshot::default()),
        })
    }

    #[test]
    fn insert_get() {
        let store = ActionRunStore::new();
        store.insert("r1".into(), handle());
        assert!(store.get("r1").is_some());
        assert!(store.get("nope").is_none());
    }

    #[tokio::test]
    async fn purge_removes_old_runs() {
        let store = ActionRunStore::new();
        store.insert("r1".into(), handle());
        tokio::time::sleep(Duration::from_millis(50)).await;
        store.purge_older_than(Duration::from_millis(10));
        assert!(store.get("r1").is_none());
    }
}
```

- [ ] **Schritt 5: Test laufen + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server actions::store::tests
git add server/Cargo.toml server/src/actions/mod.rs server/src/actions/store.rs server/src/lib.rs
git commit -m "feat(server): ActionRunStore + RunSnapshot"
```

---

### Task 3: `ActionHandler`-Trait + Registry + `ActionEmitter`

**Files:**
- Neu: `server/src/actions/handler.rs`

- [ ] **Schritt 1: Modul-Code**

`server/src/actions/handler.rs` mit dem Trait + Emitter + Error aus "Architektur-Details", plus:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock;

static REGISTRY: OnceLock<RwLock<HashMap<String, Arc<dyn ActionHandler>>>> = OnceLock::new();

pub fn registry() -> &'static RwLock<HashMap<String, Arc<dyn ActionHandler>>> {
    REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

pub async fn register(handler: Arc<dyn ActionHandler>) {
    let def = handler.definition();
    let mut r = registry().write().await;
    r.insert(def.id.clone(), handler);
}

pub async fn lookup(id: &str) -> Option<Arc<dyn ActionHandler>> {
    let r = registry().read().await;
    r.get(id).cloned()
}

pub async fn all_definitions() -> Vec<shared::ActionDefinition> {
    let r = registry().read().await;
    r.values().map(|h| h.definition()).collect()
}

pub struct Registry; // Marker type for tests/exports
```

- [ ] **Schritt 2: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 3: Commit**

```
git add server/src/actions/handler.rs
git commit -m "feat(server): ActionHandler trait + global registry"
```

---

### Task 4: Echo-Test-Handler

**Files:**
- Neu: `server/src/actions/echo.rs`

- [ ] **Schritt 1: Implementierung**

```rust
use async_trait::async_trait;
use std::time::Duration;
use std::collections::BTreeMap;
use crate::actions::handler::{ActionHandler, ActionContext, ActionEmitter, ActionError};

pub struct EchoHandler;

#[async_trait]
impl ActionHandler for EchoHandler {
    fn definition(&self) -> shared::ActionDefinition {
        shared::ActionDefinition {
            id: "echo-progress".into(),
            title_key: "action-echo-title".into(),
            description_key: Some("action-echo-desc".into()),
            kind: shared::ActionKind::Global,
            input_schema: None,
            permissions: vec![],
            estimated_duration_seconds: Some(3),
        }
    }

    async fn run(
        &self,
        ctx: ActionContext,
        input: serde_json::Value,
        emitter: ActionEmitter,
    ) -> Result<shared::ActionReport, ActionError> {
        let start = std::time::Instant::now();
        let steps = input.get("steps").and_then(|v| v.as_u64()).unwrap_or(5) as u32;

        emitter.started(&ctx.run_id, Some(steps));

        for i in 1..=steps {
            if ctx.cancel.is_cancelled() {
                emitter.log(shared::LogSeverity::Warn, "cancelled by user");
                return Err(ActionError::Cancelled);
            }
            emitter.step(i, "action-echo-step");
            for p in 0..=100 {
                if ctx.cancel.is_cancelled() {
                    return Err(ActionError::Cancelled);
                }
                emitter.progress((i as u64 - 1) * 100 + p, Some(steps as u64 * 100), None);
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            emitter.log(shared::LogSeverity::Info, format!("step {i} done"));
        }

        let mut counts = BTreeMap::new();
        counts.insert("processed".into(), steps as i64);
        let report = shared::ActionReport {
            duration_ms: start.elapsed().as_millis() as u64,
            counts, warnings: vec![], artifacts: vec![],
        };
        emitter.completed(report.clone());
        Ok(report)
    }
}
```

- [ ] **Schritt 2: Beim Server-Start registrieren**

In `server/src/lib.rs::boot` (oder Equivalent — die Funktion, die `fresh_test_setup` aufruft):

```rust
crate::actions::handler::register(
    std::sync::Arc::new(crate::actions::echo::EchoHandler)
).await;
```

- [ ] **Schritt 3: Commit**

```
git add server/src/actions/echo.rs server/src/lib.rs
git commit -m "feat(server): echo-progress test action handler"
```

---

### Task 5: SSE-Endpoints `/actions/start`, `/actions/stream/:run_id`, `/actions/cancel/:run_id`

**Files:**
- Neu: `server/src/actions/sse.rs`

- [ ] **Schritt 1: Route-Implementierung**

```rust
use axum::{
    extract::{Path, State, Json as AxumJson},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Router,
};
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::sync::CancellationToken;

use crate::actions::store::{ActionRunHandle, ActionRunStore, RunSnapshot};
use crate::actions::handler::{ActionContext, ActionEmitter};

pub fn actions_router(store: ActionRunStore) -> Router {
    Router::new()
        .route("/actions/start", post(start_handler))
        .route("/actions/stream/:run_id", get(stream_handler))
        .route("/actions/cancel/:run_id", post(cancel_handler))
        .with_state(store)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartRequest {
    action_id: String,
    #[serde(default)]
    input: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartResponse { run_id: String }

async fn start_handler(
    State(store): State<ActionRunStore>,
    AxumJson(req): AxumJson<StartRequest>,
) -> Response {
    let handler = match crate::actions::handler::lookup(&req.action_id).await {
        Some(h) => h,
        None => return (StatusCode::NOT_FOUND, "unknown action").into_response(),
    };

    let run_id = uuid::Uuid::new_v4().to_string();
    let (tx, _rx) = broadcast::channel(256);
    let cancel = CancellationToken::new();
    let snapshot = std::sync::Arc::new(std::sync::Mutex::new(RunSnapshot::default()));

    let handle = Arc::new(ActionRunHandle {
        started_at: Instant::now(),
        cancel: cancel.clone(),
        sender: tx.clone(),
        snapshot: std::sync::Mutex::new(RunSnapshot::default()), // doppelt — siehe Hinweis unten
    });
    store.insert(run_id.clone(), handle.clone());

    // Spawn the handler-Task
    let run_id_for_ctx = run_id.clone();
    let emitter = ActionEmitter {
        sender: tx.clone(),
        snapshot: snapshot.clone(),
    };
    let cancel_for_ctx = cancel.clone();
    tokio::spawn(async move {
        let ctx = ActionContext {
            run_id: run_id_for_ctx.clone(),
            user: "anonymous".into(), // TODO: AuthSession integrieren
            cancel: cancel_for_ctx.clone(),
        };
        let result = handler.run(ctx, req.input, emitter.clone()).await;
        if let Err(e) = result {
            match e {
                crate::actions::handler::ActionError::Cancelled => {
                    emitter.failed("cancelled", shared::ActionReport::default());
                }
                crate::actions::handler::ActionError::Other(msg) => {
                    emitter.failed(msg, shared::ActionReport::default());
                }
            }
        }
    });

    (StatusCode::OK, AxumJson(StartResponse { run_id })).into_response()
}

async fn stream_handler(
    State(store): State<ActionRunStore>,
    Path(run_id): Path<String>,
) -> Response {
    let Some(handle) = store.get(&run_id) else {
        return (StatusCode::NOT_FOUND, "unknown run").into_response();
    };

    let rx = handle.sender.subscribe();

    // Snapshot zuerst (zur Initial-Replay)
    let snap = handle.snapshot.lock().unwrap().clone();
    let initial: Vec<shared::ActionEvent> = snap.started.into_iter()
        .chain(snap.current_step.into_iter())
        .chain(snap.last_progress.into_iter())
        .chain(snap.warnings.into_iter())
        .chain(snap.terminal.into_iter())
        .collect();

    let initial_stream = futures::stream::iter(initial.into_iter().map(Ok::<_, std::io::Error>));
    let live = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(ev) => Some(Ok::<_, std::io::Error>(ev)),
            Err(_) => None, // Lagged: Gap — Client soll Snapshot neu holen
        }
    });

    let combined = initial_stream.chain(live).map(|res| {
        res.map(|ev| {
            let json = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".into());
            Event::default().data(json)
        }).map_err(|e| -> std::io::Error { e })
    });

    Sse::new(combined.map(|r| r.map_err(|e| -> Infallible { panic!("{e}") })))
        .keep_alive(KeepAlive::default())
        .into_response()
}

async fn cancel_handler(
    State(store): State<ActionRunStore>,
    Path(run_id): Path<String>,
) -> Response {
    if let Some(h) = store.get(&run_id) {
        h.cancel.cancel();
    }
    (StatusCode::OK, AxumJson(serde_json::json!({"ok": true}))).into_response()
}
```

Hinweis (Doppel-`snapshot`): die obige Implementation hat zwei Snapshots — einen im `ActionRunHandle` und einen im `ActionEmitter`. Korrekte Form: nur **eine** Instanz, gemeinsam in `Arc<Mutex<RunSnapshot>>`. Refactor: `ActionRunHandle.snapshot: Arc<Mutex<RunSnapshot>>` und im `start_handler` denselben `Arc` an Handle und Emitter geben. Diesen Refactor in Schritt 2 ausführen.

- [ ] **Schritt 2: Refactor `ActionRunHandle.snapshot` zu `Arc<Mutex<...>>`**

In `server/src/actions/store.rs` ändern:

```rust
pub struct ActionRunHandle {
    pub started_at: Instant,
    pub cancel: CancellationToken,
    pub sender: broadcast::Sender<shared::ActionEvent>,
    pub snapshot: Arc<Mutex<RunSnapshot>>,
}
```

Inline-Tests in `store.rs` anpassen.

- [ ] **Schritt 3: `start_handler` korrigieren**

```rust
let snapshot = Arc::new(Mutex::new(RunSnapshot::default()));
let handle = Arc::new(ActionRunHandle {
    started_at: Instant::now(),
    cancel: cancel.clone(),
    sender: tx.clone(),
    snapshot: snapshot.clone(),
});
// ... emitter sieht denselben Arc:
let emitter = ActionEmitter { sender: tx.clone(), snapshot: snapshot.clone() };
```

- [ ] **Schritt 4: Cargo-Deps für SSE**

`server/Cargo.toml`:

```toml
[dependencies]
tokio-stream = { version = "0.1", features = ["sync"] }
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Schritt 5: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 6: Router mounten**

In `server/src/lib.rs` (wo der App-Router gebaut wird):

```rust
let action_store = crate::actions::ActionRunStore::new();
let app = Router::new()
    .route("/graphql", post(graphql_handler))
    // …
    .merge(crate::actions::sse::actions_router(action_store.clone()))
    // …
;
```

(Wenn der Store auch in anderen Pfaden gebraucht wird, in `AppState` legen — hier bewusst ad-hoc, weil F1 darauf zugreift.)

- [ ] **Schritt 7: Commit**

```
git add server/src/actions/sse.rs server/src/actions/store.rs server/src/lib.rs server/Cargo.toml
git commit -m "feat(server): SSE routes /actions/start | stream | cancel"
```

---

### Task 6: End-to-End-Test `server/tests/action_run.rs`

**Files:**
- Neu: `server/tests/action_run.rs`

- [ ] **Schritt 1: Test schreiben**

```rust
use serde_json::json;

#[tokio::test]
#[serial_test::serial]
async fn echo_action_completes_with_report() {
    server::fresh_test_setup().await;

    // Start
    let resp: serde_json::Value = server::test_helpers::post_json(
        "/actions/start",
        &json!({"actionId": "echo-progress", "input": {"steps": 2}}),
    ).await;
    let run_id = resp["runId"].as_str().unwrap().to_string();
    assert!(!run_id.is_empty());

    // Stream — collect alle Events bis Completed
    let events = server::test_helpers::collect_sse_events(
        &format!("/actions/stream/{}", run_id),
        std::time::Duration::from_secs(5),
    ).await;

    assert!(events.iter().any(|e| matches!(e, shared::ActionEvent::Started { .. })));
    assert!(events.iter().any(|e| matches!(e, shared::ActionEvent::Step { .. })));
    assert!(events.iter().any(|e| matches!(e, shared::ActionEvent::Progress { .. })));
    let completed = events.iter().find(|e| matches!(e, shared::ActionEvent::Completed { .. }));
    assert!(completed.is_some(), "no Completed event in stream");
}

#[tokio::test]
#[serial_test::serial]
async fn cancel_stops_action() {
    server::fresh_test_setup().await;
    let resp: serde_json::Value = server::test_helpers::post_json(
        "/actions/start",
        &json!({"actionId": "echo-progress", "input": {"steps": 20}}),
    ).await;
    let run_id = resp["runId"].as_str().unwrap().to_string();

    // Sofort cancel
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let _: serde_json::Value = server::test_helpers::post_json(
        &format!("/actions/cancel/{}", run_id),
        &json!({}),
    ).await;

    let events = server::test_helpers::collect_sse_events(
        &format!("/actions/stream/{}", run_id),
        std::time::Duration::from_secs(3),
    ).await;

    let failed = events.iter().find(|e| matches!(e, shared::ActionEvent::Failed { .. }));
    assert!(failed.is_some(), "expected Failed (cancelled) event");
}

#[tokio::test]
#[serial_test::serial]
async fn unknown_action_returns_404() {
    server::fresh_test_setup().await;
    let status = server::test_helpers::post_status(
        "/actions/start",
        &json!({"actionId": "does-not-exist", "input": {}}),
    ).await;
    assert_eq!(status, 404);
}
```

- [ ] **Schritt 2: `test_helpers` bereitstellen**

In `server/src/lib.rs` unter `#[cfg(test)] pub mod test_helpers`:

```rust
pub async fn post_json<T: serde::de::DeserializeOwned>(path: &str, body: &serde_json::Value) -> T {
    // Wenn ein in-process Test-Router existiert (tower::ServiceExt::oneshot), den nutzen.
    // Sonst spinnt der Test einen lokalen Server auf.
    todo!("an existing test-helper bauen oder neu anlegen — siehe muster in server/tests/")
}

pub async fn post_status(path: &str, body: &serde_json::Value) -> u16 { todo!() }

pub async fn collect_sse_events(path: &str, timeout: std::time::Duration) -> Vec<shared::ActionEvent> {
    // SSE-Read mit timeout, bis ein terminales Event kommt oder timeout abläuft
    todo!()
}
```

Konkret implementieren: Tower-Service mit `oneshot` aufrufen; SSE-Body als Chunks lesen und `data: <json>\n\n`-Blöcke parsen.

- [ ] **Schritt 3: Tests laufen — iterativ bis grün**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test action_run
```

- [ ] **Schritt 4: Commit**

```
git add server/tests/action_run.rs server/src/lib.rs
git commit -m "test(server): action-run SSE end-to-end (start/stream/cancel)"
```

---

### Task 7: `availableActions` GraphQL-Query

**Files:**
- Modify: `server/src/schema.rs`

- [ ] **Schritt 1: Resolver**

```rust
async fn available_actions(
    &self,
    _ctx: &Context<'_>,
    target: Option<async_graphql::Json<shared::ActionTarget>>,
) -> async_graphql::Result<async_graphql::Json<Vec<shared::ActionDefinition>>> {
    let _t = target.map(|j| j.0);
    let defs = crate::actions::handler::all_definitions().await;
    // Heute keine target-basierte Filterung — kommt mit konkreten Handlers in T5/T10/T16.
    Ok(async_graphql::Json(defs))
}
```

- [ ] **Schritt 2: Inline-Test**

In `server/tests/action_run.rs` ergänzen:

```rust
#[tokio::test]
#[serial_test::serial]
async fn available_actions_lists_echo() {
    server::fresh_test_setup().await;
    let body = serde_json::json!({"query": "query { availableActions(target: null) }"});
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let arr = resp["data"]["availableActions"].as_array().unwrap();
    assert!(arr.iter().any(|d| d["id"] == "echo-progress"));
}
```

- [ ] **Schritt 3: Test laufen + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test action_run available_actions_lists_echo
git add server/src/schema.rs server/tests/action_run.rs
git commit -m "feat(server): availableActions GraphQL query"
```

---

### Task 8: Client REST-Adapter `graphql/actions.rs`

**Files:**
- Neu: `client/src/graphql/actions.rs`
- Modify: `client/src/graphql/mod.rs`

- [ ] **Schritt 1: Adapter**

`client/src/graphql/actions.rs`:

```rust
use serde::{Deserialize, Serialize};
use shared::ActionEvent;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartActionRequest<'a> {
    pub action_id: &'a str,
    pub input: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartActionResponse {
    pub run_id: String,
}

pub async fn start(action_id: &str, input: serde_json::Value) -> Result<String, gloo_net::Error> {
    let req = StartActionRequest { action_id, input };
    let resp = gloo_net::http::Request::post("/actions/start")
        .header("Content-Type", "application/json")
        .json(&req)?
        .send().await?;
    let body: StartActionResponse = resp.json().await?;
    Ok(body.run_id)
}

pub async fn cancel(run_id: &str) -> Result<(), gloo_net::Error> {
    let _ = gloo_net::http::Request::post(&format!("/actions/cancel/{}", run_id))
        .header("Content-Type", "application/json")
        .body("{}")?
        .send().await?;
    Ok(())
}

/// Öffnet ein `EventSource` und liefert pro empfangenem `data`-Block ein
/// dekodiertes `ActionEvent`. Aufrufer subscribed mit Closures.
pub fn subscribe<F: Fn(ActionEvent) + 'static>(run_id: &str, on_event: F) -> web_sys::EventSource {
    let es = web_sys::EventSource::new(&format!("/actions/stream/{}", run_id))
        .expect("EventSource");
    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |ev: web_sys::MessageEvent| {
        let Ok(data) = ev.data().dyn_into::<wasm_bindgen::JsValue>() else { return; };
        let Some(s) = data.as_string() else { return; };
        if let Ok(action_ev) = serde_json::from_str::<ActionEvent>(&s) {
            on_event(action_ev);
        }
    }) as Box<dyn FnMut(_)>);
    es.set_onmessage(Some(closure.as_ref().unchecked_ref()));
    closure.forget();
    es
}
```

(Imports: `use wasm_bindgen::JsCast;`.)

- [ ] **Schritt 2: In `graphql/mod.rs` einhängen**

```rust
pub mod actions;
```

- [ ] **Schritt 3: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/graphql/actions.rs client/src/graphql/mod.rs
git commit -m "feat(client): REST+SSE adapter for actions"
```

---

### Task 9: Unit-tested Reducer `client/src/components/action/reducer.rs`

**Files:**
- Neu: `client/src/components/action/reducer.rs`

- [ ] **Schritt 1: Reducer + Tests**

```rust
use shared::{ActionEvent, ActionReport, LogSeverity};

#[derive(Default, Clone, Debug, PartialEq)]
pub struct ActionState {
    pub status: Status,
    pub current_step: Option<u32>,
    pub total_steps: Option<u32>,
    pub progress_current: u64,
    pub progress_total: Option<u64>,
    pub log: Vec<LogLine>,
    pub final_report: Option<ActionReport>,
    pub error: Option<String>,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    #[default] Idle,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    pub severity: LogSeverity,
    pub message: String,
}

pub fn reduce(state: &mut ActionState, event: ActionEvent) {
    match event {
        ActionEvent::Started { total_steps, .. } => {
            state.status = Status::Running;
            state.total_steps = total_steps;
        }
        ActionEvent::Step { index, .. } => {
            state.current_step = Some(index);
        }
        ActionEvent::Progress { current, total, .. } => {
            state.progress_current = current;
            state.progress_total = total;
        }
        ActionEvent::Log { severity, message } => {
            state.log.push(LogLine { severity, message });
            if state.log.len() > 500 { state.log.remove(0); }
        }
        ActionEvent::Warning { message_key, .. } => {
            state.log.push(LogLine { severity: LogSeverity::Warn, message: message_key });
        }
        ActionEvent::Completed { report } => {
            state.status = Status::Done;
            state.final_report = Some(report);
        }
        ActionEvent::Failed { error, partial_report } => {
            state.status = Status::Failed;
            state.error = Some(error);
            state.final_report = Some(partial_report);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn started_sets_running() {
        let mut s = ActionState::default();
        reduce(&mut s, ActionEvent::Started { run_id: "r".into(), total_steps: Some(3) });
        assert_eq!(s.status, Status::Running);
        assert_eq!(s.total_steps, Some(3));
    }

    #[test]
    fn progress_updates_counts() {
        let mut s = ActionState::default();
        reduce(&mut s, ActionEvent::Progress { current: 50, total: Some(100), message_key: None });
        assert_eq!(s.progress_current, 50);
        assert_eq!(s.progress_total, Some(100));
    }

    #[test]
    fn log_caps_at_500() {
        let mut s = ActionState::default();
        for _ in 0..600 {
            reduce(&mut s, ActionEvent::Log { severity: LogSeverity::Info, message: "x".into() });
        }
        assert_eq!(s.log.len(), 500);
    }

    #[test]
    fn completed_sets_done() {
        let mut s = ActionState::default();
        let r = ActionReport { duration_ms: 100, ..ActionReport::default() };
        reduce(&mut s, ActionEvent::Completed { report: r.clone() });
        assert_eq!(s.status, Status::Done);
        assert_eq!(s.final_report, Some(r));
    }
}
```

- [ ] **Schritt 2: Tests laufen**

```
CARGO_TARGET_DIR=../target-test cargo test --target wasm32-unknown-unknown -p client components::action::reducer::tests
```

(Falls wasm-Tests im Repo nicht laufen, denselben Reducer als crate-untested-Logik isolieren — Tests sind reine Rust, kein WASM nötig — und mit normalem `cargo test -p client --lib` laufen.)

- [ ] **Schritt 3: Commit**

```
git add client/src/components/action/reducer.rs
git commit -m "feat(client): ActionState reducer + unit tests"
```

---

### Task 10: `<ProgressView>` (Re-usable Progress-Bar + Log-Pane)

**Files:**
- Neu: `client/src/components/action/progress_view.rs`

- [ ] **Schritt 1: Komponente**

```rust
use leptos::prelude::*;
use shared::LogSeverity;
use crate::components::action::reducer::{ActionState, Status};
use crate::styling::use_design;
use crate::i18n::t;

#[component]
pub fn ProgressView(state: ReadSignal<ActionState>) -> impl IntoView {
    let design = use_design();

    let percent = move || {
        let s = state.get();
        match (s.progress_current, s.progress_total) {
            (c, Some(t)) if t > 0 => ((c as f64 / t as f64) * 100.0) as u32,
            _ => 0,
        }
    };

    view! {
        <div style=design.surface().inline.clone()>
            // Progress bar
            <div style="position:relative;width:100%;height:8px;background:#eee;border-radius:4px;overflow:hidden;">
                <div style=move || format!("width:{}%;height:100%;background:#0066cc;transition:width 0.2s;", percent())></div>
            </div>
            <p>
                {move || {
                    let s = state.get();
                    match (s.current_step, s.total_steps) {
                        (Some(c), Some(tot)) => format!("Step {} / {}", c, tot),
                        _ => String::new(),
                    }
                }}
            </p>

            // Log pane
            <div style="max-height:240px;overflow-y:auto;border:1px solid #ddd;padding:4px;font-family:monospace;font-size:12px;">
                {move || {
                    let s = state.get();
                    s.log.iter().map(|line| {
                        let color = match line.severity {
                            LogSeverity::Info => "#333",
                            LogSeverity::Warn => "#b45309",
                            LogSeverity::Error => "#b91c1c",
                        };
                        let style = format!("color:{};", color);
                        view! { <div style=style>{line.message.clone()}</div> }
                    }).collect_view()
                }}
            </div>
        </div>
    }
}
```

(Styles inline pragmatisch — wenn DesignSystem semantische Accessoren kennt, durch `design.progress_bar()` / `design.log_line(severity)` ersetzen.)

- [ ] **Schritt 2: Compile-Check**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

- [ ] **Schritt 3: Commit**

```
git add client/src/components/action/progress_view.rs
git commit -m "feat(client): ProgressView (progress bar + log pane)"
```

---

### Task 11: `<ActionRunModal>` + i18n

**Files:**
- Neu: `client/src/components/action/action_run_modal.rs`
- Neu: `client/src/components/action/mod.rs`
- Modify: `client/src/lib.rs`
- Modify: `client/locales/de/main.ftl`, `en/main.ftl`

- [ ] **Schritt 1: Modul-Root**

`client/src/components/action/mod.rs`:

```rust
pub mod action_run_modal;
pub mod progress_view;
pub mod reducer;

pub use action_run_modal::ActionRunModal;
```

- [ ] **Schritt 2: Modal-Komponente**

```rust
use leptos::prelude::*;
use crate::components::action::reducer::{reduce, ActionState, Status};
use crate::components::action::progress_view::ProgressView;
use crate::graphql::actions;
use crate::i18n::t;
use crate::styling::use_design;

#[component]
pub fn ActionRunModal(
    action_id: String,
    #[prop(optional)] input: Option<serde_json::Value>,
    on_close: Callback<()>,
) -> impl IntoView {
    let design = use_design();
    let (state, set_state) = create_signal(ActionState::default());
    let run_id = create_rw_signal::<Option<String>>(None);
    let event_source = create_rw_signal::<Option<web_sys::EventSource>>(None);

    // Start beim Mount
    let action_id_for_effect = action_id.clone();
    let input_for_effect = input.unwrap_or(serde_json::json!({}));
    create_local_resource(
        move || (action_id_for_effect.clone(), input_for_effect.clone()),
        move |(id, input)| async move {
            let started = actions::start(&id, input).await;
            if let Ok(run) = started {
                let set_state_for_cb = set_state.clone();
                let es = actions::subscribe(&run, move |ev| {
                    set_state_for_cb.update(|s| reduce(s, ev));
                });
                run_id.set(Some(run));
                event_source.set(Some(es));
            }
        },
    );

    let on_cancel = move |_| {
        if let Some(id) = run_id.get() {
            wasm_bindgen_futures::spawn_local(async move {
                let _ = actions::cancel(&id).await;
            });
        }
    };

    let on_close_local = move |_| {
        // EventSource schliessen
        if let Some(es) = event_source.get() { es.close(); }
        on_close.call(());
    };

    view! {
        <div style="position:fixed;inset:0;background:rgba(0,0,0,0.4);display:flex;align-items:center;justify-content:center;z-index:1000;">
            <div style=design.surface().inline.clone() style:max-width="640px" style:width="100%" style:padding="16px">
                <header style="display:flex;justify-content:space-between;align-items:center;">
                    <h2>{ t("action-running") }</h2>
                    <span>{move || match state.get().status {
                        Status::Idle => "—",
                        Status::Running => "RUNNING",
                        Status::Done => "DONE",
                        Status::Failed => "FAILED",
                    }}</span>
                </header>

                <ProgressView state=state/>

                {move || state.get().final_report.as_ref().map(|r| {
                    view! {
                        <section style="margin-top:12px;">
                            <h3>{ t("action-final-report") }</h3>
                            <ul>
                            {r.counts.iter().map(|(k, v)| view! {
                                <li>{format!("{}: {}", k, v)}</li>
                            }).collect_view()}
                            </ul>
                        </section>
                    }
                })}

                <footer style="display:flex;gap:8px;margin-top:12px;justify-content:flex-end;">
                    {move || match state.get().status {
                        Status::Running => view! { <button on:click=on_cancel>{ t("action-cancel") }</button> }.into_any(),
                        _ => view! { <span/> }.into_any(),
                    }}
                    <button on:click=on_close_local>{ t("action-close") }</button>
                </footer>
            </div>
        </div>
    }
}
```

- [ ] **Schritt 3: i18n-Keys**

`client/locales/de/main.ftl`:
```
action-running = Aktion läuft
action-cancel = Abbrechen
action-close = Schließen
action-final-report = Ergebnis
action-echo-title = Echo-Aktion
action-echo-desc = Demo-Aktion mit Fortschritt
action-echo-step = Schritt
```

`client/locales/en/main.ftl`:
```
action-running = Action running
action-cancel = Cancel
action-close = Close
action-final-report = Result
action-echo-title = Echo action
action-echo-desc = Demo action with progress
action-echo-step = Step
```

- [ ] **Schritt 4: Modul exportieren**

`client/src/lib.rs`:

```rust
pub mod components { pub mod action; /* ... */ }
```

- [ ] **Schritt 5: Compile-Check**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

- [ ] **Schritt 6: Commit**

```
git add client/src/components/action/ client/src/lib.rs client/locales/
git commit -m "feat(client): ActionRunModal with progress view"
```

---

### Task 12: Demo-Trigger im Dashboard / Test-Tabelle

**Files:**
- Modify: `client/src/routes/mod.rs` (oder `dashboard.rs`)

- [ ] **Schritt 1: Demo-Button**

In der DashboardPage einen "Echo-Action starten"-Button ergänzen, der `<ActionRunModal action_id="echo-progress"/>` einblendet. Dient als manuelle Smoke-Verifikation.

```rust
let (modal_open, set_modal_open) = create_signal(false);

view! {
    <div>
        <button on:click=move |_| set_modal_open.set(true)>{ t("demo-run-echo") }</button>
        {move || modal_open.get().then(|| view! {
            <ActionRunModal
                action_id="echo-progress".into()
                input=None
                on_close=Callback::new(move |_| set_modal_open.set(false))
            />
        })}
    </div>
}
```

- [ ] **Schritt 2: i18n-Key**

`demo-run-echo = Echo-Aktion starten` / `Run echo action`.

- [ ] **Schritt 3: Manuelle Verifikation**

```
cargo run -p server -- --data-dir ./examples/shop &
cd client && trunk serve
```

Im Dashboard "Echo-Aktion starten" klicken → Modal öffnet, Progress-Bar läuft, Log-Lines erscheinen, "Cancel" funktioniert, "Schließen" am Ende.

- [ ] **Schritt 4: Commit**

```
git add client/src/routes/ client/locales/
git commit -m "feat(client): dashboard demo trigger for echo action"
```

---

### Task 13: Workspace-Cleanup

- [ ] **Schritt 1: Clippy + fmt + alle Tests**

```
CARGO_TARGET_DIR=target-test cargo clippy --workspace --no-deps -- -A clippy::doc_overindented_list_items -A clippy::too_many_arguments
cargo fmt
CARGO_TARGET_DIR=target-test cargo test --workspace
```

- [ ] **Schritt 2: Self-Review**

- Spec §3.1 ActionDefinition Wire → Task 1 ✓
- Spec §3.2 ActionEvent Wire → Task 1 ✓
- Spec §3.3 SSE-Transport → Tasks 5, 6 ✓
- Spec §3.4 ActionRunStore + Replay → Task 2 + Task 5 (Snapshot-Replay) ✓
- Spec §3.5 Handler-Registry + Emitter → Task 3 ✓
- Spec §3.6 Modal → Task 11 ✓
- Spec §3.7 Permissions → TODO im start_handler (Schritt 1, AuthSession-Integration markiert)
- Spec §5 Edge-Cases: Cancel (Test in Task 6 ✓), Handler-Panic (TODO — Tokio task wrapper catch_unwind in einer F2-Folge), Client-Disconnect (Replay-Snapshot deckt), Backpressure (lagged-Filter im stream_handler ✓), Lange Logs (Reducer-Cap 500 in Task 9 ✓)
- Spec §6 Komponenten-Inventar → alle abgedeckt
- Spec §7 Tests → Tasks 1 (wire), 9 (reducer-unit), 6 (e2e), Task 11 Schritt 6 (manuell smoke)

- [ ] **Schritt 3: Commit Cleanup**

```
git diff --quiet || git commit -am "style: cargo fmt"
```

---

## Was später kommt

- AuthSession-Permission-Check im `/actions/start`-Handler (heute TODO)
- Tokio-Task-Wrapper `catch_unwind` für Handler-Panic-Schutz
- Replay-Lagged-Event-Recovery (heute: lagged → einfach skip; idealerweise: Client-Signal "Snapshot neu holen")
- Konkrete Handler T5/T10/T16 als eigene Specs
- Persistente `action_runs`-Tabelle (gehört zu F1)
- Plugin-getriebene Handler (Phase 2)
