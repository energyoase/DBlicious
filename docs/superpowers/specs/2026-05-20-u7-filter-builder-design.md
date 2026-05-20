# U7 — Filter-Builder + Saved-Filters Spec

Date: 2026-05-20
Status: Draft — awaiting user review
Trigger: T3 (Buchungs-Suche) — generisch für jede Listenansicht.
Quelle: Gap-Analyse §4.1 U7, ROADMAP §1.8 U7.
Visual: `.superpowers/brainstorm/.../u7-filter-builder.html`.

## 1. Ziel

Erweitert das heutige Per-Spalten-Quickfilter-Modell um

- **boolesche Kombinatoren** (AND/OR + verschachtelbare Gruppen) in `shared::FilterCriteria`,
- einen **Drag-and-Drop-Builder** als alternative UI zum heutigen Spalten-Header-Filter,
- **Saved Filters pro User**: serverseitig persistiert, in der Tabellen-Toolbar als Chips ausgewählbar.

Bestehende Per-Spalten-Quickfilter (`filters/*.rs`) bleiben erhalten — der Builder ist die "Pro"-Sicht für komplexe Anfragen.

## 2. Nicht-Ziele

- Builder-Canvas-Integration aus Phase 1 — der Filter-Builder ist eigenständig, kann später als `UiNode` eingebunden werden.
- SQL-Editor / "Schreib selber"-Modus — "SQL ansehen ⌄" ist nur read-only (Power-User-Debug).
- Cross-Entity-Filter (Joins) — bleibt am aktuellen Entity-Skelett.
- Geteilte Saved Filters über User hinweg — Phase 0.7-Topic (Group-Sharing).
- Filter-Vorlagen aus YAML auf Disk (analog Wizards/Reports) — Filter sind nutzergeneriert, nicht Admin-deklariert.

## 3. Architektur

### 3.1 Erweiterung von `FilterCriteria`

Heute:

```rust
pub struct FilterCriteria {
    pub global_search: Option<String>,
    pub predicates: Vec<ColumnFilter>,         // implizites AND
}
```

Neu (additiv, backwards-compatible):

```rust
pub struct FilterCriteria {
    #[serde(default)]
    pub global_search: Option<String>,
    /// Bestehender Pfad — implizites AND.
    #[serde(default)]
    pub predicates: Vec<ColumnFilter>,
    /// Neuer Pfad — strukturierter Builder-Tree.
    /// Wenn gesetzt, *zusätzlich* zu `predicates` (AND-verknüpft).
    /// Old-Clients senden None, Server-Verhalten unverändert.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<FilterExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FilterExpression {
    Predicate(ColumnFilter),
    All { children: Vec<FilterExpression> },   // AND
    Any { children: Vec<FilterExpression> },   // OR
    Not { child: Box<FilterExpression> },      // NOT (kein UI-Switch initially, aber Wire-Slot reserviert)
}
```

**Begründung Tagged Enum**: gleiches Muster wie `FieldType`, `MenuAction`. JSON: `{"kind":"all","children":[{"kind":"predicate","predicate":{...}}, …]}`.

Bestehender `Vec<ColumnFilter>`-Pfad bleibt unverändert. Beide werden im Server vor Anwendung in einen einheitlichen Expression-Tree konsolidiert (`predicates` → `All(predicates.map(Predicate))` ∧ `expression`).

### 3.2 Saved Filters: Server-Persistenz

Neue SeaORM-Tabelle `saved_filter`:

```rust
pub struct saved_filter::Model {
    pub id: String,                       // UUID
    pub user_id: String,                  // owner
    pub entity_type: String,
    pub name: String,                     // "Q1 unverarbeitet"
    pub expression_json: Json,             // serialisiertes FilterExpression
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_default: bool,                  // ein Default pro entity_type + user
}
```

GraphQL:

```graphql
extend type Query {
    savedFilters(entityType: String!): [SavedFilter!]!
}

extend type Mutation {
    createSavedFilter(input: SavedFilterCreateInput!): SavedFilter!
    updateSavedFilter(id: String!, input: SavedFilterUpdateInput!): SavedFilter!
    deleteSavedFilter(id: String!): Boolean!
}

type SavedFilter {
    id: String!
    name: String!
    entityType: String!
    expression: JSON!
    isDefault: Boolean!
    updatedAt: String!
}
```

### 3.3 Client-Komponente `<FilterBuilder>`

Pfad: `client/src/components/table/filter_builder/`.

- `mod.rs` — Top-Level: Saved-Filter-Chip-Bar + Builder-Panel + Action-Footer.
- `group.rs` — `<GroupNode>` rekursive Komponente (rendert `All`/`Any`/`Not` mit Switch).
- `predicate_row.rs` — `<PredicateRow>` (Spalte/Op/Wert).
- `column_picker.rs` — Dropdown der filterbaren Spalten (`ColumnMeta.filterable`).
- `operator_picker.rs` — Operatoren je nach `FieldType` (Mapping in einer Tabelle pro FieldType).
- `value_input.rs` — passender Editor je nach Op (Range = zwei Inputs, EnumIn = MultiSelect, …).
- `dnd.rs` — leichter Drag-and-Drop-Reducer; HTML5 DnD (kein extra Crate).

State: ein einziger `RwSignal<FilterExpression>` reicht — Builder mutiert ihn, "Anwenden" merged ihn in `TableState::filter`.

### 3.4 Operator-Vokabular (Bezug zu existing `FilterPredicate`)

Heutiges `FilterPredicate` wird zum gemeinsamen Vokabular zwischen Quickfilter und Builder. Pro `FieldType` gibt's eine Liste erlaubter Predicates (in `OpsRegistry`-Style):

| FieldType | Mögliche Operatoren |
|---|---|
| Text | TextContains, TextEquals, IsNull, IsNotNull |
| Number | NumberEquals, NumberRange, IsNull |
| Date | DateRange, IsNull |
| Bool | BoolEquals |
| Reference | EnumIn (über ReferenceCandidates), TextEquals (raw ID), IsNull |
| Enum / IntEnum | EnumIn |

`shared::ops::ops_for(field_type)` (existiert bereits) liefert das Mapping. Wenn fehlende Operatoren auftauchen (z.B. "existiert nicht" als `Not(IsNull)`), werden sie als syntaktischer Zucker im Builder gerendert und in den Tree übersetzt.

### 3.5 Toolbar-Integration

`<EntityTable>`-Toolbar erweitert um:

- **Saved-Filter-Chip-Bar**: zeigt Liste der Filter; aktiver Filter ist farbig markiert; "+ Neu" öffnet leeres Builder-Panel.
- **Builder-Toggle-Button** "Filter ▾" öffnet das Builder-Panel (Inline-Drawer unter der Toolbar).
- Quickfilter (Spalten-Header) bleiben unverändert — sie schreiben in `predicates`, der Builder schreibt in `expression`. Anwender können beide gleichzeitig nutzen.

### 3.6 "SQL ansehen ⌄" (read-only)

Ein zusammenklappbarer Bereich im Builder-Panel zeigt die generierte WHERE-Clause als String (z.B. `(posted_at BETWEEN '2025-01-01' AND '2025-03-31') AND ((is_published = false) OR NOT EXISTS (SELECT 1 FROM datev_account_entries WHERE entry_id = …))`). Hilft Power-Usern, das Verhalten zu debuggen. Nicht editierbar. Renderer: `server/src/data.rs` hat die Übersetzungslogik schon — eine Helper-Funktion `expression_to_sql_debug(...)` liefert sie als String für die GraphQL-Query `filterDebug(entityType, expression)`.

## 4. Daten-Flow

```
User öffnet Tabelle DatevEntry
  ↓ client-query savedFilters("DatevEntry")
  ← [SavedFilter…]
User klickt Chip "Q1 unverarbeitet"
  ↓ TableState.filter ← { expression: <gespeichert>, predicates: [] }
  ↓ reload → fetch_entity_page(... filter ...)
Server mergt expression ∧ predicates → Tree → SourceLayer.list_page(..., expression)
Source (managed-sqlite): Tree → SQL-WHERE
```

Für nicht-relationale Sources (memory, REST) wird der Tree clientseitig nachgewertet (Phase-0.6-Source-Capabilities-Slot — Capabilities erweitert um `supports_expression`).

### 3.7 Source-Capability-Erweiterung

In `shared::source::Capabilities` (Phase 0.6) wird ein Flag `supports_expression: bool` ergänzt. Sources, die `false` melden, werden vom Server vor-gefiltert (Server lädt Page, filtert in Rust). Begründung: nicht jede Source kann boolean-trees übersetzen — Memory-Source ja, SQL-Source ja, REST-Source typisch nein.

## 5. Fehler-/Edge-Cases

- **Empty `expression` + non-empty `predicates`**: alter Pfad wirkt; Builder zeigt "+ Bedingung"-CTA.
- **Saved Filter referenziert eine inzwischen gelöschte Spalte**: Builder rendert die Bedingung mit Warn-Banner "Spalte 'foo' nicht mehr vorhanden", User kann sie entfernen.
- **Save unter existierendem Namen** ("Q1 unverarbeitet" gibt's schon): UI fragt "Überschreiben?"
- **Filter ist null oder kaputt** (JSON-Parse-Error in DB): Saved Filter wird beim Laden mit Error-Status geladen, Chip ist deaktiviert.
- **Zwei Default-Filter pro entity_type**: Server-Constraint via Application-Logic, kein Unique-Index (Default-Toggle setzt alle anderen aus).

## 6. Komponenten-Inventar

**Neu**:
- `shared/src/filter.rs` — `FilterExpression` (oder direkt in `lib.rs` erweitert).
- `shared/tests/filter_expression_wire.rs` — Roundtrip.
- `server/src/entity/saved_filter.rs` — SeaORM-Modell.
- `server/src/data.rs` — `merge_expression(predicates, expression)` + `expression_to_sql_debug(...)`.
- `server/src/schema.rs` — neue Saved-Filter-Mutations/Queries; `entities` akzeptiert `expression`.
- `client/src/components/table/filter_builder/*` — Builder-UI.
- `client/src/graphql/saved_filters.rs`.

**Erweitert**:
- `shared::source::Capabilities` — `supports_expression: bool`.
- `server::source::Source::list_page` — neuer Param `expression: Option<FilterExpression>`.
- `client/src/components/table/shell.rs` — Chip-Bar + Builder-Toggle.
- `client/src/components/table/state.rs` — Filter-State erweitert.

## 7. Tests

- `shared/tests/filter_expression_wire.rs` — Roundtrip aller `kind`-Varianten, inkl. nested.
- `server/tests/expression_filter.rs` — `entities` mit nested AND/OR-Trees gegen managed-sqlite.
- `server/tests/saved_filters.rs` — CRUD, Permission (User darf nur eigene sehen/ändern), Default-Toggle.
- Client Unit: Group-Reducer (add/remove/move Predicate), Drag-Reorder, "SQL ansehen"-Render.

## 8. Backwards-Compat

- `expression`-Feld ist `Option<>` mit `#[serde(default)]` — alte Clients senden None, Server-Verhalten unverändert.
- Saved-Filters-Tabelle wird in `db::init` idempotent angelegt.
- Bestehende Quickfilter-Komponenten bleiben unangetastet.

## 9. Größe + Risiken

**Größe**: M. Wire-Erweiterung + Builder-UI + Saved-Filters-Persistenz.

**Risiken**:
- **Komplexität der Operatoren-Übersicht**: jede `FieldType`-Operator-Kombination braucht einen passenden Value-Input. Mitigation: Default-Fallback `<input type=text>` wenn kein spezialisierter Editor existiert.
- **Sources ohne `supports_expression`**: post-Fetch-Filter ist langsam bei großen Datasets. Mitigation: Capability-Doku, Cap-Banner ähnlich U3.
- **Source-API-Break (`list_page`-Signatur)**: managed-sqlite, foreign-sqlite (B1), memory müssen angepasst werden. Mitigation: neue Methode `list_page_v2` mit Default-Impl, die zur alten delegiert + lokal nachfiltert.
- **Komponente wird groß**: 5+ Unter-Komponenten. Mitigation: jede Komponente fokussiert auf einen Slot (rekursive Group, einzelne Predicate-Row, …).

## 10. Spätere Erweiterungen

- Gruppen-Sharing (Saved Filter als Group-Resource, Phase 0.7).
- Sub-Query-Predicates ("AccountEntries existieren nicht" als echtes `EXISTS`-Subquery).
- Filter-Templates aus Disk (auf Wunsch, heute YAGNI).
- Drag&Drop einer Bedingung zwischen Gruppen mit Live-SQL-Reaktion.

## 11. Decisions

1. **Additive `expression`-Feld**, kein Bruch des bestehenden `predicates`-Pfads.
2. **Tagged Enum** `FilterExpression` — konsistent mit `FieldType`/`MenuAction`.
3. **Saved Filters pro User in DB**, nicht in LocalStorage — überlebt Browser-Wechsel und legt Audit-Trail nahe.
4. **HTML5-DnD** statt Drag-Library — minimiert WASM-Bundle-Size.
5. **Quickfilter bleibt** — User wählt: einfach (Spalten-Header) oder mächtig (Builder).
6. **`supports_expression`-Capability** macht Heterogenität der Sources sauber sichtbar.

## 12. Referenzen

- `shared/src/lib.rs` (`FilterCriteria`, `FilterPredicate`, `ColumnFilter`).
- `shared/src/ops.rs` (`ops_for`).
- `server/src/data.rs` (Predicate-Anwendung).
- `client/src/components/table/filters/` (existing Quickfilter).
- ROADMAP §1.8 U7, Gap-Analyse §4.1 U7.
- D2V-Inspiration: `DatevEntryControls/DatevEntryFilter.xaml`.
