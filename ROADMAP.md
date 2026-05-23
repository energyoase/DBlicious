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

## Phase 0.6 — Source-Architektur

**Ziel**: Datenzugriff als austauschbare `Source`-Trait. DBlicious wird vom "Framework mit eingebauter SQLite" zum "Framework ueber beliebige Datenquellen". `managed-sqlite` (heutiges Verhalten) und `foreign-sqlite` (fremde Bestands-DB lesen/schreiben) sind die ersten zwei Implementierungen; PostgreSQL/MySQL/REST/Datei/In-Memory folgen als weitere Trait-Impls ohne Architektur-Bruch.

**Begruendung**: heute ist der Datenzugriff hart an *eine* SQLite-Datei gebunden, mit JSON-Blob-Storage in einer `entities`-Tabelle, ohne Composite-PKs und ohne SQL-Pushdown. Das blockiert (a) D2V2019-Bestandsdaten-Integration (Track A), (b) andere Engines, (c) Nicht-DB-Quellen. Source-Pluggability ist Voraussetzung fuer alle drei.

**Schluesseltechnologie**: ein schmaler `Source`-Trait (`init`, `list_page`, `get`, `create`, `update`, `delete`) plus ein `EntityBinding`-Datentyp (Source-Name, Locator, Composite-PK-Spalten, optionales Column-Mapping). `SourceRegistry` routet pro Entity-Type. Detail-Spec: [`docs/superpowers/specs/2026-05-19-dblicious-source-architecture-design.md`](./docs/superpowers/specs/2026-05-19-dblicious-source-architecture-design.md).

### Arbeitspakete

| # | Paket | Groesse | Bezug | Akzeptanz | Status |
|---|---|---|---|---|---|
| 0.6.B0 | `Source`-Trait, `EntityBinding`, `SourceRegistry`, Config-Loader `sources.toml`. Heutiges Verhalten als `managed-sqlite`-Source refaktoriert. `shop` migriert (eine neue Datei `sources.toml`, keine Aenderung an Entity-Layouts). | M | neu: `server/src/source/{mod,managed_sqlite}.rs`, `shared/src/source.rs`; refaktoriert: `server/src/data.rs`, `server/src/schema.rs`, `server/src/db.rs` | `cargo test --workspace` gruen ohne Test-Aenderung; `shop` verhaelt sich identisch zu vorher | ✅ |
| 0.6.B1 | `foreign-sqlite` Source-Implementierung: `sqlite_master`/`PRAGMA`-Introspektion beim Boot, Composite-PK-Support, SQL-Pushdown von Filter/Sort/Pagination, Read-only-Bindings, Column-Name-Mapping (PascalCase ↔ camelCase). | M | neu: `server/src/source/foreign_sqlite.rs`; Fixtures `server/tests/d2v_e2e.rs` + `server/tests/d2v_all_17_listable.rs` | `examples/d2v/` mit Fixture-DB bootet; alle 17 D2V-Entity-Types laden; Composite-PK-Editor-Roundtrip; `EXPLAIN QUERY PLAN` zeigt Index-Use bei vorhandenen Indizes | ✅ |
| 0.6.B2 | Weitere relationale Engines: `postgres`, `mysql` (jeweils via `sqlx` oder SeaORM-Backend). Trait-Impl, kein Architektur-Eingriff. | M-L | neu: `server/src/source/{postgres,mysql,sql}.rs`; Tests: `server/tests/source_postgres_mysql.rs` | Trait-Wiring + Boot-Erkennung + Capabilities gepinnt; CRUD via `sql::relational_*` (Backend-Tag pro Source). Echte Container-Roundtrips bleiben Hand-Smoke. | ✅ |
| 0.6.B3 | Nicht-DB-Quellen: `rest` (HTTP-API hinter generischem JSON-CRUD-Vertrag), `file` (statisches JSON/TOML), `memory` (Mock fuer Tests/Demos). | L | neu: `server/src/source/{rest,file,memory}.rs`; Tests: `server/tests/source_memory_file_rest.rs` | Drei Sources implementiert, Boot-Recognition + Capabilities + CRUD-Roundtrip (Memory/File) + Locator-Recognition (Rest). FilterCriteria-Pushdown auf REST out-of-scope (heute lokaler Rust-Fallback). | ✅ |
| 0.6.B4 | `shop` (oder weitere Examples) auf echte SQL-Spalten umgestellt statt JSON-Blob — innerhalb `managed-sqlite` ein zweiter Locator-Modus `Table`. Kein Bedarf fuer die Architektur, optional. | M | `server/src/source/managed_sqlite.rs` (Table-Locator-Pfad), `server/src/source/sql.rs` (shared helpers), `server/tests/source_managed_table_locator.rs` | Locator::Table per managed-sqlite voll CRUD-faehig inkl. Composite-PK + Sort + Filter-Pushdown; shop-Migration optional, on-demand via Designer (B5) moeglich | ✅ |
| 0.6.B5 | Designer (`saveDbSchema` / `try_apply_schema`) erzeugt Schemas via `Source::supports_ddl`-Capability; jede Source kann DDL anbieten oder ablehnen. | M | `server/src/ddl.rs`, `server/src/source/mod.rs`, `server/tests/source_apply_schema.rs` | Designer schreibt auf `managed-sqlite`; foreign-sqlite lehnt mit `read_only` ab; Konzept dokumentiert | ✅ |

### Dependencies

- Keine harten Vorbedingungen — die Phase ist ein Refactoring plus eine neue Source-Implementierung.
- Phase 0.5 (Konsolidierung) ist Voraussetzung fuer ruhige Refactoring-Arbeit (kein laufender Umbau parallel).
- **Phase 0.6.B0+B1 ist Vorbedingung fuer Track A** (`examples/d2v`-Datenport, siehe `docs/superpowers/specs/2026-05-19-d2v-data-port-design.md` §3).

### Deliverable

- `Source`-Trait mit zwei Implementierungen produktiv.
- `examples/shop` laeuft ueber `managed-sqlite`-Source (Bestandsschutz).
- `examples/d2v` laeuft ueber `foreign-sqlite`-Source gegen eine Fixture-`d2v.db`.
- Composite-PKs als Encoded-Strings im Wire-Format dokumentiert und property-getestet.
- VISION.md gibt Source-Pluggability als Architektur-Leitsatz wieder.

### Risiken

- **Boot-Reihenfolge**: Auth/Audit/Plugin/Translatable/Builder-Tabellen brauchen `managed-sqlite` *vor* dem Source-Routing. Mitigation: zwei-phasiger Boot (interne Tabellen zuerst, dann andere Sources, dann Bindings).
- **Plugin-Host-Functions** (`host.db.query` aus Phase 2.4) sind heute hart auf den einen Pool gebunden. Bei Phase-2-Arbeit muss `host.db.query(source_name, …)` werden — Notiz fuer Phase 2, hier kein Umbau.
- **Performance-Regression im Refactoring**: zusaetzlicher Trait-Dispatch pro CRUD. Im us-Bereich, vernachlaessigbar — gemessen wird trotzdem.
- **Schema-Drift bei Foreign-DBs**: introspizierter Cache veraltet, wenn externe Tools die DB aendern. Out-of-scope hier; Refresh-Mechanismus optional in B2+.

---

## Phase 0.7 — Auth- & Permission-Modell

**Ziel**: Konsolidiertes, in `shared` definiertes Permission-Modell mit dem Server als Single Source of Truth. Client-seitige Auswertung ist projiziertes Spiegelbild, niemals Autoritaet.

**Status quo (Mai 2026)**: `client/src/auth/` haelt `AuthContext` + `PermissionOp` (CRUD-Ops); Server hat SeaORM-Modelle fuer `users`/`groups`/`user_groups`/`sessions`; Loader liest `security/{users,groups}` aus `--data-dir`. Permission-Pruefung passiert heute primaer im Client — serverseitiges Enforcement ist rudimentaer und keine konsistente Schicht.

**Target-Modell**:

```
Subject  := User | Group | Role
Resource := EntityType
          | EntityProperty       (z.B. "product.price")
          | EntityInstance       (Row-Level, in 0.7 noch nicht enforced — siehe Granularitaet)
          | Action               (z.B. "exportCsv")
          | ImplementationId     (z.B. filter:"number-range")
          | Migration            (id)
Op       := Create | Read | Update | Delete | Execute | Choose
          | Approve | Cutover | Contract | Rollback                    // Migration-Ops
Effect   := Allow | Deny
```

- **Group** ist Mitgliedschafts-Einheit (User ↔ Group, n:m). **Role** ist Permission-Buendel (Role → Permissions, n:m). Beide treten als `Subject` einer Permission auf. Ein User erbt Permissions ueber drei Wege: direkt zugewiesen, ueber eine seiner Groups, oder ueber eine Role, die ihm oder einer seiner Groups zugewiesen ist. Zwei bewusst orthogonale Konzepte: Organisations-Struktur (Group, z.B. "Marketing") und Rechte-Struktur (Role, z.B. "ContentEditor") decken sich in der Praxis selten — beide werden gebraucht.
- **Granularitaet (`EntityType`/`EntityProperty`/`EntityInstance`)**: alle drei sind im Enum, aber Phase 0.7 enforced *zunaechst* nur `EntityType` und `EntityProperty`. Fuer `EntityInstance` (Row-Level) wirft der Resolver `not_implemented`; per Server-Config (`config.toml` ⇒ `permissions.enable_row_level = false`) kontrolliert. Spaeter aktivierbar, sobald die Performance-Frage (per-Row-Evaluierung vs. WHERE-Klausel-Rewriting vs. opt-in Plugin-Trigger `beforeRead`) entschieden ist. Maximale Konfigurierbarkeit ist Ziel, schrittweise Implementierung der Pfad dorthin.
- **Vererbung**: Permissions auf `EntityType` vererben auf alle `EntityProperty` desselben Typs, sofern nicht explizit ueberschrieben. Property-Permissions auf `EntityInstance` analog (sobald enforced). Spezifischere Ebene gewinnt; bei gleicher Spezifitaet gewinnt **Deny vor Allow**.
- **Choose-Permission**: aus Phase 1.5; gibt dem User das Recht, eine bestimmte `ImplementationId` (oder Wildcard `filter.*`) zu waehlen.
- **Persistenz**: flache Tabelle `permissions(subject_id, subject_kind, resource_kind, resource_id, op, effect, priority, tenant_id NULL)`. Indizierbar, auditierbar, ohne tiefe Hierarchien. Daneben Mitgliedschafts-Tabellen: `roles(id, name, ...)`, `role_assignments(subject_kind, subject_id, role_id)` (Role an User oder Group), sowie das bereits existierende `user_groups`.
- **Multi-Tenancy** (Schema-Vorbereitung, kein Enforcement in 0.7): Default ist single-tenant — `tenant_id` in allen `permissions`-Eintraegen `NULL`. Die Spalte ist von Anfang an Teil des Schemas, damit spaetere Multi-Tenant-Szenarien (gemeinsame User-Base ueber mehrere Apps, Sub-Mandanten innerhalb eines Servers) ohne Schema-Migration auskommen. Der Resolver beachtet `tenant_id` nur, wenn `config.toml` Multi-Tenancy aktiviert — Phase 0.7 implementiert den Single-Tenant-Pfad vollstaendig und reserviert den Multi-Tenant-Pfad als spaetere Erweiterung.
- **Anonymous-User**: existiert als impliziter `Subject::Anonymous` (kein Login). Default-Role `"Anonymous"` mit minimalen Read-Permissions auf oeffentlich markierte Resources. Per `auth.allow_anonymous = false` im `config.toml` abschaltbar — dann verweigert der Server alle Anfragen ohne Session-Token mit `unauthenticated`.
- **Wildcard-Grammatik** (Permission-Werte und Manifest-Capabilities): echtes Glob via `globset`-Crate (`product.*`, `*.price`, `filter.*`, `*` als Catch-All). Eine dokumentierte POSIX-Glob-Untermenge — kein `**`, kein Regex.
- **Evaluation**: Server-Resolver berechnet `effective(user, resource, op) -> Effect` pro Anfrage; Ergebnis pro Session gecached. Client bekommt eine **projizierte Liste** seiner erlaubten Operationen, darf aber nichts erzwingen.

### Arbeitspakete

| # | Paket | Groesse | Bezug | Akzeptanz | Status |
|---|---|---|---|---|---|
| 0.7.1 | `Permission`, `Subject` (User\|Group\|Role), `Resource`, `Op`, `Effect` als `shared`-Typen | S | neu: `shared/src/auth.rs` | Wire-Format gepinnt durch `shared/tests/auth_wire_format.rs` | ✅ |
| 0.7.2 | `permissions`-Tabelle (mit `tenant_id NULL`) + `roles` + `role_assignments` (SeaORM) + Loader-Format `security/{permissions,roles,role_assignments}.{toml,json}` | M | `server/src/entity/{permissions,roles,role_assignments}.rs`, `server/src/example/loader.rs` | Loader-Roundtrip-Test gruen; Role-Zuweisungen an User UND Group werden gelesen | ✅ |
| 0.7.3 | Server-Resolver `effective(user, resource, op)` mit Group-Vererbung + Role-Vererbung + Deny-Priority + Session-Cache; Row-Level wirft `not_implemented` bei deaktivierter Config | M | `server/src/auth/resolver.rs` (918 Zeilen + Inline-Tests) | Unit-Tests pro Praedikat (inkl. User-via-Group-via-Role), Integration ueber gemockte Sessions | ✅ |
| 0.7.4 | Enforcement in CRUD-Resolvern + GraphQL-Mutation-Schicht | M | `server/src/schema.rs::require_permission` an allen CRUD-Resolvern | Negative Tests: nicht autorisierter Aufruf → Error-Code `forbidden` | ✅ |
| 0.7.5 | Client-`AuthContext` aus projizierter Liste speisen | S | `client/src/auth/`; `shared::auth::EffectivePermission` neu, `AuthSession.effective: Option<Vec<…>>` additiv (`skip_serializing_if`) | UI-Gating-APIs (`can`, `can_entity_type`, `can_property`, `can_execute`, `can_choose`, `can_migration`) und LocalStorage-Persist; UI-Aufrufer migriert (15 Call-Sites in `navigation`, `editor`, `table/{shell,view}`, `routes/mod`). Wildcard-Designer-Gate bleibt Legacy. | ✅ |
| 0.7.6 | Audit-Log: alle Permission-Aenderungen + alle verweigerten Zugriffe | S | `server/src/audit.rs` | Tabelle `audit_log` enthaelt Eintraege bei jedem Deny | ✅ |
| 0.7.7 | Debug-Endpoint `whyAllowed(user, resource, op) -> Trace` | S | `server/src/schema.rs::why_allowed` + `auth::resolver::trace_effective` | Liefert geordnete Liste aller passenden Regeln + Effekte | ✅ |
| 0.7.8 | Migrations-Skript: heutiges `security/{users,groups}` → neues `permissions`-Format | S | `cli/src/main.rs::run_migrate_security` (subcommand `migrate-security`) | Idempotent, dry-run-faehig | ✅ |

**Abweichungen / Lessons learned**:
- 0.7.5 wurde **vor** 0.7.4 gezogen, weil die Client-API in der Form jetzt schon Tests verträgt und keinen Server-Zustand braucht. Mechanismus: solange `AuthSession.effective` `None` ist, faellt der Client auf das Legacy-`Permission`-Modell zurueck (CRUD-Flags pro Entity-Typ, Non-Entity-Resources permissiv). Sobald 0.7.4 die Liste fuellt, gilt strikte Membership ohne Client-Aenderung. Tests in `client/src/auth/mod.rs` decken beide Modi ab; die destruktive `clear()`-Pfad-Spur ist nur in wasm-bindgen-Tests vollstaendig pruefbar, weil `web_sys::window()` auf nativem Test-Target panikt.
- `AuthSession.effective` ist additiv und mit `skip_serializing_if = "Option::is_none"` serialisiert — das bestehende Wire-Format und alle GraphQL-Queries bleiben unveraendert. Der Server setzt das Feld heute explizit auf `None` (Marker fuer 0.7.4).

### Dependencies

- Keine harten. Vorbedingung fuer Phase 1.5 (Choose-Permissions) und impliziter Unterbau von Phase 2 (Plugin-Aufruf-Permissions) sowie Phase 3 (Migration-Approval-Permissions).

### Deliverable

- Server-Enforcement ist die Wahrheit; Client zeigt nur projizierte Sicht.
- `permissions`-Tabelle + Loader-Format ist in `examples/shop/` benutzt und dokumentiert.
- Audit-Log + `whyAllowed`-Debug-Endpoint sind nutzbar.

### Risiken

- **Performance des Resolvers**: pro CRUD-Aufruf darf nicht erneut die ganze Permission-Tabelle gescannt werden. Session-Cache mit Invalidierung bei Permission-Aenderung ist Pflicht.
- **Komplexitaet von Deny-vor-Allow + Wildcards + Groups+Roles**: kann zu schwer debuggbaren Effekten fuehren. Der `whyAllowed`-Endpoint ist die Notbremse und muss die Herkunfts-Kette ausweisen (direkt / via Group X / via Role Y / via Role Y an Group X).
- **Migrations-Pfad** des bestehenden security-Formats: Bestandsdaten unter `examples/shop/security/` muessen automatisch konvertiert werden, sonst bricht der Loader nach 0.7.2.
- **Row-Level deferred**: das Enum erlaubt `EntityInstance`-Permissions, die der Resolver heute ablehnt. Risiko: User/Designer modellieren Regeln, die ins Leere laufen. Mitigation: Loader warnt mit `WARN row-level permission ignored (enable_row_level=false)`.

---

## Phase 1 — Builder-Foundation (Reaktive UI-State)

**Ziel**: Reaktiver In-Memory-State fuer den Visual Builder. Heute sind `columns.toml`/`editor.toml`/`settings.toml` statisch; nach dieser Phase laesst sich der Datentyp `product`/etc. live im Editor komponieren.

**Schluesseltechnologie**: **Plain Rust + Leptos-Signals**. Der Builder-State ist eine `RwSignal<UiTree>` mit einer simplen, codegen-naheliegenden Datenstruktur (siehe Spezifikation unten). Kein ECS-Framework — begruendet im Architektur-Leitprinzip "Dev/Prod-Asymmetrie": die Designer-Session arbeitet mit kleinen Knoten-Mengen (5–50 sichtbar), und Performance-Wunder wie kolumnarer Memory zahlen sich erst bei vier- bis fuenfstelligen Knoten aus. Das Endprodukt entsteht via Codegen als fixierte Komponenten-Crate.

**Status**: 1.1 + 1.2 + 1.3 + 1.4 + 1.5 + 1.7 erledigt (2026-05-13). Datenstrukturen, Signal-Wrapper, EventTrigger-Vertrag, Guard-Mini-DSL, `UiTree`→`Vec<ColumnMeta>`-Projektion, `BuilderCanvas` mit Drag&Drop + Toolbar + Route `/builder/:entity_type`, `BuilderPreviewSource`, Undo/Redo. **Offen**: 1.6 (Persistenz `saveEntityDesign`, M) — Server-Schema + GraphQL-Mutation + Live-Reload, deshalb separat.

### Arbeitspakete

| # | Paket | Groesse | Bezug | Status |
|---|---|---|---|---|
| 1.1 | `UiTree`/`UiNode`-Datenstruktur in `client/src/builder/` (Rust-Struct, `RwSignal<UiTree>`) | S | neu: `client/src/builder/mod.rs`, `client/src/builder/tree.rs` | ✅ |
| 1.2 | `UiNode`-Felder definieren: `transform`, `style`, `bound_field`, `event_trigger: Option<EventTrigger>`, `draggable: bool`, `children: Vec<UiNode>` | M | neu: `client/src/builder/node.rs`; `EventTrigger`/`GuardExpr` in `shared/src/builder/`; Guard-Parser+Evaluator in `shared/src/builder/guard.rs` | ✅ |
| 1.3 | Bruecke `UiTree` ↔ `shared::ColumnMeta`/`FieldType`: Projektions-Funktion exportiert UI-State als `Vec<ColumnMeta>` | M | neu: `client/src/builder/project.rs`; `BoundField` um optionale `field_type`/`label_key`/`sortable`/`filterable` erweitert (alle `skip_serializing_if`, Wire-Vertrag aus Phase 1.2 unveraendert) | ✅ |
| 1.4 | Drag&Drop-Canvas in Leptos: Mouse-Events mutieren `UiTree`-Signal, Leptos rendert reaktiv | L | neu: `client/src/builder/canvas.rs` (`BuilderCanvas` + `BuilderNodeView`, pointer-events am Container, Drag = 1 History-Snapshot), neue Route `/builder/:entity_type` in `app.rs` + `routes::BuilderPage` mit Live-Preview ueber `BuilderPreviewSource`; i18n-Strings `builder-*` in `de/en/fr` | ✅ |
| 1.5 | Live-Preview: gleiche `EntityTable` wie heute, gespeist aus den per Projektion erzeugten `ColumnMeta` | M | neu: `client/src/components/table/builder_preview.rs` (`BuilderPreviewSource` wrappt `LocalSource`, `synthesize_preview_rows` erzeugt deterministische Platzhalterwerte pro `FieldType`) | ✅ |
| 1.6 | Persistenz des Builder-State: GraphQL-Mutation `saveEntityDesign` + Subscription `entityDesignUpdated` + CLI-Befehl `dblicious design reset <entity_type>` (idempotent, dry-run-faehig) | M | `server/src/{schema,data,events}.rs`, Tabelle `entity_designs`, `cli/src/main.rs::handle_design`; Tests `server/tests/design_reset.rs` + `design_subscription.rs` | ✅ |
| 1.7 | Undo/Redo via `Vec<UiTree>`-Snapshot-Stack | S | neu: `client/src/builder/history.rs` (`BuilderHistory` mit `past`/`future`-Stacks, Capacity-Cap `100`, FIFO-Drop am Boden; Convenience `mutate_with_history` / `undo` / `redo`) | ✅ |

**Abweichungen / Lessons learned**:
- Paket 1.2 wurde leicht erweitert, weil der Architektur-Vertrag (Abschnitt "Builder ↔ Plugin") `EventTrigger`/`EventKind`/`TriggerTarget`/`GuardExpr` ohnehin in `shared/src/builder.rs` verlangt und den Guard-Parser unter `shared/src/builder/guard.rs` als Teil dieses Pakets nennt. Beide Wege wurden gleichzeitig erledigt — das Wire-Format der Event-Vertraege ist nun in `shared/tests/builder_wire_format.rs` gepinnt; der Guard hat Parser **und** einen kleinen Evaluator (JS-aehnliche Truthiness, Typ-Mismatch ⇒ `false` bei `Eq`).
- `UiTree`/`UiNode` leben (wie in der Roadmap angegeben) im Client. Sollte Phase 1.6 ergeben, dass `saveEntityDesign` denselben JSON-Tree als Wire-Format akzeptieren muss, koennen sie ohne API-Bruch nach `shared` gehoben werden — die Serialisierung folgt bereits den `camelCase`/`skip_serializing_if`-Konventionen aus dem Roadmap-Beispiel (`"id": 42`, `"tokenRef": "table_cell"`, optionale Felder weggelassen).
- Paket 1.3 brauchte einen Platz fuer die projektions-relevanten Metadaten. Loesung: `BoundField` haelt nur den Binding-Key strikt, optionale Override-Felder (`field_type`/`label_key`/`sortable`/`filterable`) sind durchgehend `skip_serializing_if = "Option::is_none"`, damit der minimale Designer-Stand aus Phase 1.2 (`{"key": "..."}`) unveraendert auf der Leitung bleibt. Defaults der Projektion: `label_key = "column.<key>"`, `sortable/filterable = FieldType::is_scalar()`.
- Paket 1.5 wurde in zwei Schichten zerlegt: `synthesize_preview_rows` (reine Datenfunktion ohne Leptos-Bezug, gut testbar) und `BuilderPreviewSource` (DataSource-Wrapper um `LocalSource`). Die Quelle delegiert Sort/Filter/Pagination an die bestehende `LocalSource`-Auswertung; der Builder-Host konstruiert die Quelle via `Memo` aus dem `UiTreeSignal` neu, sobald sich der Tree aendert.
- Paket 1.7 nutzt Leptos-Signals fuer beide Stacks und ist deshalb `Copy`-faehig (Context-tauglich). Tests koennen Signals ausserhalb einer `App` instantiieren, indem sie `Owner::new().with(|| …)` umrahmen — analoge Technik wird auch fuer zukuenftige Builder-Tests funktionieren.
- Paket 1.4 setzt das Drag-Snapshot-Pattern bewusst auf "ein Snapshot pro `pointerdown`", nicht pro `pointermove`. Pointermove mutiert das `UiTreeSignal` ohne weiteren History-Eintrag — sonst kaeme pro Pixel-Schritt ein Snapshot in den Stack. Add/Delete-Aktionen sind je ein eigener Snapshot ueber `mutate_with_history`.
- Die Live-Preview im `BuilderPage` darf **kein** `Memo<Rc<dyn DataSource>>` benutzen — `Rc` ist `!Send`, `Memo` braucht aber `Send + Sync`. Loesung: die `BuilderPreviewSource` wird direkt in der Render-Closure von `EntityTableShell` neu konstruiert; Reaktivitaet kommt vom `tree_sig.tree.with(...)` darin.
- 0.7.5 wartet auf 0.7.4: der Client-Code ist fertig, aber die UI-Komponenten gaten **noch immer** ueber die Legacy-API (`is_allowed`/`PermissionOp`). Migration auf die neue `can_*`-API ist eine reine Search-and-Replace-Operation, sobald der Server die Projektion liefert.

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

**Verhaeltnis zu `db_schemas`** (bestehend aus dem Pre-Phase-0-Designer): die zwei Tabellen sind **orthogonal, koennen koexistieren und sollen sich nicht widersprechen**.

- `db_schemas` (bestehend) speichert *typisierte Tabellen-DDL* — Resultat von `saveDbSchema` ⇒ `ddl::try_apply_schema`. Optional: wer kein Schema setzt, lebt mit der generischen `entities`-Tabelle.
- `entity_designs` (Phase 1.6) speichert das *UI/Editor/Settings/Columns-Metadatum* — den Builder-Output. Pflicht, sobald der Designer benutzt wird.
- Wenn beide fuer denselben `entity_type` existieren, ist die Spalten-Wahrheit `entity_designs.projection.columns`. Beim `saveEntityDesign` validiert der Server, dass `projection.columns` mit dem `db_schemas`-Schema vereinbar ist (kompatible Field-Types, keine im Schema fehlenden Pflichtspalten). Konflikt ⇒ Fehler `design_schema_mismatch`. Empfehlung: `saveEntityDesign` mit optionalem `apply_schema_migration: bool`-Flag, das eine `db_schemas`-Aktualisierung mit anstoesst; ohne das Flag bleibt `db_schemas` unveraendert.
- Phase 3 (Migrationen) operiert primaer auf `db_schemas`-Veraenderungen; `entity_designs.projection.columns` wird im selben Migration-Schritt mitmigriert (siehe Spezifikation Phase 3 — `MigrationStep::AddColumn`/`DropColumn` projizieren auf beide).

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

## Phase 1.7 — ERP-Plattform-Bausteine

**Ziel**: Plattform-Features, die echte ERP-Anwendungen brauchen und nicht sauber als User-Entity oder Plugin abbildbar sind. Liefert die Building Blocks fuer Rechnungsstellung (DE/EU rechtskonform), Belegverwaltung, Workflows, Background-Verarbeitung, Reporting, Compliance.

**Begruendung**: ohne diese Schicht zwingt ein ERP-Bauer entweder Pattern in Plugins, die transaktionssicher nicht abbildbar sind (Gapless-Number-Sequence), oder muss Framework-Internals umgehen (File-Storage). Phase 1.7 liegt **vor Phase 2**, weil Plugins die meisten dieser Bausteine als Host-Functions konsumieren.

**Schluesseltechnologie**: vorhandener Stack, ergaenzt um wenige fokussierte Crates: `typst-cli`/`lopdf` (PDF), `lettre` (SMTP), `apalis` oder `tokio-cron-scheduler` (Job-Scheduler), SeaORM FTS5-Erweiterung (Volltextsuche), optional `aws-sdk-s3` (Storage-Backend).

### Arbeitspakete

#### A — Buchhaltung & Finanzen

| # | Paket | Groesse | Bezug | Akzeptanz |
|---|---|---|---|---|
| 1.7.1 | Number-Sequence-Service: gapless, concurrent-safe, FY-Reset; Tabelle `number_sequences(scope, year, current, format_template)` | M | `server/src/sequences/{mod,format}.rs`, `server/src/entity/number_sequences.rs`; GraphQL `nextNumber(scope, year?) -> String`; Tests `server/tests/sequences.rs` | ✅ Property-Test 1000-paralleler Aufrufe gapless; FY-Reset via `reset_sequence` |
| 1.7.2 | FX-Rate-Store + Conversion-Helper; Tabelle `fx_rates(date, from, to, rate, source)` mit historischen Saetzen; konfigurierbarer Provider (ECB Daily-Feed o.ae.) | M | neu: `server/src/fx/`; `host.fx.convert(amount, from, to, date?)`-Host-Function | Historische Buchung mit damaligem Kurs reproduzierbar; Banker-Rounding korrekt |
| 1.7.3 | Period-Locks: pro Entity-Typ konfigurierbare Sperre nach Datum; CRUD wirft `period_locked` bei Schreibzugriff auf gesperrte Datensaetze; Lock per CLI/UI durch berechtigten User | M | `server/src/schema.rs`, neue Tabelle `period_locks` | Monats-/Quartals-/Jahresabschluss moeglich; Backdating geblockt |
| 1.7.4 | GoBD-Unveraenderbarkeit als Entity-Setting: `EntitySettings.append_only = true` ⇒ Update/Delete liefern `gobd_protected`-Fehler; nur "Storno"-Operation (neue Gegenbuchung, alter Datensatz bleibt) zulaessig | M | `shared/src/settings.rs`, `server/src/schema.rs` | Gebuchte Rechnung kann nicht editiert werden, nur storniert |

#### B — Workflow & Prozesse

| # | Paket | Groesse | Bezug | Akzeptanz |
|---|---|---|---|---|
| 1.7.5 | State-Machine-Engine: pro Entity-Typ optionale `state_machine`-Konfiguration mit Transitionen, Guards (Mini-DSL wie `GuardExpr`), Permission pro Uebergang | L | neu: `shared/src/state_machine.rs`, `server/src/sm/` | Invoice geht nur per `post`-Transition von `draft` → `posted`; Permission `Invoice.post` erforderlich; Audit-Eintrag pro Transition |
| 1.7.6 | Approval-Workflow auf State-Machine: Multi-Stage-Approvals, Delegation, Stellvertreter-Regelung | M | wie 1.7.5 + `server/src/approval/` | Bestellung > X € braucht Vorgesetzten-Bestaetigung; Delegations-Pfad transparent im Audit |
| 1.7.7 | Background-Job-Scheduler: cron-style + adhoc; Retries; Audit pro Job; Job-Code ist Plugin oder Builtin | L | `server/src/jobs/`, `server/src/entity/{jobs,job_runs}.rs`; Tests `server/tests/jobs.rs`. MVP: trigger_now + Retry + Audit (eigene Tabelle job_runs wegen duration_ms + attempt). Cron-Loop ueber `cron`-Crate (5- oder 6-Felder; intern 6-Felder mit Sekunden-Prefix); `is_due` + `run_scheduler_tick` (testbar mit fixierter `now`) + `start_scheduler_loop` (Tokio-Spawn aus `main.rs`, Tick 30s, ueberlebt fehlende Handler & Disabled-Races). Plugin/Script-Handler bleiben Folge-Item (Phase 2 / Q0009). | ✅ MVP + Cron-Loop; Plugin/Script-Handler offen |

#### C — Dokumente & Kommunikation

| # | Paket | Groesse | Bezug | Akzeptanz |
|---|---|---|---|---|
| 1.7.8 | File-Storage-Abstraction mit pluggable Backends (Local-FS, S3, MinIO); Anhang-Tabelle `attachments(entity_type, entity_id, blob_ref, mime, size, hash)` | M | `server/src/storage/{mod,local_fs,s3}.rs`, `server/src/entity/attachments.rs`; sha2 dep; Tests `server/tests/attachments.rs` + `server/tests/storage_s3.rs`. Local-FS + S3/MinIO-Backend (aws-sdk-s3, `endpoint_url` + `force_path_style` fuer MinIO, `rt-tokio`+`rustls`). S3-Konstruktor + `dyn Storage`-Konformitaet ohne I/O getestet; echter put/get/delete-Roundtrip als `#[ignore]`-Smoke gegen MinIO (env `DBLICIOUS_S3_TEST_*`) — im Dev-Env mangels Docker nicht lokal verifiziert. host.storage.*-Host-Functions kommen mit Plugins (Phase 2). | ✅ Local-FS + S3 (MinIO-Roundtrip als ignore-Smoke); Streaming-Upload offen |
| 1.7.9 | PDF-Generierung via Template (typst preferred, fallback wkhtmltopdf); Templates im neuen `pdf_templates`-Eintrag oder per Designer pflegbar | M | `server/src/pdf/{mod,stub}.rs`; Tests `server/tests/pdf_stub.rs`. PdfRenderer-Trait + StubRenderer (ASCII-PDF mit `%PDF-`-Magic) heute; Typst-Backend als Folge-Item — typst-Crate ~30MB. pdf_templates-Tabelle + Designer als Folge. | 🟡 Trait + Stub; Typst-Backend offen |
| 1.7.10 | Email-Versand mit SMTP-Config + Template-Rendering; Bounce-Handling; Versand-Audit; Doku-Pflichthinweis fuer DKIM/SPF | M | `server/src/email/{mod,stub,smtp}.rs`; Audit kind=`email_sent`/`email_failed`; Tests `server/tests/email_stub.rs` (4) + `server/tests/email_smtp.rs` (9). EmailSender-Trait + StubSender + SmtpSender (lettre 0.11 mit rustls-tls). `SmtpSender::stub(...)` deckt den vollen `EmailMessage → lettre::Message → MIME`-Pfad via `AsyncStubTransport` ohne echten Mailserver ab. Template-Rendering / Bounce-Handling / DKIM/SPF-Operator-Handbuch bleiben Folge-Items. | ✅ Stub + SMTP (lettre); Bounce + DKIM-Doku offen |
| 1.7.11 | Digitale Signatur (eIDAS-fortgeschritten) via X.509-Keypair pro Tenant/User; Pflicht ab XRechnung-Versand | M | neu: `server/src/signing/`, integriert in 1.7.9 + 1.7.10 | Signierte XRechnung-XML wird von externem Validator akzeptiert |

#### D — Reporting & Search

| # | Paket | Groesse | Bezug | Akzeptanz |
|---|---|---|---|---|
| 1.7.12 | Aggregation-Queries: GraphQL-Resolver `entityAggregation(entity_type, group_by, measures: [{kind, field}])` mit Sum/Avg/Count/Min/Max/Pivot | M | `server/src/schema.rs` | Umsatz pro Monat per einem GraphQL-Call moeglich; Group-By auf Joins (Kunde) funktioniert |
| 1.7.13 | Volltextsuche: SQLite FTS5 ueber `entities`-Tabelle und ausgewaehlte typisierte Tabellen; konfigurierbar pro Entity-Typ | M | `server/src/search/` | Globale Suchbox findet "Rechnung 2024-0042" oder "Mueller GmbH" ueber alle Entities |
| 1.7.14 | Excel/CSV-Bulk-Export pro Entity-Liste mit aktiven Filtern/Sortierung; Permission-gegated | S | `server/src/export/mod.rs` (entries_to_csv + export_csv + entries_to_xlsx + export_xlsx); 12 Inline-Tests (7 CSV/RFC-4180 + 5 XLSX-Roundtrip via `calamine` dev-dep). XLSX-Typed-Cells (String/Number/Bool/Null + Array/Object als kompaktes JSON) ueber `rust_xlsxwriter`. Filter/Sort-Pushdown bereits in `entities_page_raw`; **GraphQL-Resolver-Anbindung an Filter-UI** (Permission-Gating + Streaming-Download) bleibt Folge-Item. | ✅ Service-Layer (CSV+XLSX); Resolver-Anbindung offen |
| 1.7.15 | Bulk-Import mit Validierung, Dry-Run, transaktionalem Rollback; CSV/Excel; pro-Spalten-Mapping-UI | M | neu: `server/src/import/`; CLI + UI | 5000 Kunden aus CSV importierbar mit Validierungs-Report; Fehler rollbacken |

#### E — Compliance & Recht

| # | Paket | Groesse | Bezug | Akzeptanz |
|---|---|---|---|---|
| 1.7.16 | DSGVO-Tooling: Subject-Daten-Export (`exportUserData(subject_id) -> ZIP`); Anonymisierung mit GoBD-Konflikt-Resolution (Pseudonymisierung gebuchter Datensaetze statt Loeschung) | M | neu: `server/src/gdpr/` | Auskunfts-Anfrage in <24h erfuellbar; Loeschung respektiert GoBD-Aufbewahrungsfristen mit Pseudonymisierung |
| 1.7.17 | Encryption-at-Rest: SQLCipher als Build-Option + Field-Level-Encryption fuer markierte Felder (`ColumnMeta.encrypted = true`) | M | `server/Cargo.toml`, `shared/src/lib.rs` | Datei-DB ohne Key nicht lesbar; sensitive Felder (Bankverbindung, Personaldaten) mit Per-Field-Key |
| 1.7.18 | Long-Term-Archive: PDF/A-Konvertierung beim Buchen; immutable WORM-Storage (Filesystem-Read-Only-Mount oder S3-Object-Lock) | M | wie 1.7.8 + 1.7.9, eigener Archiv-Pfad | 10-Jahres-Pflicht-Aufbewahrung machbar; Audit zeigt Archivierungs-Zeitpunkt; Original-Hash bleibt verifizierbar |

#### F — Integration

| # | Paket | Groesse | Bezug | Akzeptanz |
|---|---|---|---|---|
| 1.7.19 | Webhooks: externe Subscriber auf CRUD-Events; Tabelle `webhooks(url, event_pattern, secret, retry_policy)`; HMAC-Signatur pro Delivery; Delivery-Audit | M | neu: `server/src/webhooks/` | Externes System wird ueber neue Rechnung via POST informiert; Retry bei 5xx; Signatur per Secret verifizierbar |
| 1.7.20 | Hierarchien-Helper: rekursive CTE-Queries fuer Tree-FieldType; Cycle-Detection; Path-Materialisierung; neuer FieldType `Tree { parent_field: String }` | M | `server/src/data.rs`, `shared/src/lib.rs` | Kontenplan mit beliebiger Tiefe abbildbar; Drill-Down (alle Kinder von Konto 4400) funktioniert |
| 1.7.21 | OpenAPI/REST-Adapter neben GraphQL fuer Konsumenten ohne GraphQL-Support | M | neu: `server/src/rest/` | `/api/v1/products` liefert paginierte JSON-Liste; OpenAPI-Schema-Doc generiert |

### Dependencies

- Phase 0.7 (Auth) ist Vorbedingung fuer 1.7.4 (Append-Only-Permissions), 1.7.5 (Permissions auf State-Transitions), 1.7.6 (Approvals), 1.7.14 (Export-Permissions).
- Phase 1.5 (Implementations-Resolution) ist hilfreich fuer 1.7.8 (Storage-Backend-Wahl), 1.7.10 (Email-Provider-Wahl), 1.7.13 (Search-Backend-Wahl).
- **Plugins (Phase 2) konsumieren viele 1.7-Features als Host-Functions** — daher 1.7 vor 2.

### Deliverable

- Eine Demo-ERP-Anwendung (Rechnungsstellung, Kunden, Produkte, Mahnungen) ist mit vertretbarem Aufwand auf der Plattform baubar.
- DE-rechtskonform: gapless Nummerierung, GoBD-Append-Only, DSGVO-Auskunft.
- E-Rechnung (XRechnung/ZUGFeRD) ist mit zusaetzlichem User-Plugin moeglich.

### Risiken

- **Scope-Explosion**: 21 Arbeitspakete sind viel. Mitigation: Sub-Phasen A–F koennen unabhaengig laufen; **A (Finanzen) + C (Dokumente) + E.1 (DSGVO) als MVP**, Rest schrittweise.
- **PDF/A + eIDAS-Komplexitaet** (1.7.11, 1.7.18): koennen sich aufblasen. Mitigation: erste Version nur PDF (kein /A), Signing als optionales Modul.
- **Job-Scheduler vs. Plugin-async-Triggers**: ueberlappende Mechaniken. Klar abgrenzen: Scheduler ist **zeit-getriggerte** Jobs (cron), Plugin-Triggers sind **event-getriggerte** (afterSave). Doku in Architektur-Vertraegen ergaenzen, wenn 1.7.7 gebaut wird.
- **GoBD vs. DSGVO-Konflikt**: das "Recht auf Loeschung" kollidiert mit der Pflicht zur Aufbewahrung gebuchter Belege. 1.7.16 muss Pseudonymisierung statt Loeschung implementieren und juristische Beratung in der Doku verweisen.

### Empfohlene Reihenfolge innerhalb Phase 1.7

**MVP-Pfad — rechtskonforme Rechnungsstellung**: 1.7.1 → 1.7.4 → 1.7.8 → 1.7.9 → 1.7.10 → 1.7.3 → 1.7.16.

**Erweiterung — Workflow & Auswertung**: 1.7.5 → 1.7.12 → 1.7.13 → 1.7.14 → 1.7.15.

**Spaeter — produktive Reife**: 1.7.6, 1.7.7, 1.7.2, 1.7.11, 1.7.17, 1.7.18, 1.7.19, 1.7.20, 1.7.21.

---

## Phase 1.8 — UI-Patterns (Wiederverwendbare Anwendungs-Muster)

**Ziel**: Wiederkehrende UI-Patterns, die echte ERP-Anwendungen brauchen und in der heutigen `EntityTable`-orientierten Architektur kein Pendant haben. Erkannt beim Inventar von D2V2019 (UWP `*Controls/`-Ordner) und konsolidiert in [`docs/superpowers/specs/2026-05-20-d2v-on-dblicious-gap-analysis.md`](./docs/superpowers/specs/2026-05-20-d2v-on-dblicious-gap-analysis.md) §4.1+4.2. Die Patterns sind **nicht D2V-spezifisch** — jede ernsthafte ERP-Anwendung braucht sie.

**Begruendung**: Track-C-Funktions-Ports (T1-T23 aus dem D2V-Datenport-Spec) sind ohne diese Patterns nicht umsetzbar — `T5 GenerateAccountEntries` braucht U2 (Save-Report), `T7 Calculations` braucht U3 (Report-View), `T16 Reconciliation` braucht U4 (Wizard) + U6 (Long-Running-Action).

### Arbeitspakete

| # | Paket | Groesse | Trigger | Status |
|---|---|---|---|---|
| U1 | **Reference-Picker** mit `display_template` + `search_columns` + Autocomplete; ersetzt heutiges ID-als-Text-Rendering von `FieldType::Reference` | M | jede D2V-Form (T3, T11–T12); plus Phase 1.5 Picker-Registry | Plan da: [`docs/superpowers/plans/2026-05-20-u1-reference-picker.md`](./docs/superpowers/plans/2026-05-20-u1-reference-picker.md) — empfohlen in Phase 0.6 ziehbar |
| U2 | **Save-Report / Preview-vor-Commit**: `previewSave` GraphQL-Mutation liefert Diff aller Aenderungen inkl. abgeleiteter Berechnungen; Client-Modal zur Bestaetigung | M-L | T5/T6 (`GenerateAccountEntries`), T15 (Bulk-Import) | offen |
| U3 | **Report-View-Component**: deklarative Cross-Table-Sicht (Tree/Pivot/Linear) auf Basis von Phase 1.7.12 Aggregationen | L | T6 (Account-View), T7 (Calculations), T17 (SuSa) | offen |
| U4 | **Multi-Step-Wizard**: `wizard.json` deklariert Schritte + Validatoren + Computed-Inputs; Server-State pro Wizard-Session | M | T16 (Reconciliation), T13 (CSV-Import-Mapping), T18 (Jahresabschluss-Setup) | offen |
| U5 | **App-Context-Channel**: `shared::AppContext { active_year, … }` als reaktiver Client-Signal + Server-Header; Entity-Settings koennen Context-Filter referenzieren | S-M | T20 (YearSelector), alle Buchungs-Listen | offen |
| U6 | **Long-Running-Action-Pipeline**: Action-Registry + Server-`Stream<ActionEvent>` + Client-Progress-Modal + Final-Report | M | T5 (Regenerate), T10 (Invert), T16 (Reconciliation) | offen |
| U7 | **Filter-Builder + Saved-Filters**: boolsche Kombinatoren in `FilterCriteria`, Drag&Drop-Builder, Per-User-Persistenz | M | T3 (Buchungs-Suche), generisch fuer alle Listen | offen, koennte in Phase 1 (Builder-Canvas) integriert werden |
| F1 | **Operations-Resource**: neue GraphQL-Resource `operations` neben `entities`; `OperationDefinition` als Top-Level-Konzept; verbindet sich mit U6 + Phase-2-Plugin-Triggers | M-L | T1, T10, T13, T16 — Cross-Entity-Operationen aus `FunctionControls/` | offen |
| F2 | **Single-Entity-Excel-Template**: pro Buchungs-Zeile XLSX-Export mit gestaltetem Layout (Voucher) | S | T19 (Voucher) | offen |

### Dependencies

- **U1** ist die hoechste Hebelwirkung mit kleinster Groesse — Empfehlung: in Phase 0.6 mitziehen (Plan existiert). Verbindet sich mit Phase 1.5 (Picker-Registry).
- **U5** ist klein und wirkt auf alle Listen — kann parallel zu Phase 0.7 laufen.
- **U2** ist Vorbedingung fuer T5/T6 (`GenerateAccountEntries`-Funktions-Port).
- **U3** baut auf Phase 1.7.12 (Aggregation-Queries).
- **U6** verbindet sich mit Phase 2 (Plugin-Triggers liefern Long-Running-Actions).
- **F1** Operations sind eine alternative Resource-Achse zu `entities`; eigentliche Wirkung erst mit Plugins.

### Empfohlene Reihenfolge

**MVP-Pfad — D2V-Editor-Erfahrung**: U1 (Phase-0.6-Anhaengsel) → U5 (parallel) → U2 → U6 → U3 → U4 → F1 → F2 → U7.

Pakete sind groesstenteils unabhaengig; mehrere parallel moeglich.

### Deliverable

- Echte ERP-UI-Patterns als wiederverwendbare Bausteine in dblicious.
- Track-C-Funktions-Ports werden umsetzbar, ohne pro Funktion Patterns neu zu erfinden.
- Quelle: Gap-Analyse [`2026-05-20-d2v-on-dblicious-gap-analysis.md`](./docs/superpowers/specs/2026-05-20-d2v-on-dblicious-gap-analysis.md) §4-§6.

### Risiken

- **Scope-Explosion**: 9 Pakete. Mitigation: U1+U5 zuerst (klein, hoher Hebel), Rest schrittweise getrieben von Track-C-Bedarf.
- **Patterns sind generisch — sollen sie wirklich heute spezifiziert werden?** Argument dafuer: ohne sie ist Track C blockiert. Argument dagegen: vorausgreifende Generalisierung kann zu falschen Abstraktionen fuehren. Mitigation: U2/U3/U4/U6 erst dann committen, wenn das erste Track-C-Item sie braucht (Pull statt Push).

---

## Schema-Sprache-Backlog (G1-G7)

**Quelle**: [`docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md`](./docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md). Sieben Konzept-Luecken in `shared::DbSchema`, gesammelt beim D2V-Ziel-Schema-Design. **Kein eigenständiger Phasenplatz** — Items werden gepullt, wenn ein konkreter Trigger (Domain-Anwendung) sie braucht.

| ID | Paket | Groesse | Default-Prioritaet | Plan |
|---|---|---|---|---|
| G1 | **Soft-Delete** als first-class (`AuditRole::DeletedAt`, `EntitySettings.soft_delete`, `restore`/`purge` Mutationen, Permission `Restore`) | M | **mittel** (hoch sobald Phase 1.7.4 GoBD trigert) | [g1-soft-delete-first-class.md](./docs/superpowers/plans/2026-05-20-g1-soft-delete-first-class.md) |
| G2 | **CHECK-Constraints**: `DbColumn.check` + `DbTable.checks` als typisierter Mini-AST (`Compare`, `In`, `Between`, `IsNull`, And/Or/Not) | L | **niedrig** (hoch sobald Plugin-/Source-Layer ohne Domain-Service Tabellen schreibt) | [g2-check-constraints.md](./docs/superpowers/plans/2026-05-20-g2-check-constraints.md) |
| G3 | **Bitflags-FieldType**: `FieldType::Bitflags { values: Vec<BitflagValue> }` + Standard-Editor `bitflags-checkbox-group` | M | **niedrig** (hoch sobald D2V's `account_type` portiert wird) | [g3-bitflags-field-type.md](./docs/superpowers/plans/2026-05-20-g3-bitflags-field-type.md) |
| G4 | **Computed Spalten**: `DbColumn.compute` + AST mit Op/Refs/Literale; SQL-Generierung fuer Postgres/SQLite GENERATED | L | **niedrig** (Domain-Service-Berechnung reicht meist) | [g4-computed-columns.md](./docs/superpowers/plans/2026-05-20-g4-computed-columns.md) |
| G5 | **Materialized Views**: `DbView` als Top-Level-Entitaet mit `ViewDefinition`-Query-DSL + `RefreshStrategy` | L-XL | **niedrig** (Session-Cache reicht heute, eigene Query-DSL ist hoher Aufwand) | [g5-materialized-views.md](./docs/superpowers/plans/2026-05-20-g5-materialized-views.md) |
| G6 | **Partial Indexes**: `DbIndex.where_clause` (gleiche AST wie G2) | S | **mittel** (direkter Folgeschritt von G1 fuer UNIQUE-mit-Soft-Delete-Filter) | [g6-partial-indexes.md](./docs/superpowers/plans/2026-05-20-g6-partial-indexes.md) |
| G7 | **IntEnum-FieldType**: `FieldType::IntEnum { values: Vec<IntEnumValue> }` mit Integer-DB-Speicherung + Wire-String-Mapping | S | **mittel** (klein, direkter Wert fuer jede Status-Spalte) | [g7-int-enum-field-type.md](./docs/superpowers/plans/2026-05-20-g7-int-enum-field-type.md) |

**Default-Reihenfolge** (wenn kein konkreter Trigger hochpriorisiert): **G1 → G6 → G7 → G2 → G3 → G4 → G5**.

**Lessons learned aus dem Backlog-Stand**:
- G1 + G6 + G7 sind kleine, hochgehebelte Items — koennen jederzeit gezogen werden.
- G2 + G4 brauchen eine konsistente Mini-DSL (analog zu `shared::builder::GuardExpr`). Wenn eines dran kommt, lohnt es sich, Bausteine fuer das andere mitzunehmen.
- G5 ist die teuerste Lücke. Bis das relevant wird, reicht eine Server-Session-Cache-Strategie.

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
- **Rollback-Fenster nach Cutover**: konfigurierbar via `MigrationProposal.cutover_window` (Pro-Migration-Override) oder global `config.toml ⇒ migrations.cutover_rollback_window = "24h"`. Default 24h. Innerhalb des Fensters Rollback via `restoreSnapshot(migration_id)`; danach ist nur eine neue inverse Migration der Weg.
- **Verifikations-Run**: System-Job vergleicht stichprobenartig fuer N Rows, ob `transform(old) == new`. N konfigurierbar via `MigrationProposal.verification_sample` oder global `migrations.verification_sample = 100`. Default 100.

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

## Vision ueber Phase 4 hinaus

Phase 4 schliesst die Meta-Framework-Vision (Codegen + Audit). Was darueber hinaus kaeme, um mit Enterprise-Plattformen wie SAP, Oracle, Microsoft Dynamics zu konkurrieren, ist hier als **Trajektorie** festgehalten — keine verbindliche Planung, sondern Richtungs-Skizze. Aufnahme einer dieser Phasen in den verbindlichen Plan erfordert eigene Entscheidung mit ADR.

**Realismus**: SAP-Paritaet ist eine 10-Jahre-Mehrteam-Investition mit 60+ Jahren Branchenwissen. dblicious zielt **nicht** auf 1:1-Funktionsumfang, sondern auf Plattform-Eigenschaften, die einen modernen Open-Source-Konkurrenten in **spezifischen Maerkten** ermoeglichen: KMU/Mid-Market, Greenfield-Toechter, Industry-Vertical-Nischen, Open-Source-affine Kaeufer (GovTech, Health, Forschung).

Strategisch differenziert sich dblicious durch: moderner Rust-Stack, WASM-Plugin-Komposition, AI-assistierte Schema-Evolution, Codegen-zu-Standalone, Open-Source-First. Diese Eigenschaften sind in SAP nicht nachrüstbar.

Phase 5 und 6 sind **Pflicht** fuer jeden Enterprise-Vertrieb und daher mit Arbeitspaket-Skizzen versehen. Phase 7–11 sind strategische Optionen, fuer die zum Zeitpunkt der Festlegung eigene Detail-Planung noetig ist.

---

### Phase 5 — Enterprise Identity & Compliance (Pflicht-Schicht)

**Ziel**: ohne diese Schicht kommt dblicious bei keinem Enterprise-IT-Security-Review durch.

**Arbeitspakete-Skizze**:
- 5.1 SAML 2.0 / OIDC / OAuth 2.0 Federation (Login via Okta, Azure AD, Keycloak, Authelia)
- 5.2 SCIM-Provisioning: User/Group automatisch aus IdP synchronisiert
- 5.3 LDAP/AD-Bridge fuer legacy Enterprise
- 5.4 MFA: TOTP, FIDO2/WebAuthn (SMS optional)
- 5.5 Risk-based Authentication: anomale Logins detektieren, Step-up-Auth
- 5.6 Segregation of Duties (SoD): konfigurierbare Constraints ("gleiche Person darf nicht X UND Y")
- 5.7 SoX/COSO/ISO-27001 Compliance-Framework: Controls-Katalog mit nachweisbarem Reporting
- 5.8 Data Loss Prevention (DLP) Hooks auf Exports + Outbound-Integrationen
- 5.9 Privileged Access Management: Admin-Zugriff mit Begruendung + erweitertem Audit
- 5.10 Tamper-Evident Audit-Hashing-Chain auf `audit_log` (Blockchain-aehnlich, Pflicht fuer SoX)

**Dependencies**: Phase 0.7 (Auth-Modell) ist Vorbedingung.

**Markt-Begruendung**: ohne 5.1–5.4 ist kein Enterprise-Pilot moeglich. 5.5–5.10 ist die Schicht, die bei der Compliance-Pruefung kommt.

---

### Phase 6 — Skalierung & High Availability (Pflicht ab Mid-Market+)

**Ziel**: aus SQLite-Single-Server wird eine produktionsreife Multi-Instance-Plattform.

**Arbeitspakete-Skizze**:
- 6.1 PostgreSQL/MySQL als Production-Backend-Option neben SQLite (Backend-Abstraktion via SeaORM-Backend-Plug; SQLite bleibt fuer Dev/KMU)
- 6.2 Read-Replicas + Failover-Logik
- 6.3 Geo-Replikation, Active-Active-Option
- 6.4 Sharding-/Partitionierungs-Strategie fuer grosse Mandanten oder einzelne riesige Tabellen
- 6.5 Distributed Cache (Redis-style) fuer Hot-Reads (Session, Permission, Resolver)
- 6.6 Resource-Governance per Tenant: Query-Quota, RPS-Limit, Storage-Cap
- 6.7 Point-in-Time Recovery + Backup-Verification (Restore-Tests als Pflicht-CI)
- 6.8 Performance-SLA-Monitoring mit query-level Insights (p50/p95/p99-Reports)
- 6.9 In-Memory Column Store (DuckDB embedded) als Option fuer Analytics-Workload — SAP-HANA-Pendant
- 6.10 Capacity-Planning-Tools (Forecast aktuelle Wachstumsrate ⇒ wann erreicht der Mandant die Limits)

**Dependencies**: Phase 1.7 (Plattform-Bausteine) sollte stabil sein; viele 1.7-Tabellen brauchen Sharding-Strategie-Anpassung.

**Markt-Begruendung**: KMU laufen mit SQLite single-Server bis ~50 User. Ab Mid-Market wird PostgreSQL erwartet. Geo-Replikation ist ab Konzern-Pilot Pflicht.

---

### Phase 7 — Process Orchestration & Event-Driven Architecture (strategische Skizze)

**Differenzierungsfeld**: dort, wo SAP-Workflows und SAP-Process-Mining ansetzen.

- BPMN-Engine fuer lang-laufende Prozesse (Pausen, Eskalation, Wiederaufnahme)
- Saga-Pattern fuer verteilte Transaktionen ueber Modulgrenzen
- Event-Streaming-Backbone (Kafka/NATS/Redpanda-Integration; fuer KMU embedded `tokio-broadcast`)
- Outbox-Pattern fuer transactional events ohne Doppel-Delivery
- Schema-Registry fuer Events
- Process-Mining: aus Audit-Log echte Prozessfluesse rekonstruieren, Bottlenecks zeigen
- Compensation-Handler fuer Rollback in verteilten Workflows

---

### Phase 8 — Advanced Analytics & Embedded AI (strategische Skizze)

**Differenzierungsfeld**: SAP-HANA-Analytics + SAP-Joule-Copilot-Pendant.

- Real-time OLAP: multidimensionale Cubes mit Drill-Down bis auf den Beleg
- Materialized Views mit Auto-Refresh
- In-Database Time-Series + Window-Functions
- Embedded ML-Inference via WASI-NN (greift Phase 4.5 auf)
- Document AI: OCR + strukturierte Extraktion (Rechnungs-Scan → MigrationProposal-aehnlich)
- Conversational AI / Copilot: natural-language Queries gegen Operational Data
- Anomaly Detection im Audit-Log (Fraud, ungewoehnliche Buchungen)
- Self-Service BI fuer Business-User

---

### Phase 9 — Globalisierung & Master Data Management (strategische Skizze)

**SAPs groesster Burggraben** — 60+ Country-Localizations.

- Multi-Country Tax-Engines (pro Land Regeln, mehrere Rates, Withholding-Tax)
- Per-Country E-Invoicing-Mandate (XRechnung DE, FatturaPA IT, SAT MX, Peppol EU, …)
- Multi-Calendar (Hijri, Buddhist, Hebrew, Fiscal-Year-Varianten)
- RTL-Sprachen (Arabisch, Hebraeisch, Persisch)
- Currency-Formats per Country
- MDM: Golden-Records, Match-Merge, Survivorship-Rules (gleicher Kunde aus 3 Systemen ⇒ 1 Wahrheit)
- Cross-Reference-Tabellen fuer ID-Mapping zwischen Systemen
- Data-Quality-Scoring + Cleansing-Workflows

---

### Phase 10 — Integration Hub, ECM, Mobile (strategische Skizze)

**Bruecke zur restlichen Systemlandschaft + mobile Workforce.**

- iPaaS-Style-Adapter (Salesforce, Stripe, HubSpot, Slack, MS-Teams, …) als Plugin-Bundle
- EDI-Layer: ANSI X12, EDIFACT, UBL, SAP-IDoc-Bridge
- SOAP-Adapter (legacy Enterprise spricht oft nur SOAP)
- Enterprise Content Management mit Retention-Policies
- DAM (Digital Asset Management)
- Mobile-First-Apps pro Rolle (Sales-Aussendienst, Lager-Picker, Manager-Dashboards)
- Offline-First-Sync mit Konfliktaufloesung
- B2B-Customer-Self-Service-Portals (Kunde sieht eigene Rechnungen, Bestellungen, Tickets)
- WCAG 2.1 AAA Accessibility

---

### Phase 11 — Lifecycle Management & Ecosystem (strategische Skizze)

**SAP-Transport-System-Pendant + Marketplace-Aufbau.**

- Multi-Environment-Lifecycle (Dev/Test/QA/Prod) mit Transport-System-aehnlicher Promotion-Pipeline
- Customer-Specific Extensions ohne Plattform-Forking
- Upgrade-Tooling: Plattform-Version X.Y → X.Z auf Customer-Deployment anwenden, mit Regressionstest-Pipeline
- Configuration-Management: Settings als versionierte Artefakte
- Blue/Green Deployments + Canary Releases
- Plugin-Marketplace mit echtem Bezahl- und Zertifizierungs-Layer (erweitert Phase 4.2)
- Partner-Zertifizierung + Industry-Vertical-Bundles
- Customer-Health-Scores + Usage-Analytics

---

### Bleibt auch in dieser Vision aussen vor

Auch ueber Phase 4 hinaus:

- **60+ Jahre Branchenwissen** in vorgefertigten Modulen (FI/CO/MM/SD/PP/QM/PS/HR/WM/…). Das bauen Domain-Experten und Partner auf der Plattform — dblicious liefert die Bausteine, nicht die Branchen-Apps.
- **Proprietaere Branchen-Anwendungen** wie SAP IBP, SAP TM, SAP S/4HANA Cloud Public — eigene Produkte mit eigener Roadmap.
- **Enterprise-Sales- und -Support-Apparat**, Customer-Success-Manager, weltweite Niederlassungen — Business-Model-Frage, kein Framework-Feature.

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

- `event` greift **client- oder serverseitig**, je nach Variante. `BeforeSave`/`AfterSave`/`BeforeDelete` laufen im Server-Resolver; `Click`/`Change`/`Submit` im Client (Extism-WASM-Runtime im Browser, siehe Abschnitt 5 dieses Vertrags). `Custom` ist explizit aus Code oder anderem Plugin getriggert; Runtime wird im Manifest des Ziel-Plugins gesetzt.
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
runtime        = "server"                     # "server" | "client" | "both"
triggers       = ["beforeSave", "deriveField", "validate"]
db_read        = ["product", "category"]      # Entity-Typen mit Read-Capability (server-only)
db_write       = ["product"]                  # Entity-Typen mit Write-Capability (server-only)
http_fetch     = ["https://api.example.com/*"]
fs_paths       = []                            # server-only
dom_access     = false                         # client-only: Lese-/Schreibzugriff auf Builder-projizierte DOM-Nodes
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
- **Dependencies**: topologisch geladen; Cargo-Style **SemVer-Range-Intersect** bei Diamond-Dependencies (A^1.2 ∩ B^1.5 ⇒ neueste Version in Schnittmenge). Zyklen oder leerer Intersect ⇒ Plugin disabled mit klarer Fehlermeldung inkl. Auflistung der konfligierenden Ranges.
- **Signing**: Phase 2 optional. Ab Phase 4 (Marketplace) Pflicht — Server prueft Signatur gegen registrierte Public Keys.

### 3. Trigger-Point-Vertraege

Jeder Trigger hat einen festen Input/Output-JSON-Vertrag. **Plugin liest stdin als JSON, schreibt stdout als JSON** (Extism-Pattern).

| Trigger          | Runtime           | Input                                                          | Output                                                | Sync? | Fehler-Effekt                  |
|------------------|-------------------|----------------------------------------------------------------|-------------------------------------------------------|-------|--------------------------------|
| `beforeSave`     | server            | `{ entity_type, fields_before?, fields_after, user }`          | `{ fields_after?, validation? }`                      | sync  | Save abgebrochen               |
| `afterSave`      | server            | `{ entity_type, entity, op: "create"\|"update", user }`        | `void`                                                | async | nur Audit-Log                  |
| `beforeDelete`   | server            | `{ entity_type, entity, user }`                                | `{ validation? }`                                     | sync  | Delete abgebrochen             |
| `deriveField`    | server oder client | `{ entity_type, fields, target_field }`                        | `{ value: <json> }`                                   | sync  | Feld bleibt unveraendert       |
| `validate`       | server oder client | `{ entity_type, fields, user }`                                | `{ errors: [{field, code, message}] }`                | sync  | Save abgebrochen, Errors im UI |
| `customAction`   | server oder client | `{ name, args, user, context }`                                | `{ result: <json> }`                                  | sync  | UI zeigt Fehlermeldung         |
| `onClick`        | client            | `{ node_id, event: {...}, fields, user }`                      | `{ effects?: [Effect] }`                              | sync  | nur Audit-Log                  |
| `onChange`       | client            | `{ node_id, field, old_value, new_value, fields, user }`       | `{ fields_after?, effects?: [Effect] }`               | sync  | UI rollt Aenderung zurueck     |
| `onSubmit`       | client            | `{ form_id, fields, user }`                                    | `{ proceed: bool, fields_after?, validation? }`       | sync  | Submit blockiert               |

- **`validate`-Doppelregel**: client-seitig fuer Live-Feedback (Tastatur-Echtzeit), server-seitig als Autoritaet. Beide Plugins koennen dieselbe Plugin-ID haben (Code wird in beide Runtimes deployed) oder unterschiedliche. Server-Validate gilt; Client-Validate ist UX.
- **`customAction`** entscheidet ueber `manifest.runtime`, wo es laeuft. `"both"` heisst: Server hat die Autoritaet, Client darf optimistisch vorrechnen.
- **`Effect`** im Client-Trigger-Output: deklarative UI-Mutationen. Plugin manipuliert nicht direkt DOM — der Host fuehrt Effects aus.

**`Effect`-Typisierung** (definiert in `shared/src/builder/effect.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Effect {
    Navigate    { route: String },
    Toast       { level: ToastLevel, message: String },
    SetField    { field: String, value: serde_json::Value },
    FocusNode   { node_id: u64 },
    Reload,
    Custom      { name: String, args: serde_json::Value },   // Erweiterungspunkt
}
```

`Custom` ist der **Erweiterungspunkt**: Plugin-spezifische Effects, die der Host nicht selbst kennt, werden an registrierte Effect-Handler (UI-Code oder anderes Plugin) weitergegeben. Ist kein Handler registriert ⇒ Warn-Log + Effect ignoriert. Core-Varianten sind hart-typisiert; Aenderung der Variantenmenge erfordert Schema-Version-Bump (analog `EventTrigger`-Schema-Drift in Phase 1).

**Effect-Ergebnis-Pfad**: nach Ausfuehrung gibt der Host eine Liste `[{ effect_id, success, error? }]` an das Plugin zurueck — entweder synchron als Anhang am naechsten Trigger-Aufruf oder via `host.effects.last_results()`-Host-Function. Plugin kann auf Fehler reagieren (z.B. Toast bei Navigate-Fail), ist aber nicht verpflichtet; der Host loggt jeden Effect-Fail unabhaengig in `plugin_invocations`.

**Fehler-Semantik**: jedes Plugin kann strukturiert
```json
{ "error": { "code": "...", "message": "...", "details": {} } }
```
zurueckgeben. Der Host wandelt das in einen GraphQL-Error mit `extensions.code` um.

**Timeout**: hart `manifest.capabilities.max_runtime_ms`; Default 200 ms fuer sync, 5000 ms fuer async. Ueberschreiten ⇒ Trigger-Fehler `timeout`.

**Reentrancy-Schutz**: ein Plugin im `beforeSave`-Trigger fuer entity_type X darf nicht `host.db.update(X, …)` aufrufen — sonst Endlosrekursion. Der Host blockiert das mit `forbidden_reentrancy`.

### 4. Host-Function-Katalog

Funktionen, die der Host dem Plugin ueber `extism::Function` exponiert. Alle sind capability-gegated und auditiert. Runtime-Spalte: `S` = nur server, `C` = nur client, `S+C` = beide.

```text
[S+C]  host.log(level, message, fields?) -> void
            immer erlaubt; landet in plugin_invocations-Audit

[S+C]  host.now() -> ISO-8601-String
            immer erlaubt

[S+C]  host.crypto.random(n: u32) -> bytes
            kryptografisch sicher (OS-RNG); fuer IDs, Tokens, Nonces.
            Keys sollten ueber eine dedizierte API (TBI) erzeugt werden,
            nicht aus diesen Bytes von Hand.

[S+C]  host.entity.hash(entity_type, fields) -> u64
            fuer Concurrency-Checks; gleicher Algorithmus wie
            shared::EntityHeader::hash

[S+C]  host.i18n.t(key, locale?, args?) -> String
            Server: gegen Loader-Translatables; Client: gegen
            i18n::I18nContext

[S]    host.db.query(query: { entity_type, filter, sort, page }) -> EntityPage
            capability: db_read enthaelt entity_type
            quota:      max 100 Calls pro Trigger-Aufruf

[S]    host.db.insert(entity_type, fields) -> EntityChangeResult
            capability: db_write enthaelt entity_type
            Reentrancy-blockiert in *Save-Triggern desselben entity_type

[S]    host.db.update(entity_type, id, fields, expected_hash?) -> EntityChangeResult
            capability: db_write enthaelt entity_type

[S]    host.db.delete(entity_type, id, expected_hash?) -> EntityChangeResult
            capability: db_write enthaelt entity_type
            Reentrancy-blockiert in beforeDelete desselben entity_type

[S]    host.http.fetch(method, url, headers?, body?) -> { status, headers, body }
            capability-check gegen manifest.http_fetch (Glob, voller URL inkl. Query-String)
            redirects: nur folgen, wenn Ziel-URL ebenfalls in Allowlist matcht;
                       sonst Fehler `redirect_to_disallowed_host`
            timeout: 5000 ms hart

[C]    host.http.fetch_browser(method, url, headers?, body?) -> { status, headers, body }
            client-only, geht durch Browser-fetch (unterliegt CORS);
            capability-check wie [S] (voller URL inkl. Query-String);
            Allowlist-Redirects: Browser folgt nativ, Host wertet manifest.http_fetch
            erneut gegen die finale URL aus, sonst Fehler `redirect_to_disallowed_host`

[C]    host.ui.read(node_id, prop) -> Value
            capability: dom_access = true; liest projizierte UiNode-Properties

[C]    host.ui.dispatch(effect: Effect) -> void
            capability: dom_access = true; reicht einen Effect (siehe oben)
            an den Host weiter, der ihn ausfuehrt (kein direkter DOM-Zugriff)
```

**Capability-Format**: `db_read`/`db_write` sind Listen von Entity-Type-Pattern mit echtem Glob (`product`, `product.*`, `inventory.*`, `*`). `http_fetch` ist eine Liste von URL-Glob-Pattern (gegen vollen URL inkl. Query-String). Glob-Implementierung: `globset`-Crate, dokumentierte POSIX-Glob-Untermenge (kein `**`, kein Regex). Selbe Grammatik wie bei Phase 0.7-Permissions.

**Audit**: jeder Host-Function-Call wird mit `(plugin_id, runtime: "server"|"client", trigger_event, host_fn, duration_ms, outcome)` in `plugin_invocations` geloggt (siehe 2.8). Client-Audits werden in Batches an den Server geschickt — kein Real-Time-Garantie auf Client-Seite, aber bei Page-Unload geflusht. Quota- oder Capability-Verletzungen sind eigene Outcome-Werte.

### 5. Client-WASM-Laufzeit (Browser)

Plugins mit `runtime = "client"` oder `"both"` werden bei Page-Load vom Server geliefert (capability-gegated: User muss `Execute`-Permission auf das Plugin haben, sonst nicht ausgeliefert) und in einer Extism-WASM-Instanz **innerhalb** des Browser-WASM-Clients ausgefuehrt — also WASM-in-WASM.

- **Performance**: Extism im Browser ist messbar (mehrere ms pro Aufruf). Fuer Hot-Path-Validierung (Tastatur-Echtzeit) sollte das Plugin idempotent und unter ~5 ms bleiben; andernfalls Trigger entkoppeln (debounce).
- **`runtime = "both"`**: derselbe WASM-Blob laeuft in beiden Runtimes. Der Host stellt jeweils nur die fuer die Runtime erlaubten Host-Functions zur Verfuegung; ein Aufruf nicht-verfuegbarer Functions (z.B. `host.db.query` im Client) ist Laufzeitfehler mit Outcome `host_function_unavailable`. Plugin-Autoren fragen Runtime via `host.runtime()` (immer verfuegbar) ab und bauen Fallback-Pfade.
- **Capability-Doppelpruefung**: Server liefert nur Plugins, die der Client *darf*. Innerhalb des Client-WASM prueft der Host nochmals jede Capability — Browser-Code ist nicht trustworthy, der Doppel-Check ist Pflicht.
- **Storage & Cache-Invalidierung**: Plugin-Blobs werden im IndexedDB des Browsers nach `(plugin_id, version)` gecached. Beim Page-Load fragt der Client `/plugins/manifest` ab (gibt aktive Plugin-Liste mit Versionen zurueck) und laedt fehlende oder veraltete Blobs nach. **Pull-basiert**: kein Server-Push, kein WebSocket noetig — Trade-off ist eine eventual consistency von Sekunden bis Minuten, was fuer Plugin-Updates akzeptabel ist.
- **Kein File-System-Zugriff**, kein direkter DOM-Zugriff — alle UI-Effekte gehen ueber `host.ui.dispatch(Effect)`.
- **Reentrancy-Schutz auch im Client**: `onChange → host.ui.dispatch({setField})` darf nicht zu einem rekursiven `onChange` desselben Felds fuehren (Loop-Detection in der Effect-Pipeline).

---

## Cross-Cutting Concerns

Diese Punkte sind keiner Phase exklusiv zugeordnet und laufen mit:

- **Observability**: ab Phase 2 strukturiertes Logging (tracing-crate), spaetestens Phase 4 Metrics-Export (OpenTelemetry).
- **Testing**: jede Phase liefert Tests. Integration-Tests gegen reales SQLite, keine Mocks fuer DB-Pfade.
- **Security**: WASM-Sandbox-Audit in Phase 2 vorbereiten, Audit-Trail in Phase 3 (jede AI-Aktion auditierbar), externer Audit in Phase 4.
- **Audit-Retention**: alle Audit-Tabellen (`audit_log`, `plugin_invocations`, `migrations`) werden konfigurierbar geprunt — Default `audit.retention = "30d"`. Manuelles Aufraeumen via `dblicious audit prune --older-than <duration>`. Privacy-Compliance (DSGVO etc.) bleibt User-Verantwortung; Roadmap markiert das als Risiko, kein technisches Enforcement.
- **System-User**: ein User mit reservierter ID `"system"` ist beim DB-Init in `users` geseedet, kein Login. Audit-Anker fuer Seed-Daten (`entity_designs.created_by`, Loader-importierte Eintraege), automatisierte Schritte (Migration-System, DB-Init) und alle Operationen ohne menschlichen Akteur.
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
- **Phase 1.7**: Eine Demo-Rechnungsanwendung (gapless Nummerierung, GoBD-Append-Only, PDF + Email, DSGVO-Auskunft) ist auf der Plattform mit vertretbarem Aufwand baubar.
- **Phase 2**: Ein nicht-triviales Plugin (z.B. Slug-Generator + Validator) laeuft sandboxed in einem CRUD-Pfad.
- **Phase 3**: Eine `messy_orders.json` mit ~5 fremden Feldern wird per Agent in eine non-destruktive Migration uebersetzt und akzeptiert.
- **Phase 4**: `dblicious export` produziert ein Workspace, das `cargo build --release` ohne Aenderungen besteht und im Browser laeuft.

---

## Konsolidiertes Risiko-Register

Auflistung aller in den Phasen genannten Risiken, plus Cross-Cutting. Bewertung: **Wahrscheinlichkeit** (niedrig/mittel/hoch) wie oft das Risiko ohne Mitigation eintreten wuerde; **Schaden** (niedrig/mittel/hoch) wie viel es kostet, wenn es eintritt. Eigentuemer ist bei Solo-Entwicklung "n/a".

| # | Risiko | Phase | W. | Schaden | Mitigation |
|---|---|---|---|---|---|
| R-01 | Permission-Resolver-Performance: pro CRUD-Aufruf scannt die ganze Permissions-Tabelle | 0.7 | hoch | hoch | Session-Cache mit Invalidierung; Indizes auf `(subject_id, resource_kind)` |
| R-02 | Deny-vor-Allow + Wildcards + Groups+Roles wird schwer debuggbar | 0.7 | mittel | mittel | `whyAllowed`-Debug-Endpoint zeigt Herkunfts-Kette; im Audit-UI sichtbar |
| R-03 | Bestehende `security/{users,groups}` brechen nach Schema-Migration | 0.7 | hoch | mittel | Automatisches Migrations-Skript (0.7.8), idempotent + dry-run |
| R-04 | Row-Level-Permissions modelliert, aber nicht enforced — User irrt sich | 0.7 | mittel | niedrig | Loader warnt `WARN row-level permission ignored`; Config-Flag dokumentiert |
| R-05 | `UiTree`-Iteration O(n) skaliert nicht ueber ~1000 Knoten pro Session | 1 | niedrig | niedrig | Falls noetig: `HashMap<NodeId, UiNode>` als lokaler Refactor |
| R-06 | `EventTrigger`-DSL: Builder-Intent leakt in Plugin-Implementierungs-Details | 1 | mittel | hoch | Architektur-Vertrag bindet beide Phasen; Tests fuer Wire-Format |
| R-07 | Implementations-ID-Drift: String-IDs fehleranfaellig zwischen Server und Client | 1.5 | mittel | mittel | `register!`-Macro oder enum-basierte `as_str()`-Variante; CI-Test, dass jede ID im Server-Schema existiert |
| R-08 | Per-User-Persistenz der Implementations-Wahl ist nicht klar | 1.5 | mittel | mittel | Entscheidung in 1.5.3: vermutlich User-Profile, Sync ueber alle Geraete |
| R-09 | Plugin-Calls in heissen CRUD-Pfaden adden Latenz | 2 | hoch | mittel | Latenz-Budget pro Trigger (`max_runtime_ms`); async fuer `afterSave`; Caching |
| R-10 | Security-Audit der WASM-Sandbox: Capability-Escape moeglich | 2 | mittel | hoch | Externer Audit vor Production-Freigabe (4.6); strikte Host-Function-Validierung |
| R-11 | LLM-Halluzinationen produzieren ungueltige `MigrationProposal` | 3 | hoch | mittel | Function-Calling + deterministischer Diff-Validator hart geblockt; adversarielle Tests |
| R-12 | Drop-Column-Backfill-Pfad in Migrationen ist heikel | 3 | mittel | hoch | Destruktive Schritte nur in Contract-Phase, mit explizitem User-Confirm; `backup_to`-Tabelle |
| R-13 | LLM-Kosten laufen aus dem Ruder | 3 | mittel | mittel | Cost-Cap pro User-Stunde; AI-Provider-Abstraktion erlaubt spaeteren WASI-NN-Switch |
| R-14 | Codegen-Drift: Templates synchron zu Laufzeit-Crates halten | 4 | hoch | mittel | CI-Test, der Codegen-Output baut + Smoke-Test laufen laesst |
| R-15 | WASI-NN-Switch zu WasmEdge bricht Plugin-Kompatibilitaet | 4 | niedrig | mittel | Nur wenn Inference-Anforderung real ist; Phase 4+, nicht in 2 |
| R-16 | Audit-Log-Wachstum unbeschraenkt | cross | mittel | niedrig | `audit.retention = "30d"`-Default; CLI-Prune `dblicious audit prune` |
| R-17 | API-Stabilitaet `shared`-Wire-Format vs. Plugin-Manifeste | 2+ | mittel | hoch | `api_version`-Feld im Manifest; Compatibility-Range; Breaking-Changes nur mit Major-Bump |
| R-18 | Number-Sequence-Gapless-Garantie unter Last bricht (Concurrent-Inserts) | 1.7 | mittel | hoch | Transaktion mit Row-Lock; Property-Test mit 1000 parallelen Aufrufen; Doku, dass Sequenz nicht ueber Replikas hinweg garantiert ist |
| R-19 | GoBD vs. DSGVO-Konflikt: Loeschen vs. Aufbewahren | 1.7 | hoch | hoch | 1.7.16 implementiert Pseudonymisierung statt Loeschung fuer gebuchte Datensaetze; Doku-Verweis auf juristische Beratung |
| R-20 | Scope-Explosion Phase 1.7 (21 Arbeitspakete) | 1.7 | hoch | mittel | MVP-Pfad (A + C + 1.7.16) zuerst; Rest schrittweise; klare Sub-Phasen A–F als unabhaengige Einheiten |

---

## Bewusst Out-of-Scope

Liste der **Nicht-Ziele**, damit nicht jemand sie versehentlich ins Scope holt. Wenn doch reinkommt: eigene Roadmap-Phase, eigene Begruendung.

### Architektur / Stack

- **Loco** als Full-Stack-Framework. axum + async-graphql ist gesetzt; siehe `VISION.md` "Architekturentscheidungen".
- **NoSQL als Datenbank** (Surreal, Mongo, …). SeaORM + SQLite bleibt; das Schemaless-Konzept loesen wir ueber generische `entities`-Tabelle vs. typisierte Designer-Tabellen.
- **Bevy ECS / generische Runtime-Engine im Builder**. Siehe [ADR-0002](./docs/adr/0002-no-ecs-framework.md).
- **Dioxus** oder andere Multi-Plattform-GUI-Frameworks. Leptos CSR/WASM ist gesetzt; Mobile/Desktop sind kein Ziel.
- **SSR (Server-Side Rendering von Leptos)**. CSR + WASM bleibt — vereinfachte Architektur, gute UX-Latenz nach dem ersten Load.

### Multi-User / Verteilung

- **CRDT-basierte Live-Kollaboration** auf demselben `entity_design` zwischen mehreren Editoren. Optimistic-Locking mit `expected_version` ist die Antwort. Wenn doch noetig: eigene Phase mit eigener Begruendung.
- **Multi-Region-Deployment / Read-Replicas** in Phase 1–3. Single-Server-Default. `tenant_id`-Vorbereitung in 0.7 erlaubt spaetere Erweiterung; Phase 4+ kann Replikation evaluieren.
- **Real-Time-Sync zwischen mehreren Server-Instanzen**. Single-Server bleibt der Default.

### Plugin- und Code-Distribution

- **Plugin-Hot-Reload zur Laufzeit** ohne Page-Reload (Client) bzw. Server-Reload. Pull-basierter IndexedDB-Cache mit Version-Check auf Page-Load reicht.
- **Eigene Sprache/DSL fuer Plugins** ausser dem, was Extism-PDKs (TS/Python/Go/Rust) bereitstellen. Wer mehr will, schreibt sein PDK selbst.
- **Generierte Bundles mit anderen Frontend-Frameworks** (Angular/React/Vue). Codegen-Output ist Rust + Leptos.

### Auth / Sicherheit

- **Eigener Auth-Provider** (OAuth/OIDC-Server-Implementierung). Session-Mechanik bleibt; falls OAuth/OIDC noetig wird, integrieren wir einen externen Provider (z.B. Authelia, Keycloak) als Identitaets-Quelle.
- **Strict ACID-Compliance** mit Distributed-Transactions. SQLite WAL ist Default; einzelnes Schreib-Lock ist okay fuer den Anwendungsfall.
- **Strikte Capability-basierte Security** (Macaroons, Zanzibar-Style-Relationships). RBAC mit Groups+Roles + Resource-Granularitaet reicht.

### Operatives

- **Eingebauter Embedded-DB-Browser**. SQLite-Standard-Tools (`sqlite3` CLI, DBeaver, …) sind ausreichend.
- **Eigener Hosting-Service / SaaS-Plattform**. dblicious ist eine Bibliothek/Framework, die der User selbst hostet.
