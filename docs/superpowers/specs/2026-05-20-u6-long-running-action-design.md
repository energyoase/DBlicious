# U6 — Long-Running-Action-Pipeline Spec

Date: 2026-05-20
Status: Draft — awaiting user review
Trigger: T5 (`GenerateAccountEntries`-Bulk), T10 (Invert), T16 (Reconciliation).
Quelle: Gap-Analyse §4.1 U6, ROADMAP §1.8 U6.
Visual: `.superpowers/brainstorm/.../u6-action-progress.html`, Variante B (Progress + Live-Log).

## 1. Ziel

Eine wiederverwendbare Pipeline für Operationen, die

- länger als ~2 s dauern,
- inkrementellen Fortschritt liefern können (Schritte, Counts, Logs),
- am Ende einen strukturierten Bericht produzieren (Counts + Warnings + Failure-Details),
- abbrechbar sein müssen,
- pro Row/Selection/Entity triggerbar sein können.

Die Pipeline ist **die einzige Stelle**, an der "Action mit Progress" implementiert wird — alle konkreten Operations (T5 Regen, T10 Invert, T16 Reconcile) hängen sich daran an.

## 2. Nicht-Ziele

- Persistenz/Resumability nach Server-Crash (Phase 2/Job-Scheduler).
- Hintergrund-Jobs ohne aktive UI-Session ("Im Hintergrund laufen lassen") — Button im Modal ist disabled in dieser Spec.
- Globale Operations-Liste in Navigation (das ist F1).
- Action-Definitionen aus Plugins (Phase 2 ergänzt das).

## 3. Architektur

### 3.1 `shared::ActionDefinition`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionDefinition {
    pub id: String,                       // "datev.regenerate-account-entries"
    pub title_key: String,                 // i18n-Schlüssel
    pub description_key: Option<String>,
    pub kind: ActionKind,
    pub input_schema: Option<serde_json::Value>, // optional JSON-Schema
    pub permissions: Vec<String>,           // required Permission-IDs
    pub estimated_duration_seconds: Option<u32>, // grobe Erwartung für UI
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActionKind {
    RowAction,      // einzelne Entity (z.B. T10 Invert eine Buchung)
    BulkAction,     // Mehrfachauswahl
    EntityAction,   // ganze Tabelle (T5 Regen für alle)
    Global,         // ohne Entity-Bezug (T16 Reconcile bank vs. datev)
}
```

### 3.2 `shared::ActionEvent` (Wire-Format für Stream)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ActionEvent {
    /// Initial-Event, einmalig.
    Started { run_id: String, total_steps: Option<u32> },
    /// Schritt-Wechsel.
    Step { index: u32, title_key: String },
    /// Inkrementelles Counter-Update innerhalb eines Schritts.
    Progress { current: u64, total: Option<u64>, message_key: Option<String> },
    /// Freitext-Log (i18n-Key oder Plain). Severity bestimmt die Farbe.
    Log { severity: LogSeverity, message: String },
    /// Warnung mit strukturiertem Bezug auf eine Entity (für Final-Report).
    Warning { target: ActionTarget, message_key: String, message_args: serde_json::Value },
    /// Abschluss-Event mit Final-Report.
    Completed { report: ActionReport },
    /// Abbruch durch User oder Fehler.
    Failed { error: String, partial_report: ActionReport },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogSeverity { Info, Warn, Error }

pub struct ActionTarget {
    pub entity_type: String,
    pub id: String,
}

pub struct ActionReport {
    pub duration_ms: u64,
    pub counts: BTreeMap<String, i64>,    // generisch: "processed", "created", "skipped", …
    pub warnings: Vec<ActionWarning>,
    pub artifacts: Vec<ActionArtifact>,   // optionale Dateien (Export-Logs)
}

pub struct ActionWarning {
    pub target: ActionTarget,
    pub message: ValidationMessage,
}

pub struct ActionArtifact {
    pub label_key: String,
    pub url: String,                       // download-Endpoint
    pub mime: String,
}
```

`ActionEvent` ist eine getaggte Union — Pattern wie `FieldType`. `tag = "type"` macht Wire-JSON klar (`{"type":"progress","current":620,"total":1000}`).

### 3.3 Server-Side Transport: SSE

GraphQL hat in dieser Codebase keine Subscriptions (keine Infrastruktur). Wir nutzen **Server-Sent Events** über einen klassischen HTTP-Endpoint:

```
POST /actions/start
  Body: { action_id, input }
  → { run_id }

GET  /actions/stream/:run_id          # SSE-Stream
  events: data: {"type":"started",...}\n\n

POST /actions/cancel/:run_id
  → { ok }
```

Begründung: SSE ist nativ in axum (`tower-http`/`axum::response::sse`), kein WebSocket-Setup; Reverse-Proxy-tauglich; Client-Implementation in `web-sys` straight forward. Subscriptions als GraphQL-Feature würde eine zweite Resolver-Achse und ein Connection-Lifecycle einbauen, das wir nicht brauchen.

Action selbst läuft als Tokio-Task auf dem Server; die Task pusht in einen `tokio::sync::broadcast::Sender<ActionEvent>`. Der SSE-Handler subscribes und streamt jedes Event raus.

### 3.4 Server-State: `ActionRunStore`

```rust
pub struct ActionRunStore {
    runs: DashMap<RunId, ActionRunHandle>,
}

pub struct ActionRunHandle {
    pub started_at: Instant,
    pub cancel: CancellationToken,
    pub sender: broadcast::Sender<ActionEvent>,
    pub final_report: ArcSwapOption<ActionReport>,
}
```

TTL: 30 min nach `Completed`/`Failed` — Re-Connect aus einem zweiten Tab kann letztes Event + Final-Report nachholen (Client kann Replay anfordern).

**Replay**: Sender hat `capacity = 256`. SSE-Endpoint sendet vor Subscribe einmalig den letzten bekannten Snapshot (gecachet: Started + Current-Step + Progress + bisherige Warnings). Reicht, um eine Modal-Re-Mount nicht "leer" zu lassen.

### 3.5 ActionHandler-Registry

```rust
#[async_trait]
pub trait ActionHandler: Send + Sync + 'static {
    fn definition(&self) -> ActionDefinition;
    async fn run(
        &self,
        ctx: ActionContext,
        input: serde_json::Value,
        emitter: ActionEmitter,
    ) -> Result<ActionReport, ActionError>;
}

pub struct ActionEmitter { /* wrapper über broadcast::Sender */ }
impl ActionEmitter {
    pub fn started(&self, total_steps: Option<u32>);
    pub fn step(&self, index: u32, title_key: &str);
    pub fn progress(&self, current: u64, total: Option<u64>, msg: Option<&str>);
    pub fn log(&self, severity: LogSeverity, msg: impl Into<String>);
    pub fn warning(&self, target: ActionTarget, message_key: &str, args: serde_json::Value);
}
```

Registry: `HashMap<ActionId, Arc<dyn ActionHandler>>`. Built-in Default Handler `noop` für Tests. Spätere konkrete Handler (T5 etc.) registrieren sich beim Server-Start.

### 3.6 Client-Komponente `<ActionRunModal>`

Datei: `client/src/components/action/action_run_modal.rs`.

Open-Trigger:
- Row-Action mit `action_id` (existing `row_actions.rs` → erweitert um `kind: ActionKind::RowAction`)
- Bulk-Selektion: neues `bulk_actions` Top-Bar-Element
- Entity-Action: `entity_menu.rs` → "Aktionen ▾"-Dropdown
- Global Action: F1 (eigene Spec) liefert den Einstieg

UI (Variante B aus Mockup):
1. Header: Title + Status-Pill (`RUNNING` / `DONE` / `FAILED`).
2. Progress-Bar mit `current/total` + Schritt-Indikator.
3. Live-Log-Pane (scrollend, autoscroll wenn am Ende).
4. Footer: `Cancel` (während Running) + `Schließen` (nach Done).
5. Nach Completed: blendet Final-Report-Sektion ein (Counts-Banner + Warning-Tabelle mit "Öffnen ↗" Deep-Link, Download-Buttons für Artifacts).

Lifecycle:
- Modal-Open → `POST /actions/start` → `run_id` → öffnet SSE-Stream.
- `EventSource.onmessage` parst JSON → Reduzer aktualisiert State (`current_step`, `progress`, `log_lines`, `warnings`).
- Cancel-Button → `POST /actions/cancel/:run_id` (server CancellationToken).
- Final → Footer-Button "Schließen" (kein Cancel).

### 3.7 Berechtigungen

`ActionDefinition.permissions` ist eine Liste von Permission-IDs (Format wie `Permission` aus `shared::security`). Server prüft beim `/actions/start` via `AuthSession`. Client filtert Action-Liste analog zu CRUD-Aktionen.

## 4. Daten-Flow

```
User klickt "Regenerate Account Entries" (Entity-Action auf Tabelle DatevEntry)
  ↓ Modal öffnet → POST /actions/start { action_id, input }
Server:
  • ActionRunStore.spawn(action_id, input)
  • Spawn Tokio-Task: handler.run(ctx, input, emitter)
  • Emitter pushed ActionEvent in broadcast
  ← { run_id: "uuid" }
Client öffnet EventSource(/actions/stream/uuid)
  • Started → setze total_steps
  • Step → setze current step
  • Progress → bar + label
  • Log → push log line
  • Warning → push log line + sammle für Final-Report
  • Completed → Modal mode = "done", show report
User klickt "Cancel"
  → POST /actions/cancel/uuid
  → CancellationToken.cancel() → handler.run muss kooperativ canceln und Failed-Event emitten
```

## 5. Fehler-/Edge-Cases

- **Handler-Panic**: Tokio-Task wrapper fängt panic, emittiert `Failed { error: "internal" }`. Server-Log enthält den Stack.
- **Client-Disconnect**: Handler läuft weiter; auf Re-Connect bekommt der Client den Snapshot + folgende Events.
- **Server-Restart**: Run weg. Akzeptabel (kein Persistenz-Ziel). Wenn UI sich wieder verbindet und `run_id` unbekannt ist, zeigt es "Verbindung verloren".
- **Cancel nach Completed**: idempotent, `ok=true` ohne Effekt.
- **Mehrere Tabs subscribe gleicher run_id**: SSE multicastet via broadcast — alle bekommen Updates.
- **`total: None`-Progress**: UI zeigt indeterminate Progress-Bar.
- **Lange Log-Streams**: Modal limitiert Anzeige auf letzte 500 Lines; Download-Button "Log exportieren" lädt das volle Log aus `ActionReport.artifacts` (Handler legt es ab).

## 6. Komponenten-Inventar

**Neu**:
- `shared/src/action.rs` — `ActionDefinition`, `ActionEvent`, `ActionReport`, …
- `server/src/actions/store.rs` — `ActionRunStore`
- `server/src/actions/handler.rs` — Trait + Registry
- `server/src/actions/sse.rs` — HTTP-Routes `/actions/*`
- `client/src/components/action/action_run_modal.rs`
- `client/src/components/action/progress_view.rs` — Progress-Bar + Log-Pane (re-usable)
- `client/src/graphql/actions.rs` — REST-Adapter (POST start/cancel, SSE-Subscribe)

**Erweitert**:
- `server/src/main.rs` — Routes registrieren, Store provisionieren
- `server/src/schema.rs` — neue Query `availableActions(target?: ActionTarget): [ActionDefinition]` (für UI-Discovery)
- `client/src/components/table/row_actions.rs` — Branch auf `ActionKind::RowAction`
- `client/src/components/table/top_menu.rs` — Bulk-Actions
- `client/src/components/table/entity_menu.rs` — Entity-Actions

## 7. Tests

- `shared/tests/action_event_wire.rs` — Roundtrip aller `ActionEvent`-Varianten.
- `server/tests/action_run.rs` — Happy-Path mit einem Test-Handler "echo-progress" (10 Schritte, kontrollierbarer Stream); Cancel mitten im Run; Concurrent Subscribers; Handler-Panic.
- Client-Unit: SSE-Reducer-Tests (gegebene Eventsequenz → State).
- E2E (manuell): T5-Stub registriert, Modal öffnen, Cancel/Done.

## 8. Backwards-Compat

- Neue REST-Routes additiv; kein Bestandsfeature betroffen.
- `availableActions`-Query: ohne registrierte Handler liefert leeres Array.

## 9. Größe + Risiken

**Größe**: M. Server-State + 3 REST-Routes + Wire-Typen + Modal mit Stream-Reducer.

**Risiken**:
- **CancellationToken-Disziplin**: Handler müssen kooperativ cancellable sein. Lint/Doku-Check beim Schreiben jedes Handlers.
- **Broadcast-Backpressure**: bei langsamen Clients überlaufen Channel-Slots → Events verloren. Mitigation: Modal-Reducer akzeptiert "Gap"-Hinweis und holt Snapshot neu.
- **SSE über Reverse-Proxies**: nginx + buffering muss disabled sein. Doku-Hinweis.
- **GraphQL-Mismatch**: Actions sind nicht in GraphQL ⇒ Auth-Header (`Authorization: Bearer …`) muss vom Client gesetzt werden; heutiger GraphQL-Client kennt den Pfad. Adapter dokumentiert.

## 10. Spätere Erweiterungen

- "Im Hintergrund laufen lassen" + Notifications (Phase 2-Job-Scheduler).
- Persistente `action_runs`-Tabelle (für Audit-Trail).
- Plugin-getriebene Actions (Phase 2): Plugin-Manifest registriert `ActionHandler`-WASM.
- Globale "Aktivität"-Liste in der Navigation (verbindet sich mit F1).

## 11. Decisions

1. **SSE statt GraphQL-Subscription**: kein Subscription-Setup nötig, einfacher.
2. **In-memory Store + 30-min-TTL**: Resumability ist Phase-2-Topic.
3. **Variante B-UI** (Progress + Live-Log): T5/T16-Operations brauchen Live-Warnings.
4. **`ActionDefinition` als Wire-Typ**: erlaubt es F1 (Operations-Resource) später, dieselben Definitionen als eigenständige Top-Level-Resource zu navigieren.

## 12. Referenzen

- `shared/src/security.rs::Permission` (Berechtigungs-Pattern)
- `shared/src/validation.rs::ValidationMessage` (Warning-Pattern)
- `server/src/main.rs` (Route-Registration)
- `client/src/components/table/row_actions.rs`
- ROADMAP §1.8 U6, Gap-Analyse §4.1 U6
- D2V-Inspiration: `FunctionControls/CalcStarMoney.xaml`, `FunctionControls/PublishDatev.xaml`, `ContextControls/UpdateDatevAccountEntries.xaml`
