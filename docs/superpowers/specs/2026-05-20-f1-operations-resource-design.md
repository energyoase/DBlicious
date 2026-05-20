# F1 — Operations-Resource Spec

Date: 2026-05-20
Status: Draft — awaiting user review
Trigger: T1, T10 (Invert), T13 (CSV-Import), T16 (Reconciliation) — generelle Cross-Entity-Operationen aus D2V's `FunctionControls/`.
Quelle: Gap-Analyse §4.2 F1, ROADMAP §1.8 F1.
Visual: `.superpowers/brainstorm/.../f1-operations.html` (Nav-Bereich + Detail-Seite).
Abhängigkeiten: **U6** (Ausführungs-Pipeline).

## 1. Ziel

Operations (Cross-Entity-Logiken wie "Account-Entries regenerieren", "Generalumkehr", "Reconciliation Bank↔DATEV") sind **keine Entities** — sie haben keine CRUD-Semantik. F1 hebt sie zu einer **eigenständigen Top-Level-Resource** neben `entities` und `reports`:

- eigener Navigations-Bereich,
- Detail-Seite mit Input-Form (deklariert via `input_schema`) + Run-History + "Ausführen ▾",
- GraphQL-Query `operations` parallel zu `entities` / `reports`.

Operations selbst sind exakt die in U6 definierten `ActionDefinition`s — F1 ist die **Sichtbarkeitsschicht** für Actions, U6 ist der **Ausführungspfad**.

## 2. Nicht-Ziele

- Eigene Ausführungs-Mechanik — sind in U6 abgedeckt.
- Cron/Scheduling von Operations — Phase 2 Job-Scheduler.
- Composition von Operations (DAG, "diese Op nach jener") — YAGNI.

## 3. Architektur

### 3.1 Beziehung zu U6

U6 definiert `ActionDefinition` + `ActionHandler` + Ausführungs-Pipeline. Bisher werden Actions im Kontext einer Entity-Liste sichtbar (`row_actions`, `top_menu`, `entity_menu`).

F1 erweitert das um eine zweite Sichtbarkeits-Achse: **standalone als Top-Level-Resource**. Eine `ActionDefinition` mit `kind: Global` (und optional `EntityAction`, wenn auch ohne Entity-Kontext sinnvoll) erscheint im Operations-Bereich.

### 3.2 GraphQL-Schicht

```graphql
extend type Query {
    operations(filter: OperationFilter): [OperationView!]!
    operation(id: String!): OperationView
}

type OperationView {
    id: String!
    titleKey: String!
    descriptionKey: String
    category: String                  # "datev" | "starmoney" | "report" | "admin"
    kind: ActionKind!                  # aus U6
    inputSchema: JSON
    estimatedDurationSeconds: Int
    permissions: [String!]!
    /// Letzte Runs (bis zu 10), aus dem U6-`ActionRunStore`-History.
    recentRuns: [OperationRunSummary!]!
}

type OperationRunSummary {
    runId: String!
    startedAt: String!
    completedAt: String
    status: String!                   # "running" | "completed" | "failed" | "cancelled"
    durationMs: Int
    initiatedBy: String!              # Username
}

input OperationFilter {
    category: String
    kind: String
}
```

Server-seitig: in `data.rs::operations()` werden alle registrierten `ActionDefinition` mit Permission-Filter (`AuthSession`) und Category-Filter zurückgegeben.

### 3.3 Run-History

U6 hält Runs als `DashMap<RunId, ActionRunHandle>` mit 30-min-TTL. Für F1 nicht ausreichend. **Eigene `operation_runs` SeaORM-Tabelle**:

```rust
pub struct operation_run::Model {
    pub run_id: String,
    pub operation_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub initiated_by: String,
    pub input_json: Json,                 // Original-Input für Re-Run
    pub final_report_json: Option<Json>,  // Final-Report nach Completion
    pub error: Option<String>,
}
```

U6's `ActionHandler` läuft → schreibt `Started`-Row, bei Completed/Failed → Update der Row. Die in-memory `ActionRunHandle` bleibt nur für aktive Live-Streams.

Damit ist die Run-History persistent und überlebt Server-Restarts.

### 3.4 Client-Routen + UI

- `/operations` — Liste aller sichtbaren Operations, gruppiert nach `category`.
- `/operations/:id` — Detail-Seite (Mockup):
  - Header: Titel + Description + Permission/Duration-Metadata
  - Input-Form: generiert aus `inputSchema` via `<FieldRenderer>` (Wiederverwendung).
  - Action-Buttons: "Ausführen ▾" (öffnet U6-Modal); "Letzten Run ansehen" → öffnet `/operations/:id/runs/:runId`.
  - Run-History-Tabelle: Liste der `recentRuns` (Filter auf Status, User, Datum). Klick auf Run öffnet:
- `/operations/:id/runs/:runId` — Read-only-Sicht eines abgeschlossenen Runs (zeigt `final_report` aus DB), wiederverwendet das U6-Modal im "completed"-Mode.

### 3.5 Navigation

Erweiterung der `NavigationNode`/`MenuAction`-Schicht:

```rust
pub enum MenuAction {
    // … bestehende
    OpenOperation(String),
    OpenOperationsList,
}
```

Loader liest optional `operations.toml` aus `--data-dir`, dort werden Categories und Display-Reihenfolge konfiguriert (analog `navigation.toml`).

### 3.6 Re-Run

"Letzten Run wiederholen" ist nur ein UI-Helper, der das gespeicherte `input_json` in das Formular vorbefüllt — keine eigene Server-Mutation. Vermeidet "Re-Run mit veralteten Inputs"-Foot-Guns: User muss bewusst "Ausführen" klicken.

## 4. Daten-Flow (Operations-Liste)

```
User navigiert nach /operations
  ↓ client-query operations(filter)
Server:
  • holt alle ActionDefinitions aus ActionRegistry
  • filter durch AuthSession.permissions
  • merged recentRuns(operation_id) aus operation_runs-Tabelle
  ← OperationView[]
Client rendert Liste, gruppiert nach category.

User klickt eine Operation
  ↓ client-query operation(id)
Server returns Detail inkl. recentRuns(operation_id) limit 10
Client rendert /operations/:id

User klickt "Ausführen"
  → POST /actions/start { action_id, input } (U6-Pfad)
  → operation_runs-Row insert
  → Client öffnet <ActionRunModal>
Stream läuft (U6), Final-Event:
  → operation_runs-Row update mit Completed/Failed + final_report_json
```

## 5. Fehler-/Edge-Cases

- **Permission fehlt**: Operation taucht nicht in der Liste auf, Direkt-URL liefert 403.
- **Operation deregistriert (z.B. nach Plugin-Disable)**: Detail-Seite zeigt "Operation nicht verfügbar"; Run-History bleibt aber sichtbar (Historisches Audit).
- **`input_schema` fehlt**: Detail-Seite zeigt nur "Ausführen"-Button ohne Form (z.B. `RegenerateAll` ohne Parameter).
- **Run-History übergroß**: `recentRuns` limited; volle History via separater Pagination-Query `operationRuns(operationId, page, pageSize)`.

## 6. Komponenten-Inventar

**Neu**:
- `server/src/entity/operation_run.rs` — SeaORM-Modell.
- `server/src/db.rs` — Tabellen-Erstellung erweitern.
- `server/src/operations.rs` — Query-Resolver + Run-History-Persistenz-Hooks.
- `server/src/example/loader.rs` — `operations.toml` Branch (Categories/Order).
- `shared/src/operation.rs` — `OperationView`, `OperationRunSummary` (Wire-Format identisch zur GraphQL-Definition).
- `client/src/routes/operations.rs` — Routes `/operations`, `/operations/:id`, `/operations/:id/runs/:runId`.
- `client/src/components/operations/list.rs`, `detail.rs`, `run_history.rs`.

**Erweitert**:
- `shared/src/menu.rs` — `MenuAction::OpenOperation`, `OpenOperationsList`.
- `server/src/schema.rs` — `operations`, `operation` Queries.
- `client/src/components/navigation.rs` — Branch auf neue MenuActions.
- `client/src/components/action/action_run_modal.rs` (U6) — "completed"-Mode für read-only Run-Sicht.

## 7. Tests

- `shared/tests/operation_wire.rs` — Roundtrip.
- `server/tests/operations.rs` — Query mit Permission-Filter; Run-History wird bei Action-Completion persistiert; Detail-Query mit `recentRuns`.
- Integration: Stub-Handler "echo-op" registriert; Detail-Seite zeigt Run-History; Re-Open eines Runs zeigt persistierten Final-Report.

## 8. Backwards-Compat

- Vollständig additiv. Operations-Bereich erscheint nur, wenn `ActionDefinition`s registriert sind.
- `operation_runs`-Tabelle wird im `db::init` idempotent angelegt.

## 9. Größe + Risiken

**Größe**: M-L. Persistenz-Layer + Routes + Detail-UI + Run-History-View.

**Risiken**:
- **Permission-Modell**: Operations sind nicht entity-gebunden — heutiges `Permission { entity_type, ... }`-Schema passt nicht 1:1. Mitigation: neuer Permission-Slot `entity_type = "operation:<id>"` (String-Konvention), bestehender Mechanismus tut's. Doku-Hinweis.
- **Run-History-Tabelle-Wachstum**: ohne Cap wächst sie unbegrenzt. Mitigation: Daily-Purge-Task (älter als N Tage) konfigurierbar; in dieser Spec nicht implementiert (Backlog-Item).
- **Race bei Concurrent-Runs**: zwei User starten dieselbe Op gleichzeitig — U6 erlaubt das, F1 zeigt beide in der History. Keine Locks; Operations müssen idempotent sein (Doku-Anforderung). Wenn nicht idempotent → ActionHandler kann selbst prüfen und Failed-Event mit "Bereits aktiv"-Message liefern.

## 10. Spätere Erweiterungen

- Cron-Scheduling pro Operation (verbindet sich mit Phase 2.7 Job-Scheduler).
- Plugin-getriebene Operations (Phase 2): Plugin liefert `ActionDefinition` + Handler.
- Composition: "Op B startet automatisch nach Op A", als deklarative Pipeline.
- Approval-Workflow (Operation-Run muss freigegeben werden).

## 11. Decisions

1. **F1 = Sichtbarkeitsschicht über U6** — kein neuer Ausführungspfad.
2. **Persistente `operation_runs`-Tabelle** — Run-History überlebt Server-Restart und macht Audit/Re-Run sinnvoll.
3. **`MenuAction::OpenOperation`** als neues Nav-Item statt eigene Route-Konvention — konsistent mit Entities/Reports.
4. **`input_schema` als JSON-Schema-Subset** — gerendert via `<FieldRenderer>` analog zu Editor/Wizard.
5. **Operations als String-Permission "operation:<id>"** — keine Schema-Änderung am Permission-Typ.

## 12. Referenzen

- U6 Spec — `ActionDefinition`, `ActionKind`, `ActionReport`.
- `server/src/entity/` — SeaORM-Pattern.
- `client/src/components/field/` — `<FieldRenderer>` für Input-Form.
- ROADMAP §1.8 F1, Gap-Analyse §4.2 F1.
- D2V-Inspiration: `FunctionControls/` (insbes. `PublishDatev.xaml`, `CalcStarMoney.xaml`, `GenerateDatevExternalIds.xaml`, `FindDatevInversePairs.xaml`, `FullImportExport.xaml`).
