# VISION & ROADMAP

Diese Datei skizziert die strategische Weiterentwicklung von **dblicious** von einer Admin-UI mit dynamischen Entitaeten zu einem vollwertigen **Rust-Meta-Framework** mit Dual-Mode-Execution, reaktivem UI-State-Modell, WASM-Plugin-Sandbox und AI-gestuetzter Schema-Evolution.

Grundlage ist ein architektonischer Blueprint (interner Strategiereport, eingearbeitet 2026-05-13). Der Blueprint wurde **nicht 1:1 uebernommen**, sondern auf den existierenden `dblicious`-Stack gemappt. Zentrale Tech-Entscheidungen (axum, SeaORM, SQLite, Leptos) bleiben bestehen; abweichende Empfehlungen des Blueprints (Loco, SurrealDB, SurrealKit) werden weiter unten begruendet verworfen.

---

## Strategische Positionierung

Das Marktumfeld bewegt sich zwischen zwei Extremen:

- **Dynamic Low-Code-Plattformen** liefern hohe Iterationsgeschwindigkeit, leiden aber an Skalierungs-Limits, Vendor-Lock-in und Unmoeglichkeit komplexer CPU-Logik.
- **High-Performance Rust-Apps** liefern Memory-Safety, Concurrency und minimalen Footprint, brauchen aber lange Entwicklungszyklen und spezialisiertes Personal.

`dblicious` zielt darauf, beide Welten zu fusionieren: ein **kompositionsbasiertes UI-State-Modell** (heute: `examples/shop/` als statisches Set, kuenftig: reaktiver `UiTree`), eine **Multi-Mode-Execution** (heute: generische `entities`-Tabelle vs. typisierte Designer-Tabellen, kuenftig: vollwertiger Dev-Mode/Prod-Mode-Split), ein **WASM-Sandbox** fuer User-Logik und eine **AI-Schema-Resolution**.

Architekturleitsatz: **maximale Wiederverwendung reifer Open-Source-Crates**, keine Foundational Systems from scratch.

---

## Status quo (Mai 2026)

Bereits vorhanden im Repo:

- **Workspace** `shared` / `server` / `client` / `cli` mit `wasm32-unknown-unknown`-Pin und Release-Profil fuer WASM-Size.
- **Server**: axum + async-graphql, GraphiQL unter `/`, GraphQL POST `/graphql`. CORS offen fuer Dev.
- **Persistenz**: SeaORM + SQLite (in-memory oder Datei via `DBLICIOUS_DATABASE_URL`). Generische `entities`-Tabelle als universeller Speicher; optionale typisierte Tabellen ueber den Designer (`saveDbSchema` → `ddl::try_apply_schema`). Diese Persistenz wird in Phase 0.6 auf eine `Source`-Trait gehoben — sie bleibt der Default, ist aber dann eine austauschbare Implementierung neben `foreign-sqlite`/`postgres`/`rest`/usw.
- **Daten-Loader**: `--data-dir`-basiert, Format-Dispatch fuer `.json`/`.toml`, erweiterbar (YAML, Skripte).
- **Client**: Leptos CSR/WASM mit `trunk`, fine-grained signals, kein Virtual DOM, Project-Fluent-i18n.
- **Generische `EntityTable`** auf `ColumnMeta` + `Rc<dyn DataSource>`. Sort/Filter/Pagination signal-verdrahtet (Server ignoriert die Args heute).
- **DesignSystem-Trait** als Trennung Struktur/Design (heute `InlineDesign`).
- **CLI** `dblicious` fuer DB-Mutationen ohne `--data-dir`.

Was **fehlt** gegenueber der Vision:

- Kein reaktiver Builder-State (heute: statisches `examples/shop/`).
- Keine User-eingebrachte Custom-Logic (heute: kein Plugin-Mechanismus).
- Keine AI-gestuetzte Schema-Ingestion.
- Keine zweiphasige Migration (Expansion/Contract).
- Kein Codegen-Pfad zu einer eigenstaendigen Production-Binary.
- Persistenz ist hart an die mitgelieferte SQLite-Konfiguration gekoppelt — keine austauschbaren Sources (fremde DBs, REST-APIs, Datei-Quellen). Wird in Phase 0.6 aufgeloest.

---

## Architekturentscheidungen: Adoption & Abweichungen

| Blueprint-Empfehlung | dblicious-Entscheidung | Begruendung |
|---|---|---|
| **Loco** (Rails-artiges Full-Stack-MVC) | **axum + async-graphql** beibehalten | Loco zwingt MVC-Konventionen, die nicht zum GraphQL-zentrierten Modell passen. axum ist bereits produktiv, schlanker, und die `shared`-Typen sind direkt in async-graphql verdrahtet. Migrationsaufwand stuende in keinem Verhaeltnis zum Nutzen. |
| **Leptos** fuer den Visual Builder | **uebernommen** | Bereits im Einsatz. Fine-grained Signals + direktes DOM-Manipulieren passen perfekt fuer den geplanten Drag&Drop-Editor. |
| **Bevy ECS** als Metadaten-Engine | **verworfen, Konzept ohne Framework adaptiert** | Komposition-statt-Vererbung wird mit einer simplen `UiNode`-Struktur in Plain Rust + Leptos-Signals erreicht. Performance-Vorteile von ECS (kolumnarer Memory, tausende Knoten pro Frame) sind im Builder-Kontext irrelevant: eine Designer-Session zeigt 5–50 Elemente, und das Endprodukt entsteht via Codegen als fixierte Komponenten-Crate (Phase 4) — zur Laufzeit gibt es keine generische UI-Engine. Siehe Architektur-Leitprinzip "Dev/Prod-Asymmetrie" in `ROADMAP.md`. |
| **SurrealDB** (SCHEMALESS↔SCHEMAFULL toggle) | **Source-Pluggability**, SeaORM+SQLite als Default-Source | Phase 0.6 (siehe `ROADMAP.md`) hebt den DB-Layer auf eine `Source`-Trait — `managed-sqlite` (heutiges Verhalten), `foreign-sqlite` (fremde Bestands-DB), spaeter `postgres`/`mysql`/`rest`/`file`/`memory` sind austauschbare Implementierungen. Damit ist nicht das DB-Produkt fixiert, sondern das Konzept: jede Source kann mit der `entities`-Tabelle (schemaless) oder mit echten typisierten Tabellen (schemafull) arbeiten. Der DB-Wechsel ist nicht mehr Voraussetzung, sondern eine Implementierungs-Wahl pro Source. |
| **Extism** fuer WASM-Plugin-Sandbox | **uebernehmen** (Phase 3) | Extism abstrahiert wasmtime/Component-Model/WIT weg und liefert Manifest-basierte Security (`max_pages`, `allowed_hosts`, `allowed_paths`) sowie Host-Functions. Mehrsprachige Guest-PDKs (TS, Python, Go, Rust). |
| **WASI-NN** via WasmEdge | **optional, spaeter** (Phase 4+) | Lokale ML-Inference im Plugin (PyTorch/TFLite/OpenVINO). Erst sinnvoll, wenn die Plugin-Schicht steht. |
| **AI-Schema-Resolution** mit `rusty_schema_diff` | **uebernehmen** (Phase 3) | Schema-first Function Calling + deterministische Diff-Validierung. Loest das heutige manuelle Mapping bei JSON-Imports und ergaenzt den Designer-Pfad um einen AI-Assist. |
| **SurrealKit Rollouts** (Expansion/Contract) | **Konzept uebernehmen, mit SeaORM-Migrationen implementieren** | Die zweiphasige Migration (additive vor destruktiver Aenderung, mit Rollback-Pfad) ist DB-agnostisch. Wir bauen das ueber SeaORM-Migrationen oder einen schmalen eigenen Layer. |
| **AST-Codegen** zu standalone Rust-Binary | **uebernehmen** (Phase 4) | Endpunkt der Meta-Framework-Vision: `UiTree` + locked Schema → eigenstaendige axum/Leptos-App ohne Builder-Overhead. Pro User-Konfiguration wird eine fixierte Komponente generiert; zur Laufzeit findet keine dynamische Auswahl statt. |
| **Rebranding (`alloy` ist auf crates.io belegt)** | **n/a** | `dblicious` als Name etabliert, keine Kollision. |
| **Berlin-Talent-Acquisition-Strategie** | **n/a** | Ausserhalb des Repo-Scopes. |

---

## Schluesselkonzepte (gemappt auf dblicious)

### 1. Dual-Mode-Execution

**Blueprint-Idee**: Dev-Mode ist Interpreter-artig und dynamisch; Prod-Mode ist eine statisch gelinkte, optimierte Binary.

**dblicious-Mapping**:
- **Dev-Mode** = laufender Server mit `--data-dir`, generischer `entities`-Tabelle, schemalose JSON-Felder, Hot-Reload des Datensets.
- **Prod-Mode** = via Designer locked Schemas (`saveDbSchema`), strikt typisierte Tabellen, ggf. eine ueber Codegen erzeugte standalone Binary ohne Builder-UI (Phase 4).

### 2. UI-State-Modell des Visual Builders (Phase 1)

Heute sind `ColumnMeta`, `FieldType`, Editor- und Settings-Layouts statische TOML/JSON-Definitionen. Phase 1 ersetzt das durch eine **reaktive UI-State-Struktur** in Plain Rust + Leptos-Signals:

- **Komposition statt Vererbung**: ein UI-Knoten ist eine `UiNode`-Struktur mit optionalen Feldern (`event_trigger: Option<EventTrigger>`, `bound_field: Option<BoundField>`, `draggable: bool`, …). Neue Verhaltensweisen werden durch zusaetzliche Felder oder Enum-Varianten erweitert — keine Klassen-Hierarchie noetig, weil Rust keine Vererbung kennt.
- **Reaktivitaet ueber Leptos-Signals**: der gesamte Builder-State lebt in einem `RwSignal<UiTree>`; Komponenten subskribieren feingranular auf die Teile, die sie rendern.
- **Codegen-Nahe**: die Datenstruktur ist absichtlich so geschnitten, dass Phase 4 sie nahezu 1:1 in Rust-Quellcode uebersetzen kann (ein `UiNode` ⇒ ein generiertes Leptos-Component-View).

**Bewusst kein ECS-Framework**: Bevy/`hecs`/`legion` waeren fuer einen Web-Admin-Builder ueberdimensioniert. Performance-Vorteile zahlen sich erst bei vier- bis fuenfstelligen Knotenzahlen aus; der Builder zeigt 5–50 sichtbare Elemente gleichzeitig. Siehe Architektur-Leitprinzip "Dev/Prod-Asymmetrie" in `ROADMAP.md`. Falls spaeter doch hunderte Tabellen mit hunderten Spalten in *einer* Designer-Session noetig sind, kann eine indexierte `HashMap<NodeId, UiNode>` ohne grossen Umbau nachgezogen werden.

### 3. WASM-Plugin-Sandbox via Extism (Phase 2)

User-Logic (z.B. Validatoren, Hooks, Berechnungen pro Entity) laeuft als WASM-Plugin. Extism liefert:

- **Manifest-Security**: `max_pages` (Memory-Limit), `max_http_response_bytes`, `allowed_hosts`, `allowed_paths` als Whitelist.
- **Host-Functions**: kontrollierter API-Zugriff aus dem Plugin (z.B. validierte DB-Queries, niemals direkter Connection-Pool-Zugriff).
- **Multi-Language-Guest**: TypeScript, Python, Go, Rust kompilieren zu einer einheitlichen Plugin-Form.
- **Memory-Isolation**: Plugin-Memory ist vom Host getrennt, automatisch freigegeben → keine Leaks/Segfaults im Parent.

### 4. AI-Schema-Ingestion (Phase 3)

Beim Import unstrukturierter JSON-Daten oder bei Designer-Eingaben kommt ein LLM-Agent zum Einsatz, **streng beschraenkt auf strukturierte Outputs**:

1. **RAG-before-act**: Agent holt aktuelles produktives Schema (`db_schemas`-Tabelle) + neuen Payload.
2. **Expert-Routing**: klassifiziert die Aktion (neue Tabelle, neue Spalte, FK-Mapping, Semantic Match wie `user_email_address` → `email`).
3. **Schema-first Function Calling**: LLM-Antwort gegen festes JSON-Schema validiert, keine Freitext-SQL-Generierung.
4. **Deterministische Diff-Validierung** ueber `rusty_schema_diff`: erzeugt `CompatibilityReport`. Destruktive Aenderungen ohne Migrationspfad werden hart abgelehnt → Reflection-Loop fordert non-destruktive Alternative.
5. **Human-in-the-loop-Approval**: `MigrationPlan` geht vor Ausfuehrung zum User.

### 5. Versionierte Migrationen (Phase 3)

Zweiphasiges Rollout-Modell, implementiert ueber SeaORM-Migrationen:

- **Expansion**: alle additiven Aenderungen (neue Tabellen, nullable Spalten) zuerst.
- **Cutover**: neue App-Logik schaltet auf neues Schema.
- **Contract**: erst nach erfolgreichem Cutover destruktive Schritte (Spalten droppen, Legacy-Tabellen entfernen).
- **Rollback**: Reversal der In-Flight-Aenderungen jederzeit moeglich.

Lokale Tests profitieren von schnellen DB-Forks. Solange wir bei SQLite bleiben, ist ein Snapshot trivial (File-Copy bzw. in-memory `restore_from`).

### 6. AST-Codegen zur Production-Binary (Phase 4)

Single Source of Truth: finaler `UiTree` + locked SeaORM-Schemas + OpenAPI/GraphQL-Schema. Daraus generiert die Framework-Engine:

- axum-Routes und async-graphql-Resolver
- Leptos-Views ohne Builder-Overhead
- SeaORM-Entity-Typen
- `cargo`-fertiges Workspace-Layout

Werkzeuge: Rust derive/attribute macros, optional `cargo-scaffold` fuer das Repo-Bootstrap.

---

## Phasenplan (Kurzfassung)

| Phase | Ziel | Schluesselarbeit |
|---|---|---|
| **Phase 0 — IST** | Grundgerueste | axum/async-graphql/Leptos-Workspace, SeaORM/SQLite-Persistenz, `--data-dir`-Loader, Designer (`saveDbSchema`), CLI |
| **Phase 0.5 — Konsolidierung** ✅ | Lose Enden schliessen | Server-seitiges Sort/Filter, Spalten vom Server, Reference-/Collection-Formatter, `LocalSource`, Table-Dekomposition, Property-Filter-Pipeline |
| **Phase 0.7 — Auth- & Permission-Modell** | Server als Single Source of Truth fuer Permissions | `Subject`/`Resource`/`Op`/`Effect` als shared-Typen; parallel Groups+Roles; Row-Level deferred; `tenant_id NULL` als Schema-Vorbereitung |
| **Phase 1 — Builder-Foundation** | Reaktiver Visual Editor | `UiTree`/`UiNode` als Plain-Rust + Leptos-Signals; Drag&Drop-Canvas in Leptos; Projektion `UiTree → shared::ColumnMeta` |
| **Phase 1.5 — Implementations-Resolution** | Server-priorisierte Auswahl austauschbarer Komponenten | Pro Property erlaubte Filter/Editor/Formatter/Action-IDs; Client-Registries; Resolution-Reihenfolge Pflicht→Defaults→Fallback |
| **Phase 1.7 — ERP-Plattform-Bausteine** | Building Blocks fuer rechtskonforme ERP-Anwendungen | Number-Sequences (gapless), Period-Locks, GoBD-Append-Only, File-Storage, PDF, Email, State-Machine, Job-Scheduler, DSGVO-Tooling, Encryption, Aggregations, Volltextsuche, Webhooks |
| **Phase 2 — WASM-Plugin-Sandbox** | Sichere User-Logik (Server + Client) | Extism Host-SDK; Manifest-Security; **Server- UND Client-WASM-Runtime**; Host-Functions fuer DB-Lesezugriff |
| **Phase 3 — AI-Schema-Engine + sichere Migrationen** | Automatisiertes Schema-Mapping | LLM mit strict Function Calling; `rusty_schema_diff`-Validierung; zweiphasige Migrationen mit Dual-Write/Dual-Read |
| **Phase 4 — Codegen & Optimierung** | Standalone-Prod-Binaries | AST-Codegen aus `UiTree`; optionale WASI-NN; Security-Audit |

Jede Phase ist unabhaengig wertvoll. Detailliertes Arbeitspaket-Breakdown mit Bezuegen, Akzeptanzkriterien und Risiken in [`ROADMAP.md`](./ROADMAP.md).

---

## Was bewusst NICHT uebernommen wird

- **Loco**: axum + async-graphql bleibt, die GraphQL-zentrierte Architektur passt nicht zu Loco-MVC.
- **SurrealDB / SurrealKit**: SeaORM + SQLite ist produktiv. Das Dual-Schema-Konzept existiert bei uns ueber die Trennung *generische `entities`-Tabelle* vs. *typisierte Designer-Tabellen* — kein DB-Wechsel noetig.
- **Dioxus**: kein Multi-Plattform-GUI-Ziel.
- **Rebranding**: `dblicious` ist eindeutig auf crates.io.
- **Berlin-Hiring-Strategie**: ausserhalb des Repo-Scopes.

---

## Offene Fragen / zu klaeren

- **Builder-State-Lokalitaet** — geklaert: clientseitig, serverseitig nur als JSON-Blob in `entity_designs`. Optimistic-Locking via `expected_version`; CRDT-basierte Live-Kollaboration explizit out-of-scope (siehe [`ROADMAP.md` Out-of-Scope](./ROADMAP.md#bewusst-out-of-scope)).
- **AI-Provider** — offen: Lokales LLM (via WASI-NN) vs. externe API (Anthropic/OpenAI)? Hat Implikationen fuer Latency, Kosten, Datenschutz. Entscheidung in Phase 3, vermutlich startend mit externer API + Provider-Abstraktion fuer spaeteren WASI-NN-Switch.
- **Migrations-Tooling** — geklaert: eigener Layer ueber SeaORM-Migrationen (`server/src/migrations/`), siehe Phase 3.7-Spezifikation in `ROADMAP.md`.
- **Plugin-Distribution** — teilweise geklaert: DB-Tabelle `plugins` als Storage, IndexedDB-Cache im Client (pull-basiert, siehe Architektur-Vertraege §5 in `ROADMAP.md`). Discovery/Marketplace bleibt offen fuer Phase 4.

---

## Referenzen

Der zugrundeliegende Blueprint zitiert u.a.:

- [Leptos](https://leptos.dev/), [Loco](https://loco.rs/), [Dioxus](https://dioxuslabs.com/) — Frontend/Full-Stack-Frameworks
- [Bevy ECS](https://bevy.org/learn/quick-start/getting-started/ecs/) — Entity Component System (referenziert, **nicht** uebernommen — siehe Architekturentscheidungs-Tabelle und Architektur-Leitprinzip "Dev/Prod-Asymmetrie" in `ROADMAP.md`)
- [SurrealDB](https://surrealdb.com/), [surrealdb-migrations](https://github.com/Odonno/surrealdb-migrations) — Dual-Mode-DB-Konzept (referenziert, nicht uebernommen)
- [Extism](https://extism.org/) — WASM-Plugin-Framework
- [WasmEdge / WASI-NN](https://wasmedge.org/docs/category/ai-inference) — Edge-AI-Inference
- [rusty_schema_diff](https://docs.rs/rusty-schema-diff/latest/rusty_schema_diff/), [diffly](https://crates.io/crates/diffly) — Schema-Diff-Tooling
- [cargo-scaffold](https://docs.rs/cargo-scaffold) — Repo-Bootstrap
- [Jaiqu](https://github.com/AgentOps-AI/Jaiqu) — Semantic JSON-Schema-Mapping
- [rustmemodb](https://crates.io/crates/rustmemodb) — In-Memory-DB-Forking fuer Tests
