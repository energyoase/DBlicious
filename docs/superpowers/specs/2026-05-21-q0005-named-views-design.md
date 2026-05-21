# Q0005 — Named Views (In-Place Column-Editor + 3-Layer-Overlay) — Spec

Date: 2026-05-21
Status: Draft — awaiting user review
Trigger: User-Feedback während D2V-UI-Test ([Q0005-Queue-Item](../../queue/Q0005-in-place-column-editor-auf-entity-table.md)).
Verwandt: [[Q0002-tabelle-horizontaler-scroll-nicht-sichtbar]] (gleiche Tabelle), [[ROADMAP §1.8 U7]] (Saved-Filters — parallel zu konsolidieren).

## 1. Ziel

Eine `EntityTable`-Listenansicht ist nicht länger eine starre Projektion einer
einzelnen `EntitySettings`-Konfiguration pro Entity-Typ. Statt dessen:

- **Mehrere benannte Views pro Entity-Typ** (`default`, `reduced`, `full`, …),
  jede mit eigener Spalten-Konfiguration. UI-Stellen können explizit eine
  View per Name aufrufen (z. B. eine Delete-Bestätigung mit `?view=reduced`).
- **3-Layer-Overlay pro View**: Global-Default (Admin-bearbeitet) →
  Group-Overlay (z. B. „HR sieht zusätzlich Feld X") → User-Overlay
  („meine persönliche Anpassung"). Jede höhere Schicht überschreibt nur die
  expliziten Felder ihrer Property-Overrides; Rest erbt durch.
- **In-Place-Editor** in der laufenden Tabelle: Edit-Mode-Toggle in der
  TopMenu → Spalten-Header klickbar → Popover neben dem Header bearbeitet
  die Properties dieser Spalte. Die Tabelle bleibt jederzeit sichtbar; bei
  null Rows treten synthetische Demo-Rows ein.

## 2. Nicht-Ziele

- **Sicherheits-Schicht**: Views entscheiden nicht, welche Felder ein User
  *sehen darf*. Das bleibt das Geschäft von `PropertyAccess` /
  `PermissionOp` (siehe `shared/src/security.rs`).
- **Versionsverlauf pro View**: heutige `entity_designs` ist append-only;
  `entity_views` ist nicht — die einfache Versions-Spalte dient nur dem
  Optimistic-Locking. Verlauf kommt erst in Phase 2, wenn der Bedarf
  konkret ist.
- **UI-Tab-Strip / Dropdown zum View-Wechsel**: für MVP ist die View per
  URL-Param `?view=<name>` adressierbar. Ein sichtbarer Picker im TopMenu
  ist eigenes Q-Item für Phase 2.
- **Group-/User-Layer-Edit-UI** im MVP: das Datenmodell trägt alle drei
  Layer, aber die Edit-UI bearbeitet immer Layer=Global. Group/User-Edit
  folgt in Phase 2.
- **Visual-Regression-Tests** für die Popover-Position.

## 3. Shared Wire-Types

Neuer Modul `shared/src/view.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityView {
    pub id: String,                          // UUID
    pub entity_type: String,
    pub view_name: String,                   // "default", "reduced", "full", …
    pub layer: ViewLayer,
    pub owner_id: Option<String>,            // None bei layer=Global
    pub properties: Vec<ViewPropertyOverride>,
    pub default_filter: Option<FilterCriteria>,
    pub default_sort: Option<Sort>,
    pub default_page_size: Option<u32>,
    pub version: i32,                        // optimistic locking
    pub updated_at: String,                  // ISO-8601
    pub updated_by: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ViewLayer { Global, Group, User }

/// Sparse — alle Felder Option, nur Übergeschriebenes belegt.
/// Merge-Semantik: present-in-upper-Layer überschreibt; fehlt → vom Layer
/// darunter erben.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ViewPropertyOverride {
    pub key: String,                         // = ColumnMeta.key, Pflicht
    pub visibility: Option<Visibility>,
    pub order: Option<i32>,
    pub min_width: Option<u32>,
    pub label_override_key: Option<String>,
    pub sortable: Option<bool>,
    pub filter_id_override: Option<String>,
    pub formatter_id_override: Option<String>,
}
```

**Bestand bleibt unangetastet:**
- `ColumnMeta` (`shared/src/lib.rs:166`) → server-canonical, unverändert.
  `sortable`/`filterable`/`filter_id`/`formatter_id` bleiben dort als
  Default-Verhalten.
- `PropertySettings` (`shared/src/settings.rs:98`) → unverändert.
  `ViewPropertyOverride` ist die *Edit-Form*, `PropertySettings` die
  *resolved Form*. Der Resolver erzeugt aus den `ViewPropertyOverride`-Stacks
  eine vollständige `Vec<PropertySettings>`.

**Tag-/Case-Convention:** durchgehend `camelCase`. `ViewLayer` serialisiert
als `"global"`/`"group"`/`"user"`.

**Pinning:** `shared/tests/view_wire_format.rs` (neu) — exakter
JSON-Roundtrip pro Layer, `skip_serializing_if = "Option::is_none"`
beweist sparse-Property-Schreibweise.

## 4. Server — Persistenz, Resolver, GraphQL

### 4.1 SeaORM-Entity

Neue Datei `server/src/entity/entity_views.rs`:

```
entity_views (
  id              TEXT PRIMARY KEY,
  entity_type     TEXT NOT NULL,
  view_name       TEXT NOT NULL,
  layer           TEXT NOT NULL,            -- "global" | "group" | "user"
  owner_id        TEXT NULL,                -- NULL gdw layer="global"
  payload         TEXT NOT NULL,            -- JSON: ViewPayload (s. u.)
  version         INTEGER NOT NULL,
  updated_by      TEXT NULL,
  updated_at      TEXT NOT NULL,
  UNIQUE (entity_type, view_name, layer, owner_id)
)
```

`payload`-Form (JSON-Blob, analog `entity_designs.state`):

```json
{
  "properties":       [ViewPropertyOverride…],
  "defaultFilter":    FilterCriteria | null,
  "defaultSort":      Sort | null,
  "defaultPageSize":  int | null
}
```

Eintragung in `db.rs::init` via `Schema::create_table_from_entity(...)`
analog zu den bestehenden Tabellen.

**Code-seitiges Invariant** (zusätzlich zur DB-UNIQUE wegen SQLite-NULL-
Semantik): `assert layer == Global iff owner_id is None` in der
Save-Mutation.

### 4.2 Resolver

Neue Datei `server/src/views.rs`:

```rust
pub struct ResolvedView {
    pub entity_type: String,
    pub view_name: String,
    pub properties: Vec<PropertySettings>,
    pub default_filter: Option<FilterCriteria>,
    pub default_sort: Option<Sort>,
    pub default_page_size: Option<u32>,
    /// Layer + Source-View-IDs, die in den Merge eingeflossen sind —
    /// nützlich für Audit/Debug, dem Client als read-only mitgegeben.
    pub provenance: Vec<ResolvedLayerRef>,
}

pub struct ResolvedLayerRef {
    pub layer:    ViewLayer,
    pub view_id:  String,                    // entity_views.id
    pub owner_id: Option<String>,
    pub version:  i32,
}

pub async fn resolve_view(
    entity_type: &str,
    view_name: &str,
    user: Option<&SecurityUser>,
) -> ResolvedView {
    let base    = load_layer(entity_type, view_name, Global, None).await;
    let groups  = match user {
        Some(u) => future::join_all(
            u.group_ids.iter().map(|g| load_layer(entity_type, view_name, Group, Some(g)))
        ).await.into_iter().flatten().collect::<Vec<_>>(),
        None    => Vec::new(),
    };
    let user_v  = match user {
        Some(u) => load_layer(entity_type, view_name, User, Some(&u.id)).await,
        None    => None,
    };
    merge_layers(base, groups, user_v)
}
```

Merge-Regeln:
- **Property-Merge pro `key`**: für jedes Feld in `ViewPropertyOverride`
  gewinnt das oberste `Some(_)` (User > Group > Global).
- **Group-Stack-Reihenfolge**: stabile Sortierung nach `group.id`
  (lexikographisch), deterministisch ohne neue Konfiguration.
- **Default-Filter/Sort/PageSize**: oberster `Some(_)` gewinnt; keine
  Tiefen-Merge in `FilterCriteria` (zu komplex für MVP).
- **Unbekannte Override-Keys** (Spalte existiert in `ColumnMeta` nicht
  mehr): silent ignore, Info-Log
  `view '<entity>/<name>' enthält {n} Overrides für unbekannte Spalten: [...]`.

### 4.3 GraphQL-API

Erweiterung in `server/src/schema.rs`:

```graphql
type Query {
  entityView(entityType: String!, viewName: String! = "default"): EntityView
  entityViews(entityType: String!): [EntityViewSummary!]!

  # Bestand: liefert weiterhin die resolved EntitySettings, nur jetzt
  # transparent über resolve_view(entity_type, "default", currentUser).
  entitySettings(entityType: String!): EntitySettings
}

type Mutation {
  saveEntityView(input: SaveEntityViewInput!): SaveEntityViewResult!
  revertEntityView(
    entityType: String!,
    viewName:   String!,
    layer:      ViewLayer!,
    ownerId:    String
  ): RevertEntityViewResult!
}

input SaveEntityViewInput {
  entityType:      String!
  viewName:        String!
  layer:           ViewLayer!
  ownerId:         String                  # bei layer=Global ignoriert
  payload:         Json!                   # ViewPayload (s. 4.1)
  expectedVersion: Int                     # None bei Create
}

enum SaveEntityViewResultKind { OK, CONFLICT, FORBIDDEN }

type SaveEntityViewResult {
  kind:    SaveEntityViewResultKind!
  view:    EntityView                      # bei OK / CONFLICT (aktueller Stand)
  message: String                          # bei FORBIDDEN / Validation-Fehler
}

type EntityViewSummary {
  viewName:  String!
  layers:    [ViewLayer!]!                 # welche Layers existieren
  updatedAt: String!
}
```

**Backwards-Compat**: `entitySettings` (heute von
`EntityListPage` genutzt) bleibt identisch in der Signatur — intern wird
es zu `resolve_view(entity_type, "default", currentUser)`. Bestehender
Client-Code (`apply_settings_to_columns`) braucht keine Anpassung.

### 4.4 Loader-Bootstrap

In `db.rs::seed_if_empty` neu am Ende:

```rust
crate::data::seed_entity_views_from_example(db).await?;
```

Implementierung:

```rust
pub async fn seed_entity_views_from_example(db: &DatabaseConnection) -> …
{
    let Some(set) = crate::example::current() else { return Ok(()); };
    for (entity_type, type_set) in set.entities_by_type() {
        let Some(settings) = type_set.settings.as_ref() else { continue; };
        let key = (entity_type, "default", ViewLayer::Global, None);
        if entity_views::Entity::find_by_uq(key, db).await?.is_none() {
            let payload = settings_to_payload(settings);
            entity_views::ActiveModel { …, version: Set(0), updated_by: Set(Some("system".into())), … }
                .insert(db).await?;
        }
    }
    Ok(())
}
```

Idempotent — bestehende Rows werden nicht angefasst.

### 4.5 Auth

- MVP: `Update`-Permission auf `entity_type` reicht für `saveEntityView`
  auf **alle drei** Layer (analog zur in [[Q0003]] gefixten Builder-Auth).
- Granularität (z. B. „nur Admin darf Global", „nur ich darf meinen
  User-Layer") ist bewusst Phase 2 — eigenes Q-Item.
- `updated_by`/`updated_at` werden bei jedem Save aus dem `AuthContext`
  gefüllt → einfache Audit-Spur. Append-Versionierung wie bei
  `entity_designs`: **nein** (siehe Sektion 2 Nicht-Ziele).

## 5. Client — State, GraphQL, Routing

### 5.1 GraphQL-Adapter

Neu in `client/src/graphql/queries.rs`:

```rust
pub async fn fetch_entity_view(entity_type: &str, view_name: &str)
    -> Result<EntityView, GqlError>;
pub async fn fetch_entity_views(entity_type: &str)
    -> Result<Vec<EntityViewSummary>, GqlError>;
pub async fn save_entity_view(input: SaveEntityViewInput)
    -> Result<SaveOutcome<EntityView>, GqlError>;
pub async fn revert_entity_view(
    entity_type: &str, view_name: &str, layer: ViewLayer, owner_id: Option<&str>
) -> Result<(), GqlError>;
```

`fetch_entity_settings` bleibt — liefert nach 4.3 schon resolved.

### 5.2 EntityListPage-State

In `client/src/routes/mod.rs::EntityListPage`:

```rust
let view_name = move || {
    leptos_router::hooks::use_query_map()
        .with(|q| q.get("view").map(|s| s.to_string()))
        .unwrap_or_else(|| "default".into())
};
let edit_mode: RwSignal<bool> = RwSignal::new(false);
// MVP: konstant Global. Als Signal angelegt, damit Phase 2 (Group/User-Edit)
// den Wert ändern kann, ohne dass die UI-Komponenten umgebaut werden müssen.
let edit_layer: RwSignal<ViewLayer> = RwSignal::new(ViewLayer::Global);
let pending_overrides: RwSignal<HashMap<String, ViewPropertyOverride>> =
    RwSignal::new(HashMap::new());
let current_view: LocalResource<EntityView> = LocalResource::new(move || {
    let et = entity_type();
    let vn = view_name();
    async move { fetch_entity_view(&et, &vn).await.ok() }
});
```

### 5.3 View-Picker (MVP-Cut)

- URL-Query `?view=<name>` → `current_view` lädt entsprechenden Eintrag.
- View-Name unbekannt → Server liefert resolved-Default-View + Header
  `X-Dblicious-Fallback-View: default`. Client zeigt unauffällige Pill
  in TopMenu: „Ansicht `<name>` nicht gefunden — zeige Default".
- **Default-View muss nicht existieren**: wenn weder `<name>` noch
  `default` als Row vorliegt (z. B. weil der Loader keine
  `settings.json` für diesen Entity-Typ hatte), liefert `resolve_view`
  ein leeres `ResolvedView` und der Client rendert die rohe
  `ColumnMeta`-Liste mit Default-Verhalten. Kein Spezialfall im Client.
- **Kein** UI-Picker (Tab-Strip/Dropdown) — eigenes Q-Item für Phase 2.

### 5.4 Sample-Data-Fallback bei 0 Rows

Nur im Edit-Mode + tatsächlich `rows.len() == 0`:

```rust
let display_source: Rc<dyn DataSource> =
    if edit_mode.get() && page.rows.is_empty() {
        Rc::new(BuilderPreviewSource::new(columns.clone(), DEFAULT_PREVIEW_ROWS))
    } else {
        remote_source.clone()
    };
```

Wiederverwendung von `synthesize_preview_rows`
(`client/src/components/table/builder_preview.rs`).

### 5.5 `apply_settings_to_columns` + Live-Pending-Overrides

`apply_settings_to_columns` bleibt unverändert — der Server liefert schon
resolved Settings. Zusätzlich:

```rust
fn apply_pending_overrides(
    cols:      &mut Vec<ColumnMeta>,
    settings:  &mut EntitySettings,
    overrides: &HashMap<String, ViewPropertyOverride>,
) {
    for (key, ov) in overrides {
        // Sortable/filter_id/formatter_id direkt auf ColumnMeta
        if let Some(col) = cols.iter_mut().find(|c| &c.key == key) {
            if let Some(s)  = ov.sortable               { col.sortable    = s; }
            if let Some(id) = ov.filter_id_override.as_deref()
                                                          { col.filter_id    = Some(id.into()); }
            if let Some(id) = ov.formatter_id_override.as_deref()
                                                          { col.formatter_id = Some(id.into()); }
        }
        // visibility/order/min_width/label → in EntitySettings.properties spiegeln
        let p = settings.ensure_property(key);
        if let Some(v) = ov.visibility            { p.visibility         = v; }
        if let Some(o) = ov.order                 { p.order              = o; }
        if let Some(w) = ov.min_width             { p.min_width          = Some(w); }
        if let Some(l) = ov.label_override_key.clone()
                                                   { p.label_override_key = Some(l); }
    }
}
```

Wird **vor** dem bestehenden `apply_settings_to_columns` aufgerufen,
sodass Live-Preview vor Save funktioniert.

### 5.6 Save-Flow

1. „Speichern"-Button in der TopMenu (sichtbar wenn
   `edit_mode && !pending_overrides.is_empty()`)
2. `save_entity_view(SaveEntityViewInput { entity_type, view_name,
   layer: edit_layer, owner_id: None, payload, expected_version:
   current_view.version })`
3. Bei `Ok(SaveOutcome::Ok { view })` → `pending_overrides.clear();
   edit_mode.set(false);` und `current_view.refetch()`
4. Bei `Ok(SaveOutcome::Conflict { current })` → Modal:
   „Konflikt: andere Bearbeitung hat Version `current.version`
   gespeichert. [Neu laden & nochmal anwenden] [Überschreiben]"
5. Bei `Err(_)` → Toast/Inline-Fehler, Edits bleiben im Buffer

## 6. Client — UI: Edit-Mode + Header-Popover

### 6.1 TopMenu-Erweiterung

```
[Suchen]  [Neu]  [Layout bearbeiten ▼]
                  ├ Edit-Mode aktivieren / verlassen
                  ├ ─────────
                  ├ Verwirft Änderungen   (nur wenn pending > 0)
                  └ Speichern              (nur wenn pending > 0)
```

Im Edit-Mode:
- Tabellen-Wrapper bekommt dezenten Accent-Border (Token aus
  `DesignSystem`).
- Status-Pill rechts in TopMenu: „Layer: Global (für alle)
  · 3 ungespeicherte Änderungen". Layer-Anzeige auch im MVP sichtbar
  (verhindert falsches mentales Modell, wenn Phase 2 die anderen
  Layer hinzufügt).

### 6.2 ColumnEditorPopover

Neue Komponente `client/src/components/table/column_editor.rs`:

```rust
#[component]
pub fn ColumnEditorPopover(
    column:           ColumnMeta,
    current_override: Signal<Option<ViewPropertyOverride>>,
    on_change:        Callback<ViewPropertyOverride>,
    on_reset:         Callback<()>,
    on_close:         Callback<()>,
) -> impl IntoView { … }
```

Layout (top-down, alle Strings via `DesignSystem`-Tokens / i18n-Keys):

```
┌──────────────────────────────────┐
│ Spalte „Betrag"      [Reset] [✕] │
├──────────────────────────────────┤
│ ☑ Sichtbar                       │  → visibility
│ Position    [▲] 2 [▼]            │  → order
│ Min-Breite  [_____] px           │  → min_width
│ Label       [Betrag (EUR)____]   │  → label_override_key (freier Text)
│ ☑ Sortierbar                     │  → sortable
│ Filter      [Range ▼]            │  → filter_id_override
│ Format      [EUR Symbol ▼]       │  → formatter_id_override
│ ┌────────────────┐               │
│ │ 123,45 €       │               │  → Live-Preview, synthesize_preview_rows
│ └────────────────┘               │
└──────────────────────────────────┘
```

**Positionierung**: `position: absolute` + Floating-Logic (rechts-unter
dem geklickten Header, an Viewport-Rand klemmen) via
`getBoundingClientRect()`. Eine Popover-Instanz zur Zeit; Klick auf
anderen Header schließt aktuelles, öffnet neues.

**Verhalten**:
- Eingabe in irgendeinem Feld → `on_change(ViewPropertyOverride { … })`
- `Reset` → `on_reset()` (entfernt nur diesen Spalten-Override aus
  `pending_overrides`)
- `Esc` / Außenklick → `on_close()`
- `Tab` durch Felder in obenstehender Reihenfolge

### 6.3 Drag-Reorder am Header

Im Edit-Mode:
- `pointerdown` auf Header → 1 Snapshot in `pending_overrides`
- `pointermove` → live Tabellen-Spalten umordnen (rein visuell)
- `pointerup` → finale Reihenfolge als `order`-Werte in
  `pending_overrides`
- Eine reine Funktion `compute_reorder(headers: &[Rect], drag:
  &DragState) -> Vec<usize>` → isoliert testbar (Sektion 9.5).

### 6.4 FilterRegistry- und FormatterRegistry-Discovery

`client/src/components/table/filters/registry.rs` bekommt:

```rust
pub trait FilterRegistry {
    /// Bestand
    fn for_filter_id(&self, id: &str) -> Option<Arc<dyn FilterFactory>>;
    /// Neu:
    fn compatible_for(&self, ft: &FieldType) -> Vec<FilterDescriptor>;
}

pub struct FilterDescriptor {
    pub id:        String,
    pub label_key: String,
}
```

Neuer Modul `client/src/components/table/formatters.rs` erweitern:

```rust
pub trait FormatterRegistry {
    fn for_formatter_id(&self, id: &str) -> Option<Arc<dyn Formatter>>;
    fn compatible_for(&self, ft: &FieldType) -> Vec<FormatterDescriptor>;
}
```

Default-Implementierung pro `FieldType` (Beispiel `FieldType::Money`):

```rust
vec![
    FormatterDescriptor { id: "money-symbol".into(),   label_key: "formatter.money.symbol".into()   },
    FormatterDescriptor { id: "money-code".into(),     label_key: "formatter.money.code".into()     },
    FormatterDescriptor { id: "money-decimals".into(), label_key: "formatter.money.decimals".into() },
]
```

Erweiterbar pro Plugin in Phase 2.

### 6.5 i18n-Keys

In `client/locales/{de,en,fr}/main.ftl` (neue Sektion `## Column-Editor`):

```
column-editor-title        = Spalte „{ $name }"
column-editor-visibility   = Sichtbar
column-editor-position     = Position
column-editor-min-width    = Min-Breite
column-editor-label        = Label
column-editor-sortable     = Sortierbar
column-editor-filter       = Filter
column-editor-format       = Format
column-editor-reset        = Zurücksetzen
column-editor-preview      = Vorschau

table-actions-edit-mode    = Layout bearbeiten
table-actions-save-view    = Speichern
table-actions-discard-view = Verwerfen
table-status-edit-layer    = Layer: { $layer }
table-status-pending       = { $n } ungespeicherte Änderungen
table-fallback-view        = Ansicht „{ $name }" nicht gefunden — zeige Default
```

Englisch und Französisch parallel.

### 6.6 Accessibility-Basics

- `Esc` schließt Popover ohne Save.
- Tab-Reihenfolge: Toggles → Inputs → Dropdowns → Reset/Close.
- Header bekommt `aria-pressed` bei aktivem Popover, `role="button"` im
  Edit-Mode, `aria-keyshortcuts="Enter Space"`.

## 7. Datenfluss (End-to-End)

```
[Client EntityListPage Mount]
  ↓
fetch_entity_view(entity_type, view_name="default") + fetch_columns(entity_type)
  ↓
[Server schema.rs: entityView resolver]
  ↓
resolve_view(entity_type, "default", currentUser)
  ↓
  ├ load_layer(…, Global, NULL)        → base
  ├ load_layer(…, Group, group_id) × n → group_stack (sortiert nach group.id)
  └ load_layer(…, User, user.id)       → user
  ↓
merge_layers → ResolvedView { properties: Vec<PropertySettings>, defaultFilter, … }
  ↓
[Server → Client] EntityView (mit provenance) + ColumnMeta-Liste
  ↓
[Client]
  apply_pending_overrides(cols, settings, pending_overrides)   // nur im Edit-Mode
  apply_settings_to_columns(cols, settings)                    // wie heute
  ↓
EntityTableShell render
  ↓
[User-Aktion: Save] saveEntityView(input) → Server → reload
[User-Aktion: Edit] pointerdown/click auf Header → ColumnEditorPopover
                  → on_change → pending_overrides.update(…)
                  → Live-Preview neu rendern
```

## 8. Edge Cases & Error Handling

| ID | Fall | Verhalten |
|----|------|-----------|
| E1 | View enthält Overrides für nicht-existierende Spalten | Silent ignore; Info-Log auf Server |
| E2 | Concurrent Saves auf demselben Layer | `expected_version`-Mismatch → `Conflict { current }`; UI: Reload-vs-Überschreiben |
| E3 | Layer-Discriminator im MVP | TopMenu-Pill + Popover-Header zeigen „Layer: Global (für alle)" — auch wenn aktuell konstant |
| E4 | View-Name unbekannt | Server-Fallback auf `default`-View (oder leere `ResolvedView`, wenn auch `default` fehlt) + Header `X-Dblicious-Fallback-View`; Client zeigt unauffällige Pill |
| E5 | Reset einzelner Spalte vs. ganze View | `Reset` im Popover entfernt nur diesen Override; `revertEntityView` löscht den Layer-Row komplett |
| E6 | UNIQUE-Constraint mit NULL owner_id | `assert layer == Global iff owner_id is None` zusätzlich im Code, nicht nur in der DB |
| E7 | Default-Sort aus Group-Layer vs. Session-Sort des Users | Resolver merged nur die *Defaults*; aktive `TableState`-Sort-Klicks gewinnen pro Session |
| E8 | Sample-Data im Edit-Mode bei tatsächlich vorhandenen Rows | Greift **nicht** — echte Rows weiter zeigen; nur `rows.is_empty() && edit_mode` triggert `BuilderPreviewSource` |
| E9 | Auth / Audit | MVP: `Update` auf entity_type reicht für alle drei Layer; `updated_by`/`updated_at` pro Save; keine Versionshistorie |

## 9. Testing-Strategie

### 9.1 Wire-Format-Pinning

`shared/tests/view_wire_format.rs` — exakter JSON-Roundtrip pro Layer,
`skip_serializing_if`-Beweis für sparse Properties.

### 9.2 Resolver-Merge-Unit-Tests

In `server/src/views.rs` (Test-Modul, Tabellen-gesteuert):

- Nur Global → Identity
- Global + Group → Group überschreibt explizite Felder; Rest aus Global
- Global + 2 Groups → deterministische Sortierung nach `group.id`
- Global + Group + User → User > Group > Global feldweise
- Sparse `ViewPropertyOverride` → andere Felder durchsichtig
- E1: unbekannte Override-Keys → silent ignore + Info-Log
- Default-Filter/Sort/PageSize → oberster `Some(_)` gewinnt

### 9.3 Persistenz-Roundtrip

`server/tests/entity_views.rs` (neu), `#[serial_test::serial]`:

- Save + Read-Back → JSON-Identität
- Concurrent Save mit gleichem `expected_version` → zweiter Conflict
- Loader-Bootstrap: pro `entity_type` mit Settings genau 1 Row
  `(view_name="default", layer="global", owner_id=NULL, version=0,
  updated_by="system")`

### 9.4 GraphQL-E2E

`server/tests/e2e.rs` (Erweiterung):

- `entityView` mit/ohne Auth-Header → user-spezifischer Group-Overlay
- `saveEntityView` mit gestaffeltem `expectedVersion` → Conflict
- `entitySettings` (Bestand-Query) → liefert für `bookkeeper@local`
  effektive View-Resolution, nicht mehr nur den Loader-Stand
- `revertEntityView` → Layer-Row weg, Resolver fällt auf unteren Layer

### 9.5 Client-Logik (pure Funktionen)

- `apply_pending_overrides` — Tabellen-Test
- `FilterRegistry::compatible_for(field_type)` — Test pro FieldType
- `FormatterRegistry::compatible_for(field_type)` — dito
- `compute_reorder(headers, drag)` — Test mit synthetischen Rects

### 9.6 Manual Browser-Smoke (Akzeptanz)

Wird in der Plan-Akzeptanz-Checkliste festgeschrieben:

- D2V-Beispiel, `bookkeeper@local`, `?view=default`:
  → Layout bearbeiten → Spalte verstecken / umordnen / MinWidth /
    Label / Filter / Format / Sortable → Speichern → Reload → persistent
- `?view=reduced` (existiert nicht) → fällt auf Default + Pill zurück
- 0-Row-Filter aktivieren, Edit-Mode an → synthetische Demo-Rows
- Zwei Tabs parallel speichern → zweiter bekommt Conflict-UI

### 9.7 Bewusst NICHT im MVP

- Pixel-perfekte Popover-Position (Visual-Regression-Test) — kein
  Screenshot-Test-Stack im Repo.
- Performance bei >100 Spalten — keine realen Stressoren in d2v
  (~20 Spalten max).
- WASM-Komponenten-Tests für die Popover-Komponente selbst — pure
  Sub-Funktionen reichen.

## 10. Migration & Coexistence

- Keine bestehende Tabelle umbenannt, keine bestehende Spalte verändert.
- Bestehende Loader-`settings.json`-Dateien bleiben — `db.rs::seed_if_empty`
  produziert daraus pro Entity-Typ einen `(view_name="default",
  layer="global")`-Eintrag (idempotent).
- `EntitySettings` als Wire-Typ unverändert — `entitySettings`-Query
  liefert weiter, jetzt aber transparent resolved.
- Keine Wire-Bruch-Migration für Clients.

## 11. Out-of-Scope (eigene Folge-Items)

- View-Picker-UI (Tab-Strip oder Dropdown im TopMenu) → eigenes Q-Item
- Group-/User-Layer-Edit-UI → eigenes Q-Item
- View-Versionshistorie + Revert-zu-Version-N → eigenes Q-Item
- Per-View-Permission-Granularität („nur Admin darf Global") →
  eigenes Q-Item; baut auf [[Q0003]] auf
- Saved-Filters mit komplexer boolescher Logik → konsolidieren mit
  [[ROADMAP §1.8 U7]]
- Q0002 (horizontale Scrollbar bei langen Tabellen) bleibt eigenes
  Item, wird aber in derselben Tabellen-Komponente angefasst —
  Reihenfolge: Q0002 vor Q0005 ist sinnvoll, damit der Edit-Mode in
  der korrigierten Tabelle gebaut wird.

## 12. Akzeptanz für „Spec abgeschlossen"

- [ ] Alle 12 Sektionen vom User freigegeben
- [ ] Spec-Self-Review (Placeholders / Inkonsistenzen / Scope) durch
- [ ] User-Review-Gate passiert
- [ ] Plan-Schritt (`writing-plans`) ist die nächste Aktion
