# GLOSSARY — Begriffe in dblicious

Zentrales Nachschlagewerk fuer projektspezifische Begriffe. Verlinkt von `VISION.md` und `ROADMAP.md`. Generische Begriffe (GraphQL, WASM, SemVer, …) sind hier *nicht* erklaert — nur was im dblicious-Kontext eine besondere Bedeutung hat.

Alphabetisch sortiert.

---

## A

### ADR (Architecture Decision Record)
Strukturierter Eintrag in `docs/adr/`, der eine architektonische Entscheidung mit Kontext, Alternativen und Konsequenzen festhaelt. Format siehe [`docs/adr/README.md`](./docs/adr/README.md).

### Anonymous-User
Impliziter `Subject::Anonymous` fuer Anfragen ohne Session-Token. Bekommt Default-Role `"Anonymous"`. Per `auth.allow_anonymous = false` abschaltbar. Siehe Phase 0.7.

### AuthContext
Client-seitiger Leptos-Context (`client/src/auth/`), der die projizierten Permissions des aktuellen Users haelt. Liefert `is_allowed(resource, op) -> bool`. **Niemals autoritativ** — Server bleibt Quelle der Wahrheit (Leitprinzip "Server-as-Authority").

## B

### BoundField
Optionales Feld auf einem `UiNode`, das den Knoten an `Entity.fields[key]` bindet. Lesezugriff zeigt den Wert; Schreibzugriff aendert das Feld (typisiert ueber das DesignSystem).

### BuiltinAction
`TriggerTarget`-Variante: keine Plugin-Aufruf, sondern eine vom Host bereitgestellte Standard-Aktion (z.B. `"saveDraft"`, `"openEditor"`).

## C

### Capability (Plugin)
Whitelisted Erlaubnis im Plugin-Manifest (z.B. `db_read = ["product.*"]`). Nicht im Manifest gelistete Calls werden vom Host abgewiesen — auch wenn der Code sie versucht.

### Choose (Permission-Op)
Op, die einem Subject das Recht gibt, eine bestimmte `ImplementationId` zu waehlen (z.B. zwischen `filter:text-contains` und `filter:text-fuzzy`). Aus Phase 1.5.

### Codegen
Phase-4-Schritt: aus dem finalen `UiTree` + locked Schema + Plugin-Bindings wird ein eigenstaendiges Rust-Workspace generiert. Das ist das **Produkt** — der Builder ist nur das Werkzeug. Siehe Leitprinzip "Dev/Prod-Asymmetrie".

### ColumnMeta
Wire-Type in `shared/src/lib.rs`, beschreibt eine Tabellenspalte: `key`, `label_key`, `field_type`, `sortable`, `filterable`, `comparator_id`, `filter_id`.

### Contract (Migration-Phase)
Letzte Phase einer zweiphasigen Migration: destruktive Schritte (`DropColumn`, `MapColumn(keep_source=false)`). Nicht rollback-faehig.

### Custom (Effect / EventKind / Variante)
Erweiterungspunkt-Variante in typisierten Enums (`Effect::Custom { name, args }`, `EventKind::Custom { name }`). Erlaubt projektspezifische Erweiterungen ohne `shared`-Aenderung.

### Cutover
Migration-Phase zwischen `Expanded` und `Contracting`: Code/Builder/Plugins lesen ab jetzt aus neuen Spalten (Dual-Read mit Fallback). Rollback noch moeglich, eingeschraenkt durch 24h-Fenster.

## D

### DataSource (Trait)
Trait in `client/src/components/table/data_source.rs`, der die Datenbeschaffung der Tabelle abstrahiert. Implementierungen: `RemoteSource` (Server-GraphQL), `LocalSource` (clientseitig).

### `db_schemas` (Tabelle)
Bestehende Tabelle aus dem Pre-Phase-0-Designer. Speichert optional typisierte Tabellen-DDL pro `entity_type`. Resultat von `saveDbSchema` ⇒ `ddl::try_apply_schema`. **Orthogonal zu `entity_designs`** — siehe ROADMAP Phase 1.6.

### Deny-vor-Allow
Permission-Konfliktregel auf gleicher Spezifitaet: bei zwei passenden Regeln mit unterschiedlicher Wirkung gewinnt `Deny`. Spezifischere Ebene gewinnt vor weniger spezifischer. Siehe Phase 0.7.

### Designer
Visueller Builder, mit dem ein User einen Entity-Type zusammenklickt. Phase 1. Route `/builder/:entity_type`.

### DesignSystem (Trait)
Trait in `client/src/styling/`, der Komponenten von konkreten Style-Implementierungen entkoppelt. Aktuelle Impl: `InlineDesign` (CSS-in-Rust). Zukunft: Tailwind/Stylance.

### Dev/Prod-Asymmetrie
Architektur-Leitprinzip: Builder (Dev) ist Entwicklungsumgebung, keine Hochlast-Engine; Production-Artifact entsteht via Codegen als fixierte Komponenten-Crate. **Keine generische Runtime-Engine im Builder.** Siehe ROADMAP-Leitprinzipien und [ADR-0001](./docs/adr/0001-codegen-not-runtime.md).

### Dual-Write / Dual-Read
Migration-Mechanik in `Expanded`/`CuttingOver`: Server schreibt in alte und neue Spalte (Dual-Write) und liest aus der neuen mit Fallback auf die alte (Dual-Read). Erlaubt zerstörungsfreies Schema-Refactoring.

## E

### Effect
Typisiertes Enum in `shared/src/builder/effect.rs` (Phase 1 / Architektur-Vertraege). Client-Trigger-Output: deklarative UI-Mutationen (`Navigate`, `Toast`, `SetField`, `FocusNode`, `Reload`, `Custom`). Plugin manipuliert nie direkt DOM — Host fuehrt Effects aus.

### EntityChangeResult
Wire-Type, Antwort einer Create-/Update-/Delete-Operation. Enthaelt `ok: bool`, `validation: Vec<…>` (Fehler), und optional neue Entity-Daten.

### `entity_designs` (Tabelle)
Phase-1.6-Tabelle. Speichert den Builder-Output (UI/Editor/Settings/Columns-Metadatum) pro `entity_type` als versionierter, append-only JSON-Blob. **Orthogonal zu `db_schemas`** — siehe ROADMAP Phase 1.6.

### EntityHeader
Header-Block einer Entity mit `hash: u64` fuer Concurrency-Checks (Optimistic Locking bei Update/Delete via `expected_hash`).

### EntityInstance (Permission-Resource)
Permission-Granularitaet auf Row-Level (z.B. `("product", "p-42")`). **In Phase 0.7 noch nicht enforced** (Resolver wirft `not_implemented`); per `permissions.enable_row_level`-Config aktivierbar, sobald die Performance-Frage geklaert ist.

### EntityProperty (Permission-Resource)
Permission-Granularitaet auf Spalten-Level (z.B. `"product.price"`). In Phase 0.7 enforced.

### EntitySettings
Pro-Typ-Konfiguration: `default_page_size`, `default_sort`, `default_filter`. In Phase 1.5 zusaetzlich `field_type_defaults` fuer Implementations-Resolution.

### EntityTable / EntityTableShell
Generische Leptos-Komponente fuer Entity-Tabellen. `EntityTableShell` (Phase 0.5.7) ist die voll-komponierte Variante: stellt Context bereit (`TableState`, `SelectionState`, `DataResource`, `FilterRegistry`); Aufruf-Code stellt den Baum aus `<TopMenu>`, `<Table>`, `<BottomMenu>` + Bausteinen selbst zusammen. Alte `EntityTable`-Komponente ist deprecated.

### EntityType (Permission-Resource)
Permission-Granularitaet auf Typ-Level (z.B. `"product"`). In Phase 0.7 enforced.

### EventKind
Variante in `EventTrigger.event`. UI-Triggers (`Click`, `Change`, `Submit`) laufen client-seitig; CRUD-Triggers (`BeforeSave`, `AfterSave`, `BeforeDelete`) server-seitig; `Custom` laeuft wo das Plugin gesetzt ist.

### EventTrigger
Optionales Feld auf `UiNode` (in Phase 1) bzw. Entity-Hook (Server-CRUD). Bindet Ereignis an `TriggerTarget` mit optionalem `guard` und `debounce_ms`. Siehe Architektur-Vertraege §1.

### Expansion (Migration-Phase)
Erste aktive Phase einer zweiphasigen Migration: additive Schritte (`AddTable`, `AddColumn`, `MapColumn(keep_source=true)`). Voll rollback-faehig.

## F

### FieldType
Tagged Enum in `shared/src/lib.rs`: `Text`, `Number`, `Bool`, `Date`, `DateTime`, `Money`, `Enum`, `Reference`, `Collection`. Wire-Format: `{"kind":"money","currency_code_field":"currency"}`.

### FilterCriteria
Aggregat aus `global_search: Option<String>` und `predicates: Vec<ColumnFilter>`. Wird konjunktiv ausgewertet.

### FilterPredicate
Pro-Spalte-Filter: `TextContains`, `NumberRange`, `DateRange`, `EnumIn`, `IsNull`, `IsNotNull`, … Tagged Enum, in der `LocalSource`/`RemoteSource` per `ops_for_named` ausgewertet.

### FilterRegistry
Client-seitige Registry (Phase 0.5.8), die Filter-Komponenten nach String-ID haelt. Resolution: `ColumnMeta.filter_id` → Client-Default pro `FieldType`. Erweiterbar ohne Aenderung an `<Table>`. Verallgemeinert in Phase 1.5.

## G

### Glob (Wildcard-Grammatik)
Pattern-Sprache fuer Permission-Werte und Manifest-Capabilities. Implementiert via `globset`-Crate; dokumentierte POSIX-Glob-Untermenge (`product.*`, `*.price`, `filter.*`, `*`). Kein `**`, kein Regex.

### Group
Mitgliedschafts-Einheit. User ↔ Group (n:m). Tritt als `Subject` einer Permission auf. **Orthogonal zu Role**: Group bildet Organisationsstruktur ab, Role bildet Rechte-Buendel ab. Siehe Phase 0.7.

### GuardExpr
Mini-Expression-Sprache ueber Entity-Felder (`fields.status == "draft" && fields.price > 0`) als boolesche Bedingung an einem `EventTrigger`. Parser in `shared/src/builder/guard.rs`. Grammatik noch zu spezifizieren.

## H

### Host-Function
Vom Host an WASM-Plugins exponierte Funktion (`host.db.query`, `host.log`, `host.now`, `host.ui.dispatch`, …). Capability-gegated. Im Client-WASM ist eine eingeschraenkte Untermenge verfuegbar. Siehe Architektur-Vertraege §4.

## I

### Implementations-ID
String-Identifikator fuer eine austauschbare Komponente (Filter, Editor, Formatter, Action). Server schreibt vor oder erlaubt eine Auswahl; Client schlaegt unter dieser ID in der passenden Registry nach. Resolution-Reihenfolge siehe Phase 1.5.

### Implementations-Registry
Client-seitige Registry pro Komponentenart (`FilterRegistry`, `EditorRegistry`, `FormatterRegistry`, `ActionRegistry`). Aus Phase 1.5.

## L

### Leitprinzip
Architektonische Vorgabe, die bei Konflikt mit elegantem Design gewinnt. Siehe ROADMAP-Sektion "Architektur-Leitprinzipien": Dev/Prod-Asymmetrie, Server als Authority, Konzepte uebernehmen + Tech pragmatisch.

### Loader (`--data-dir`)
Server-Komponente in `server/src/example/loader.rs`, die beim Start ein Verzeichnis liest (config, navigation, security, translatables, entities) und in den `RwLock`-Slot legt. Format-Dispatch `.json`/`.toml`/erweiterbar. Wirkt als **Seed**, nicht als Laufzeit-Quelle (siehe Phase 1.6 Koexistenz).

### LocalSource / RemoteSource
DataSource-Implementierungen. `RemoteSource` reicht Filter/Sort/Pagination an den GraphQL-Server durch. `LocalSource` haelt eine `Vec<Entity>` clientseitig und wertet `FieldOps` lokal aus (Sort, Filter, Pagination).

## M

### Manifest (Plugin)
TOML-Datei im WASM-Bundle (`plugin.toml`). Enthaelt `id`, `version`, `api_version`, `compatibility`, `capabilities` (inkl. `runtime`), `functions`, optional `signing`. Siehe Architektur-Vertraege §2.

### MigrationProposal
Phase-3-Wire-Type in `shared/src/migration.rs`. Vom LLM erzeugt, deterministisch validiert, von User approved. Enthaelt `steps: Vec<MigrationStep>`, `rationale`, `impact`. Siehe Phase-3-Spezifikation.

### MigrationStep
Varianten: `AddTable`, `AddColumn`, `MapColumn`, `DropColumn`, `AddIndex`. Tagged Enum.

### Multi-Tenancy
Mehrere logische Tenants in einer Server-Instanz. **Default in 0.7: single-tenant** — `tenant_id` in `permissions` ist `NULL`. Spalte ist Schema-Vorbereitung; Enforcement erst, wenn `config.toml` Multi-Tenancy aktiviert.

## O

### Op (Permission)
Operationen: `Create`, `Read`, `Update`, `Delete`, `Execute` (Plugins), `Choose` (Implementations-Wahl), `Approve`/`Cutover`/`Contract`/`Rollback` (Migrations).

## P

### Permission
Datensatz `(Subject, Resource, Op, Effect, Priority, TenantId)`. Flache Tabelle. Siehe Phase 0.7.

### Plugin
WASM-Bundle mit Manifest, dass in einer Extism-Sandbox laeuft. Ist `runtime = "server"`, `"client"` oder `"both"`. Reagiert auf Trigger-Events und kann Host-Functions aufrufen. Phase 2.

## R

### Resource (Permission)
Was wird geschuetzt: `EntityType`, `EntityProperty`, `EntityInstance`, `Action`, `ImplementationId`, `Migration`.

### Role
Permission-Buendel. Role ↔ Permissions (n:m), User/Group ↔ Role (n:m). **Orthogonal zu Group**: Role bildet Rechte-Struktur ab, Group bildet Organisationsstruktur ab.

### Row-Level-Permission
Permissions auf `EntityInstance`. **In Phase 0.7 noch nicht enforced.** Siehe Phase 0.7 Granularitaet.

### runtime (Plugin)
Manifest-Capability mit Werten `"server" | "client" | "both"`. `"both"` heisst: derselbe WASM-Blob laeuft in beiden Runtimes; Aufruf nicht-verfuegbarer Host-Functions ist Laufzeitfehler. Siehe Architektur-Vertraege §5.

## S

### `saveDbSchema` / `saveEntityDesign`
GraphQL-Mutationen. `saveDbSchema` (bestehend) aktualisiert `db_schemas` und triggert `ddl::try_apply_schema`. `saveEntityDesign` (Phase 1.6) speichert eine neue Version in `entity_designs`. Siehe ROADMAP Phase 1.6 fuer das Verhaeltnis.

### Selection-State
Reaktiver Signal-Wert in `EntityTableShell`-Context (Phase 0.5.7), der die ausgewaehlten Entity-IDs haelt. Wird von `<SelectionColumn>` und `<EntityMenu>`-Actions gelesen.

### Server-as-Authority
Architektur-Leitprinzip: Server ist Single Source of Truth fuer Schema, Permissions, Implementations-IDs und Migrationen. Client zeigt projizierte Sichten, darf aber nichts erzwingen.

### shared (Crate)
Top-Level-Crate mit on-the-wire-Typen (`ColumnMeta`, `FieldType`, `Entity`, `EntityPage`, `Sort`, `FilterCriteria`, …). Server und Client haengen daran. Wire-Format ist API; Aenderungen brauchen Migration auf beiden Seiten.

### Subject (Permission)
Wer hat die Permission: `User`, `Group`, `Role`.

### System-User
User mit reservierter ID `"system"`, kein Login. Audit-Anker fuer Seed-Daten und automatisierte Schritte. Siehe Cross-Cutting Concerns.

## T

### Tenant / `tenant_id`
Logischer Mandant. Spalte in `permissions` (NULL = global single-tenant). Enforcement nur bei aktivierter Multi-Tenancy-Config.

### TransformExpr
Typisiertes Mini-DSL fuer `MigrationStep::MapColumn.transform`. Spezifikation noch offen — sicher (terminiert, seiteneffektfrei), aus einer Whitelist erlaubter Operatoren.

### Trigger
Doppelt belegt: (a) ECS-`EventTrigger`-Component im `UiNode`, (b) Plugin-Trigger-Point (`beforeSave`, `validate`, `onClick`, …). Beide haengen am Vertrag in Architektur-Vertraege §1+§3.

## U

### UiNode
Phase-1-Baustein des Builder-State: Komposition aus optionalen Feldern (`transform`, `style`, `bound_field`, `event_trigger`, `draggable`, `children`). Plain-Rust-Struct, **kein** ECS-Framework. Codegen-nah.

### UiTree
Reaktiver `RwSignal<...>` mit der Wurzel des Builder-State pro Entity-Typ. Speichert als JSON in `entity_designs.state.tree`.

## W

### WASM-in-WASM
Client-WASM-Plugins laufen in einer Extism-Runtime **innerhalb** des Browser-WASM-Clients (Leptos). Doppelte Sandbox-Boundary. Performance-Implikation: mehrere ms pro Plugin-Call, sollte unter ~5 ms bleiben fuer Hot-Path-Validierung. Siehe Architektur-Vertraege §5.
