# Design: DBlicious Source-Architektur (Track B, B0+B1)

Date: 2026-05-19
Status: Draft — awaiting user review

## 1. Problem

DBlicious ist heute fest an **eine** Datenquelle gebunden: ein lokales SQLite, das DBlicious selbst beim Start mit eigenen Tabellen (`entities`, `users`, `sessions`, …) bestückt. Alle CRUD-Resolver lesen aus der generischen `entities`-Tabelle, die Geschäftsdaten als JSON-Blob in `fields_json: TEXT` hält. Composite-PKs existieren nicht (Schema: `(entity_type: TEXT, id: TEXT)`). Filter/Sort/Pagination laufen in Rust **nachdem** alle Zeilen eines Entity-Typs geladen wurden — kein SQL-Pushdown.

Das ist ein Sackgassen-Design für die strategische Ausrichtung:
- D2V2019's existierende `d2v.db` lässt sich so nicht als Quelle nutzen (Track A blockiert).
- Andere Engines (PostgreSQL, MySQL) sind nicht erreichbar.
- Nicht-DB-Quellen (REST-APIs, statische Dateien, In-Memory-Mocks) sind konzeptionell ausgeschlossen.

**Strategische Richtung** (User-Klärung 2026-05-19): DBlicious ist ein Framework über beliebige Datenquellen. Source-Implementierungen sind austauschbar; managed-SQLite (heutiges Verhalten) ist eine davon, nicht der Default-Pfad mit allem anderen als Sonderfall.

Diese Spec legt die **Source-Architektur** als Fundament und liefert **zwei Source-Implementierungen** — `managed-sqlite` (Refactoring des Bestehenden) und `foreign-sqlite` (was Track A braucht).

## 2. Scope

| Sub-Track | Inhalt | In dieser Spec? |
|---|---|---|
| **B0** | `Source`-Trait, `EntityBinding`-Konzept, Config-Loader, `SourceRegistry`. Heutiges Verhalten wird zu `managed-sqlite`-Source refaktoriert. `shop` läuft über die neue Schicht. | **Ja** |
| **B1** | `foreign-sqlite`-Source: Introspektion, Composite-PKs, SQL-Pushdown von Filter/Sort/Pagination, Read-only-Bindings. | **Ja** |
| B2 | Weitere relationale Engines (PostgreSQL, MySQL) als Source-Implementierungen | nein, eigener Spec |
| B3 | Nicht-DB-Quellen: REST, statische Datei, In-Memory-Mock | nein, eigener Spec |
| B4 | `shop` (oder andere Beispiele) auf echte SQL-Spalten umstellen statt JSON-Blob — optional, kein Bedarf für die Architektur | nein |
| B5 | Designer (`saveDbSchema`) erzeugt Schemas via Source-Trait (jede Source kann DDL anbieten oder ablehnen) | nein |

Diese Spec begleitet einen **VISION.md/ROADMAP.md-Patch** im selben Commit, damit die Architektur-Tabelle und der Phasenplan dem neuen Stand entsprechen (§13).

## 3. Architektur

### 3.1 Begriffe

| Begriff | Bedeutung |
|---|---|
| **Source** | Eine Datenquelle als Ganzes. Kennt Verbindungs-Details und Capabilities. Beispiele: ein konkretes SQLite-File, eine PostgreSQL-DB, eine REST-API. Benannt (z.B. `"local"`, `"d2v_legacy"`). |
| **SourceKind** | Die Implementierungsklasse einer Source: `managed-sqlite`, `foreign-sqlite`, später `postgres`, `rest`, `file`, … |
| **EntityBinding** | Beschreibt, wo ein Entity-Type konkret liegt — Source, Tabelle/Pfad, Primärschlüssel-Spalten, optional Spalten-Mapping. Pro Entity-Type genau eine Binding. |
| **EntityId** | Logische Identifier-Form: einzelner Schlüssel oder Composite (mehrere Schlüssel-Spalten). Wire-codiert in `Entity.id: String` (§3.5). |

### 3.2 `Source`-Trait (Server-Seite)

Pseudocode-Skizze, finaler Code im Plan:

```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    /// Wird einmal beim Server-Start aufgerufen. Source darf Connections
    /// öffnen, Schema introspizieren, Caches aufbauen. Liefert Liste der
    /// Tabellen/Endpunkte, die diese Source bekannt macht (Diagnostik;
    /// nicht für Routing — Routing geht über EntityBinding).
    async fn init(&mut self) -> Result<Vec<DiscoveredSchema>>;

    async fn list_page(&self, binding: &EntityBinding, query: &PageQuery)
        -> Result<EntityPage>;
    async fn get(&self, binding: &EntityBinding, id: &EntityId)
        -> Result<Option<Entity>>;
    async fn create(&self, binding: &EntityBinding, fields: &serde_json::Map<String, Value>)
        -> Result<Entity>;
    async fn update(&self, binding: &EntityBinding, id: &EntityId,
        patch: &serde_json::Map<String, Value>) -> Result<Option<Entity>>;
    async fn delete(&self, binding: &EntityBinding, id: &EntityId) -> Result<bool>;
}

pub struct Capabilities {
    pub supports_write: bool,
    pub supports_transactions: bool,
    pub supports_sql_pushdown: bool,
    pub supports_introspection: bool,
    pub supports_composite_pk: bool,
}
```

`PageQuery` bündelt `page`, `page_size`, `Option<Sort>`, `FilterCriteria` — also genau die Args, die heute an `data::entities_page` gehen, nur ohne `entity_type` (das kommt aus der Binding).

### 3.3 `EntityBinding`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityBinding {
    pub source: String,                    // Name aus sources.toml
    pub locator: BindingLocator,           // wohin in der Source
    pub primary_key: Vec<String>,          // Spalten-Keys (von ColumnMeta)
    #[serde(default)]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub column_map: BTreeMap<String, String>,  // ColumnMeta.key → DB-Spalte
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BindingLocator {
    /// Echte relationale Tabelle in der Source.
    Table { table: String },
    /// Heutiges Verhalten: Zeile in der gemeinsamen `entities`-Tabelle,
    /// gefiltert nach `entity_type`. Nur für `managed-sqlite`.
    GenericEntityRow { entity_type: String },
    /// REST-Endpoint (B3 — Trait kennt es schon, Implementation kommt später).
    RestEndpoint { path: String },
}
```

`column_map` ist optional: leerer Map = `ColumnMeta.key` ist gleichzeitig der DB-Spaltenname. D2V braucht den Map (PascalCase ↔ camelCase), `shop` braucht ihn nicht.

`primary_key` ist immer eine `Vec` — ein-elementig für Single-PK, mehr-elementig für Composite. Vermeidet Sonderfälle im Trait.

### 3.4 `SourceRegistry`

Hält die aktiven Sources nach Namen. Wird beim Server-Boot aus der Config aufgebaut:

```rust
pub struct SourceRegistry {
    sources: BTreeMap<String, Box<dyn Source>>,
}

impl SourceRegistry {
    pub fn get(&self, name: &str) -> Option<&dyn Source>;
    pub fn route(&self, binding: &EntityBinding) -> Result<&dyn Source>;
}
```

Resolver in `server/src/schema.rs` rufen nicht mehr `data::entities_page(entity_type, …)` direkt, sondern:

```rust
let binding = settings_for(entity_type).binding;
let source = registry.route(&binding)?;
source.list_page(&binding, &query).await
```

### 3.5 `EntityId` und Wire-Format

```rust
pub enum EntityId {
    Single(String),
    Composite(Vec<String>),
}
```

Wire-Format auf `shared::Entity.id: String` bleibt unverändert:

- Single-PK: direkt der String (`"42"`, `"p-001"`).
- Composite-PK: `"::"`-getrennt (`"1234::ACC"`).
- Eskalation, falls eine Komponente selbst `"::"` enthält: JSON-Array-Form als String (`"[\"a::b\", 7]"`). Erkennung: erstes Zeichen ist `[`. Diese Form wird auch erzeugt, wenn eine Komponente leer ist oder Whitespace am Rand hat (Eindeutigkeit).
- Trennzeichen ist als Konstante in `shared::source::COMPOSITE_KEY_SEPARATOR` exportiert.

Die Client-Seite muss bei Routen wie `/entities/:entity_type/:id` den `:id`-Parameter URL-en-/dekodieren. Das ist eine Lib-Funktion in `shared::source` (`encode_id` / `decode_id`), beide Seiten nutzen sie.

### 3.6 Konfigurations-Format

Neue Datei `<data-dir>/sources.toml`:

```toml
# Default-Source: DBlicious legt seine eigenen Tabellen an.
[sources.local]
kind = "managed-sqlite"
url  = "${DBLICIOUS_DATABASE_URL:-sqlite::memory:}"

# Beispiel D2V: fremde DB, DBlicious lässt das Schema in Ruhe.
[sources.d2v_legacy]
kind = "foreign-sqlite"
url  = "sqlite:///pfad/zu/d2v.db"
```

`${VAR:-default}`-Expansion ist ein Detail des Configloaders.

Per-Entity-Binding hängt an den existierenden `examples/<name>/entities/<type>/`-Layout an, in einer neuen Datei `binding.{toml,json}`:

```toml
# examples/d2v/entities/datev_account/binding.toml
source = "d2v_legacy"
primary_key = ["number"]
read_only = false

[locator]
kind = "table"
table = "DatevAccounts"

[column_map]
number = "Number"
name   = "Name"
# ...
```

Fehlt die Datei, gilt eine **Default-Binding** (Backwards-Compat für `shop`):

```rust
EntityBinding {
    source: "local".to_string(),
    locator: BindingLocator::GenericEntityRow { entity_type: <type> },
    primary_key: vec!["id".to_string()],
    read_only: false,
    column_map: BTreeMap::new(),
}
```

Damit funktioniert jedes bestehende `examples/*`-Verzeichnis ohne Änderung.

## 4. B0 — Refactoring auf die Source-Schicht

### 4.1 Heutigen Code wrappen

`server/src/data.rs::entities_page/create_entity/update_entity/delete_entity` ziehen in `server/src/source/managed_sqlite.rs` um und werden Methoden auf einer `ManagedSqliteSource`-Struktur. Trait-Methoden delegieren an die existierende Logik. Verhalten bleibt identisch.

`BindingLocator::GenericEntityRow { entity_type }` ist der einzige Locator, den `ManagedSqliteSource` initial unterstützt — er beschreibt genau das heutige `WHERE entity_type = ?`-Verhalten. (`BindingLocator::Table` wird später, in B4, ergänzt, falls jemand `managed-sqlite` mit echten Spalten will. Außerhalb dieser Spec.)

### 4.2 Resolver-Refactoring

`server/src/schema.rs` ruft heute direkt `data::entities_page(…)`. Diese Aufrufe werden auf `registry.route(&binding).list_page(…)` umgestellt. Die Bindings kommen aus `settings_for(entity_type).binding` (neues optionales Feld auf `EntitySettings`).

### 4.3 Boot-Sequenz

`server/src/db.rs::init` wird zu `server/src/boot.rs::init` (oder bleibt in `db.rs`, Detail im Plan). Neue Ablauf:

1. Sources aus `sources.toml` lesen; falls Datei fehlt, Default `{ name="local", kind="managed-sqlite", url=<env-or-memory> }` synthetisieren (heutiges Verhalten).
2. Für jede Source `Source::init(&mut self)` aufrufen. `managed-sqlite` erzeugt seine internen Tabellen (was `db::create_schema` heute macht). `foreign-sqlite` introspiziert.
3. `SourceRegistry` bauen, prozessweit ablegen.
4. Seed-Schritte (bisher `seed_if_empty`) laufen pro Source, deren `Capabilities::supports_write` true ist UND deren Locator `GenericEntityRow` ist. Foreign Sources werden nicht geseedet.

### 4.4 `shop`-Migration

`examples/shop/sources.toml` (neu) deklariert die Default-`local`-Source. Pro Entity-Type wird **keine** `binding.toml` angelegt — die Default-Binding (§3.6) deckt alles ab. Damit ist die Migration ein einziges neues 5-Zeilen-File.

Tests in `cargo test --workspace` müssen ohne Änderung grün bleiben (Hauptakzeptanz für B0).

### 4.5 Was NICHT umgebaut wird

- Auth (`users`, `groups`, `sessions`, `permissions`, …): bleibt fest auf `managed-sqlite` mit eigenen Tabellen. Diese Tabellen sind DBlicious-interne Infrastruktur, keine Geschäftsdaten — sie gehören nicht durch das Source-Routing.
- Audit-Log, Plugin-Storage, Translatables, Builder-Designs: dito.
- Designer-Pfad (`saveDbSchema`/`try_apply_schema`): bleibt am `managed-sqlite`-Pool. Generalisierung ist B5.

## 5. B1 — `foreign-sqlite` Source

### 5.1 Boot-Verhalten

Bei `kind = "foreign-sqlite"`:
- Connection öffnen — nur lesen-fähige Connection initial.
- Schema introspizieren über `sqlite_master` + `PRAGMA table_info(<table>)` + `PRAGMA foreign_key_list(<table>)`.
- Für jede Tabelle ein `IntrospectedTable`-Record cachen: Name, Spalten (Name, SQLite-Typ-Affinität, NotNull, PK-Position), FK-Constraints.
- **Kein** `CREATE TABLE` gegen die Source absetzen.

### 5.2 CRUD-Pfad

```rust
async fn list_page(&self, binding: &EntityBinding, q: &PageQuery) -> Result<EntityPage> {
    // 1. Tabelle aus binding.locator.Table.
    // 2. SELECT-Spalten: alle Spalten aus binding.primary_key + binding.column_map.values().
    // 3. WHERE: q.filter → SQL-Prädikate, gemappt durch column_map.
    // 4. ORDER BY: q.sort → SQL, ebenfalls gemappt.
    // 5. LIMIT / OFFSET aus Pagination.
    // 6. COUNT(*) für total separat (mit gleichem WHERE).
}
```

Filter-Pushdown deckt initial nur die Operatoren ab, die `shared::FilterPredicate` heute anbietet (`eq`, `contains`, `range`, …). Operatoren, die nicht SQL-übersetzbar sind, fallen auf "in Rust filtern" zurück — selbe Strategie wie heute beim Blob-JSON.

### 5.3 Composite-PK

`get(binding, id)`:
- `id` ist `EntityId`, dekodiert vom Wire-String.
- Bei Composite-PK: WHERE-Klausel ist `AND` der `(pk_col = ?)`-Vergleiche, Reihenfolge wie in `binding.primary_key`.

### 5.4 Read-only-Binding

Wenn `binding.read_only == true`, liefern `create`/`update`/`delete` `Err(SourceError::ReadOnly)`. GraphQL-Resolver übersetzen das in `forbidden`-Code mit Message `"binding is read-only"`.

### 5.5 Capabilities

```rust
Capabilities {
    supports_write: true,             // (außer read_only-Bindings)
    supports_transactions: true,      // SQLite-nativ
    supports_sql_pushdown: true,
    supports_introspection: true,
    supports_composite_pk: true,
}
```

### 5.6 D2V-Binding-Konfiguration (Track A's §3 Contract)

Die Track-A-Spec verlangt vier konkrete Composite-PKs:

| Entity | Binding-PK |
|---|---|
| `star_money_account` | `["bankCode", "code"]` |
| `datev_account_entry` | `["entryId", "accountNr", "offsetAccountNr"]` |
| `datev_calculation_value` | `["calculationId", "year", "method"]` |
| `susa_entry` | `["accountNr", "year"]` |

Diese werden in `examples/d2v/entities/<type>/binding.toml` materialisiert — nicht in B1's Code, sondern in B1's Beispieldateien. Damit ist Track A's Contract erfüllt.

## 6. Wire-Format und API-Verträglichkeit

**Was sich am Wire-Format ändert** (additiv, keine Breaks):

- `shared::settings::EntitySettings` bekommt neues optionales Feld `binding: Option<EntityBinding>` mit `#[serde(default, skip_serializing_if = "Option::is_none")]`. Bestehende Beispiele ohne dieses Feld bekommen die Default-Binding (§3.6).
- `shared::Entity.id` semantisch erweitert: kann jetzt Composite-Form `"a::b"` oder JSON-Array-Form sein. Wer das nicht erwartet, behandelt es als opaken String — funktioniert weiter.
- Neue Modul-Pfade `shared::source::{EntityBinding, BindingLocator, EntityId, encode_id, decode_id, COMPOSITE_KEY_SEPARATOR}`.

**Was sich NICHT ändert** (Stabilitäts-Garantie):

- GraphQL-Schema-Form (Queries `entities`, `entityById`, Mutations `createEntity`, …).
- Client-Resolver-Aufrufe.
- Field-Type-Tag-Format (`{"kind": "money", …}`).

## 7. Tests

### 7.1 Existierende Tests bleiben grün

`cargo test --workspace` ohne Anpassungen — das ist die Hauptakzeptanz für B0.

### 7.2 Neue Tests B0

- `source::managed_sqlite` Trait-Konformität: Pro-Methoden-Tests, dass das Verhalten zum bisherigen `data::*`-Pfad identisch ist.
- `SourceRegistry`-Routing: korrekte Source pro Binding; Fehler bei unbekannter Source.
- Config-Loader: `sources.toml` parsen; Env-Var-Expansion; Default-Synthese ohne Datei.
- `binding.toml`-Parser inkl. Backwards-Compat (kein Datei → Default-Binding).
- `shared::source::encode_id`/`decode_id`: Round-Trip-Property-Test mit zufälligen Composite-Parts (inkl. `::`-enthaltenden Strings, Leerstrings, Whitespace).

### 7.3 Neue Tests B1

- `foreign-sqlite::init`: Test mit einer Fixture-`.db` mit drei Tabellen unterschiedlicher PK-Form (single, composite, FK).
- CRUD-Round-Trip pro Operator (eq/contains/range): WHERE/ORDER BY/LIMIT gehen ans SQL durch, `EXPLAIN QUERY PLAN` zeigt erwarteten Index-Use bei vorhandenen Indizes.
- Composite-PK get/update/delete: korrekte AND-Verkettung.
- Read-only-Binding: jede Mutation liefert `SourceError::ReadOnly`.
- Spalten-Mapping: `ColumnMeta.key = "number"` wird zu `"Number"` in SQL übersetzt.

### 7.4 Integrationstest D2V

`server/tests/foreign_d2v.rs`: lädt eine kleine fixture-`d2v.db` (anonymisierter Auszug, ein paar Zeilen pro Tabelle, **nicht** die echte Production-DB), bootet den Server gegen `examples/d2v/`, ruft pro Entity-Type `list_page` auf, prüft Spalten-Verfügbarkeit und Composite-PK-Roundtrip.

## 8. Risiken

- **Boot-Reihenfolge**: Auth-Tabellen brauchen `managed-sqlite` *vor* dem Bindings-Resolving (CRUD-Resolver brauchen Sessions). Lösung: zwei-phasiger Boot — erst die `managed-sqlite`-internen Tabellen, dann die anderen Sources, dann Bindings. Im Plan detailliert.
- **Plugin-Host-Functions** (`host.db.query` aus Phase 2.4) sind heute hart auf den einen Pool gebunden. Sobald Plugins angefasst werden, müssen sie wissen, welche Source sie befragen — das ist eine Phase-2-Folge, hier nur Notiz, **kein** Umbau.
- **Performance-Regression B0**: jede CRUD-Operation geht durch einen Trait-Dispatch mehr. `async fn` über Trait-Object ist <1 µs, vernachlässigbar.
- **`sources.toml` vs `config.toml`**: zwei Files pro Beispiel statt einem. Alternative: `[sources.*]`-Sektionen in `config.toml` selbst. Entscheidung in dieser Spec: **separate `sources.toml`-Datei**, weil Sources eine eigene Konfigurations-Achse sind und in größeren Setups deutlich umfangreicher werden als der heutige `config.toml`-Inhalt. Loader akzeptiert beides; `sources.toml` hat Vorrang.
- **Schema-Drift in Foreign-DBs**: wird die DB außerhalb von DBlicious geändert während der Server läuft, weiß der introspierte Cache nichts davon. Out-of-scope hier — Refresh-Endpoint kann später kommen.
- **Multi-Tenancy** (`tenant_id` aus Phase 0.7): orthogonal. Bindings haben kein Tenant-Konzept; Tenancy lebt eine Ebene tiefer in den Source-Implementierungen, falls jemand das braucht. Phase 0.7 wird im Single-Tenant-Pfad gebaut, das passt.

## 9. Out-of-Scope (explizit verwiesen)

- **B2**: PostgreSQL/MySQL als weitere `Source`-Implementierungen. Ist nur ein "neuer Trait-Impl" — keine Architektur-Änderung.
- **B3**: REST/File/In-Memory-Sources. Trait deckt sie ab (`BindingLocator::RestEndpoint` ist im Trait-Datentyp schon vorgesehen, Implementierung folgt).
- **B4**: `shop`-Migration auf echte SQL-Spalten. Heute kein Druck — JSON-Blob-Storage funktioniert für das Beispiel.
- **B5**: Designer (`saveDbSchema`) erzeugt Schemas über die Source-Trait. Heutiger Designer-Pfad bleibt am `managed-sqlite`-Pool.
- **FK-Auto-Resolution**: `FieldType::Reference` zeigt heute die ID; eine echte Label-Auflösung über die FK ist eigenes Backlog-Item (gehört eher in Phase 1.5 / 1.7).
- **Schema-Migration für Foreign-Tables**: außerhalb. `foreign-sqlite` ist Bestandsschutz, nicht Schema-Evolution.

## 10. Akzeptanz

Diese Spec ist erfüllt, wenn:

1. `cargo test --workspace` läuft ohne Test-Anpassung grün — Bestandsschutz B0.
2. `cargo run -p server -- --data-dir ./examples/shop` zeigt identisches Verhalten zu vor B0 (manuelle Smoke-Tests: Liste laden, editieren, speichern).
3. `cargo run -p server -- --data-dir ./examples/d2v` (gegen Fixture-`d2v.db`) bootet ohne Fehler. Alle 17 Entity-Types aus Track A laden Daten aus der fremden DB.
4. Eine Composite-PK-Zeile (`star_money_account` oder `susa_entry`) öffnet ihren Detail-Editor über die URL `/entities/<type>/<encoded-id>` und kann editiert/zurückgeschrieben werden.
5. `EXPLAIN QUERY PLAN` auf einem `list_page`-Aufruf gegen die Foreign-DB zeigt SQL-Pushdown (Index-Use bei vorhandenen Indizes, nicht "SCAN TABLE … alle Zeilen → Rust-Filter").
6. Eine Mutation auf eine `read_only`-Binding (`datev_entry_change_tracking`, `star_money_booking_text`, …) liefert GraphQL-Error mit Code `forbidden` und Message-Substring `read-only`.
7. VISION.md und ROADMAP.md im selben Commit aktualisiert (§13).

## 11. Decisions

1. **Eine Spec**, B0+B1 zusammen (User-Bestätigung 2026-05-19) — B0 alleine ist ohne zweiten Source-Kind nicht real validierbar.
2. **`shop` migriert in B0** (User-Bestätigung 2026-05-19) — beweist die Abstraktion an einem zweiten Beispiel und vermeidet Sonderpfad.
3. **VISION/ROADMAP-Patch im selben Commit** (User-Bestätigung 2026-05-19).
4. **Composite-PK-Encoding**: primär `"::"`-joined, JSON-Array-Form als Eskalation. Eindeutigkeit ist Property-getestet.
5. **Source-Config in `sources.toml`**, nicht in `config.toml`-Sektionen — eigene Konfigurations-Achse.
6. **Auth/Audit/Plugin/Translatable/Builder-Tabellen bleiben hart auf `managed-sqlite`** — interne Infrastruktur, kein Geschäftsdaten-Routing.
7. **`EntityBinding.column_map` ist optional** mit Default "Key ist DB-Spalte" — `shop` braucht nichts, `d2v` braucht den Map.

## 12. Migrations-Notiz für bestehende User

Es gibt heute genau einen Konsumenten dieses Codes (Repository selbst). Trotzdem als Doku:

- Wer `DBLICIOUS_DATABASE_URL` gesetzt hatte, ändert nichts — die Default-Source-Synthese (§4.3 Punkt 1) übernimmt den Env-Var-Pfad.
- Wer `examples/*`-Layouts hat, ändert nichts — Default-Binding (§3.6) deckt sie ab.
- Wer den Designer-Pfad nutzt: unverändert; B5 (später) generalisiert ihn.

## 13. Begleitende Änderungen — VISION.md + ROADMAP.md

Im selben Commit:

**VISION.md**:
- Architekturentscheidungs-Tabelle: Zeile zu SurrealDB/SeaORM wird umformuliert. Kern: SeaORM-mit-SQLite ist die **Default-Source-Implementierung**, nicht der einzig vorgesehene Pfad. Sources sind pluggable; weitere (PostgreSQL, REST, Datei) sind Folge-Implementierungen derselben Trait, keine Architektur-Brüche.
- Neuer Leitsatz unter "Architekturentscheidungen": **Source-Pluggability**. Knapp, mit Verweis auf den Phasenplan.

**ROADMAP.md**:
- Neue **Phase 0.6 — Source-Architektur** zwischen 0.5 (Konsolidierung) und 0.7 (Auth). Enthält B0 + B1 als Arbeitspakete mit Größen-Schätzung M+M.
- B2/B3/B4/B5 als spätere Items in derselben Phase aufgelistet, ohne Detail (Größe-Schätzungen platzhalter).
- Hinweis am Phase-0-Block: "Persistenz" wird umformuliert ("eine der Source-Implementierungen, die Default ausgeliefert wird").

Genauer Wortlaut der Patches im Plan.

## 14. Spec-Selbst-Review (für mich vorm Commit)

- [ ] Keine offenen `TBD`. Markierte `TODO`s sind absichtlich (Backlog-Hinweise auf B2-B5).
- [ ] Interne Konsistenz: §3 Architektur stimmt mit §4 B0-Refactor und §5 B1-Impl überein.
- [ ] Scope: eine Spec für B0+B1; B2-B5 explizit verwiesen.
- [ ] Ambiguität: Composite-PK-Eskalation (§3.5) ist die einzige nicht-triviale Encoding-Entscheidung — durch Property-Test pinable.
