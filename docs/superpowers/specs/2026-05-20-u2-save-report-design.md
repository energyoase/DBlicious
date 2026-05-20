# U2 — Save-Report (Preview-vor-Commit) Spec

Date: 2026-05-20
Status: Draft — awaiting user review
Trigger: T5/T6 (`GenerateAccountEntries`), T15 (Bulk-Import).
Quelle: Gap-Analyse §4.1 U2, ROADMAP §1.8 U2.
Visual: `.superpowers/brainstorm/.../u2-save-report.html`, Variante B (Domain-Tabellen, semantisch gruppiert).

## 1. Ziel

Jede Mutation (`create`/`update`/`delete` + Bulk-Operationen) kann optional erst eine **Preview** liefern, die exakt zeigt:

- welche **Eigentliche** Entity wird geändert (alt/neu pro Feld),
- welche **abgeleiteten** Rows entstehen, verändern oder verschwinden (z.B. computed AccountEntries aus `GenerateAccountEntries`),
- welche **Side-Effects** der Server warnt (z.B. "Buchung ist published — es wird automatisch storniert").

Nach User-Bestätigung wird die Preview commited; ohne Bestätigung verfällt sie serverseitig nach kurzer Zeit.

## 2. Nicht-Ziele

- Generische "Time-Travel"-Funktion (jede Mutation rückgängig machbar): kein Snapshot-Speicher.
- Long-Running-Berechnungen mit Progress-UI — die deckt U6 ab. Preview ist synchron, ≤ ~3 s.
- UI-Builder-Integration des Preview-Modals — ist eigenständige Komponente.

## 3. Architektur

### 3.1 Server-State: `PreviewSession`

Eine Preview ist eine kurzlebige Server-Session mit:

```rust
pub struct PreviewSession {
    pub id: PreviewId,           // UUID
    pub entity_type: String,
    pub kind: PreviewKind,       // Create | Update | Delete | Bulk
    pub input: serde_json::Value, // original Request-Payload (für Re-Commit)
    pub diff: SavePreview,        // Diff-Daten
    pub warnings: Vec<ValidationMessage>,
    pub expires_at: Instant,     // 5 Minuten Default
}
```

In-Memory `DashMap<PreviewId, PreviewSession>` in `server/src/preview_store.rs`. Hintergrund-Task `purge_expired()` (Tokio-Interval 60 s). Keine Persistenz — eine Preview ist nur für die Dauer der User-Entscheidung gedacht.

### 3.2 `shared::SavePreview` (Wire-Format)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavePreview {
    pub preview_id: String,
    pub primary: EntityDiff,
    /// Computed/abgeleitete Rows pro Entity-Typ.
    /// Map(entity_type → [DerivedRow]).
    pub derived: BTreeMap<String, Vec<DerivedRow>>,
    /// Pure Validation-/Warn-Messages, die der Commit zulassen würde.
    pub warnings: Vec<ValidationMessage>,
    /// Counts pro Operation; vom Client als Header-Pill gerendert.
    pub counts: PreviewCounts,
}

pub struct EntityDiff {
    pub entity_type: String,
    pub id: Option<String>,           // None bei Create vor ID-Vergabe
    pub kind: DiffKind,               // Create | Update | Delete
    /// Pro Feld: nur die geänderten/neuen, nicht die unveränderten.
    pub fields: Vec<FieldDiff>,
}

pub struct FieldDiff {
    pub key: String,
    pub old: Option<serde_json::Value>,
    pub new: Option<serde_json::Value>,
}

pub struct DerivedRow {
    pub kind: DiffKind,
    pub id: Option<String>,
    pub fields: serde_json::Map<String, serde_json::Value>,
    /// Optionaler Grund, warum diese Row entsteht.
    /// Beispiel: "USt-Split", "Hauptbuchung", "Bank-Gegenkonto".
    pub reason_key: Option<String>,
}

pub struct PreviewCounts {
    pub creates: u32,
    pub updates: u32,
    pub deletes: u32,
}
```

### 3.3 GraphQL-Schicht

Neue Mutations:

```graphql
extend type Mutation {
    previewCreateEntity(input: EntityCreateInput!): SavePreviewResult!
    previewUpdateEntity(input: EntityUpdateInput!): SavePreviewResult!
    previewDeleteEntity(input: EntityDeleteInput!): SavePreviewResult!
    commitPreview(previewId: String!): EntityChangeResult!
    cancelPreview(previewId: String!): Boolean!
}

type SavePreviewResult {
    ok: Boolean!
    preview: SavePreview
    /// Bei ok=false: blockierende Validation-Errors (kein Preview erzeugt).
    validation: JSON!
}
```

Die drei `preview*Entity`-Mutationen rufen denselben Validation-Pfad wie die jeweiligen `*Entity`-Schreib-Mutationen, gehen aber bis vor das Persistenz-Commit, sammeln dort Diff + Derived und legen die `PreviewSession` an. `commitPreview` re-runs den Persistenz-Pfad mit dem gespeicherten Input — nicht aus dem Diff (Verträge bleiben klar).

**Bewusste Entscheidung**: `commitPreview` validiert **erneut**. Damit kann ein zwischenzeitlicher Konflikt (anderer User hat dieselbe Row geändert) sauber gemeldet werden. Die Preview ist eine **Vorschau**, kein zugesicherter Snapshot.

### 3.4 Computed/Derived-Rows

Heutiger Server kennt keine "abgeleiteten" Rows — jede Mutation schreibt explizit. Damit Phase-1.8-Specs nicht alle DB-Triggers vorausgreifen, definieren wir:

- Ein Entity-Settings-Feld `derive_handler: Option<String>` deklariert, welcher Server-seitige Handler für diese Entity in den Preview hineinläuft.
- Heute eingebauter Handler: `noop` (nur primärer Diff). Späterer Trigger T5 (`GenerateAccountEntries`) registriert sich als Handler `datev-account-entries`.
- Plugin-Triggers (Phase 2) können Handler liefern (`before_save` mit Capability `preview`).

Damit ist diese Spec ohne T5/T6 funktionsfähig (zeigt nur den primären Diff), und der Erweiterungsweg ist klar.

### 3.5 Client-Komponente `<SavePreviewModal>`

Datei: `client/src/components/preview/save_preview_modal.rs`.

Inputs: `SavePreview`, plus Callbacks `on_commit` / `on_cancel`. Layout aus Variante B des Mockups:

1. Header-Pill mit Counts.
2. Section "Primary": Tabelle mit `Feld | Alt | Neu` (Alt durchgestrichen, Neu fett).
3. Section per Derived-Entity-Typ: Tabelle mit den wichtigsten Spalten aus `ColumnMeta` für diesen Typ; zusätzlich `reason_key` lokalisiert als letzte Spalte.
4. Warnings-Block: gelbe Boxen mit `ValidationMessage` Severity ≥ Warning.
5. Action-Footer: `Cancel` (ruft `cancelPreview`) / `Speichern` (ruft `commitPreview`).

Der Editor-Flow ändert sich so:

- Heute: `EditorView::on_save` → `mutate(EntityCreate|Update|Delete)`.
- Neu: wenn `EntitySettings.preview_on_save = true` (Default `false` für minimale Disruption) → erst `previewCreateEntity` / `previewUpdateEntity` / `previewDeleteEntity` (je nach Operation) → bei Erfolg Modal öffnen.
- Modal-Bestätigung → `commitPreview`. Modal-Cancel oder Modal-Close → `cancelPreview` (Cleanup).

### 3.6 EntitySettings-Erweiterung

```rust
#[serde(default)]
pub preview_on_save: bool,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub derive_handler: Option<String>,
```

Heute beide `false`/`None`. T5/T6-Roll-out schaltet sie für `DatevEntry` an.

## 4. Daten-Flow

```
User klickt "Save" im Editor (DatevEntry)
  ↓ EntitySettings.preview_on_save = true
  ↓ Client mutate previewUpdateEntity({entity_type, id, fields, expectedHash})
Server:
  • validiert Input (gleicher Pfad wie save)
  • ruft derive_handler "datev-account-entries"
  • baut SavePreview { primary, derived["DatevAccountEntry"]=[...], warnings, counts }
  • DashMap.insert(previewId, PreviewSession { input, diff, expires_at: now+5min })
  ← SavePreviewResult { ok: true, preview }
Client zeigt <SavePreviewModal preview=...>
  • User klickt "Speichern"
  ↓ mutate commitPreview(previewId)
Server:
  • holt PreviewSession aus DashMap (oder Error wenn expired)
  • re-validiert + persistiert mit gespeichertem Input
  • DashMap.remove(previewId)
  ← EntityChangeResult { ok, entity, validation }
Client schließt Modal, refresht Tabelle (oder zeigt Fehler).
```

## 5. Fehler-/Edge-Cases

- **Preview expired**: Server gibt strukturierten Error → Modal zeigt "Vorschau abgelaufen, bitte erneut versuchen", schließt sich.
- **Validation-Error bei `previewSave`**: `ok=false`, kein Modal, Editor markiert Felder wie heute (Validation-Result wird zurückgegeben).
- **Validation-Error bei `commitPreview` (Race)**: Modal bleibt offen, zeigt Warning-Block neu, User entscheidet erneut.
- **Server crasht zwischen Preview + Commit**: alle Previews futsch (in-memory). Akzeptabel, da Preview-Sessions kurzlebig.
- **Client-Disconnect ohne Cancel**: TTL räumt auf (5 min).
- **Multi-Tab**: jede Tab hat eigene PreviewId — keine Kollisionen.

## 6. Komponenten-Inventar

**Neu**:
- `shared/src/preview.rs` — `SavePreview`, `EntityDiff`, `DerivedRow`, `FieldDiff`, `PreviewCounts`, `DiffKind`.
- `server/src/preview_store.rs` — `DashMap<Uuid, PreviewSession>` + TTL-Purger.
- `server/src/preview.rs` — Handler-Registry `derive_handler` + Default-Noop.
- `client/src/components/preview/save_preview_modal.rs` — Modal.
- `client/src/components/preview/diff_table.rs` — generische Diff-Tabelle (für Primary + Derived-Sections).

**Erweitert**:
- `shared/src/settings.rs` — `EntitySettings.preview_on_save`, `derive_handler`.
- `server/src/schema.rs` — neue Mutations.
- `client/src/graphql/queries.rs` — `preview_create_entity`, `preview_update_entity`, `preview_delete_entity`, `commit_preview`, `cancel_preview`.
- `client/src/components/field/editor.rs` (`EditorView`) — Branch auf `preview_on_save`.
- i18n-Keys `preview-*`.

## 7. Tests

- `shared/tests/save_preview_wire.rs` — Roundtrip aller Diff-Typen.
- `server/tests/preview_flow.rs` — `previewUpdateEntity` + `commitPreview` happy-path; expired-Preview; concurrent-Conflict bei Commit; invalid-payload bei Preview.
- `server/tests/preview_handlers.rs` — Default-Noop liefert leere `derived`; Test-Handler "echo" liefert eine künstliche Derived-Row.
- Client-Integration: Editor mit `preview_on_save=true` öffnet Modal; Cancel ruft `cancelPreview`.

## 8. Backwards-Compat

- `preview_on_save` Default `false` ⇒ kein bestehender Flow ändert sich.
- Alte Clients ignorieren neue Mutations. Server-Schema kompatibel.

## 9. Größe + Risiken

**Größe**: M-L. Server-Store + 4 Mutations + Wire-Typen + Modal + Diff-Tabelle.

**Risiken**:
- **Wenn die Preview die Welt verändert**: Default-Validation darf keine DB-Schreiboperationen ausführen. Code-Review-Punkt.
- **Latenz**: Preview-Mutation muss zügig sein (≤3 s). Wenn ein derive_handler langsam ist (T5 mit vielen Sub-Berechnungen), gehört das in U6 (Long-Running-Action), nicht in U2.
- **Zwei Persistenz-Pfade**: Tests müssen sicherstellen, dass `commitPreview` exakt denselben Pfad wie `save*` durchläuft. Mitigation: ein einziger interner `persist(...)`-Aufruf, beide Mutations rufen ihn.
- **Concurrency**: PreviewSession wird mit User-ID assoziiert (`session_id` aus `AuthContext`), damit ein User nicht die Preview eines anderen committen kann. Server-Check.

## 10. Spätere Erweiterungen

- Preview-Vergleich von Bulk-Imports (U2 + 1.7.15).
- "Was-wäre-wenn"-Vorschau im Editor (Live-Preview ohne Speichern-Klick) — Stretch, eigener Spec.
- Server-side Caching der Diff-Berechnung, falls Re-Open häufig.

## 11. Decisions

1. **Server-State, in-memory, TTL** statt Client-Cache: der Diff muss verbindlich sein, nur Server kennt die abgeleiteten Rows; TTL macht Crash-Recovery trivial.
2. **`commitPreview` re-runs die Persistenz**: Conflict-Detection bleibt scharf; Preview ist Vorschau, kein Reservierungs-Lock.
3. **Variante B-Layout** (Domain-Tabellen, gruppiert): generisch ohne Per-Entity-Templates, lesbar wie ein Buchhaltungs-Bericht.
4. **`derive_handler`-Registry**: explizit registrierbar; verhindert, dass jede Entity automatisch teure Derived-Berechnungen triggert.

## 12. Referenzen

- `shared/src/mutation.rs` (Vorlage für Wire-Typen)
- `server/src/schema.rs` (Mutation-Resolver-Pattern)
- `client/src/components/field/mod.rs` (Editor-Save-Pfad)
- ROADMAP §1.8 U2, Gap-Analyse §4.1 U2
- D2V-Inspiration: `DatevEntryControls/SaveReport.xaml`, `ContextControls/ContextSaveReport.xaml`
