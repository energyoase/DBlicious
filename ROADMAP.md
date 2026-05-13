# ROADMAP — Entwicklungsplan

Operative Umsetzung der Vision aus [`VISION.md`](./VISION.md). Diese Datei beschreibt **wie** und **in welcher Reihenfolge** gebaut wird, mit konkretem Code-Bezug pro Arbeitspaket.

**Lesehinweise**:
- Groessen-Schaetzung: **S** (1–3 Tage), **M** (1–2 Wochen), **L** (3–6 Wochen), **XL** (mehrere Monate).
- Reihenfolge ist absichtlich gewaehlt: jede Phase liefert eigenstaendigen Nutzen und ist von der naechsten **nicht** strikt abhaengig.
- "Bezug" zeigt konkrete Datei-/Modulpfade, an denen die Arbeit ansetzt.

---

## Architektur-Leitprinzipien

Diese Prinzipien priorisieren konkrete Designentscheidungen in der gesamten Roadmap. Bei Konflikt zwischen "elegantem Generalismus" und einem Prinzip gewinnt das Prinzip.

### Dev/Prod-Asymmetrie (Single-Compile-Strategie)

Der Dev-Mode (Visual Builder) ist eine **Entwicklungsumgebung**, kein Hochlastsystem. Performance dort ist sekundaer. Das Endnutzer-Produkt entsteht via Codegen (Phase 4) als **eigene, fixierte Komponenten-Crate**, die jede User-Konfiguration ausgewalzt enthaelt — pro Konfigurations-Variante eine eigene generierte Komponente.

Konsequenzen fuer die Roadmap:

1. **Keine generische Laufzeit-Engine im Builder.** Der Builder-State darf eine simple Rust-Datenstruktur sein (`Vec<UiNode>`, `HashMap`, `RwSignal<...>`). ECS-Frameworks, Reflection-Apparate, generische Komposition zur Laufzeit sind nicht noetig, weil zur Laufzeit ohnehin keine Iteration ueber tausende dynamische Knoten stattfindet — in einer Designer-Session sind 5–50 Elemente sichtbar.

2. **Austauschbarkeit existiert nur ueber User-Configs, nicht zur Laufzeit.** Sobald die Konfiguration steht, wird sie kompiliert. Die generierte App kennt keinen Modus, in dem sie zur Laufzeit Komponenten austauscht. Plugins (Phase 2) sind die einzige Ausnahme — sie laufen sandboxed, aber als zusaetzlicher Layer, nicht als Konfigurations-Engine.

3. **Codegen ist nicht "ein optionales Feature in Phase 4", sondern das Produkt.** Der Builder ist das Werkzeug, der Codegen ist das Ergebnis. Daher haelt man im Builder alles in einer Form, die sich **trivial in Rust-Quellcode uebersetzen** laesst — Datenstrukturen, die nahe am Ziel-AST sind, schlagen abstrakte ECS-Modelle.

4. **Performance-Vorgaben fuer den Builder**: "muss interaktiv reagieren" (Mensch-Skala, ~16 ms), nicht "muss tausende Knoten pro Frame iterieren". Optimierungen ueber das hinaus sind verfrueht.

### Server als Authority

Der Server ist die einzige Quelle der Wahrheit fuer Schema, Permissions, Implementations-IDs und Migrationen. Der Client zeigt projizierte Sichten an, darf aber nichts erzwingen. Siehe Phase 0.7 (Permissions) und Phase 1.5 (Implementations-Resolution).

### Konzepte uebernehmen, Tech-Choices pragmatisch

Der Blueprint nennt Bevy, SurrealDB, Loco, SurrealKit. Wir uebernehmen die **Konzepte** (Komposition statt Vererbung, Dual-Mode-Schema, Convention-over-Config, zweiphasige Migrationen) und implementieren sie mit dem Stack, der bereits laeuft (axum, SeaORM/SQLite, Leptos, plain Rust). Siehe `VISION.md` fuer das Mapping.

---

## Phase 0 — IST (abgeschlossen)

Beschreibt den vorhandenen Stand zur Kalibrierung der Folge-Phasen. Siehe `CLAUDE.md` und `README.md` fuer Details.

- axum + async-graphql Server (`server/`)
- SeaORM + SQLite Persistenz (`server/src/db.rs`, `server/src/entity/`)
- Leptos CSR/WASM Client mit Trunk (`client/`)
- Generische `EntityTable` + `DataSource`-Trait (`client/src/components/table/`)
- `DesignSystem`-Trait + `InlineDesign`-Impl (`client/src/styling/`)
- Daten-Loader fuer `--data-dir` (`server/src/example/`)
- Designer-Pfad fuer typisierte Tabellen (`saveDbSchema` → `ddl::try_apply_schema`)
- CLI `dblicious` fuer DB-Mutationen (`cli/`)
- Hand-geschriebener GraphQL-Client (`client/src/graphql/`)
- Project-Fluent-i18n (`client/locales/`, `client/src/i18n/`)

---

## Phase 0.5 — Konsolidierung (Pre-Vision) ✅ abgeschlossen (2026-05-13)

**Ziel**: Lose Enden aus den README-"Erweiterungspunkten" schliessen, **bevor** die grossen Vision-Phasen starten. Diese Arbeit ist klein, hochgradig wertstiftend und entkoppelt von der Vision.

**Status**: alle 8 Arbeitspakete erledigt. Commits: `543fc05` (0.5.2), `556adf4` (0.5.6), `b5cbc61` (0.5.1/0.5.3/0.5.5), `9ae8367` (0.5.7), `087116d` (0.5.8). 0.5.4 war bereits vor dieser Phase im Code (LocalSource).

| # | Paket | Groesse | Bezug | Akzeptanz | Status |
|---|---|---|---|---|---|
| 0.5.1 | Server-seitiges Sort/Filter im `entities`-Query auswerten | M | `server/src/schema.rs::QueryRoot::entities`, `server/src/data.rs` | `sort_by`/`sort_dir`/`filter`-Args werden nicht mehr ignoriert; bestehende UI funktioniert ohne Aenderung | ✅ |
| 0.5.2 | Spalten-Metadaten ausschliesslich vom Server | S | `client/src/routes/mod.rs::EntityListPage` (von `column_set_for` auf `fetch_columns` umstellen) | Kein Hardcoding von Spalten im Client mehr | ✅ |
| 0.5.3 | Reference- und Collection-Formatter | M | `client/src/components/field/mod.rs::DefaultFieldRegistry` (statt heute hartkodierte `formatters.rs`) | Felder mit `FieldType::Reference`/`Collection` rendern korrekt statt Rohwert | ✅ |
| 0.5.4 | `LocalSource` als alternative `DataSource` | S | `client/src/components/table/data_source.rs` | Client-seitiges Sort/Filter ohne Server-Roundtrip moeglich (fuer kleine Datasets) | ✅ (vor 0.5) |
| 0.5.5 | Weitere Sprachen vorbereiten | S | `client/locales/`, `client/src/i18n/mod.rs::Locale` | Mind. eine dritte Sprache (z.B. `fr`) als Skelett, dokumentiertes Add-Verfahren | ✅ |
| 0.5.6 | Tests fuer den Daten-Loader und CRUD-Pfade | M | `server/src/` (neue `tests/`-Module) | `cargo test` deckt Loader-Roundtrip + Entity-CRUD ab | ✅ |
| 0.5.7 | Table-Dekomposition: voll-komponiert via Shell + Bausteine | M | `client/src/components/table/` (neue Module: `shell.rs`, `top_menu.rs`/`bottom_menu.rs`, `table_view.rs`, `selection.rs`, `selection_column.rs`, `row_actions.rs`, `entity_menu.rs`, `global_filter.rs`, `pager.rs`, `page_size.rs`) | `<EntityTableShell>` legt Context an (`TableState`, `SelectionState` neu, `DataResource`, `Rc<dyn DataSource>`, `Rc<Vec<ColumnMeta>>`, `entity_type`, Auth-Caps, `FilterRegistry`); Aufruf-Code komponiert den Baum selbst (`<TopMenu><GlobalFilter/><EntityMenu>…</EntityMenu></TopMenu><Table/><BottomMenu><Pager/></BottomMenu>`); Selektion (`<SelectionColumn mode=Single\|Multi/>`), Per-Row-Aktionen (`<RowActions>{…}</RowActions>` mit `RowContext`), Bulk-Aktionen (`<EntityAction enabled_when=…/>`, `<DeleteAction/>`, `<EditAction/>`) sind alle opt-in als eigene Komponenten, nicht als Props; bestehende `view.rs::EntityTable` mit `#[deprecated(note = "use EntityTableShell + composable blocks")]` markiert; einziger Consumer `routes/mod.rs::EntityListPage` auf neue Komposition migriert. | ✅ |
| 0.5.8 | Property-Filter-Pipeline: Filter-Registry + Standard-Komponenten | M | `client/src/components/table/filters/` (neu: `registry.rs`, `text_contains.rs`, `number_range.rs`, `enum_in.rs`, `bool_equals.rs`, `date_range.rs`); Konsument: `<Table>` aus 0.5.7 | Jeder Filter ist eine eigene Leptos-Komponente mit Prop `column: ColumnMeta`; Registrierung per String-ID (`"text-contains"`, `"number-range"`, …) in der `FilterRegistry`; Resolution pro Spalte: `ColumnMeta.filter_id` → Client-Default pro `FieldType` → kein Filter-UI; Filter schreiben in `FilterCriteria.predicates`, bestehende `LocalSource`/`RemoteSource`-Auswertung greift unveraendert; Custom-Filter erweitern die Registry ohne Aenderung an `<Table>`. | ✅ |

**Deliverable**: README-Erweiterungstabelle ist auf "erledigt" bei den genannten Punkten. Codebase ist sauber genug, um auf ihr die Vision aufzubauen.

**Risiko**: gering. Reines Konsolidieren bekannter Arbeit.

**Abweichungen / Lessons learned**:
- 0.5.3 setzt nicht in `formatters.rs` (heute reine Fassade) sondern in `field::DefaultFieldRegistry` an. Pfad in der Tabelle entsprechend korrigiert.
- 0.5.4 war bereits vollstaendig im Code (`LocalSource` mit Filter/Sort/Pagination ueber `shared::ops_for_named`). Status-Snapshot in der ROADMAP war zu pessimistisch.
- CLAUDE.md hatte zwei falsche Aussagen, die im Zuge von 0.5.6 korrigiert wurden: "no tests in the workspace" (es gab 60+) und das Wire-Format-Beispiel fuer `FieldType` (`rename_all = "camelCase"` greift nicht auf innere Felder einer Struct-Variante).

---

## Phase 0.7 — Auth- & Permission-Modell

**Ziel**: Konsolidiertes, in `shared` definiertes Permission-Modell mit dem Server als Single Source of Truth. Client-seitige Auswertung ist projiziertes Spiegelbild, niemals Autoritaet.

**Status quo (Mai 2026)**: `client/src/auth/` haelt `AuthContext` + `PermissionOp` (CRUD-Ops); Server hat SeaORM-Modelle fuer `users`/`groups`/`user_groups`/`sessions`; Loader liest `security/{users,groups}` aus `--data-dir`. Permission-Pruefung passiert heute primaer im Client — serverseitiges Enforcement ist rudimentaer und keine konsistente Schicht.

**Target-Modell**:

```
Subject  := User | Group
Resource := EntityType
          | EntityProperty   (z.B. "product.price")
          | EntityInstance   (z.B. ("product", "p-42"))
          | Action           (z.B. "exportCsv")
          | ImplementationId (z.B. filter:"number-range")
          | Migration        (id)
Op       := Create | Read | Update | Delete | Execute | Choose
          | Approve | Cutover | Contract | Rollback           // Migration-Ops
Effect   := Allow | Deny
```

- **Rollen** = benannte Group-Sets; Group-Membership ist die Persistenz-Einheit.
- **Vererbung**: Permissions auf `EntityType` vererben auf alle `EntityProperty` desselben Typs, sofern nicht explizit ueberschrieben. Property-Permissions auf `EntityInstance` analog. Spezifischere Ebene gewinnt; bei gleicher Spezifitaet gewinnt **Deny vor Allow**.
- **Choose-Permission**: aus Phase 1.5; gibt dem User das Recht, eine bestimmte `ImplementationId` (oder Wildcard `filter.*`) zu waehlen.
- **Persistenz**: flache Tabelle `permissions(subject_id, subject_kind, resource_kind, resource_id, op, effect, priority)`. Indizierbar, auditierbar, ohne tiefe Hierarchien.
- **Evaluation**: Server-Resolver berechnet `effective(user, resource, op) -> Effect` pro Anfrage; Ergebnis pro Session gecached. Client bekommt eine **projizierte Liste** seiner erlaubten Operationen, darf aber nichts erzwingen.

### Arbeitspakete

| # | Paket | Groesse | Bezug | Akzeptanz |
|---|---|---|---|---|
| 0.7.1 | `Permission`, `Subject`, `Resource`, `Op`, `Effect` als `shared`-Typen | S | neu: `shared/src/auth.rs` | Wire-Format gepinnt durch `shared/tests/auth_wire_format.rs` |
| 0.7.2 | `permissions`-Tabelle (SeaORM) + Loader-Format `security/permissions.{toml,json}` | M | `server/src/entity/permission.rs`, `server/src/example/loader.rs` | Loader-Roundtrip-Test gruen |
| 0.7.3 | Server-Resolver `effective(user, resource, op)` mit Vererbung + Deny-Priority + Session-Cache | M | neu: `server/src/auth/resolver.rs` | Unit-Tests pro Praedikat, Integration ueber gemockte Sessions |
| 0.7.4 | Enforcement in CRUD-Resolvern + GraphQL-Mutation-Schicht | M | `server/src/schema.rs` | Negative Tests: nicht autorisierter Aufruf → Error-Code `forbidden` |
| 0.7.5 | Client-`AuthContext` aus projizierter Liste speisen | S | `client/src/auth/` | UI versteckt Aktionen, fuer die der Server keine Erlaubnis liefert |
| 0.7.6 | Audit-Log: alle Permission-Aenderungen + alle verweigerten Zugriffe | S | neu: `server/src/audit.rs` | Tabelle `audit_log` enthaelt Eintraege bei jedem Deny |
| 0.7.7 | Debug-Endpoint `whyAllowed(user, resource, op) -> Trace` | S | `server/src/schema.rs` | Liefert geordnete Liste aller passenden Regeln + Effekte |
| 0.7.8 | Migrations-Skript: heutiges `security/{users,groups}` → neues `permissions`-Format | S | neu: `cli/src/cmd/migrate_security.rs` | Idempotent, dry-run-faehig |

### Dependencies

- Keine harten. Vorbedingung fuer Phase 1.5 (Choose-Permissions) und impliziter Unterbau von Phase 2 (Plugin-Aufruf-Permissions) sowie Phase 3 (Migration-Approval-Permissions).

### Deliverable

- Server-Enforcement ist die Wahrheit; Client zeigt nur projizierte Sicht.
- `permissions`-Tabelle + Loader-Format ist in `examples/shop/` benutzt und dokumentiert.
- Audit-Log + `whyAllowed`-Debug-Endpoint sind nutzbar.

### Risiken

- **Performance des Resolvers**: pro CRUD-Aufruf darf nicht erneut die ganze Permission-Tabelle gescannt werden. Session-Cache mit Invalidierung bei Permission-Aenderung ist Pflicht.
- **Komplexitaet von Deny-vor-Allow + Wildcards**: kann zu schwer debuggbaren Effekten fuehren. Der `whyAllowed`-Endpoint ist die Notbremse und wird vom Audit-UI sichtbar gemacht.
- **Migrations-Pfad** des bestehenden security-Formats: Bestandsdaten unter `examples/shop/security/` muessen automatisch konvertiert werden, sonst bricht der Loader nach 0.7.2.

---

## Phase 1 — Builder-Foundation (Reaktive UI-State)

**Ziel**: Reaktiver In-Memory-State fuer den Visual Builder. Heute sind `columns.toml`/`editor.toml`/`settings.toml` statisch; nach dieser Phase laesst sich der Datentyp `product`/etc. live im Editor komponieren.

**Schluesseltechnologie**: **Plain Rust + Leptos-Signals**. Der Builder-State ist eine `RwSignal<UiTree>` mit einer simplen, codegen-naheliegenden Datenstruktur (siehe Spezifikation unten). Kein ECS-Framework — begruendet im Architektur-Leitprinzip "Dev/Prod-Asymmetrie": die Designer-Session arbeitet mit kleinen Knoten-Mengen (5–50 sichtbar), und Performance-Wunder wie kolumnarer Memory zahlen sich erst bei vier- bis fuenfstelligen Knoten aus. Das Endprodukt entsteht via Codegen als fixierte Komponenten-Crate.

### Arbeitspakete

| # | Paket | Groesse | Bezug |
|---|---|---|---|
| 1.1 | `UiTree`/`UiNode`-Datenstruktur in `client/src/builder/` (Rust-Struct, `RwSignal<UiTree>`) | S | neu: `client/src/builder/mod.rs`, `client/src/builder/tree.rs` |
| 1.2 | `UiNode`-Felder definieren: `transform`, `style`, `bound_field`, `event_trigger: Option<EventTrigger>`, `draggable: bool`, `children: Vec<UiNode>` | M | neu: `client/src/builder/node.rs` |
| 1.3 | Bruecke `UiTree` ↔ `shared::ColumnMeta`/`FieldType`: Projektions-Funktion exportiert UI-State als `Vec<ColumnMeta>` | M | neu: `client/src/builder/project.rs` |
| 1.4 | Drag&Drop-Canvas in Leptos: Mouse-Events mutieren `UiTree`-Signal, Leptos rendert reaktiv | L | neu: `client/src/builder/canvas.rs`, neue Route `/builder/:entity_type` |
| 1.5 | Live-Preview: gleiche `EntityTable` wie heute, gespeist aus den per Projektion erzeugten `ColumnMeta` | M | `client/src/components/table/` (neue `DataSource`-Variante `BuilderPreviewSource`) |
| 1.6 | Persistenz des Builder-State: Speichern via GraphQL-Mutation `saveEntityDesign` (analog `saveDbSchema`) | M | `server/src/schema.rs`, neue Tabelle `entity_designs` |
| 1.7 | Undo/Redo via `Vec<UiTree>`-Snapshot-Stack | S | `client/src/builder/history.rs` |

### Dependencies / Vorbedingungen

- Phase 0.5.2 (Spalten vom Server) ist hilfreich, aber **nicht** harte Voraussetzung.

### Deliverable

- Neuer Tab/Route "Designer" im Client.
- User kann fuer einen Entity-Type Spalten per Drag&Drop hinzufuegen, neuanordnen, Field-Types aendern.
- Speichern persistiert in `entity_designs`-Tabelle. Beim naechsten Server-Start wird der Builder-State geladen (zusaetzlich oder anstelle von `--data-dir`).

### Risiken

- **Skalierung jenseits 1000 Knoten**: bei naiver `Vec<UiNode>`-Iteration ist O(n) pro Mutation; bei kleinen Designer-Sessions vernachlaessigbar. Wenn der Builder spaeter doch hunderte Tabellen mit hunderten Spalten parallel halten muss, kann eine indexierte `HashMap<NodeId, UiNode>` nachgezogen werden. Datenmodell so halten, dass dieser Schritt ein lokaler Refactor bleibt.
- **DSL fuer EventTrigger**: was darf ein `EventTrigger`-Feld referenzieren? Klare Trennung "Builder beschreibt Intent, Plugin (Phase 2) implementiert Logik" einhalten — siehe [Architektur-Vertraege: Builder ↔ Plugin](#architektur-vertraege-builder--plugin).

### Spezifikation: Builder-Persistenz (zu 1.6)

Detaillierung der 1.6-Zeile aus der Arbeitspaket-Tabelle.

**Storage-Schema** (`entity_designs`-Tabelle):

```sql
CREATE TABLE entity_designs (
    entity_type     TEXT    NOT NULL,
    version         INTEGER NOT NULL,         -- monoton, jeder Save = +1
    state           TEXT    NOT NULL,         -- JSON, siehe unten
    schema_version  INTEGER NOT NULL,         -- shape des state-Blobs
    created_at      TEXT    NOT NULL,
    created_by      TEXT    NOT NULL,         -- user_id
    locked          INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (entity_type, version)
);
```

- **Append-only**: alte Versionen bleiben.
- `locked = 1` ⇒ Codegen-Snapshot (Phase 4), keine weiteren Mutationen erlaubt.

**Serialisierungs-Format** der `state`-Spalte:

```json
{
  "schemaVersion": 1,
  "tree": {
    "nodes": [
      { "id": 42,
        "transform":    { "x": 0, "y": 0, "w": 200, "h": 32 },
        "style":        { "tokenRef": "table_cell" },
        "boundField":   { "key": "price" },
        "eventTrigger": { "event": {"kind":"click"},
                          "target": {"kind":"plugin","id":"...","function":"...","args":{}} },
        "draggable":    false,
        "children":     []
      }
    ]
  },
  "projection": {
    "columns":  [/* ColumnMeta[]  — projiziert aus tree.nodes */],
    "settings": {/* EntitySettings — projiziert */},
    "editor":   {/* EntityEditor   — projiziert */}
  }
}
```

- **Wahrheit ist `tree.nodes`.** Die `projection`-Sektion ist redundant, wird beim Save aus dem Tree-Zustand neu generiert und mitgespeichert — damit der Server sie ohne Builder-Code lesen kann (`data::columns_for` etc. arbeiten weiter auf `ColumnMeta` und brauchen den Tree nicht zu kennen).
- **Schema-Drift**: aendert sich die `UiNode`-Form (z.B. `EventTrigger` wird erweitert), wird `schemaVersion` hochgezaehlt. `shared/src/builder/migrate.rs` haelt fuer jede Erhoehung eine `migrate_state(old, json) -> json`-Funktion. Loader bricht auf nicht-migrierbare States nicht — er nimmt die letzte erfolgreich migrierte Version.
- **Performance**: bei kleinen Designs vernachlaessigbar; der `projection`-Block ist der dominierende Read-Pfad — Cache pro `(entity_type, version)` im Server.
- **Codegen-Nahe**: die `UiNode`-Struktur ist absichtlich so geschnitten, dass Phase 4 sie nahezu 1:1 in Rust-Quellcode uebersetzen kann (eine `UiNode` ⇒ ein generiertes Leptos-Component-View). Keine Indirektion ueber Components/Queries noetig.

**Koexistenz mit dem `--data-dir`-Loader**:

```
Server-Start:
  1. Loader liest --data-dir → schreibt in example::install (RwLock-Slot).
  2. DB-Init: pro entity_type im Loader-Set, das in entity_designs noch nicht
     existiert, wird Loader-Snapshot als version=0 (created_by="system")
     geschrieben. Idempotent.
  3. Aktive Version pro entity_type = MAX(version).
  4. data::* liest aus aktiver Version (DB); fallt auf RwLock-Slot nur zurueck,
     wenn fuer den entity_type keine version existiert (defensiv).
```

- **Single Source of Truth nach Boot**: die `entity_designs`-Tabelle.
- **`--data-dir` ist Seed, kein Laufzeit-Override.** Aenderungen an Loader-Dateien greifen nur, wenn die DB fuer den entity_type *leer* ist oder explizit per CLI `dblicious design reset <entity_type>` zurueckgesetzt wird.
- **Builder-Save** bumpt die `version`; Live-Clients erfahren das per GraphQL-Subscription `entityDesignUpdated(entity_type)` (neu in 1.6).
- **Revert**: `revertEntityDesign(entity_type, target_version)` schreibt den alten State als *neue* Version. Keine Loeschung.

**Konfliktbehandlung bei paralleler Bearbeitung**: Optimistic Locking via `expected_version` in der Save-Mutation. Konflikt ⇒ Fehlercode `concurrent_design_modification` mit serverseitiger Diff-Sicht.

---

## Phase 1.5 — Implementations-Resolution (server-priorisiert)

**Ziel**: Generischer Auswahlmechanismus fuer *jede* austauschbare Komponente (Filter, Editor, Formatter, Row-Action, …). Server entscheidet Default und Berechtigung; Client liefert die Implementierungen unter String-IDs. Knuepft direkt an die `FilterRegistry` aus 0.5.8 an und verallgemeinert sie.

**Idee**: jede Komponentenart hat eine Client-Registry (`FilterRegistry`, `EditorRegistry`, `FormatterRegistry`, `ActionRegistry`). Pro Property/Spalte gibt der Server eine ID vor — der Client schlaegt nach. Permission-Gates leben ausschliesslich serverseitig: der Server liefert nur IDs aus, die der angemeldete Nutzer waehlen darf.

**Resolution-Prioritaet (gilt fuer jede Komponentenart)**:

1. Server-Pflicht pro Property (`ColumnMeta.{filter,editor,formatter,…}_id`)
2. Server-Default pro Entity-Typ (`EntitySettings.{…}_defaults`, neu)
3. Server-Default pro `FieldType` (globale Settings, neu)
4. Client-Fallback pro `FieldType` (hardgecodeter Standard)

Liefert der Server fuer ein Property mehrere erlaubte Varianten zurueck, darf der Nutzer in der UI eine waehlen; die Wahl wird per User/Group persistiert.

### Arbeitspakete

| # | Paket | Groesse | Bezug |
|---|---|---|---|
| 1.5.1 | `EntitySettings.field_type_defaults` (neu) + globale Type-Defaults im Loader | M | `shared/src/settings.rs`, `server/src/example/loader.rs`, `examples/shop/` |
| 1.5.2 | `ColumnMeta` um `editor_id`/`formatter_id`/`action_ids` erweitern (analog `filter_id`) | S | `shared/src/lib.rs`, `server/src/schema.rs` (`RawColumnMeta`-Roundtrip) |
| 1.5.3 | Per-User/Group erlaubte Varianten persistieren + Server-Resolver | M | `server/src/entity/`, `server/src/schema.rs`, `server/src/auth/` |
| 1.5.4 | Client-Registries fuer Editor, Formatter, Action (`FilterRegistry` existiert nach 0.5.8) | M | `client/src/components/` (eigene Module je Registry) |
| 1.5.5 | UI: Wahl-Komponente "welche Implementierung soll fuer diese Spalte gelten?" + Persistenz | M | neu: `client/src/components/implementation_picker.rs` |
| 1.5.6 | Tests: Resolution-Reihenfolge, Permission-Negativfaelle, ID-unbekannt-Fallback | M | `shared/tests/`, `server/tests/` |

### Dependencies

- Phase 0.5.7 und 0.5.8 muessen abgeschlossen sein (Filter-Registry liefert den Templating-Mechanismus, der hier generalisiert wird).
- Profitiert von Phase 1 (Builder), ist aber per Hand-Konfiguration in `--data-dir` (TOML/JSON) bereits ohne Builder nutzbar.

### Deliverable

- Server kann pro Property eine Implementations-ID vorschreiben **und/oder** mehrere erlauben.
- Client liefert mehrere Filter/Editor/Formatter unter IDs; korrekte Implementierung wird per Resolution-Kette gefunden.
- Unautorisierte ID-Wahl ist serverseitig geblockt.
- Tests beweisen die Prioritaet und das Permission-Verhalten.

### Risiken

- **ID-Drift**: String-IDs sind fehleranfaellig. Empfehlung: pro Registry eine `register!`-Macro oder enum-basierte Variante mit `as_str()`; Test, dass jede registrierte ID auch im Server-Schema bekannt ist.
- **Per-User-Persistenz** schneidet sich mit dem Auth-/Session-Modell — 1.5.3 muss klaeren, wo die Wahl liegt (User-Profile vs. Session-Local) und wie sie zwischen Geraeten synchronisiert.

---

## Phase 2 — WASM-Plugin-Sandbox (Extism)

**Ziel**: User-Logic (Validierungen, Hooks, abgeleitete Felder) laeuft als sandboxed WASM-Plugin. Mehrsprachig (Rust/TS/Python/Go) ueber Extism-PDKs.

### Arbeitspakete

| # | Paket | Groesse | Bezug |
|---|---|---|---|
| 2.1 | Extism Host-SDK integrieren | S | `server/Cargo.toml`, neu: `server/src/plugins/` |
| 2.2 | Plugin-Manifest-Schema definieren: `id`, `version`, `triggers[]`, `allowed_hosts`, `allowed_paths`, `max_pages` | M | neu: `shared/src/plugin.rs` |
| 2.3 | Trigger-Points: `before_save`, `after_save`, `before_delete`, `derive_field`, `validate` | M | `server/src/schema.rs` (CRUD-Resolver rufen Plugin-Manager) |
| 2.4 | Host-Functions exponieren: `db_query` (validiert), `log`, `now`, `http_fetch` (mit Allowlist) | M | neu: `server/src/plugins/host_functions.rs` |
| 2.5 | Plugin-Storage: Tabelle `plugins` (id, wasm_blob, manifest_json, enabled) | S | `server/src/entity/plugin.rs` |
| 2.6 | Plugin-Upload via GraphQL-Mutation + Admin-UI im Client | M | `server/src/schema.rs`, `client/src/routes/plugins.rs` |
| 2.7 | Beispiel-Plugins: ein Rust-PDK-Plugin, ein TS-PDK-Plugin | S | neu: `examples/plugins/` |
| 2.8 | Plugin-Audit-Log: jeder Plugin-Aufruf → Eintrag in `plugin_invocations` (Timing, Erfolg/Fail, Input-Hash) | S | `server/src/entity/plugin_invocation.rs` |

### Dependencies

- Phase 1 nicht zwingend, aber `EventTrigger`-Components aus Phase 1.2 brauchen Plugin-Bindings, um zur Laufzeit Wirkung zu entfalten.

### Deliverable

- User kann ein WASM-Plugin hochladen, einem Entity-Type/Event zuordnen, und bei CRUD-Operationen wird es sandboxed ausgefuehrt.
- Strikte Manifest-Limits werden enforced (Memory, Network, FS).
- Audit-Log nachvollziehbar.

### Risiken

- **Performance**: Plugin-Calls in heissen CRUD-Pfaden. Latenz-Budget pro Trigger definieren, ggf. async/parallel.
- **Security-Audit**: vor Production-Freigabe ist ein externer Audit der Host-Functions sinnvoll (Capability-Escape).
- **WASI-NN** ist explizit **ausserhalb** dieser Phase; wird in Phase 4+ evaluiert.

---

## Phase 3 — AI-Schema-Engine + zweiphasige Migrationen

**Ziel**: Beim Import unstrukturierter JSON-Payloads (oder bei freier Designer-Eingabe) schlaegt ein LLM-Agent Schema-Mutationen vor, die deterministisch validiert und in einem sicheren Expansion/Contract-Rollout angewandt werden.

### Arbeitspakete

| # | Paket | Groesse | Bezug |
|---|---|---|---|
| 3.1 | AI-Client abstraction (Anthropic/OpenAI Trait) | S | neu: `server/src/ai/mod.rs` |
| 3.2 | Strict JSON-Schema fuer `MigrationProposal` (Add-Table, Add-Column, Map-Column, Drop-Column) | M | neu: `shared/src/migration.rs` |
| 3.3 | RAG-Pfad: aktuelles Schema (`db_schemas`) + Sample der neuen Daten → Prompt-Builder | M | neu: `server/src/ai/schema_proposer.rs` |
| 3.4 | Function-Calling-Loop mit ResponseFormat-Enforcement; Reflection bei Validierungsfehler | M | wie 3.3 |
| 3.5 | `rusty_schema_diff` integrieren: Vergleich aktuelles vs. proposed Schema → `CompatibilityReport` | M | neu: `server/src/ai/validator.rs`; `Cargo.toml` |
| 3.6 | Approval-UI: Diff visualisieren, User akzeptiert/lehnt ab | M | neu: `client/src/routes/migrations.rs` |
| 3.7 | Zweiphasige Migration: Expansion-Plan, Cutover, Contract-Plan, Rollback | L | neu: `server/src/migrations/` (eigener Layer ueber SeaORM-Migrationen) |
| 3.8 | Snapshot-Mechanismus fuer SQLite (file-copy bzw. in-memory `backup`) als Rollback-Insurance | S | `server/src/db.rs` |
| 3.9 | JSON-Import-Endpoint, der den Agent triggert | S | `server/src/schema.rs` (neue Mutation `proposeMigrationFromJson`) |

### Dependencies

- Profitieren von Phase 1 (`UiTree` als Single Source of Truth), aber unabhaengig lauffaehig.

### Deliverable

- User kann eine JSON-Datei drag&droppen, bekommt einen Migrations-Vorschlag, sieht den Diff und entscheidet.
- Akzeptierte Migrationen laufen zweiphasig: Expansion zuerst, Cutover, dann Contract.
- Rollback per einem Klick moeglich, solange Contract-Phase nicht abgeschlossen.

### Risiken

- **LLM-Halluzinationen**: durch Function-Calling + Diff-Validator hart geblockt, aber Tests mit adversariellen Inputs noetig.
- **Migration-Komplexitaet**: Drop-Column-mit-Backfill-Pfad ist heikel. Erste Version darf destruktive Operationen nur als "Contract"-Schritt anbieten und nur, wenn User explizit zustimmt.
- **Cost-Control**: LLM-Calls deckeln (max. N pro Stunde pro User).

### Spezifikation: `MigrationProposal` + zweiphasige State-Machine (zu 3.2 und 3.7)

Detaillierung der 3.2- und 3.7-Zeilen aus der Arbeitspaket-Tabelle.

**`MigrationProposal`-Schema** (in `shared/src/migration.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationProposal {
    pub id: String,
    pub generated_at: String,              // ISO-8601
    pub source: ProposalSource,
    pub steps: Vec<MigrationStep>,
    pub rationale: String,                 // LLM-Begruendung, max 2000 Zeichen
    pub impact: Impact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProposalSource {
    JsonImport   { sample_hash: String, row_count: u64 },
    DesignerEdit { user_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum MigrationStep {
    AddTable    { entity_type: String, columns: Vec<ColumnMeta> },
    AddColumn   { entity_type: String, column: ColumnMeta,
                  default: Option<serde_json::Value> },         // Pflicht wenn nullable=false
    MapColumn   { entity_type: String, from: String, to: String,
                  transform: TransformExpr,                      // typisiertes Mini-DSL
                  keep_source: bool },                           // Expansion: true; Contract: false
    DropColumn  { entity_type: String, column: String,
                  backup_to: Option<String> },                   // optionale Archive-Tabelle
    AddIndex    { entity_type: String, columns: Vec<String>, unique: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Impact {
    pub destructive: bool,
    pub estimated_rows_affected: u64,
    pub plugin_compatibility:  Vec<PluginCompatNote>,
    pub builder_compatibility: Vec<BuilderCompatNote>,
}
```

**Validierungs-Pflichten** (deterministisch, *vor* User-Approval, kein LLM):

- `AddColumn` mit `nullable=false` ohne `default` ⇒ Vorschlag REJECTED.
- `DropColumn` ohne `backup_to` ⇒ MARKED `destructive=true`; erfordert explizite User-Approval mit Confirm-Wort.
- `MapColumn.transform` muss durch den Mini-DSL-Parser kompilieren.
- Plugin-Manifeste mit Capability `db_read: <entity_type>` werden gegen die *finale* Spaltenliste nach allen Steps validiert ⇒ `plugin_compatibility[]`.
- Builder-Designs mit `BoundField.key == dropped_column` ⇒ `builder_compatibility[]`.
- `AddIndex` mit `unique=true` auf existierende Daten ⇒ deterministischer Pre-Check, ob Werte unique sind; sonst REJECTED.

**Zweiphasige State-Machine**:

```
Proposed → Approved → Expanding → Expanded → CuttingOver → Cutover → Contracting → Contracted
                                                  │
                                                  └─ RollingBack → RolledBack (terminal)
                                                  │
                                                  └─ Failed (terminal)
```

| Transition                | Trigger                    | Was passiert                                                                          | Rollback moeglich?               |
|---------------------------|----------------------------|---------------------------------------------------------------------------------------|----------------------------------|
| Proposed → Approved       | User-Approval              | Permission-Check (`Approve` auf `Migration(id)`), Lock auf entity_type                | n/a                              |
| Approved → Expanding      | System (automatisch)       | DB-Snapshot (3.8); fuehrt `AddTable`/`AddColumn`/`MapColumn(keep_source=true)` aus    | ja (Snapshot-Restore)            |
| Expanding → Expanded      | Steps fertig               | App laeuft mit beiden Spalten parallel; **Dual-Write** aktiv                          | ja                               |
| Expanded → CuttingOver    | User-Bestaetigung          | Code/Builder/Plugins lesen ab jetzt aus neuen Spalten (**Dual-Read** mit Fallback)    | ja                               |
| CuttingOver → Cutover     | Verifikations-Run gruen    | Alte Spalten existieren noch im Schema, werden nicht mehr gelesen                     | eingeschraenkt (24h-Fenster)     |
| Cutover → Contracting     | User-Bestaetigung          | `DropColumn`-Steps + `MapColumn(keep_source=false)` ausgefuehrt                       | **nein** (destruktiv)            |
| Contracting → Contracted  | Steps fertig               | Lock aufgehoben; Snapshot kann archiviert oder geloescht werden                       | nein                             |
| beliebig → RollingBack    | Manuell oder Step-Fehler   | Snapshot-Restore (vor Contract: voll; nach Contract: best effort aus `backup_to`)     | terminal                         |

- **Dual-Write/Dual-Read**: in `Expanded`/`CuttingOver` schreibt der Server in alte *und* neue Spalte (Dual-Write) und liest aus der neuen mit Fallback auf die alte (Dual-Read). Implementiert im CRUD-Resolver, gesteuert durch die `migrations`-Tabelle.
- **24h-Fenster** nach Cutover: Snapshot wird aufgehoben, Rollback ueber `restoreSnapshot(migration_id)` moeglich. Danach ist nur noch eine neue inverse Migration der Weg.
- **Verifikations-Run**: System-Job vergleicht stichprobenartig fuer N Rows, ob `transform(old) == new`. Default N=100, konfigurierbar.

**Persistenz**:

```sql
CREATE TABLE migrations (
    id            TEXT PRIMARY KEY,
    proposal      TEXT NOT NULL,            -- MigrationProposal JSON
    state         TEXT NOT NULL,            -- enum-Variante
    snapshot_ref  TEXT NOT NULL,            -- Pfad/ID des DB-Backups (3.8)
    approved_by   TEXT,
    started_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    error         TEXT
);
```

**Permission-Modell** (Bezug zu 0.7): Operationen `Approve`/`Cutover`/`Contract`/`Rollback` sind eigenstaendige `Op`-Werte auf der Resource `Migration(id)`. Wer `Approve` darf, darf nicht automatisch `Contract` — das schuetzt vor versehentlich destruktiven Aktionen.

---

## Phase 4 — Codegen & Optimierung

**Ziel**: Aus finalisiertem `UiTree` (Phase 1) + locked Schema + Plugin-Bindings → eigenstaendige axum/Leptos-App ohne Builder-Overhead. Plus Polish und externe Audits.

### Arbeitspakete

| # | Paket | Groesse | Bezug |
|---|---|---|---|
| 4.1 | AST-Codegen-Crate: `quote`/`syn`/`prettyplease` basiert | L | neu: `codegen/` |
| 4.2 | Codegen-Templates: axum-Routes, async-graphql-Resolver, SeaORM-Entities, Leptos-Views | XL | wie 4.1 |
| 4.3 | `cargo-scaffold`-Integration fuer Repo-Bootstrap der generierten App | S | wie 4.1 |
| 4.4 | CLI-Command `dblicious export --target ./generated-app` | M | `cli/` |
| 4.5 | WASI-NN-Evaluation: Wechsel von `wasmtime` auf `wasmedge` fuer Plugins? Lokale Inference-POC | M | `server/src/plugins/` |
| 4.6 | Externer Security-Audit der WASM-Sandbox + AI-Pfade | (extern) | n/a |
| 4.7 | Performance-Benchmarks: Builder mit realistischen Knotenzahlen (Default 500, Stress-Probe 5000), CRUD mit aktiven Plugins, Migration auf 100k-Row-Tabelle | M | neu: `benches/` |
| 4.8 | Dokumentation: User-Guide, Plugin-Dev-Guide, Codegen-Guide | M | `docs/` |

### Dependencies

- Phasen 1–3 muessen weitgehend stabil sein. Codegen ist die Konsolidierung.

### Deliverable

- `dblicious export` erzeugt aus einer laufenden Designer-Session ein eigenstaendiges Rust-Workspace.
- Generierte App startet ohne `dblicious`-Builder, nur mit den selektierten Plugins und finalem Schema.
- Audit-Bericht liegt vor.
- Benchmarks zeigen, dass Builder im erwarteten Bereich (siehe 4.7) interaktiv bleibt; ueber dem Stress-Threshold ist Indexierung der Datenstruktur das nachgezogene Mittel (siehe Phase-1-Risiken).

### Risiken

- **Codegen-Drift**: Templates muessen mit den Laufzeit-Crates synchron bleiben. CI-Test, der Codegen-Output baut + minimalen Smoke-Test laufen laesst, ist Pflicht.
- **WASI-NN-Switch**: WasmEdge ist weniger ergonomisch als wasmtime/Extism. Nur wenn die Inference-Anforderung real auftritt.

---

## Architektur-Vertraege: Builder ↔ Plugin

Diese Spezifikation bindet Phase 1 (Builder/UI-State), Phase 1.5 (Implementations-Resolution) und Phase 2 (WASM-Plugins) zusammen. Aenderungen erfordern Updates in mindestens zwei dieser Phasen — der Vertrag ist die Schnittstelle, an der sie sich treffen.

### 1. `EventTrigger`-DSL

Ein `EventTrigger` ist ein optionales Feld auf einem `UiNode` (oder an einem Entity-Hook) und sagt: *"Wenn Ereignis X geschieht, rufe Ziel Y mit Parametern Z auf."* Format (in `shared/src/builder.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventTrigger {
    pub event:       EventKind,
    pub target:      TriggerTarget,
    pub guard:       Option<GuardExpr>,    // boolesche Mini-Expression
    pub debounce_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EventKind {
    Click,                                 // UI
    Change       { field: String },        // UI
    Submit,                                // UI
    BeforeSave,                            // Server-CRUD
    AfterSave,                             // Server-CRUD
    BeforeDelete,                          // Server-CRUD
    Custom       { name: String },         // benutzerdefiniert
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TriggerTarget {
    Plugin         { id: String, function: String, args: serde_json::Value },
    BuiltinAction  { name: String, args: serde_json::Value },
    Navigate       { route: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuardExpr(pub String);          // Mini-DSL ueber `fields.*`
```

- `event` greift **client- oder serverseitig**, je nach Variante. `BeforeSave`/`AfterSave`/`BeforeDelete` laufen im Server-Resolver; `Click`/`Change`/`Submit` im Client. `Custom` ist explizit aus Code oder anderem Plugin getriggert.
- `guard` ist eine Mini-Expression-Sprache ueber Entity-Felder (`fields.status == "draft" && fields.price > 0`); Parser liegt unter `shared/src/builder/guard.rs` und ist Teil von Paket 1.2.
- `args` werden zum Plugin-Aufruf serialisiert (das Plugin sieht sie als JSON).
- `debounce_ms` ist UI-only und ignoriert serverseitig.

### 2. Plugin-Manifest-Schema

Manifest liegt **im** WASM-Bundle (Extism-Konvention) als `plugin.toml`:

```toml
id          = "com.example.slug-generator"
version     = "1.2.0"                         # SemVer, Pflicht
api_version = 1                                # Host-API-Version (dieser Vertrag)

[compatibility]
dblicious = ">=0.2, <0.3"                     # Server-Versionsbereich
plugins   = ["com.example.locale-utils ^1"]   # Plugin-Dependencies

[capabilities]
triggers       = ["beforeSave", "deriveField", "validate"]
db_read        = ["product", "category"]      # Entity-Typen mit Read-Capability
db_write       = ["product"]                  # Entity-Typen mit Write-Capability
http_fetch     = ["https://api.example.com/*"]
fs_paths       = []
max_pages      = 16                           # Extism Memory (à 64 KiB)
max_runtime_ms = 200                          # pro Trigger-Aufruf

[functions]
slugify  = { trigger = "deriveField", target_field = "slug", from_field = "name" }
validate = { trigger = "validate" }

[signing]                                     # optional in Phase 2, Pflicht ab 4
public_key = "ed25519:..."
signature  = "base64:..."
```

- **Versionierung**: SemVer fuer `version`. Server lehnt Inkompatible mit `compatibility.dblicious`-Range ab.
- **Capabilities sind Whitelist**: nicht im Manifest gelistete Calls werden vom Host abgewiesen — auch wenn der Plugin-Code sie versucht.
- **Dependencies**: topologisch geladen; Zyklen oder unaufloesbare Versionen ⇒ Plugin disabled mit klarer Fehlermeldung.
- **Signing**: Phase 2 optional. Ab Phase 4 (Marketplace) Pflicht — Server prueft Signatur gegen registrierte Public Keys.

### 3. Trigger-Point-Vertraege

Jeder Trigger hat einen festen Input/Output-JSON-Vertrag. **Plugin liest stdin als JSON, schreibt stdout als JSON** (Extism-Pattern).

| Trigger        | Input                                                          | Output                                                | Sync? | Fehler-Effekt                  |
|----------------|----------------------------------------------------------------|-------------------------------------------------------|-------|--------------------------------|
| `beforeSave`   | `{ entity_type, fields_before?, fields_after, user }`          | `{ fields_after?, validation? }`                      | sync  | Save abgebrochen               |
| `afterSave`    | `{ entity_type, entity, op: "create"\|"update", user }`        | `void`                                                | async | nur Audit-Log                  |
| `beforeDelete` | `{ entity_type, entity, user }`                                | `{ validation? }`                                     | sync  | Delete abgebrochen             |
| `deriveField`  | `{ entity_type, fields, target_field }`                        | `{ value: <json> }`                                   | sync  | Feld bleibt unveraendert       |
| `validate`     | `{ entity_type, fields, user }`                                | `{ errors: [{field, code, message}] }`                | sync  | Save abgebrochen, Errors im UI |
| `customAction` | `{ name, args, user, context }`                                | `{ result: <json> }`                                  | sync  | UI zeigt Fehlermeldung         |

**Fehler-Semantik**: jedes Plugin kann strukturiert
```json
{ "error": { "code": "...", "message": "...", "details": {} } }
```
zurueckgeben. Der Host wandelt das in einen GraphQL-Error mit `extensions.code` um.

**Timeout**: hart `manifest.capabilities.max_runtime_ms`; Default 200 ms fuer sync, 5000 ms fuer async. Ueberschreiten ⇒ Trigger-Fehler `timeout`.

**Reentrancy-Schutz**: ein Plugin im `beforeSave`-Trigger fuer entity_type X darf nicht `host.db.update(X, …)` aufrufen — sonst Endlosrekursion. Der Host blockiert das mit `forbidden_reentrancy`.

### 4. Host-Function-Katalog

Funktionen, die der Host dem Plugin ueber `extism::Function` exponiert. Alle sind capability-gegated und auditiert.

```text
host.db.query(query: { entity_type, filter, sort, page }) -> EntityPage
    capability: db_read enthaelt entity_type
    quota:      max 100 Calls pro Trigger-Aufruf

host.db.insert(entity_type, fields) -> EntityChangeResult
    capability: db_write enthaelt entity_type
    Reentrancy-blockiert in *Save-Triggern desselben entity_type

host.db.update(entity_type, id, fields, expected_hash?) -> EntityChangeResult
    capability: db_write enthaelt entity_type
    expected_hash analog shared::EntityHeader::hash

host.db.delete(entity_type, id, expected_hash?) -> EntityChangeResult
    capability: db_write enthaelt entity_type

host.http.fetch(method, url, headers?, body?) -> { status, headers, body }
    capability-check gegen manifest.http_fetch (Glob-Pattern)
    timeout: 5000 ms hart, nicht via Manifest weiter erhoehbar

host.log(level: "info"|"warn"|"error", message, fields?) -> void
    immer erlaubt; landet in plugin_invocations-Audit

host.now() -> ISO-8601-String
    immer erlaubt

host.crypto.random(n: u32) -> bytes
    immer erlaubt; nicht fuer Crypto-Keys gedacht

host.i18n.t(key, locale?, args?) -> String
    nur fuer customAction mit UI-Rueckmeldung

host.entity.hash(entity_type, fields) -> u64
    fuer Concurrency-Checks; gleicher Algorithmus wie shared::EntityHeader::hash
```

**Capability-Format**: `db_read`/`db_write` sind Listen von Entity-Type-Namen. `"*"` als Wildcard im Manifest erlaubt (z.B. `db_read = ["*"]`). `http_fetch` ist eine Liste von URL-Glob-Pattern.

**Audit**: jeder Host-Function-Call wird mit `(plugin_id, trigger_event, host_fn, duration_ms, outcome)` in `plugin_invocations` geloggt (siehe 2.8). Quota- oder Capability-Verletzungen sind eigene Outcome-Werte.

---

## Cross-Cutting Concerns

Diese Punkte sind keiner Phase exklusiv zugeordnet und laufen mit:

- **Observability**: ab Phase 2 strukturiertes Logging (tracing-crate), spaetestens Phase 4 Metrics-Export (OpenTelemetry).
- **Testing**: jede Phase liefert Tests. Integration-Tests gegen reales SQLite, keine Mocks fuer DB-Pfade.
- **Security**: WASM-Sandbox-Audit in Phase 2 vorbereiten, Audit-Trail in Phase 3 (jede AI-Aktion auditierbar), externer Audit in Phase 4.
- **Dokumentation**: pro Phase mind. ein Abschnitt in `CLAUDE.md` (architektonische Sicht) + README-Update.
- **i18n**: deutsche und englische `.ftl`-Files parallel pflegen; neue UI-Strings nie hardcoded.

---

## Reihenfolge & Parallelitaet

Strikt seriell waere langsam. Empfohlene Parallelisierung bei mehrkoepfigem Team:

```
0.5  ----+
         |
         v
1.x      1.x      1.x   (Builder/UI-State)
                  |
                  v
2.x ----- 2.x      (Plugins, kann parallel zu 1.5–1.7 starten)
                  |
                  v
3.x ----- 3.x      (AI/Migrationen, parallel zu 2.x machbar wenn Personal da)
                  |
                  v
4.x                (Codegen erst wenn 1–3 stabil)
```

Solo-Entwicklung: Reihenfolge wie aufgelistet, ohne Parallelisierung. Phasen sind so geschnitten, dass nach jeder Phase ein sinnvoll nutzbarer Zustand existiert (kein "halbfertiges Refactoring"-Tal).

---

## Erfolgskriterien (Definition of Done je Phase)

- **Phase 0.5**: Alle README-"Erweiterungspunkte" abgehakt oder bewusst deferred.
- **Phase 0.7**: Permissions werden serverseitig erzwungen; Client zeigt nur projizierte Sicht. `whyAllowed`-Debug-Endpoint und Audit-Log sind nutzbar.
- **Phase 1**: Ein Entity-Type laesst sich vollstaendig im UI zusammenklicken; resultierende Tabelle ist identisch zur Hand-konfigurierten Variante.
- **Phase 1.5**: Ein Property kann eine Implementations-ID vorgeben oder mehrere erlauben; nicht erlaubte Wahl wird serverseitig geblockt.
- **Phase 2**: Ein nicht-triviales Plugin (z.B. Slug-Generator + Validator) laeuft sandboxed in einem CRUD-Pfad.
- **Phase 3**: Eine `messy_orders.json` mit ~5 fremden Feldern wird per Agent in eine non-destruktive Migration uebersetzt und akzeptiert.
- **Phase 4**: `dblicious export` produziert ein Workspace, das `cargo build --release` ohne Aenderungen besteht und im Browser laeuft.
