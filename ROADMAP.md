# ROADMAP — Entwicklungsplan

Operative Umsetzung der Vision aus [`VISION.md`](./VISION.md). Diese Datei beschreibt **wie** und **in welcher Reihenfolge** gebaut wird, mit konkretem Code-Bezug pro Arbeitspaket.

**Lesehinweise**:
- Groessen-Schaetzung: **S** (1–3 Tage), **M** (1–2 Wochen), **L** (3–6 Wochen), **XL** (mehrere Monate).
- Reihenfolge ist absichtlich gewaehlt: jede Phase liefert eigenstaendigen Nutzen und ist von der naechsten **nicht** strikt abhaengig.
- "Bezug" zeigt konkrete Datei-/Modulpfade, an denen die Arbeit ansetzt.

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

## Phase 0.5 — Konsolidierung (Pre-Vision)

**Ziel**: Lose Enden aus den README-"Erweiterungspunkten" schliessen, **bevor** die grossen Vision-Phasen starten. Diese Arbeit ist klein, hochgradig wertstiftend und entkoppelt von der Vision.

| # | Paket | Groesse | Bezug | Akzeptanz |
|---|---|---|---|---|
| 0.5.1 | Server-seitiges Sort/Filter im `entities`-Query auswerten | M | `server/src/schema.rs::QueryRoot::entities`, `server/src/data.rs` | `sort_by`/`sort_dir`/`filter`-Args werden nicht mehr ignoriert; bestehende UI funktioniert ohne Aenderung |
| 0.5.2 | Spalten-Metadaten ausschliesslich vom Server | S | `client/src/routes/mod.rs::EntityListPage` (von `column_set_for` auf `fetch_columns` umstellen) | Kein Hardcoding von Spalten im Client mehr |
| 0.5.3 | Reference- und Collection-Formatter | M | `client/src/components/table/formatters.rs` | Felder mit `FieldType::Reference`/`Collection` rendern korrekt statt Rohwert |
| 0.5.4 | `LocalSource` als alternative `DataSource` | S | `client/src/components/table/data_source.rs` | Client-seitiges Sort/Filter ohne Server-Roundtrip moeglich (fuer kleine Datasets) |
| 0.5.5 | Weitere Sprachen vorbereiten | S | `client/locales/`, `client/src/i18n/mod.rs::Locale` | Mind. eine dritte Sprache (z.B. `fr`) als Skelett, dokumentiertes Add-Verfahren |
| 0.5.6 | Tests fuer den Daten-Loader und CRUD-Pfade | M | `server/src/` (neue `tests/`-Module) | `cargo test` deckt Loader-Roundtrip + Entity-CRUD ab |
| 0.5.7 | Table-Dekomposition: voll-komponiert via Shell + Bausteine | M | `client/src/components/table/` (neue Module: `shell.rs`, `top_menu.rs`/`bottom_menu.rs`, `table_view.rs`, `selection.rs`, `selection_column.rs`, `row_actions.rs`, `entity_menu.rs`, `global_filter.rs`, `pager.rs`, `page_size.rs`) | `<EntityTableShell>` legt Context an (`TableState`, `SelectionState` neu, `DataResource`, `Rc<dyn DataSource>`, `Rc<Vec<ColumnMeta>>`, `entity_type`, Auth-Caps, `FilterRegistry`); Aufruf-Code komponiert den Baum selbst (`<TopMenu><GlobalFilter/><EntityMenu>…</EntityMenu></TopMenu><Table/><BottomMenu><Pager/></BottomMenu>`); Selektion (`<SelectionColumn mode=Single\|Multi/>`), Per-Row-Aktionen (`<RowActions>{…}</RowActions>` mit `RowContext`), Bulk-Aktionen (`<EntityAction enabled_when=…/>`, `<DeleteAction/>`, `<EditAction/>`) sind alle opt-in als eigene Komponenten, nicht als Props; bestehende `view.rs::EntityTable` mit `#[deprecated(note = "use EntityTableShell + composable blocks")]` markiert; einziger Consumer `routes/mod.rs::EntityListPage` auf neue Komposition migriert. |
| 0.5.8 | Property-Filter-Pipeline: Filter-Registry + Standard-Komponenten | M | `client/src/components/table/filters/` (neu: `registry.rs`, `text_contains.rs`, `number_range.rs`, `enum_in.rs`, `bool_equals.rs`, `date_range.rs`); Konsument: `<Table>` aus 0.5.7 | Jeder Filter ist eine eigene Leptos-Komponente mit Prop `column: ColumnMeta`; Registrierung per String-ID (`"text-contains"`, `"number-range"`, …) in der `FilterRegistry`; Resolution pro Spalte: `ColumnMeta.filter_id` → Client-Default pro `FieldType` → kein Filter-UI; Filter schreiben in `FilterCriteria.predicates`, bestehende `LocalSource`/`RemoteSource`-Auswertung greift unveraendert; Custom-Filter erweitern die Registry ohne Aenderung an `<Table>`. |

**Deliverable**: README-Erweiterungstabelle ist auf "erledigt" bei den genannten Punkten. Codebase ist sauber genug, um auf ihr die Vision aufzubauen.

**Risiko**: gering. Reines Konsolidieren bekannter Arbeit.

---

## Phase 1 — Builder-Foundation (ECS-Metadaten)

**Ziel**: Reaktiver In-Memory-State fuer den Visual Builder. Heute sind `columns.toml`/`editor.toml`/`settings.toml` statisch; nach dieser Phase laesst sich der Datentyp `product`/etc. live im Editor komponieren.

**Schluesseltechnologie**: `bevy_ecs` (ohne Render/App-Loop, nur die ECS-Crate).

### Arbeitspakete

| # | Paket | Groesse | Bezug |
|---|---|---|---|
| 1.1 | `bevy_ecs` als Workspace-Dependency, neuer Crate `builder/` oder Modul `client/src/builder/` | S | `Cargo.toml`, `client/Cargo.toml` |
| 1.2 | Komponenten definieren: `Transform`, `Style`, `Interactable`, `EventTrigger`, `BoundField`, `Draggable` | M | neu: `client/src/builder/components.rs` |
| 1.3 | Bruecke ECS ↔ `shared::ColumnMeta`/`FieldType`: Reader-System exportiert ECS-State als `Vec<ColumnMeta>` | M | neu: `client/src/builder/serialize.rs` |
| 1.4 | Drag&Drop-Canvas in Leptos: Mounted ECS-World, Mouse-Events mutieren Components | L | neu: `client/src/builder/canvas.rs`, neue Route `/builder/:entity_type` |
| 1.5 | Live-Preview: gleiche `EntityTable` wie heute, gespeist aus ECS-projizierten `ColumnMeta` | M | `client/src/components/table/` (DataSource bekommt ECS-Variante) |
| 1.6 | Persistenz des Builder-State: Speichern via GraphQL-Mutation `saveEntityDesign` (analog `saveDbSchema`) | M | `server/src/schema.rs`, neue Tabelle `entity_designs` |
| 1.7 | Undo/Redo via ECS-Snapshot-Stack | S | `client/src/builder/history.rs` |

### Dependencies / Vorbedingungen

- Phase 0.5.2 (Spalten vom Server) ist hilfreich, aber **nicht** harte Voraussetzung.

### Deliverable

- Neuer Tab/Route "Designer" im Client.
- User kann fuer einen Entity-Type Spalten per Drag&Drop hinzufuegen, neuanordnen, Field-Types aendern.
- Speichern persistiert in `entity_designs`-Tabelle. Beim naechsten Server-Start wird der Builder-State geladen (zusaetzlich oder anstelle von `--data-dir`).

### Risiken

- **Bevy-ECS in WASM**: laeuft, ist aber im Web-Kontext noch ungewohnt. POC-Spike (~3 Tage) vor 1.4 empfehlen.
- **DSL fuer EventTrigger**: was darf ein `EventTrigger`-Component referenzieren? Klare Trennung "Builder beschreibt Intent, Plugin (Phase 2) implementiert Logik" einhalten.

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

- Profitieren von Phase 1 (ECS-State als Single Source of Truth), aber unabhaengig lauffaehig.

### Deliverable

- User kann eine JSON-Datei drag&droppen, bekommt einen Migrations-Vorschlag, sieht den Diff und entscheidet.
- Akzeptierte Migrationen laufen zweiphasig: Expansion zuerst, Cutover, dann Contract.
- Rollback per einem Klick moeglich, solange Contract-Phase nicht abgeschlossen.

### Risiken

- **LLM-Halluzinationen**: durch Function-Calling + Diff-Validator hart geblockt, aber Tests mit adversariellen Inputs noetig.
- **Migration-Komplexitaet**: Drop-Column-mit-Backfill-Pfad ist heikel. Erste Version darf destruktive Operationen nur als "Contract"-Schritt anbieten und nur, wenn User explizit zustimmt.
- **Cost-Control**: LLM-Calls deckeln (max. N pro Stunde pro User).

---

## Phase 4 — Codegen & Optimierung

**Ziel**: Aus finalisiertem ECS-State + locked Schema + Plugin-Bindings → eigenstaendige axum/Leptos-App ohne Builder-Overhead. Plus Polish und externe Audits.

### Arbeitspakete

| # | Paket | Groesse | Bezug |
|---|---|---|---|
| 4.1 | AST-Codegen-Crate: `quote`/`syn`/`prettyplease` basiert | L | neu: `codegen/` |
| 4.2 | Codegen-Templates: axum-Routes, async-graphql-Resolver, SeaORM-Entities, Leptos-Views | XL | wie 4.1 |
| 4.3 | `cargo-scaffold`-Integration fuer Repo-Bootstrap der generierten App | S | wie 4.1 |
| 4.4 | CLI-Command `dblicious export --target ./generated-app` | M | `cli/` |
| 4.5 | WASI-NN-Evaluation: Wechsel von `wasmtime` auf `wasmedge` fuer Plugins? Lokale Inference-POC | M | `server/src/plugins/` |
| 4.6 | Externer Security-Audit der WASM-Sandbox + AI-Pfade | (extern) | n/a |
| 4.7 | Performance-Benchmarks: Builder mit 10k UI-Knoten, CRUD mit aktiven Plugins, Migration auf 100k-Row-Tabelle | M | neu: `benches/` |
| 4.8 | Dokumentation: User-Guide, Plugin-Dev-Guide, Codegen-Guide | M | `docs/` |

### Dependencies

- Phasen 1–3 muessen weitgehend stabil sein. Codegen ist die Konsolidierung.

### Deliverable

- `dblicious export` erzeugt aus einer laufenden Designer-Session ein eigenstaendiges Rust-Workspace.
- Generierte App startet ohne `dblicious`-Builder, nur mit den selektierten Plugins und finalem Schema.
- Audit-Bericht liegt vor.
- Benchmarks zeigen, dass Builder mit hoher Knotenzahl performant bleibt (Kolumnar-ECS zahlt sich aus).

### Risiken

- **Codegen-Drift**: Templates muessen mit den Laufzeit-Crates synchron bleiben. CI-Test, der Codegen-Output baut + minimalen Smoke-Test laufen laesst, ist Pflicht.
- **WASI-NN-Switch**: WasmEdge ist weniger ergonomisch als wasmtime/Extism. Nur wenn die Inference-Anforderung real auftritt.

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
1.x      1.x      1.x   (Builder/ECS)
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
- **Phase 1**: Ein Entity-Type laesst sich vollstaendig im UI zusammenklicken; resultierende Tabelle ist identisch zur Hand-konfigurierten Variante.
- **Phase 2**: Ein nicht-triviales Plugin (z.B. Slug-Generator + Validator) laeuft sandboxed in einem CRUD-Pfad.
- **Phase 3**: Eine `messy_orders.json` mit ~5 fremden Feldern wird per Agent in eine non-destruktive Migration uebersetzt und akzeptiert.
- **Phase 4**: `dblicious export` produziert ein Workspace, das `cargo build --release` ohne Aenderungen besteht und im Browser laeuft.
