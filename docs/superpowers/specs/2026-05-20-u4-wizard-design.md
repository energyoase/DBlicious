# U4 — Multi-Step-Wizard Spec

Date: 2026-05-20
Status: Draft — awaiting user review
Trigger: T16 (Reconciliation), T13 (CSV-Import-Mapping), T18 (Jahresabschluss-Setup).
Quelle: Gap-Analyse §4.1 U4, ROADMAP §1.8 U4.
Visual: `.superpowers/brainstorm/.../u4-wizard.html` — Variante B (vertikaler Stepper).
Abhängigkeiten: U2 (letzter Step = Save-Report) und/oder U6 (letzter Step = Long-Running-Action).

## 1. Ziel

Eine wiederverwendbare Mehr-Schritt-Komponente, die

- Schritte deklarativ aus `wizard.json` lädt (jeweils mit eigener Form/Validation/Computed-Input),
- Server-side State pro Wizard-Session hält (Inputs werden inkrementell akkumuliert),
- Computed Step-Inputs vom Server liefert (z.B. "schon vor Schritt 2 das Mapping-Detection rechnen"),
- am Ende dispatched auf einen U2-Save-Preview oder einen U6-Run.

Damit ist Wizard ein Hüllen-Pattern; Persistenz-/Berechnungs-Pfade bleiben in U2/U6.

## 2. Nicht-Ziele

- WYSIWYG-Wizard-Editor — `wizard.json` wird per Hand geschrieben (analog `editor.json` heute).
- Wizard-State-Persistierung über User-Sessions hinweg (Resumability erst Phase 2).
- Sub-Wizards / verschachtelte Wizards — flat linear sequence.
- Branching/Skip-Logic in Schritten — könnte später als `next_step: WizardCondition` ergänzt werden, heute YAGNI.

## 3. Architektur

### 3.1 `shared::WizardDefinition`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WizardDefinition {
    pub id: String,                          // "datev.csv-import"
    pub title_key: String,
    pub steps: Vec<WizardStep>,
    /// Dispatch-Ziel des letzten Schritts.
    pub completion: WizardCompletion,
}

pub struct WizardStep {
    pub id: String,
    pub title_key: String,
    pub description_key: Option<String>,
    /// JSON-Schema oder vereinfachte Field-Liste — first iteration:
    /// Liste von `WizardField` analog zu `EditorPropertyMeta`.
    pub fields: Vec<WizardField>,
    /// Computed-Inputs, die der Server vor Anzeige des Steps berechnet
    /// und in die Step-State injectet (z.B. "mappingDetection" für Step 2).
    #[serde(default)]
    pub computed: Vec<WizardComputed>,
    /// Optionaler Server-Validator (handler_id), der bei "Weiter" geprüft wird.
    #[serde(default)]
    pub validator_id: Option<String>,
}

pub struct WizardField {
    pub key: String,
    pub label_key: String,
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

pub struct WizardComputed {
    pub key: String,                          // "mappingDetection"
    pub handler_id: String,                   // server-registriert
    /// Welche bisherigen Step-Inputs der Handler braucht.
    pub depends_on_step: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WizardCompletion {
    /// Letzter Schritt erzeugt eine U2-SavePreview.
    SavePreview { entity_type: String },
    /// Letzter Schritt startet eine U6-Action.
    RunAction { action_id: String },
    /// Custom-Handler (return final result).
    Custom { handler_id: String },
}
```

### 3.2 Server-State: `WizardSession`

```rust
pub struct WizardSession {
    pub id: WizardSessionId,                 // UUID
    pub definition_id: String,
    pub current_step: usize,
    pub inputs: BTreeMap<String, serde_json::Value>,    // step_id → submitted fields
    pub computed: BTreeMap<String, serde_json::Value>,  // computed_key → result
    pub created_at: Instant,
    pub expires_at: Instant,                  // 1 h Default
}
```

In-memory `DashMap`, Pattern identisch zu U2-Preview-Store. Purge-Task identisch.

### 3.3 GraphQL-Schicht

```graphql
extend type Query {
    wizards: [WizardDefinition!]!
    wizard(id: String!): WizardDefinition
}

extend type Mutation {
    startWizard(definitionId: String!): WizardSessionView!
    submitWizardStep(sessionId: String!, stepId: String!, fields: JSON!): WizardSessionView!
    completeWizard(sessionId: String!): WizardCompletionResult!
    cancelWizard(sessionId: String!): Boolean!
}

type WizardSessionView {
    sessionId: String!
    currentStep: Int!
    inputs: JSON!
    computed: JSON!
    /// Nächster Step-Definition oder null wenn am Ende.
    nextStep: WizardStep
}

union WizardCompletionResult = SavePreviewResult | ActionRunStarted | CustomResult
```

`completeWizard` macht den Dispatch:

- `SavePreview { entity_type }` → ruft intern `previewCreateEntity(input: assembled)` und liefert das `SavePreviewResult` zurück. Client zeigt das U2-Modal.
- `RunAction { action_id }` → ruft intern `POST /actions/start` und liefert `{ run_id }`. Client öffnet U6-Modal.
- `Custom { handler_id }` → Aufruf eines registrierten Custom-Handlers, der ein freies JSON-Result liefert.

### 3.4 Client-Komponente `<Wizard>`

Pfad: `client/src/components/wizard/wizard.rs`.

Layout (Variante B aus Mockup):

```
┌─────────────────────┬──────────────────────┐
│ Vertikaler Stepper  │ Step-Header          │
│  ✓ Step 1 (Done)    │ ──────────────────── │
│  ✓ Step 2 (Done)    │ Step-Form            │
│  ▸ Step 3 (Active)  │   <FieldRenderer>×N  │
│    Step 4           │ ──────────────────── │
│                     │ ‹ Zurück  Weiter ›   │
└─────────────────────┴──────────────────────┘
```

- Stepper-Items zeigen `state` (Done/Active/Pending) + optional Sub-Label (z.B. "247 Zeilen · 3 Warn." aus `computed`).
- Step-Form benutzt das existierende `<FieldRenderer>` aus `components::field` — bestehende Editor-Logik wiederverwenden.
- "Weiter": ruft `submitWizardStep`. Wenn `validator_id` einen Fehler liefert, bleibt Wizard auf aktuellem Step.
- "Zurück": rein client-side, kein Server-Call (Inputs sind bereits serverseitig gespeichert).
- "Fertigstellen" (letzter Step): ruft `completeWizard` → öffnet U2- oder U6-Modal je nach Completion-Type.
- Modal-Close (X) → `cancelWizard` (Cleanup serverseitig).

### 3.5 Validator/Computed-Handler-Registry

```rust
#[async_trait]
pub trait WizardValidator: Send + Sync + 'static {
    async fn validate(
        &self,
        session: &WizardSession,
        step_id: &str,
        fields: &serde_json::Value,
    ) -> Result<ValidationResult, anyhow::Error>;
}

#[async_trait]
pub trait WizardComputer: Send + Sync + 'static {
    async fn compute(
        &self,
        session: &WizardSession,
        computed: &WizardComputed,
    ) -> Result<serde_json::Value, anyhow::Error>;
}
```

Pro Trait eine Registry (`HashMap<HandlerId, Arc<dyn ...>>`). Built-in Default-Validators: `nonEmpty`, `regex` (für simple Fälle). Domain-Validatoren (`csvMappingDetector` etc.) registriert sich beim Server-Start.

### 3.6 Routing

`/wizards/:wizard_id` — öffnet den Wizard direkt. Alternativ als Modal aus einer Entity-Liste/Operations-Liste (F1) gestartet.

## 4. Daten-Flow (Bank-CSV-Import T13)

```
User klickt "CSV importieren" (Entity-Action auf StarMoneyEntry)
  → Wizard "datev.csv-import" gestartet
  ↓ mutation startWizard("datev.csv-import")
Server: WizardSession angelegt, returned currentStep=0 + steps[0] Definition
  ↓
Step 0 (Quelle wählen): User lädt CSV-Datei hoch (Field: file-upload)
  ↓ submitWizardStep(session, "source", {file_blob_id: ...})
  ↓ Server: computed "mappingDetection" läuft (parsed Header)
  → returned currentStep=1 + steps[1] mit computed.mappingDetection ⊂ default-Werte
Step 1 (Spalten mappen): User korrigiert das Mapping
  ↓ submitWizardStep(session, "mapping", {mapping: {...}})
  ↓ validator_id "csvMappingValidator" prüft, ob alle required Spalten zugeordnet
  → returned currentStep=2 mit computed.previewRows = die ersten 20 Zeilen
Step 2 (Vorschau): User sieht Tabelle + Warnungen, klickt "Weiter"
  → returned currentStep=3 (Final-Step)
Step 3 (Import ausführen): User klickt "Fertigstellen"
  ↓ completeWizard(session)
  → WizardCompletion::RunAction { action_id: "datev.csv-import" } → POST /actions/start
  → Returned ActionRunStarted { run_id }
Client schließt Wizard, öffnet <ActionRunModal run_id=...>
```

## 5. Fehler-/Edge-Cases

- **Session expired** zwischen Steps → Server liefert strukturierten Error, Client zeigt "Wizard abgelaufen" + bietet Neustart.
- **Browser-Refresh**: Wizard-State im URL-Hash (`#wizard-session=<id>`) oder im localStorage. Bei Reload Re-Connect zur bestehenden Session.
- **Validator-Fehler**: bleibt am Step, zeigt Fehler-Banner. Inputs werden auf Server-Seite verworfen (nicht persistiert), Client-Seite behält Eingaben.
- **Computed-Handler-Fehler**: Wizard blockiert mit Retry-Button (in Step-Header).
- **Cancel im letzten Step**: nach `completeWizard` darf nicht mehr gecancelt werden — Cleanup erfolgt durch das nachgelagerte U2/U6-Modal.

## 6. Komponenten-Inventar

**Neu**:
- `shared/src/wizard.rs` — Definitions + Sessions.
- `server/src/wizard/store.rs`, `server/src/wizard/handlers.rs`.
- `server/src/example/loader.rs` — `wizards/<id>.{toml,json}` Branch.
- `client/src/components/wizard/wizard.rs`, `stepper.rs`, `step_form.rs`.
- `client/src/routes/wizard.rs` — Route `/wizards/:id`.
- `client/src/graphql/wizards.rs`.

**Erweitert**:
- `shared/src/menu.rs` — `MenuAction::OpenWizard(String)`.
- `client/src/components/navigation.rs` — Branch auf neue MenuAction.

## 7. Tests

- `shared/tests/wizard_wire.rs` — Roundtrip.
- `server/tests/wizard_flow.rs` — Linear-Flow happy-path; expired-Session; Validator-Reject; Cancel; Completion-Dispatch (SavePreview/RunAction) mit Stub-Mocks.
- `client` Unit: Stepper-State-Reducer; Step-Form-Submit.

## 8. Backwards-Compat

- Komplett additiv. Wenn `wizards/` Verzeichnis fehlt: leere Liste, kein Fehler.

## 9. Größe + Risiken

**Größe**: M. Sessions-Store + 4 Mutations + Wizard-Komponente + Stepper. Wiederverwendung von `<FieldRenderer>` spart viel.

**Risiken**:
- **Computed-Handler-Latenz**: wenn ein Step lang braucht (z.B. CSV-Parse von 50 MB), wird Wizard hakelig. Mitigation: für solche Schritte ist U6-Action das richtige Tool, nicht ein Wizard-Step. Dokumentations-Hinweis.
- **State-Drift**: bei Browser-Refresh muss URL/localStorage konsistent mit Server-Session sein. Mitigation: Server-Session ist Source-of-Truth, Client zeigt im Fehlerfall "Wizard abgelaufen".
- **`<FieldRenderer>`-Reuse**: heutiger Editor liest aus `EntitySettings` — Wizard liefert seine eigene Field-Liste. Adapter nötig. Lösbar, aber Code-Review-Punkt.

## 10. Spätere Erweiterungen

- Branching (`next_step` als Funktion über Inputs).
- Sub-Wizards (z.B. "Konto anlegen" inline aus einem Step).
- Wizard-as-Plugin (Phase 2): Plugin liefert Definition + Handler.
- Wizard-Builder-UI (Phase 1 Builder).

## 11. Decisions

1. **Hüllen-Pattern**: Wizard ist Sequenz von Forms + Dispatch; eigentliche Persistenz/Berechnung wandert in U2/U6. Saubere Trennung.
2. **Server-State, in-memory, 1 h-TTL**: Sessions sind kurzlebig genug, dass Persistenz unnötig ist.
3. **Vertikaler Stepper**: skaliert besser ab 4+ Steps, kann Sub-Status anzeigen.
4. **Computed-Handler statt direkter Server-Logik im Step-Submit**: deklarativ, testbar; Wizard bleibt generisch.
5. **`<FieldRenderer>`-Wiederverwendung**: keine doppelte Editor-Implementation.

## 12. Referenzen

- `shared/src/editor.rs` (`EditorPropertyMeta` als Field-Vorlage).
- `client/src/components/field/mod.rs` (`<FieldRenderer>`).
- ROADMAP §1.8 U4, Gap-Analyse §4.1 U4.
- D2V-Inspiration: `FunctionControls/FindDatevForStarMoney/{Search,Conflict,Confirmation,Select}.xaml`, `JA/Export.xaml`, `FullImportExport.xaml`.
