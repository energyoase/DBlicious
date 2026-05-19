# DBlicious Source-Architektur (Phase 0.6, B0+B1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hebe den DBlicious-Datenzugriff auf eine `Source`-Trait. Refaktoriere das heutige Verhalten zu `ManagedSqliteSource` ohne Funktionsänderung (B0) und implementiere `ForeignSqliteSource` mit Schema-Introspektion, Composite-PKs, SQL-Pushdown und Read-only-Bindings (B1).

**Architecture:** Drei Schichten — `shared/src/source.rs` definiert das Wire-Format (`EntityBinding`, `EntityId`, encode/decode); `server/src/source/` enthält Trait + zwei Implementierungen + Config-Loader + `SourceRegistry`; bestehende `server/src/data.rs`-Funktionen werden zu dünnen Wrappern, die über die Registry routen. Auth/Audit/Translatables/Builder-Designs etc. bleiben hart auf `ManagedSqliteSource` — sie sind DBlicious-Infrastruktur, kein User-Daten-Routing.

**Tech Stack:** Rust + SeaORM 1.1 + SQLite (`sqlx-sqlite`); `async-trait` (neu hinzugefügt); `thiserror` für Fehler-Enum; existierendes `parking_lot`, `serde`, `serde_json`, `toml`.

---

## File Structure

**Neue Dateien:**

| Datei | Verantwortung |
|---|---|
| `shared/src/source.rs` | `EntityBinding`, `BindingLocator`, `EntityId`, Encode/Decode, `COMPOSITE_KEY_SEPARATOR` |
| `shared/tests/source_wire_format.rs` | Pin-Tests für Wire-Format inkl. JSON-Array-Eskalation |
| `server/src/source/mod.rs` | `Source`-Trait, `Capabilities`, `PageQuery`, `SourceError`, `SourceRegistry` |
| `server/src/source/managed_sqlite.rs` | `ManagedSqliteSource` — heutiges Verhalten gewrappt |
| `server/src/source/foreign_sqlite.rs` | `ForeignSqliteSource` — Introspektion + Composite-PK + SQL-Pushdown |
| `server/src/source/config.rs` | `sources.toml`-Loader inkl. `${VAR:-default}`-Expansion |
| `server/src/source/introspect.rs` | SQLite-Schema-Introspektion (`sqlite_master` + `PRAGMA`) |
| `server/tests/source_managed_sqlite.rs` | Trait-Konformität von `ManagedSqliteSource` |
| `server/tests/source_foreign_sqlite.rs` | Integration gegen Fixture-`.db` |
| `server/tests/fixtures/foreign_sample.sql` | Fixture-Schema mit single + composite + read-only Tabellen |
| `examples/shop/sources.toml` | `[sources.local]` Default-Eintrag |

**Geänderte Dateien:**

| Datei | Änderung |
|---|---|
| `server/Cargo.toml` | `async-trait`, `thiserror` als Dependencies |
| `shared/src/lib.rs` | `pub mod source;` + Re-Exports |
| `shared/src/settings.rs` | `EntitySettings.binding: Option<EntityBinding>` (additiv) |
| `server/src/lib.rs` | `pub mod source;` |
| `server/src/db.rs` | Boot-Sequenz teilt sich auf: interne Tabellen via `ManagedSqliteSource::init`, danach übrige Sources |
| `server/src/data.rs` | `entities_page`/`entity_by_id`/`create_entity`/`update_entity`/`delete_entity` werden zu Registry-Routern |
| `server/src/example/mod.rs` | `ExampleSet.sources: Vec<SourceConfig>` + `EntityTypeSet.binding: Option<EntityBinding>` |
| `server/src/example/loader.rs` | Lädt `sources.toml` und `<entity>/binding.{toml,json}` |
| `server/src/main.rs` | (falls Boot-Pfad aus `db::init` aufruft) — anpassen |

---

## Task 1: Add Cargo dependencies (async-trait, thiserror)

**Files:**
- Modify: `server/Cargo.toml`

- [ ] **Step 1: Add the two new dependencies**

In `server/Cargo.toml` unter `[dependencies]` einfügen (alphabetische Sortierung beibehalten — `async-trait` vor `axum`, `thiserror` vor `tokio`):

```toml
async-trait = "0.1"
thiserror = "1"
```

- [ ] **Step 2: Verify the workspace builds**

```bash
cd C:/Users/jz/source/DBlicious
cargo build -p server
```

Expected: Build erfolgreich, eventuelle Warnings unverändert zu vorher.

- [ ] **Step 3: Commit**

```bash
git add server/Cargo.toml Cargo.lock
git commit -m "deps: add async-trait + thiserror to server"
```

---

## Task 2: Shared types — EntityId + encode/decode

**Files:**
- Create: `shared/src/source.rs`
- Create: `shared/tests/source_wire_format.rs`
- Modify: `shared/src/lib.rs`

- [ ] **Step 1: Write the failing wire-format test**

`shared/tests/source_wire_format.rs`:

```rust
//! Pin-Tests fuer das Wire-Format der Source-Konzepte.
//!
//! Aenderungen am Encoding waeren ein Wire-Break — Server und Client
//! koennen sonst nicht mehr eindeutig dieselbe ID interpretieren.

use shared::source::{EntityId, COMPOSITE_KEY_SEPARATOR};

#[test]
fn separator_is_double_colon() {
    assert_eq!(COMPOSITE_KEY_SEPARATOR, "::");
}

#[test]
fn single_id_roundtrips_as_plain_string() {
    let id = EntityId::Single("p-001".into());
    assert_eq!(id.encode(), "p-001");
    assert_eq!(EntityId::decode("p-001"), EntityId::Single("p-001".into()));
}

#[test]
fn composite_id_uses_double_colon() {
    let id = EntityId::Composite(vec!["1234".into(), "ACC".into()]);
    assert_eq!(id.encode(), "1234::ACC");
    assert_eq!(
        EntityId::decode("1234::ACC"),
        EntityId::Composite(vec!["1234".into(), "ACC".into()])
    );
}

#[test]
fn composite_with_embedded_separator_escalates_to_json_array() {
    // Eine Komponente enthaelt selbst "::", daher JSON-Array-Form.
    let id = EntityId::Composite(vec!["a::b".into(), "7".into()]);
    let encoded = id.encode();
    assert!(encoded.starts_with('['), "expected JSON-array form, got {encoded}");
    assert_eq!(EntityId::decode(&encoded), id);
}

#[test]
fn composite_with_empty_part_escalates_to_json_array() {
    let id = EntityId::Composite(vec!["".into(), "x".into()]);
    let encoded = id.encode();
    assert!(encoded.starts_with('['), "expected JSON-array form, got {encoded}");
    assert_eq!(EntityId::decode(&encoded), id);
}

#[test]
fn composite_with_whitespace_edges_escalates_to_json_array() {
    let id = EntityId::Composite(vec![" x ".into(), "y".into()]);
    let encoded = id.encode();
    assert!(encoded.starts_with('['), "expected JSON-array form, got {encoded}");
    assert_eq!(EntityId::decode(&encoded), id);
}

#[test]
fn single_id_starting_with_bracket_roundtrips_as_single() {
    // Edge case: ein Single-PK-String, der zufaellig mit '[' beginnt.
    // Wir wollen, dass decode("[abc]") => Single("[abc]") liefert, NICHT
    // JSON-Array-Versuch. Konvention: Single-Form akzeptiert beliebige
    // Strings; JSON-Array-Form wird nur erkannt, wenn der String valides
    // JSON-Array-of-Strings ist.
    let id = EntityId::Single("[abc]".into());
    assert_eq!(id.encode(), "[abc]");
    assert_eq!(EntityId::decode("[abc]"), EntityId::Single("[abc]".into()));
}
```

- [ ] **Step 2: Run the test, verify all seven fail**

```bash
cargo test -p shared --test source_wire_format
```

Expected: alle Tests scheitern, weil `shared::source` noch nicht existiert (Build-Fehler oder "module not found").

- [ ] **Step 3: Implement the source module**

`shared/src/source.rs`:

```rust
//! Source-Architektur-Wire-Typen.
//!
//! Server-Seite implementiert die `Source`-Trait selbst (siehe
//! `server/src/source/`). Hier nur die Typen, die ueber GraphQL/JSON
//! reisen: Bindings, IDs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Trennzeichen fuer Composite-Keys im Wire-Format.
pub const COMPOSITE_KEY_SEPARATOR: &str = "::";

/// Logische ID einer Entitaet. Single-PK ist der haeufige Fall;
/// Composite-PK gibt es bei fremden DB-Schemas (z.B. D2V).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityId {
    Single(String),
    Composite(Vec<String>),
}

impl EntityId {
    /// Serialisiert in einen `String`, der ueber das Wire-Format
    /// `Entity.id` reist.
    ///
    /// - Single → der Inhalt 1:1.
    /// - Composite ohne Sonderzeichen → `"part1::part2"`.
    /// - Composite mit einer Komponente, die `::` enthaelt, leer ist,
    ///   oder mit Whitespace an den Raendern → JSON-Array-Form
    ///   `"[\"part1\",\"part2\"]"`.
    pub fn encode(&self) -> String {
        match self {
            EntityId::Single(s) => s.clone(),
            EntityId::Composite(parts) => {
                if parts.iter().any(needs_escalation) {
                    serde_json::to_string(parts).expect("Vec<String> serializes")
                } else {
                    parts.join(COMPOSITE_KEY_SEPARATOR)
                }
            }
        }
    }

    /// Dekodiert die Wire-Form zurueck.
    ///
    /// Erkennung:
    /// - String mit `'['` als erstem Zeichen UND valides JSON-Array-of-
    ///   Strings → Composite.
    /// - String mit eingebettetem `"::"` (mind. einer) und nicht JSON →
    ///   Composite (Split am Trennzeichen).
    /// - sonst → Single.
    pub fn decode(s: &str) -> Self {
        if s.starts_with('[') {
            if let Ok(parts) = serde_json::from_str::<Vec<String>>(s) {
                return EntityId::Composite(parts);
            }
            // ungueltiges JSON → als Single behandeln
        }
        if s.contains(COMPOSITE_KEY_SEPARATOR) {
            return EntityId::Composite(
                s.split(COMPOSITE_KEY_SEPARATOR).map(str::to_string).collect(),
            );
        }
        EntityId::Single(s.to_string())
    }
}

fn needs_escalation(part: &String) -> bool {
    part.contains(COMPOSITE_KEY_SEPARATOR)
        || part.is_empty()
        || part.starts_with(char::is_whitespace)
        || part.ends_with(char::is_whitespace)
}

/// Eine Entitaet-Type-Bindung: wo lebt die Entitaet, mit welchem PK,
/// und welche Spalten-Namen sind in der Source vs. im UI/Wire-Format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EntityBinding {
    pub source: String,
    pub locator: BindingLocator,
    pub primary_key: Vec<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub column_map: BTreeMap<String, String>,
}

/// Wo in der Source liegt die Entitaet konkret.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BindingLocator {
    /// Echte relationale Tabelle (foreign-sqlite, postgres, …).
    Table { table: String },
    /// Heutiges Verhalten: Zeile in der gemeinsamen `entities`-Tabelle.
    GenericEntityRow { entity_type: String },
    /// REST-Endpunkt (Phase B3 — Trait kennt es, Implementation folgt).
    RestEndpoint { path: String },
}
```

Und in `shared/src/lib.rs` direkt unter den anderen `pub mod`-Zeilen einfügen:

```rust
pub mod source;
```

Plus Re-Export bei den anderen `pub use`-Zeilen (Reihenfolge alphabetisch in der bestehenden Liste):

```rust
pub use source::{BindingLocator, EntityBinding, EntityId, COMPOSITE_KEY_SEPARATOR};
```

- [ ] **Step 4: Run the test, verify all seven pass**

```bash
cargo test -p shared --test source_wire_format
```

Expected: 7 passed.

- [ ] **Step 5: Run the whole shared test suite to confirm no regression**

```bash
cargo test -p shared
```

Expected: alle Tests grün (vorher-Stand + 7 neue).

- [ ] **Step 6: Commit**

```bash
git add shared/src/source.rs shared/src/lib.rs shared/tests/source_wire_format.rs
git commit -m "feat(shared): EntityId, EntityBinding, BindingLocator wire types"
```

---

## Task 3: Server — Source trait + Capabilities + SourceError + PageQuery

**Files:**
- Create: `server/src/source/mod.rs`
- Modify: `server/src/lib.rs`

- [ ] **Step 1: Write the failing trait-shape test**

In `server/src/source/mod.rs` schreiben wir am Ende einen `#[cfg(test)]`-Block. Aber zuerst sehen wir, dass das Modul noch nicht existiert. Wir schreiben den Test in ein dediziertes File:

Create `server/tests/source_trait_shape.rs`:

```rust
//! Statische Garantien an die `Source`-Trait. Wenn diese Datei kompiliert,
//! ist die API-Form korrekt.

use server::source::{Capabilities, PageQuery, Source, SourceError};

fn _assert_object_safe(_: &dyn Source) {}

fn _assert_send_sync<T: Send + Sync>() {}

#[test]
fn trait_is_object_safe_and_threadsafe() {
    _assert_send_sync::<Capabilities>();
    _assert_send_sync::<PageQuery>();
    _assert_send_sync::<SourceError>();
    // Object-Safety wird durch `_assert_object_safe` zur Compile-Zeit geprueft.
}
```

- [ ] **Step 2: Run, verify it fails (module not found)**

```bash
cargo test -p server --test source_trait_shape
```

Expected: Compile-Error "no module `source` in `server`".

- [ ] **Step 3: Create the module skeleton**

`server/src/source/mod.rs`:

```rust
//! Source-Architektur (Phase 0.6).
//!
//! Eine `Source` ist eine Datenquelle (lokale managed-sqlite, fremde
//! foreign-sqlite, spaeter postgres/rest/file). Die `SourceRegistry`
//! routet pro `EntityBinding` zur richtigen Implementierung. CRUD-
//! Resolver gehen ueber diesen Pfad.

pub mod managed_sqlite;
// `foreign_sqlite` kommt in Task 13.

use std::collections::BTreeMap;

use async_trait::async_trait;
use thiserror::Error;

use shared::source::{EntityBinding, EntityId};
use shared::{Entity, EntityPage, FilterCriteria, Sort};

/// Statischer Capability-Vektor pro Source-Implementierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub supports_write: bool,
    pub supports_transactions: bool,
    pub supports_sql_pushdown: bool,
    pub supports_introspection: bool,
    pub supports_composite_pk: bool,
}

/// Bündelt die Such-/Sort-/Pagination-Args fuer einen Listen-Aufruf.
#[derive(Debug, Clone, Default)]
pub struct PageQuery {
    pub page: i32,
    pub page_size: i32,
    pub sort: Option<Sort>,
    pub filter: FilterCriteria,
}

/// Fehler-Klasse fuer Source-Operationen.
#[derive(Debug, Error)]
pub enum SourceError {
    #[error("source or binding is read-only")]
    ReadOnly,
    #[error("locator not supported by this source: {0}")]
    UnsupportedLocator(String),
    #[error("source not found in registry: {0}")]
    UnknownSource(String),
    #[error("entity not found")]
    NotFound,
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("{0}")]
    Other(String),
}

/// Pluggable Datenquelle.
#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    /// Wird einmal beim Server-Start aufgerufen.
    async fn init(&mut self) -> Result<(), SourceError>;

    async fn list_page(
        &self,
        binding: &EntityBinding,
        query: &PageQuery,
    ) -> Result<EntityPage, SourceError>;

    async fn get(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<Option<Entity>, SourceError>;

    async fn create(
        &self,
        binding: &EntityBinding,
        fields: serde_json::Map<String, serde_json::Value>,
        actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError>;

    async fn update(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
        patch: serde_json::Map<String, serde_json::Value>,
        actor_user_id: Option<&str>,
    ) -> Result<Option<Entity>, SourceError>;

    async fn delete(
        &self,
        binding: &EntityBinding,
        id: &EntityId,
    ) -> Result<bool, SourceError>;
}

/// In-Process-Routing: pro Source-Name eine boxed Implementierung.
pub struct SourceRegistry {
    sources: BTreeMap<String, Box<dyn Source>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self { sources: BTreeMap::new() }
    }

    pub fn register(&mut self, source: Box<dyn Source>) {
        self.sources.insert(source.name().to_string(), source);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Source> {
        self.sources.get(name).map(|b| b.as_ref())
    }

    pub fn route(&self, binding: &EntityBinding) -> Result<&dyn Source, SourceError> {
        self.get(&binding.source)
            .ok_or_else(|| SourceError::UnknownSource(binding.source.clone()))
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.sources.keys().map(String::as_str)
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

Lege gleichzeitig die Stub-Datei `server/src/source/managed_sqlite.rs` an, damit `pub mod managed_sqlite;` nicht zum Compile-Fehler führt — Inhalt einstweilen:

```rust
//! ManagedSqliteSource — heutiges Verhalten gewrappt. Implementation
//! folgt in Tasks 4-7.

// TODO(Task 4-7): Source-Impl
```

In `server/src/lib.rs` neue Zeile:

```rust
pub mod source;
```

(Reihenfolge: nach `pub mod schema;` oder am Ende der `pub mod`-Liste — der Stil der Datei bestimmt es.)

- [ ] **Step 4: Run the test, verify it passes**

```bash
cargo test -p server --test source_trait_shape
```

Expected: 1 passed.

- [ ] **Step 5: Run the whole server test suite to confirm no regression**

```bash
cargo test -p server
```

Expected: alle bisherigen Tests grün + 1 neu.

- [ ] **Step 6: Commit**

```bash
git add server/src/source/ server/src/lib.rs server/tests/source_trait_shape.rs
git commit -m "feat(server): Source trait + Capabilities + Registry skeleton"
```

---

## Task 4: ManagedSqliteSource — skeleton + init + capabilities

**Files:**
- Modify: `server/src/source/managed_sqlite.rs`
- Create: `server/tests/source_managed_sqlite.rs`

- [ ] **Step 1: Write the failing test for the skeleton**

`server/tests/source_managed_sqlite.rs`:

```rust
//! Trait-Konformitaet von `ManagedSqliteSource`.

use server::source::managed_sqlite::ManagedSqliteSource;
use server::source::{Capabilities, Source};

#[test]
fn managed_sqlite_has_expected_capabilities() {
    let src = ManagedSqliteSource::new("local".into());
    assert_eq!(src.name(), "local");
    assert_eq!(src.kind(), "managed-sqlite");
    let cap = src.capabilities();
    assert_eq!(
        cap,
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: false,
            supports_composite_pk: false,
        }
    );
}

#[tokio::test]
#[serial_test::serial]
async fn managed_sqlite_init_is_idempotent() {
    // Sauberen Pool sicherstellen, dann zweimal init(). #[serial] weil
    // die Methode auf den globalen db::CONN-Slot zugreift.
    server::db::reset();
    std::env::set_var("DBLICIOUS_DATABASE_URL", "sqlite::memory:");
    let mut src = ManagedSqliteSource::new("local".into());
    src.init().await.expect("init #1");
    src.init().await.expect("init #2");
}
```

- [ ] **Step 2: Run the test, verify it fails**

```bash
cargo test -p server --test source_managed_sqlite
```

Expected: Compile-Error "no `new` in `ManagedSqliteSource`" oder Linker-Fehler.

- [ ] **Step 3: Implement skeleton + init**

`server/src/source/managed_sqlite.rs`:

```rust
//! `ManagedSqliteSource` — heutiges Verhalten (eigene Tabellen, JSON-Blob
//! in `entities`-Tabelle) gewrappt unter die `Source`-Trait.

use async_trait::async_trait;

use shared::source::{EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::{Capabilities, PageQuery, Source, SourceError};

pub struct ManagedSqliteSource {
    name: String,
}

impl ManagedSqliteSource {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Source for ManagedSqliteSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> &'static str {
        "managed-sqlite"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,    // potentielle Faehigkeit, heute nicht ausgenutzt
            supports_introspection: false,  // wir kennen unser Schema durch SeaORM-Entities
            supports_composite_pk: false,   // entities-Tabelle hat (entity_type, id) als PK,
                                            // aus User-Sicht ist das aber Single-PK
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        // Delegiert an den bestehenden Boot-Pfad. Idempotent durch
        // `db::init`'s OnceLock-Slot.
        crate::db::init().await.map_err(SourceError::from)?;
        Ok(())
    }

    async fn list_page(
        &self,
        _binding: &EntityBinding,
        _query: &PageQuery,
    ) -> Result<EntityPage, SourceError> {
        unimplemented!("Task 5")
    }

    async fn get(
        &self,
        _binding: &EntityBinding,
        _id: &EntityId,
    ) -> Result<Option<Entity>, SourceError> {
        unimplemented!("Task 6")
    }

    async fn create(
        &self,
        _binding: &EntityBinding,
        _fields: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Entity, SourceError> {
        unimplemented!("Task 6")
    }

    async fn update(
        &self,
        _binding: &EntityBinding,
        _id: &EntityId,
        _patch: serde_json::Map<String, serde_json::Value>,
        _actor_user_id: Option<&str>,
    ) -> Result<Option<Entity>, SourceError> {
        unimplemented!("Task 6")
    }

    async fn delete(
        &self,
        _binding: &EntityBinding,
        _id: &EntityId,
    ) -> Result<bool, SourceError> {
        unimplemented!("Task 6")
    }
}
```

- [ ] **Step 4: Run the test, verify it passes**

```bash
cargo test -p server --test source_managed_sqlite
```

Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add server/src/source/managed_sqlite.rs server/tests/source_managed_sqlite.rs
git commit -m "feat(server): ManagedSqliteSource skeleton + init"
```

---

## Task 5: ManagedSqliteSource::list_page (delegate to data::entities_page)

**Files:**
- Modify: `server/src/source/managed_sqlite.rs`
- Modify: `server/tests/source_managed_sqlite.rs`

- [ ] **Step 1: Write the failing test**

In `server/tests/source_managed_sqlite.rs` ergänzen:

```rust
use shared::source::{BindingLocator, EntityBinding};
use shared::FilterCriteria;
use server::source::PageQuery;
use std::collections::BTreeMap;

fn shop_product_binding() -> EntityBinding {
    EntityBinding {
        source: "local".into(),
        locator: BindingLocator::GenericEntityRow { entity_type: "product".into() },
        primary_key: vec!["id".into()],
        read_only: false,
        column_map: BTreeMap::new(),
    }
}

#[tokio::test]
#[serial_test::serial]
async fn managed_sqlite_list_page_returns_seeded_shop_products() {
    // Voller Stack-Boot wie in den bestehenden Tests:
    server::fresh_test_setup().await;

    let src = ManagedSqliteSource::new("local".into());
    let binding = shop_product_binding();
    let q = PageQuery { page: 1, page_size: 10, sort: None, filter: FilterCriteria::default() };

    let page = src.list_page(&binding, &q).await.expect("list_page");
    assert!(page.total_count > 0, "expected seeded products, got 0");
    assert_eq!(page.page, 1);
    assert_eq!(page.page_size, 10);
}
```

- [ ] **Step 2: Run, verify it fails with `unimplemented!`**

```bash
cargo test -p server --test source_managed_sqlite -- managed_sqlite_list_page_returns_seeded_shop_products
```

Expected: panic mit "not implemented: Task 5".

- [ ] **Step 3: Implement list_page**

In `server/src/source/managed_sqlite.rs` die `list_page`-Methode ersetzen:

```rust
async fn list_page(
    &self,
    binding: &EntityBinding,
    query: &PageQuery,
) -> Result<EntityPage, SourceError> {
    let entity_type = match &binding.locator {
        shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
        other => {
            return Err(SourceError::UnsupportedLocator(format!("{other:?}")));
        }
    };
    let page = crate::data::entities_page_raw(
        entity_type,
        query.page,
        query.page_size,
        query.sort.clone(),
        query.filter.clone(),
    )
    .await;
    Ok(page)
}
```

Damit dies kompiliert, brauchen wir eine **interne** Funktion `data::entities_page_raw`, die das `EntityPage` der `shared`-Form zurückgibt (nicht das wrap-`Entity { id, fields: Json(...) }` aus `data.rs:256`). Die bestehende `data::entities_page` bleibt vorerst unverändert (sie wird in Task 12 zum Wrapper); wir fügen daneben:

In `server/src/data.rs`, direkt vor `pub async fn entities_page`:

```rust
/// Interne Variante, die das `shared::EntityPage` liefert.
/// Wird von der Source-Schicht aufgerufen; die alte `entities_page`
/// (Async-GraphQL-Json-Wrap) ruft sie ihrerseits auf.
pub(crate) async fn entities_page_raw(
    entity_type: &str,
    page: i32,
    page_size: i32,
    sort: Option<shared::Sort>,
    filter: shared::FilterCriteria,
) -> shared::EntityPage {
    let db = &conn();

    let rows = entity::entities::Entity::find()
        .filter(entity::entities::Column::EntityType.eq(entity_type))
        .all(db)
        .await
        .unwrap_or_default();

    let mut entities: Vec<shared::Entity> =
        rows.into_iter().map(shared_entity_from_model).collect();

    let columns = shared_columns_for(entity_type);
    let columns_map: std::collections::HashMap<String, &shared::ColumnMeta> =
        columns.iter().map(|c| (c.key.clone(), c)).collect();

    if !filter.is_empty() {
        entities.retain(|e| passes_filter(e, &filter, &columns_map));
    }
    if let Some(s) = sort.as_ref() {
        sort_entities(&mut entities, s, &columns_map);
    }

    let total = entities.len() as u64;
    let page_idx = (page.max(1) - 1) as usize;
    let take = page_size.max(1) as usize;
    let start = page_idx.saturating_mul(take);
    let end = start.saturating_add(take).min(entities.len());
    let slice = if start < entities.len() {
        entities[start..end].to_vec()
    } else {
        Vec::new()
    };

    shared::EntityPage {
        items: slice,
        total_count: total,
        page: page.max(1) as u32,
        page_size: page_size.max(1) as u32,
    }
}
```

- [ ] **Step 4: Run, verify it passes**

```bash
cargo test -p server --test source_managed_sqlite
```

Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add server/src/source/managed_sqlite.rs server/src/data.rs server/tests/source_managed_sqlite.rs
git commit -m "feat(server): ManagedSqliteSource::list_page delegates to data layer"
```

---

## Task 6: ManagedSqliteSource — get / create / update / delete

**Files:**
- Modify: `server/src/source/managed_sqlite.rs`
- Modify: `server/src/data.rs`
- Modify: `server/tests/source_managed_sqlite.rs`

- [ ] **Step 1: Write the failing tests for the four CRUD methods**

In `server/tests/source_managed_sqlite.rs` ergänzen:

```rust
use shared::source::EntityId;

#[tokio::test]
#[serial_test::serial]
async fn managed_sqlite_get_returns_seeded_product() {
    server::fresh_test_setup().await;
    let src = ManagedSqliteSource::new("local".into());
    let binding = shop_product_binding();

    // Erst eine ID aus der Liste holen (Shop-Seeds haben deterministische IDs).
    let page = src
        .list_page(&binding, &PageQuery {
            page: 1, page_size: 1, sort: None, filter: FilterCriteria::default(),
        })
        .await
        .unwrap();
    let id = page.items.first().expect("at least one product").id.clone();

    let one = src.get(&binding, &EntityId::Single(id.clone())).await.unwrap();
    assert!(one.is_some());
    assert_eq!(one.unwrap().id, id);
}

#[tokio::test]
#[serial_test::serial]
async fn managed_sqlite_create_update_delete_roundtrips() {
    server::fresh_test_setup().await;
    let src = ManagedSqliteSource::new("local".into());
    let binding = shop_product_binding();

    let mut fields = serde_json::Map::new();
    fields.insert("name".into(), serde_json::json!("Test Product"));
    fields.insert("price".into(), serde_json::json!(9.99));

    let created = src.create(&binding, fields, None).await.unwrap();
    let id = EntityId::Single(created.id.clone());

    let mut patch = serde_json::Map::new();
    patch.insert("name".into(), serde_json::json!("Renamed"));
    let updated = src.update(&binding, &id, patch, None).await.unwrap().unwrap();
    assert_eq!(updated.fields.get("name").and_then(|v| v.as_str()), Some("Renamed"));

    let removed = src.delete(&binding, &id).await.unwrap();
    assert!(removed);
    let after = src.get(&binding, &id).await.unwrap();
    assert!(after.is_none());
}
```

- [ ] **Step 2: Run, verify they fail**

```bash
cargo test -p server --test source_managed_sqlite
```

Expected: 2 neue Tests scheitern mit "not implemented".

- [ ] **Step 3: Implement the four methods**

In `server/src/source/managed_sqlite.rs`:

```rust
async fn get(
    &self,
    binding: &EntityBinding,
    id: &EntityId,
) -> Result<Option<Entity>, SourceError> {
    let entity_type = match &binding.locator {
        shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    let key = match id {
        EntityId::Single(s) => s.clone(),
        EntityId::Composite(_) => {
            return Err(SourceError::UnsupportedLocator(
                "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
            ));
        }
    };
    Ok(crate::data::entity_by_id_raw(entity_type, &key).await)
}

async fn create(
    &self,
    binding: &EntityBinding,
    fields: serde_json::Map<String, serde_json::Value>,
    actor_user_id: Option<&str>,
) -> Result<Entity, SourceError> {
    if binding.read_only {
        return Err(SourceError::ReadOnly);
    }
    let entity_type = match &binding.locator {
        shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    Ok(crate::data::create_entity_raw(entity_type, None, fields, actor_user_id).await)
}

async fn update(
    &self,
    binding: &EntityBinding,
    id: &EntityId,
    patch: serde_json::Map<String, serde_json::Value>,
    actor_user_id: Option<&str>,
) -> Result<Option<Entity>, SourceError> {
    if binding.read_only {
        return Err(SourceError::ReadOnly);
    }
    let entity_type = match &binding.locator {
        shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    let key = match id {
        EntityId::Single(s) => s.clone(),
        EntityId::Composite(_) => {
            return Err(SourceError::UnsupportedLocator(
                "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
            ));
        }
    };
    Ok(crate::data::update_entity_raw(entity_type, &key, patch, actor_user_id).await)
}

async fn delete(
    &self,
    binding: &EntityBinding,
    id: &EntityId,
) -> Result<bool, SourceError> {
    if binding.read_only {
        return Err(SourceError::ReadOnly);
    }
    let entity_type = match &binding.locator {
        shared::source::BindingLocator::GenericEntityRow { entity_type } => entity_type,
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    let key = match id {
        EntityId::Single(s) => s.clone(),
        EntityId::Composite(_) => {
            return Err(SourceError::UnsupportedLocator(
                "managed-sqlite GenericEntityRow erwartet Single-PK".into(),
            ));
        }
    };
    Ok(crate::data::delete_entity(entity_type, &key).await)
}
```

In `server/src/data.rs` die drei neuen `*_raw`-Helfer ergänzen (analog zu `entities_page_raw`), die `shared::Entity` zurückgeben statt das async-graphql-`Entity { id, fields: Json(...) }`. Implementierungs-Pattern: nimm die bestehenden `pub async fn entity_by_id`/`create_entity`/`update_entity` als Vorlage und ersetze den Return-Wrap-Step:

```rust
pub(crate) async fn entity_by_id_raw(entity_type: &str, id: &str) -> Option<shared::Entity> {
    let db = &conn();
    entity::entities::Entity::find_by_id((entity_type.to_string(), id.to_string()))
        .one(db).await.ok().flatten().map(shared_entity_from_model)
}

pub(crate) async fn create_entity_raw(
    entity_type: &str,
    id: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
    actor_user_id: Option<&str>,
) -> shared::Entity {
    let db = &conn();
    let id = id.unwrap_or_else(|| next_id_prefix(entity_type));
    let mut fields_map = fields;
    fields_map.insert("id".into(), serde_json::Value::String(id.clone()));
    apply_audit_columns(entity_type, &mut fields_map, actor_user_id, AuditPhase::Create);
    let value = serde_json::Value::Object(fields_map.clone());
    let hash = hash_for_entity(&id, &fields_map);

    let model = entity::entities::ActiveModel {
        entity_type: ActiveValue::Set(entity_type.to_string()),
        id: ActiveValue::Set(id.clone()),
        fields_json: ActiveValue::Set(value.to_string()),
        hash: ActiveValue::Set(hash),
    };
    let _ = model.insert(db).await;
    shared::Entity { id, fields: fields_map }
}

pub(crate) async fn update_entity_raw(
    entity_type: &str,
    id: &str,
    field_patch: serde_json::Map<String, serde_json::Value>,
    actor_user_id: Option<&str>,
) -> Option<shared::Entity> {
    let db = &conn();
    let existing = entity::entities::Entity::find_by_id((entity_type.to_string(), id.to_string()))
        .one(db).await.ok().flatten()?;
    let mut current: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&existing.fields_json).unwrap_or_default();
    for (k, v) in field_patch {
        current.insert(k, v);
    }
    apply_audit_columns(entity_type, &mut current, actor_user_id, AuditPhase::Update);
    let value = serde_json::Value::Object(current.clone());
    let hash = hash_for_entity(id, &current);
    let am = entity::entities::ActiveModel {
        entity_type: ActiveValue::Set(entity_type.to_string()),
        id: ActiveValue::Set(id.to_string()),
        fields_json: ActiveValue::Set(value.to_string()),
        hash: ActiveValue::Set(hash),
    };
    let _ = am.update(db).await;
    Some(shared::Entity { id: id.to_string(), fields: current })
}
```

`delete_entity` ist bereits passend für die Trait-Form (gibt `bool` zurück) — kein neuer Helfer nötig.

- [ ] **Step 4: Run, verify all five tests pass**

```bash
cargo test -p server --test source_managed_sqlite
```

Expected: 5 passed.

- [ ] **Step 5: Whole-workspace regression check**

```bash
cargo test --workspace
```

Expected: alle Tests grün (kein bestehender Test wurde berührt).

- [ ] **Step 6: Commit**

```bash
git add server/src/source/managed_sqlite.rs server/src/data.rs server/tests/source_managed_sqlite.rs
git commit -m "feat(server): ManagedSqliteSource CRUD methods"
```

---

## Task 7: sources.toml config loader

**Files:**
- Create: `server/src/source/config.rs`
- Modify: `server/src/source/mod.rs`

- [ ] **Step 1: Write the failing test**

Inline-Test in `server/src/source/config.rs` (am Ende der Datei, sobald sie existiert). Wir starten mit einem File-leeren Stub und dem Test:

`server/src/source/config.rs`:

```rust
//! Loader fuer `sources.toml`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceConfig {
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesFile {
    #[serde(default)]
    pub sources: BTreeMap<String, SourceConfig>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
}

/// Liest `<dir>/sources.toml`. Existiert die Datei nicht, liefert sie ein
/// leeres `SourcesFile` (kein Fehler — der Boot synthetisiert dann den
/// Default-Eintrag).
pub fn load_from_dir(dir: &Path) -> Result<SourcesFile, ConfigError> {
    let path = dir.join("sources.toml");
    if !path.exists() {
        return Ok(SourcesFile::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let expanded = expand_env(&raw);
    Ok(toml::from_str(&expanded)?)
}

/// `${VAR:-default}`-Expansion (subset von shell-style).
/// `${VAR}` ohne Default expandiert zu leerem String, wenn unset.
pub(crate) fn expand_env(input: &str) -> String {
    let re = regex::Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)(?::-([^}]*))?\}").expect("regex");
    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let var = &caps[1];
        let default = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        std::env::var(var).unwrap_or_else(|_| default.to_string())
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_empty() {
        let tmp = tempdir_for_test();
        let f = load_from_dir(tmp.path()).unwrap();
        assert!(f.sources.is_empty());
    }

    #[test]
    fn parses_two_sources() {
        let tmp = tempdir_for_test();
        std::fs::write(
            tmp.path().join("sources.toml"),
            r#"
[sources.local]
kind = "managed-sqlite"
url  = "sqlite::memory:"

[sources.d2v_legacy]
kind = "foreign-sqlite"
url  = "sqlite:///pfad/zur/d2v.db"
            "#,
        ).unwrap();
        let f = load_from_dir(tmp.path()).unwrap();
        assert_eq!(f.sources.len(), 2);
        assert_eq!(f.sources["local"].kind, "managed-sqlite");
        assert_eq!(f.sources["d2v_legacy"].kind, "foreign-sqlite");
    }

    #[test]
    fn env_var_expansion_with_default() {
        std::env::remove_var("DBLICIOUS_TEST_VAR");
        let expanded = expand_env("url = \"${DBLICIOUS_TEST_VAR:-fallback}\"");
        assert!(expanded.contains("fallback"));
    }

    #[test]
    fn env_var_expansion_from_env() {
        std::env::set_var("DBLICIOUS_TEST_VAR2", "from_env");
        let expanded = expand_env("url = \"${DBLICIOUS_TEST_VAR2:-fallback}\"");
        assert!(expanded.contains("from_env"));
    }

    fn tempdir_for_test() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
}
```

Dieser Code zieht eine neue dev-dependency `tempfile` und eine prod-dependency `regex` herein. Beides hinzufügen:

In `server/Cargo.toml`:
```toml
# unter [dev-dependencies]
tempfile = "3"
```

`regex` ist bereits in den Dependencies — kein Schritt nötig.

In `server/src/source/mod.rs` direkt unter den existierenden `pub mod`-Zeilen:

```rust
pub mod config;
```

- [ ] **Step 2: Run the tests, verify they fail (module not yet wired)**

```bash
cargo test -p server source::config
```

Expected: erst Compile-Fehler oder leere Test-Liste. Sobald der Code drin ist:

- [ ] **Step 3: Run, verify they pass**

```bash
cargo test -p server source::config
```

Expected: 4 passed.

- [ ] **Step 4: Commit**

```bash
git add server/src/source/config.rs server/src/source/mod.rs server/Cargo.toml Cargo.lock
git commit -m "feat(server): sources.toml loader with env-var expansion"
```

---

## Task 8: EntitySettings.binding field + default-binding helper

**Files:**
- Modify: `shared/src/settings.rs`
- Modify: `shared/src/source.rs`
- Modify: `shared/tests/source_wire_format.rs`

- [ ] **Step 1: Write the failing test**

Ergänze `shared/tests/source_wire_format.rs`:

```rust
use shared::source::{default_binding_for, BindingLocator, EntityBinding};

#[test]
fn default_binding_uses_local_managed_sqlite_with_entity_type() {
    let b = default_binding_for("product");
    assert_eq!(b.source, "local");
    assert!(matches!(
        b.locator,
        BindingLocator::GenericEntityRow { ref entity_type } if entity_type == "product"
    ));
    assert_eq!(b.primary_key, vec!["id".to_string()]);
    assert!(!b.read_only);
    assert!(b.column_map.is_empty());
}

#[test]
fn entity_settings_serializes_without_binding_when_none() {
    let s = shared::EntitySettings::default();
    let json = serde_json::to_string(&s).unwrap();
    assert!(!json.contains("\"binding\""), "binding should be omitted when None, got {json}");
}

#[test]
fn entity_settings_deserializes_with_binding() {
    let json = r#"{
        "binding": {
            "source": "d2v_legacy",
            "locator": { "kind": "table", "table": "DatevAccounts" },
            "primaryKey": ["Number"]
        }
    }"#;
    let s: shared::EntitySettings = serde_json::from_str(json).unwrap();
    assert!(s.binding.is_some());
    let b = s.binding.unwrap();
    assert_eq!(b.source, "d2v_legacy");
    assert_eq!(b.primary_key, vec!["Number".to_string()]);
}
```

- [ ] **Step 2: Run, verify they fail**

```bash
cargo test -p shared --test source_wire_format
```

Expected: Compile-Fehler "no `default_binding_for` in `shared::source`" und "no field `binding` in `EntitySettings`".

- [ ] **Step 3: Implement default_binding_for + EntitySettings field**

In `shared/src/source.rs` am Ende:

```rust
/// Default-Binding fuer Entity-Types, die keine eigene `binding.toml`
/// haben. Zeigt auf die `local`-Source (managed-sqlite, gemeinsame
/// `entities`-Tabelle) mit `id` als Single-PK.
pub fn default_binding_for(entity_type: &str) -> EntityBinding {
    EntityBinding {
        source: "local".into(),
        locator: BindingLocator::GenericEntityRow {
            entity_type: entity_type.to_string(),
        },
        primary_key: vec!["id".into()],
        read_only: false,
        column_map: BTreeMap::new(),
    }
}
```

Re-Export in `shared/src/lib.rs` aktualisieren:

```rust
pub use source::{
    default_binding_for, BindingLocator, EntityBinding, EntityId, COMPOSITE_KEY_SEPARATOR,
};
```

In `shared/src/settings.rs` — `EntitySettings`-Struct um das neue Feld erweitern. Lese zuerst die existierende Definition:

```bash
grep -n "pub struct EntitySettings" shared/src/settings.rs
```

Nimm den existierenden Block und füge oberhalb der schließenden `}`-Zeile ein:

```rust
    /// Phase 0.6: Source-Binding fuer diesen Entity-Type. `None` => Default-
    /// Binding (managed-sqlite, generische entities-Tabelle).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<crate::source::EntityBinding>,
```

`#[derive(...Default...)]` bleibt unverändert; `Option<...>` hat `Default = None`.

- [ ] **Step 4: Run, verify all three new tests pass**

```bash
cargo test -p shared --test source_wire_format
```

Expected: 10 passed (7 alt + 3 neu).

- [ ] **Step 5: Whole shared regression check**

```bash
cargo test -p shared
```

Expected: alle grün.

- [ ] **Step 6: Commit**

```bash
git add shared/src/source.rs shared/src/lib.rs shared/src/settings.rs shared/tests/source_wire_format.rs
git commit -m "feat(shared): default_binding_for + EntitySettings.binding"
```

---

## Task 9: ExampleSet carries sources; loader reads sources.toml

**Files:**
- Modify: `server/src/example/mod.rs`
- Modify: `server/src/example/loader.rs`
- Modify: `server/tests/` (neu: `loader_sources.rs`)

- [ ] **Step 1: Write the failing test**

`server/tests/loader_sources.rs`:

```rust
//! Loader berücksichtigt sources.toml.

use std::collections::BTreeMap;

#[test]
fn loader_picks_up_sources_toml() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("sources.toml"),
        r#"
[sources.local]
kind = "managed-sqlite"
url  = "sqlite::memory:"

[sources.d2v_legacy]
kind = "foreign-sqlite"
url  = "sqlite::memory:"
        "#,
    ).unwrap();

    let set = server::example::loader::load(tmp.path()).expect("load");
    let names: BTreeMap<_, _> = set.sources.iter().map(|(k, v)| (k.clone(), v.kind.clone())).collect();
    assert_eq!(names.get("local").map(String::as_str), Some("managed-sqlite"));
    assert_eq!(names.get("d2v_legacy").map(String::as_str), Some("foreign-sqlite"));
}

#[test]
fn loader_synthesizes_local_when_no_sources_toml() {
    let tmp = tempfile::tempdir().unwrap();
    // kein sources.toml in tmp
    let set = server::example::loader::load(tmp.path()).expect("load");
    assert_eq!(set.sources.get("local").map(|c| c.kind.as_str()), Some("managed-sqlite"));
}
```

- [ ] **Step 2: Run, verify failing**

```bash
cargo test -p server --test loader_sources
```

Expected: Compile-Fehler "no field `sources` in `ExampleSet`".

- [ ] **Step 3: Add `sources` field + loader path**

In `server/src/example/mod.rs`, am `ExampleSet`-Struct das Feld einfügen:

```rust
    /// Phase 0.6: konfigurierte Sources. Default-`local` wird vom Loader
    /// synthetisiert, falls keine `sources.toml` vorhanden ist.
    pub sources: BTreeMap<String, crate::source::config::SourceConfig>,
```

`#[derive(Clone, Debug)]` bleibt; `BTreeMap` ist `Clone`.

In `server/src/example/loader.rs` direkt vor dem `Ok(ExampleSet { ... })`-Return:

```rust
    // ---- Sources ----
    let mut sources = crate::source::config::load_from_dir(dir)
        .map_err(|e| anyhow!("sources.toml: {e}"))?
        .sources;
    if !sources.contains_key("local") {
        sources.insert(
            "local".into(),
            crate::source::config::SourceConfig {
                kind: "managed-sqlite".into(),
                url: std::env::var("DBLICIOUS_DATABASE_URL").ok(),
            },
        );
    }
```

Und im `Ok(ExampleSet { ... })`-Block das Feld setzen:

```rust
        sources,
```

- [ ] **Step 4: Run, verify passing**

```bash
cargo test -p server --test loader_sources
```

Expected: 2 passed.

- [ ] **Step 5: Run the loader regression suite**

```bash
cargo test -p server loader
```

Expected: bestehende Loader-Tests bleiben grün + 2 neu.

- [ ] **Step 6: Commit**

```bash
git add server/src/example/mod.rs server/src/example/loader.rs server/tests/loader_sources.rs
git commit -m "feat(server): example loader reads sources.toml (with local default)"
```

---

## Task 10: Loader reads per-entity binding.{toml,json}

**Files:**
- Modify: `server/src/example/loader.rs`
- Modify: `server/tests/loader_sources.rs`

- [ ] **Step 1: Write the failing test**

In `server/tests/loader_sources.rs` ergänzen:

```rust
#[test]
fn loader_picks_up_per_entity_binding_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let entities = tmp.path().join("entities").join("datev_account");
    std::fs::create_dir_all(&entities).unwrap();
    std::fs::write(entities.join("columns.json"), "[]").unwrap();
    std::fs::write(entities.join("binding.toml"), r#"
source = "d2v_legacy"
primary_key = ["number"]
read_only = false

[locator]
kind = "table"
table = "DatevAccounts"

[column_map]
number = "Number"
name   = "Name"
    "#).unwrap();

    let set = server::example::loader::load(tmp.path()).expect("load");
    let ty = set.entities.get("datev_account").expect("entity loaded");
    let settings = ty.settings.as_ref().expect("settings synthesized");
    let binding = settings.binding.as_ref().expect("binding present");
    assert_eq!(binding.source, "d2v_legacy");
    assert_eq!(binding.primary_key, vec!["number".to_string()]);
    assert_eq!(binding.column_map.get("name"), Some(&"Name".to_string()));
}
```

- [ ] **Step 2: Run, verify it fails**

```bash
cargo test -p server --test loader_sources -- loader_picks_up_per_entity_binding_toml
```

Expected: Test scheitert mit "settings synthesized" — heute setzt der Loader keine Settings, wenn keine `settings.{toml,json}` da ist.

- [ ] **Step 3: Implement binding loading**

In `server/src/example/loader.rs`, in der `load_entity_type`-Funktion (Zeile ~158), nach dem `settings`-Read:

```rust
    let mut settings: shared::EntitySettings = read_typed_opt(find_file(dir, "settings"))?
        .unwrap_or_default();
    if settings.binding.is_none() {
        if let Some(b) = read_typed_opt::<shared::source::EntityBinding>(find_file(dir, "binding"))? {
            settings.binding = Some(b);
        }
    }
```

Den existierenden `let settings: Option<shared::EntitySettings>`-Read passt entsprechend an und im `Ok(EntityTypeSet { ... })`-Block `settings: Some(settings)` setzen (statt heute optional unsicher).

- [ ] **Step 4: Run, verify it passes**

```bash
cargo test -p server --test loader_sources
```

Expected: 3 passed.

- [ ] **Step 5: Run loader regression**

```bash
cargo test -p server loader
```

Expected: alle grün — die heutigen Loader-Tests laden Beispiele *ohne* `binding.toml`, die kommen mit `settings.binding = None` durch.

- [ ] **Step 6: Commit**

```bash
git add server/src/example/loader.rs server/tests/loader_sources.rs
git commit -m "feat(server): loader reads per-entity binding.{toml,json}"
```

---

## Task 11: SourceRegistry boot wire-up

**Files:**
- Modify: `server/src/source/mod.rs` (add `boot_registry`)
- Modify: `server/src/db.rs` or `server/src/lib.rs` (Boot-Sequenz)
- Modify: `server/tests/source_registry_boot.rs` (new)

- [ ] **Step 1: Write the failing test**

`server/tests/source_registry_boot.rs`:

```rust
use server::source::{boot_registry, registry};

#[tokio::test]
#[serial_test::serial]
async fn boot_registry_creates_local_managed_sqlite() {
    server::fresh_test_setup().await;
    let reg = registry();
    let local = reg.get("local").expect("local source registered");
    assert_eq!(local.kind(), "managed-sqlite");
    assert_eq!(local.name(), "local");
}
```

- [ ] **Step 2: Run, verify fail**

```bash
cargo test -p server --test source_registry_boot
```

Expected: "no `boot_registry` in `server::source`" oder "no `registry`".

- [ ] **Step 3: Add boot_registry + global slot**

In `server/src/source/mod.rs` am Ende:

```rust
use parking_lot::RwLock;
use std::sync::OnceLock;

use crate::source::config::SourceConfig;
use crate::source::managed_sqlite::ManagedSqliteSource;

static REGISTRY: OnceLock<RwLock<SourceRegistry>> = OnceLock::new();

fn slot() -> &'static RwLock<SourceRegistry> {
    REGISTRY.get_or_init(|| RwLock::new(SourceRegistry::new()))
}

/// Liefert eine Read-Lock-Sicht auf die Registry. Lebt prozessweit.
pub fn registry() -> parking_lot::RwLockReadGuard<'static, SourceRegistry> {
    slot().read()
}

/// Baut die Registry aus der Konfiguration auf. Idempotent — bei zweitem
/// Aufruf wird die alte Registry komplett ersetzt (nur Tests benutzen das,
/// um zwischen `fresh_test_setup`-Aufrufen zurueckzusetzen).
pub async fn boot_registry(
    sources: &std::collections::BTreeMap<String, SourceConfig>,
) -> Result<(), SourceError> {
    let mut reg = SourceRegistry::new();
    for (name, cfg) in sources {
        let mut src: Box<dyn Source> = match cfg.kind.as_str() {
            "managed-sqlite" => Box::new(ManagedSqliteSource::new(name.clone())),
            // foreign-sqlite kommt in Task 13.
            other => return Err(SourceError::Other(format!("unknown source kind: {other}"))),
        };
        src.init().await?;
        reg.register(src);
    }
    *slot().write() = reg;
    Ok(())
}

/// Reset fuer Tests — analog zu `db::reset`.
pub fn reset() {
    *slot().write() = SourceRegistry::new();
}
```

In `server/src/lib.rs::fresh_test_setup` (Erstaufruf-Pfad in `boot.rs` oder wo `fresh_test_setup` liegt) nach `db::init` einen `boot_registry`-Aufruf einfügen. Lese zuerst:

```bash
grep -n "fresh_test_setup\|pub async fn boot" server/src/lib.rs
```

Ergänze in `fresh_test_setup` (oder `boot`) **nach** dem ExampleSet-Install:

```rust
    let set = crate::example::current().expect("set installed");
    crate::source::boot_registry(&set.sources)
        .await
        .map_err(|e| anyhow::anyhow!("boot_registry: {e}"))?;
```

In den Test-Reset-Pfaden (`server/src/db.rs::reset`-Aufrufer) zusätzlich `source::reset()` aufrufen.

- [ ] **Step 4: Run, verify it passes**

```bash
cargo test -p server --test source_registry_boot
```

Expected: 1 passed.

- [ ] **Step 5: Run the full server test suite**

```bash
cargo test -p server
```

Expected: alle grün — alle bestehenden Tests rufen `fresh_test_setup()` und kriegen jetzt zusätzlich die Registry mit. Keine bestehende Funktion liest die Registry, also kein Verhalten ändert sich.

- [ ] **Step 6: Commit**

```bash
git add server/src/source/mod.rs server/src/lib.rs server/tests/source_registry_boot.rs
git commit -m "feat(server): boot SourceRegistry from sources config"
```

---

## Task 12: Route data::* through SourceRegistry

**Files:**
- Modify: `server/src/data.rs`

- [ ] **Step 1: Write the failing test (routing-pin)**

In `server/tests/source_registry_boot.rs` ergänzen:

```rust
#[tokio::test]
#[serial_test::serial]
async fn data_entities_page_routes_via_registry_for_default_binding() {
    server::fresh_test_setup().await;
    // Default-Binding fuer "product" geht ueber "local"-Source — Ergebnis
    // muss identisch zum Direkt-Aufruf der raw-Helper sein.
    let direct = server::data::entities_page_raw(
        "product", 1, 100, None, Default::default(),
    ).await;
    let routed = server::data::entities_page(
        "product", 1, 100, None, Default::default(),
    ).await;
    assert_eq!(direct.total_count, routed.total_count);
    assert_eq!(direct.items.len(), routed.items.len());
}
```

(Diese Pin ist nur dann sichtbar fehlschlagend, wenn der Routing-Code etwas anderes liefert — der Hauptzweck ist: nach dem Refactor schreibt jemand evtl. einen zweiten Code-Pfad, der wieder driftet. Der Test schützt davor.)

- [ ] **Step 2: Run, verify it passes already**

```bash
cargo test -p server --test source_registry_boot -- data_entities_page_routes_via_registry_for_default_binding
```

Expected: passing — heute geht `entities_page` direkt zu `entities_page_raw`, Resultate sind trivialerweise gleich.

- [ ] **Step 3: Refactor data.rs to route via registry**

In `server/src/data.rs`, ersetze den Body von `pub async fn entities_page(...)` durch:

```rust
pub async fn entities_page(
    entity_type: &str,
    page: i32,
    page_size: i32,
    sort: Option<shared::Sort>,
    filter: shared::FilterCriteria,
) -> EntityPage {
    let binding = binding_for(entity_type);
    let reg = crate::source::registry();
    let source = match reg.route(&binding) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(target: "server::data", "route error: {e}");
            return EntityPage {
                items: Vec::new(),
                total_count: 0,
                page,
                page_size,
            };
        }
    };
    let q = crate::source::PageQuery { page, page_size, sort, filter };
    match source.list_page(&binding, &q).await {
        Ok(p) => EntityPage {
            items: p.items.into_iter().map(|e| Entity {
                id: e.id,
                fields: Json(serde_json::Value::Object(e.fields)),
            }).collect(),
            total_count: p.total_count as i64,
            page: p.page as i32,
            page_size: p.page_size as i32,
        },
        Err(e) => {
            tracing::error!(target: "server::data", "list_page error: {e}");
            EntityPage {
                items: Vec::new(),
                total_count: 0,
                page,
                page_size,
            }
        }
    }
}
```

Hilfsfunktion `binding_for`:

```rust
fn binding_for(entity_type: &str) -> shared::source::EntityBinding {
    crate::example::current()
        .and_then(|set| set.entities.get(entity_type).cloned())
        .and_then(|ty| ty.settings)
        .and_then(|s| s.binding)
        .unwrap_or_else(|| shared::source::default_binding_for(entity_type))
}
```

Analog `pub async fn entity_by_id`, `create_entity`, `update_entity`, `delete_entity` umstellen — jeweils Binding holen, Registry routen, Source-Methode aufrufen, Ergebnis auf den Async-GraphQL-Wrap zurückkonvertieren.

- [ ] **Step 4: Run, verify still passing (incl. ALL existing tests)**

```bash
cargo test --workspace
```

Expected: alle Tests grün — kritische Akzeptanz für B0 erfüllt.

- [ ] **Step 5: Manual smoke test against shop**

```bash
cargo run -p server -- --data-dir ./examples/shop
```

In einem zweiten Terminal:

```bash
cd client && trunk serve
```

Browser auf `http://127.0.0.1:8080` öffnen, Produkte-Liste laden, einen Datensatz editieren+speichern. Erwartetes Verhalten: identisch zu vor B0.

- [ ] **Step 6: Commit**

```bash
git add server/src/data.rs server/tests/source_registry_boot.rs
git commit -m "refactor(server): data layer routes CRUD via SourceRegistry"
```

---

## Task 13: examples/shop/sources.toml + verify

**Files:**
- Create: `examples/shop/sources.toml`

- [ ] **Step 1: Create the file**

`examples/shop/sources.toml`:

```toml
# Default-Source: DBlicious legt seine eigenen Tabellen an.
# DBLICIOUS_DATABASE_URL ueberschreibt die URL.
[sources.local]
kind = "managed-sqlite"
url  = "${DBLICIOUS_DATABASE_URL:-sqlite::memory:}"
```

- [ ] **Step 2: Verify nothing changes**

```bash
cargo test --workspace
cargo run -p server -- --data-dir ./examples/shop
```

(Im zweiten Terminal Client starten, ein bisschen klicken.) Erwartetes Verhalten: identisch zu vor Task 13. Der explizit deklarierte `local`-Eintrag *ersetzt* die Default-Synthese — Verhalten ist identisch, aber jetzt steht die Konfiguration im Beispiel.

- [ ] **Step 3: Commit**

```bash
git add examples/shop/sources.toml
git commit -m "feat(examples/shop): sources.toml declares default local source"
```

---

## Task 14: ForeignSqliteSource skeleton + capabilities

**Files:**
- Create: `server/src/source/foreign_sqlite.rs`
- Modify: `server/src/source/mod.rs`
- Create: `server/tests/source_foreign_sqlite.rs`

- [ ] **Step 1: Write the failing test**

`server/tests/source_foreign_sqlite.rs`:

```rust
use server::source::foreign_sqlite::ForeignSqliteSource;
use server::source::{Capabilities, Source};

#[test]
fn foreign_sqlite_capabilities() {
    let src = ForeignSqliteSource::new("d2v_legacy".into(), "sqlite::memory:".into());
    assert_eq!(src.name(), "d2v_legacy");
    assert_eq!(src.kind(), "foreign-sqlite");
    assert_eq!(
        src.capabilities(),
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: true,
            supports_composite_pk: true,
        }
    );
}
```

- [ ] **Step 2: Run, verify it fails (module missing)**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: Compile-Fehler "no `foreign_sqlite` in `source`".

- [ ] **Step 3: Implement skeleton**

`server/src/source/foreign_sqlite.rs`:

```rust
//! `ForeignSqliteSource` — liest/schreibt eine fremde SQLite-DB. Legt
//! KEIN eigenes Schema an; introspiziert beim `init`.

use async_trait::async_trait;

use shared::source::{EntityBinding, EntityId};
use shared::{Entity, EntityPage};

use super::{Capabilities, PageQuery, Source, SourceError};

pub struct ForeignSqliteSource {
    name: String,
    url: String,
    conn: Option<sea_orm::DatabaseConnection>,
    introspected: super::introspect::Schema,
}

impl ForeignSqliteSource {
    pub fn new(name: String, url: String) -> Self {
        Self {
            name,
            url,
            conn: None,
            introspected: super::introspect::Schema::default(),
        }
    }
}

#[async_trait]
impl Source for ForeignSqliteSource {
    fn name(&self) -> &str { &self.name }
    fn kind(&self) -> &'static str { "foreign-sqlite" }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: true,
            supports_composite_pk: true,
        }
    }

    async fn init(&mut self) -> Result<(), SourceError> {
        let mut opts = sea_orm::ConnectOptions::new(self.url.clone());
        opts.sqlx_logging(false);
        let conn = sea_orm::Database::connect(opts).await?;
        self.introspected = super::introspect::read_schema(&conn).await?;
        self.conn = Some(conn);
        Ok(())
    }

    async fn list_page(&self, _b: &EntityBinding, _q: &PageQuery)
        -> Result<EntityPage, SourceError> { unimplemented!("Task 16") }
    async fn get(&self, _b: &EntityBinding, _id: &EntityId)
        -> Result<Option<Entity>, SourceError> { unimplemented!("Task 17") }
    async fn create(&self, _b: &EntityBinding,
        _f: serde_json::Map<String, serde_json::Value>, _a: Option<&str>)
        -> Result<Entity, SourceError> { unimplemented!("Task 18") }
    async fn update(&self, _b: &EntityBinding, _id: &EntityId,
        _f: serde_json::Map<String, serde_json::Value>, _a: Option<&str>)
        -> Result<Option<Entity>, SourceError> { unimplemented!("Task 18") }
    async fn delete(&self, _b: &EntityBinding, _id: &EntityId)
        -> Result<bool, SourceError> { unimplemented!("Task 18") }
}
```

Modul registrieren in `server/src/source/mod.rs`:

```rust
pub mod foreign_sqlite;
pub mod introspect;
```

Auch `boot_registry` um den neuen Kind erweitern (in `server/src/source/mod.rs`):

```rust
            "foreign-sqlite" => {
                let url = cfg.url.clone().ok_or_else(|| {
                    SourceError::Other(format!("source '{name}' (foreign-sqlite) requires url"))
                })?;
                Box::new(crate::source::foreign_sqlite::ForeignSqliteSource::new(name.clone(), url))
            }
```

Stub `introspect.rs` (Inhalt kommt in Task 15):

```rust
//! SQLite-Schema-Introspektion via sqlite_master + PRAGMA.

use std::collections::BTreeMap;

use sea_orm::DatabaseConnection;

use super::SourceError;

#[derive(Debug, Default, Clone)]
pub struct Schema {
    pub tables: BTreeMap<String, TableMeta>,
}

#[derive(Debug, Clone)]
pub struct TableMeta {
    pub name: String,
    pub columns: Vec<ColumnMeta>,
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub sql_type: String,
    pub not_null: bool,
}

pub async fn read_schema(_db: &DatabaseConnection) -> Result<Schema, SourceError> {
    // Task 15
    Ok(Schema::default())
}
```

- [ ] **Step 4: Run, verify passing**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add server/src/source/foreign_sqlite.rs server/src/source/introspect.rs server/src/source/mod.rs server/tests/source_foreign_sqlite.rs
git commit -m "feat(server): ForeignSqliteSource skeleton + introspect stub"
```

---

## Task 15: SQLite schema introspection

**Files:**
- Modify: `server/src/source/introspect.rs`
- Modify: `server/tests/source_foreign_sqlite.rs`

- [ ] **Step 1: Write the failing test**

In `server/tests/source_foreign_sqlite.rs` ergänzen:

```rust
use sea_orm::{ConnectOptions, ConnectionTrait, Database, Statement, DbBackend};

async fn build_fixture_db() -> sea_orm::DatabaseConnection {
    let opts = ConnectOptions::new("sqlite::memory:");
    let db = Database::connect(opts).await.unwrap();
    let stmts = vec![
        r#"CREATE TABLE Single (Id INTEGER PRIMARY KEY, Name TEXT NOT NULL)"#,
        r#"CREATE TABLE Composite (
            BankCode TEXT NOT NULL,
            Code     TEXT NOT NULL,
            Balance  REAL,
            PRIMARY KEY (BankCode, Code)
        )"#,
    ];
    for s in stmts {
        db.execute(Statement::from_string(DbBackend::Sqlite, s.into())).await.unwrap();
    }
    db
}

#[tokio::test]
async fn introspect_reads_single_and_composite_pk() {
    let db = build_fixture_db().await;
    let schema = server::source::introspect::read_schema(&db).await.unwrap();

    let single = schema.tables.get("Single").expect("table Single");
    assert_eq!(single.primary_key, vec!["Id".to_string()]);
    assert_eq!(single.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
               vec!["Id", "Name"]);

    let comp = schema.tables.get("Composite").expect("table Composite");
    assert_eq!(comp.primary_key, vec!["BankCode".to_string(), "Code".to_string()]);
    let col_names: Vec<&str> = comp.columns.iter().map(|c| c.name.as_str()).collect();
    assert!(col_names.contains(&"BankCode"));
    assert!(col_names.contains(&"Balance"));
}
```

- [ ] **Step 2: Run, verify it fails**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: assertion failures (`schema.tables` is empty in stub).

- [ ] **Step 3: Implement read_schema**

In `server/src/source/introspect.rs`:

```rust
use sea_orm::{ConnectionTrait, DbBackend, FromQueryResult, Statement};

#[derive(FromQueryResult, Debug)]
struct TableRow {
    name: String,
}

#[derive(FromQueryResult, Debug)]
struct ColumnRow {
    name: String,
    #[sea_orm(column_name = "type")]
    type_: String,
    notnull: i32,
    pk: i32,
}

pub async fn read_schema(db: &DatabaseConnection) -> Result<Schema, SourceError> {
    // 1. Tabellen-Liste.
    let table_rows: Vec<TableRow> = TableRow::find_by_statement(Statement::from_string(
        DbBackend::Sqlite,
        "SELECT name FROM sqlite_master \
         WHERE type='table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name".into(),
    ))
    .all(db).await?;

    let mut tables = BTreeMap::new();
    for t in table_rows {
        let col_rows: Vec<ColumnRow> = ColumnRow::find_by_statement(Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA table_info(\"{}\")", t.name.replace('"', "\"\"")),
        ))
        .all(db).await?;

        let mut columns = Vec::new();
        // PRAGMA liefert pk-Position 1-basiert; 0 heisst kein-PK. Wir sortieren
        // anschliessend nach pk-Position fuer die Primary-Key-Liste.
        let mut pk_pairs: Vec<(i32, String)> = Vec::new();
        for c in col_rows {
            if c.pk > 0 {
                pk_pairs.push((c.pk, c.name.clone()));
            }
            columns.push(ColumnMeta {
                name: c.name,
                sql_type: c.type_,
                not_null: c.notnull != 0,
            });
        }
        pk_pairs.sort_by_key(|(idx, _)| *idx);
        let primary_key: Vec<String> = pk_pairs.into_iter().map(|(_, n)| n).collect();

        tables.insert(t.name.clone(), TableMeta {
            name: t.name,
            columns,
            primary_key,
        });
    }
    Ok(Schema { tables })
}
```

- [ ] **Step 4: Run, verify passing**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add server/src/source/introspect.rs server/tests/source_foreign_sqlite.rs
git commit -m "feat(server): SQLite schema introspection (single + composite PK)"
```

---

## Task 16: ForeignSqliteSource::list_page with SQL pushdown

**Files:**
- Modify: `server/src/source/foreign_sqlite.rs`
- Modify: `server/tests/source_foreign_sqlite.rs`

- [ ] **Step 1: Write the failing tests**

In `server/tests/source_foreign_sqlite.rs` ergänzen:

```rust
use shared::source::{BindingLocator, EntityBinding};
use std::collections::BTreeMap;

async fn fixture_source_with_data() -> ForeignSqliteSource {
    // Wir bauen die DB direkt im URL — SeaORM's `sqlite::memory:` ist pro-
    // Connection-Pool, aber wir geben dem ForeignSqliteSource den selben
    // URL und lassen ihn seine eigene Connection oeffnen. Trick: `?cache=shared`
    // mit named Memory-DB, damit beide Connections dieselbe DB sehen.
    let url = format!("sqlite:file:test_{}?mode=memory&cache=shared", uuid_like());
    {
        let opts = sea_orm::ConnectOptions::new(url.clone());
        let db = sea_orm::Database::connect(opts).await.unwrap();
        let stmts = [
            "CREATE TABLE DatevAccounts (Number INTEGER PRIMARY KEY, Name TEXT NOT NULL)",
            "INSERT INTO DatevAccounts (Number, Name) VALUES (1000, 'Kasse')",
            "INSERT INTO DatevAccounts (Number, Name) VALUES (1200, 'Bank')",
            "INSERT INTO DatevAccounts (Number, Name) VALUES (1800, 'Privat')",
        ];
        for s in stmts {
            db.execute(sea_orm::Statement::from_string(sea_orm::DbBackend::Sqlite, s.into())).await.unwrap();
        }
        // Connection live halten, sonst verschwindet die shared-cache-DB,
        // sobald die letzte Connection zugemacht wird.
        // Workaround: zurueckgeben.
    }
    let mut src = ForeignSqliteSource::new("d2v_test".into(), url);
    src.init().await.unwrap();
    src
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string()
}

fn account_binding() -> EntityBinding {
    let mut map = BTreeMap::new();
    map.insert("number".into(), "Number".into());
    map.insert("name".into(), "Name".into());
    EntityBinding {
        source: "d2v_test".into(),
        locator: BindingLocator::Table { table: "DatevAccounts".into() },
        primary_key: vec!["number".into()],
        read_only: false,
        column_map: map,
    }
}

#[tokio::test]
async fn foreign_list_page_returns_all_rows() {
    let src = fixture_source_with_data().await;
    let page = src.list_page(&account_binding(), &server::source::PageQuery {
        page: 1, page_size: 10, sort: None, filter: shared::FilterCriteria::default(),
    }).await.unwrap();
    assert_eq!(page.total_count, 3);
    assert_eq!(page.items.len(), 3);
}

#[tokio::test]
async fn foreign_list_page_paginates() {
    let src = fixture_source_with_data().await;
    let page = src.list_page(&account_binding(), &server::source::PageQuery {
        page: 1, page_size: 2, sort: None, filter: shared::FilterCriteria::default(),
    }).await.unwrap();
    assert_eq!(page.total_count, 3);
    assert_eq!(page.items.len(), 2);
}
```

Wichtig: das Test-Setup braucht im `dev-dependencies` (in `server/Cargo.toml`) ggf. nichts neues — Std-`SystemTime` reicht.

Hmm, eigentlich: die Shared-Cache-Memory-DB ist tricky. Wenn der ForeignSqliteSource Sea-ORM-Pool macht und die Test-Connection im Block oben dropt, ist die DB weg. Lass eine Connection-Variable als Test-Scope-Anchor halten:

Refaktoriere `fixture_source_with_data` wie folgt:

```rust
async fn fixture_source_with_data() -> (ForeignSqliteSource, sea_orm::DatabaseConnection) {
    let url = format!("sqlite:file:test_{}?mode=memory&cache=shared", uuid_like());
    let opts = sea_orm::ConnectOptions::new(url.clone());
    let anchor = sea_orm::Database::connect(opts).await.unwrap();
    let stmts = [
        "CREATE TABLE DatevAccounts (Number INTEGER PRIMARY KEY, Name TEXT NOT NULL)",
        "INSERT INTO DatevAccounts (Number, Name) VALUES (1000, 'Kasse')",
        "INSERT INTO DatevAccounts (Number, Name) VALUES (1200, 'Bank')",
        "INSERT INTO DatevAccounts (Number, Name) VALUES (1800, 'Privat')",
    ];
    for s in stmts {
        anchor.execute(sea_orm::Statement::from_string(sea_orm::DbBackend::Sqlite, s.into())).await.unwrap();
    }
    let mut src = ForeignSqliteSource::new("d2v_test".into(), url);
    src.init().await.unwrap();
    (src, anchor)  // Aufrufer haelt anchor lebendig
}
```

Tests dann:

```rust
let (src, _anchor) = fixture_source_with_data().await;
```

- [ ] **Step 2: Run, verify fails**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: "not implemented: Task 16".

- [ ] **Step 3: Implement list_page**

In `server/src/source/foreign_sqlite.rs` die `list_page`-Methode ersetzen:

```rust
async fn list_page(
    &self,
    binding: &EntityBinding,
    query: &PageQuery,
) -> Result<EntityPage, SourceError> {
    let table = match &binding.locator {
        shared::source::BindingLocator::Table { table } => table.clone(),
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    let conn = self.conn.as_ref().ok_or_else(|| SourceError::Other("init not called".into()))?;
    let table_meta = self.introspected.tables.get(&table)
        .ok_or_else(|| SourceError::Other(format!("table not found: {table}")))?;

    // Spalten waehlen: alle Spalten aus column_map.values() + PKs, mindestens
    // alle Tabellenspalten falls keine column_map.
    let select_cols: Vec<String> = if binding.column_map.is_empty() {
        table_meta.columns.iter().map(|c| c.name.clone()).collect()
    } else {
        let mut s: std::collections::BTreeSet<String> =
            binding.column_map.values().cloned().collect();
        for pk in &table_meta.primary_key {
            s.insert(pk.clone());
        }
        s.into_iter().collect()
    };

    let cols_sql = select_cols.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let table_sql = quote_ident(&table);

    let page = query.page.max(1);
    let page_size = query.page_size.max(1);
    let offset = (page - 1) as i64 * page_size as i64;

    // ORDER BY (nur einfache Spalten-Identitaet; ColumnMeta-key → DB-Spalte via map)
    let order_sql = match &query.sort {
        Some(s) => {
            let db_col = binding.column_map.get(&s.field)
                .cloned()
                .unwrap_or_else(|| s.field.clone());
            let dir = match s.direction {
                shared::SortDirection::Asc => "ASC",
                shared::SortDirection::Desc => "DESC",
            };
            format!(" ORDER BY {} {}", quote_ident(&db_col), dir)
        }
        None => String::new(),
    };

    // Filter — siehe Task 16b unten; vorerst: Predicate-Liste ignorieren,
    // global_search ebenfalls. SQL-Pushdown der Praedikate kommt im naechsten
    // Sub-Step.
    let where_sql = String::new();

    let sql_list = format!(
        "SELECT {cols_sql} FROM {table_sql}{where_sql}{order_sql} LIMIT {page_size} OFFSET {offset}"
    );
    let sql_count = format!("SELECT COUNT(*) AS c FROM {table_sql}{where_sql}");

    use sea_orm::{ConnectionTrait, DbBackend, FromQueryResult, Statement, JsonValue};

    let rows_json: Vec<serde_json::Value> = JsonValue::find_by_statement(
        Statement::from_string(DbBackend::Sqlite, sql_list)
    ).all(conn).await?;

    #[derive(FromQueryResult)]
    struct CountRow { c: i64 }
    let count_row: CountRow = CountRow::find_by_statement(
        Statement::from_string(DbBackend::Sqlite, sql_count)
    ).one(conn).await?.ok_or_else(|| SourceError::Other("count returned no row".into()))?;

    // Spalten zurueckmappen: DB-Spalte → ColumnMeta.key (reverse map).
    let reverse_map: std::collections::BTreeMap<String, String> =
        binding.column_map.iter().map(|(k, v)| (v.clone(), k.clone())).collect();

    let items: Vec<shared::Entity> = rows_json.into_iter().map(|row| {
        let obj = row.as_object().cloned().unwrap_or_default();
        let mut fields = serde_json::Map::new();
        for (db_col, val) in obj {
            let logical = reverse_map.get(&db_col).cloned().unwrap_or(db_col);
            fields.insert(logical, val);
        }
        let id = encode_pk_from_row(&fields, &binding.primary_key);
        shared::Entity { id, fields }
    }).collect();

    Ok(EntityPage {
        items,
        total_count: count_row.c as u64,
        page: page as u32,
        page_size: page_size as u32,
    })
}

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn encode_pk_from_row(fields: &serde_json::Map<String, serde_json::Value>, pk_cols: &[String]) -> String {
    let parts: Vec<String> = pk_cols.iter().map(|c| {
        fields.get(c).map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        }).unwrap_or_default()
    }).collect();
    if parts.len() == 1 {
        shared::source::EntityId::Single(parts.into_iter().next().unwrap()).encode()
    } else {
        shared::source::EntityId::Composite(parts).encode()
    }
}
```

- [ ] **Step 4: Run, verify passing**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add server/src/source/foreign_sqlite.rs server/tests/source_foreign_sqlite.rs
git commit -m "feat(server): ForeignSqliteSource::list_page with column-map + pagination"
```

---

## Task 17: ForeignSqliteSource — get with composite-PK + filter pushdown

**Files:**
- Modify: `server/src/source/foreign_sqlite.rs`
- Modify: `server/tests/source_foreign_sqlite.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn foreign_get_single_pk() {
    let (src, _a) = fixture_source_with_data().await;
    let id = shared::source::EntityId::Single("1200".into());
    let e = src.get(&account_binding(), &id).await.unwrap();
    assert!(e.is_some());
    assert_eq!(e.unwrap().fields.get("name").and_then(|v| v.as_str()), Some("Bank"));
}

async fn fixture_composite_pk() -> (ForeignSqliteSource, sea_orm::DatabaseConnection) {
    let url = format!("sqlite:file:test_{}?mode=memory&cache=shared", uuid_like());
    let anchor = sea_orm::Database::connect(sea_orm::ConnectOptions::new(url.clone())).await.unwrap();
    let stmts = [
        "CREATE TABLE StarMoneyAccounts (
            BankCode TEXT NOT NULL,
            Code     TEXT NOT NULL,
            Balance  REAL,
            PRIMARY KEY (BankCode, Code)
        )",
        "INSERT INTO StarMoneyAccounts VALUES ('60050101', '1234', 100.0)",
        "INSERT INTO StarMoneyAccounts VALUES ('60050101', '5678', 200.0)",
    ];
    for s in stmts {
        anchor.execute(sea_orm::Statement::from_string(sea_orm::DbBackend::Sqlite, s.into())).await.unwrap();
    }
    let mut src = ForeignSqliteSource::new("sm".into(), url);
    src.init().await.unwrap();
    (src, anchor)
}

fn sm_binding() -> EntityBinding {
    let mut map = BTreeMap::new();
    map.insert("bankCode".into(), "BankCode".into());
    map.insert("code".into(), "Code".into());
    map.insert("balance".into(), "Balance".into());
    EntityBinding {
        source: "sm".into(),
        locator: BindingLocator::Table { table: "StarMoneyAccounts".into() },
        primary_key: vec!["bankCode".into(), "code".into()],
        read_only: false,
        column_map: map,
    }
}

#[tokio::test]
async fn foreign_get_composite_pk() {
    let (src, _a) = fixture_composite_pk().await;
    let id = shared::source::EntityId::Composite(vec!["60050101".into(), "5678".into()]);
    let e = src.get(&sm_binding(), &id).await.unwrap();
    assert!(e.is_some());
    let row = e.unwrap();
    assert_eq!(row.fields.get("code").and_then(|v| v.as_str()), Some("5678"));
}
```

- [ ] **Step 2: Run, verify fails**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: "not implemented".

- [ ] **Step 3: Implement get**

In `server/src/source/foreign_sqlite.rs`:

```rust
async fn get(
    &self,
    binding: &EntityBinding,
    id: &EntityId,
) -> Result<Option<Entity>, SourceError> {
    let table = match &binding.locator {
        shared::source::BindingLocator::Table { table } => table.clone(),
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    let conn = self.conn.as_ref().ok_or_else(|| SourceError::Other("init not called".into()))?;
    let table_meta = self.introspected.tables.get(&table)
        .ok_or_else(|| SourceError::Other(format!("table not found: {table}")))?;

    // PK-Spalten-Werte ermitteln (im DB-Spalten-Namensraum).
    let pk_logical = &binding.primary_key;
    let pk_values: Vec<String> = match id {
        EntityId::Single(s) => {
            if pk_logical.len() != 1 {
                return Err(SourceError::Other(format!(
                    "single-PK id but binding has {} PK columns", pk_logical.len()
                )));
            }
            vec![s.clone()]
        }
        EntityId::Composite(parts) => {
            if parts.len() != pk_logical.len() {
                return Err(SourceError::Other(format!(
                    "composite-PK id has {} parts, binding has {} columns",
                    parts.len(), pk_logical.len()
                )));
            }
            parts.clone()
        }
    };

    // WHERE: `pk_db_col = ?`-Liste, mit Werten gebunden.
    let where_clauses: Vec<String> = pk_logical.iter().map(|logical| {
        let db_col = binding.column_map.get(logical).cloned().unwrap_or_else(|| logical.clone());
        format!("{} = ?", quote_ident(&db_col))
    }).collect();
    let where_sql = format!(" WHERE {}", where_clauses.join(" AND "));

    // Spalten-Liste analog list_page.
    let select_cols: Vec<String> = if binding.column_map.is_empty() {
        table_meta.columns.iter().map(|c| c.name.clone()).collect()
    } else {
        let mut s: std::collections::BTreeSet<String> = binding.column_map.values().cloned().collect();
        for pk in &table_meta.primary_key { s.insert(pk.clone()); }
        s.into_iter().collect()
    };

    let cols_sql = select_cols.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let sql = format!("SELECT {cols_sql} FROM {} {} LIMIT 1", quote_ident(&table), where_sql);

    use sea_orm::{ConnectionTrait, DbBackend, FromQueryResult, Statement, Value, JsonValue};
    let values: Vec<Value> = pk_values.into_iter().map(Value::from).collect();
    let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, sql, values);

    let row_opt: Option<serde_json::Value> = JsonValue::find_by_statement(stmt).one(conn).await?;
    let Some(row) = row_opt else { return Ok(None) };

    let reverse_map: std::collections::BTreeMap<String, String> =
        binding.column_map.iter().map(|(k, v)| (v.clone(), k.clone())).collect();
    let obj = row.as_object().cloned().unwrap_or_default();
    let mut fields = serde_json::Map::new();
    for (db_col, val) in obj {
        let logical = reverse_map.get(&db_col).cloned().unwrap_or(db_col);
        fields.insert(logical, val);
    }
    let id_string = encode_pk_from_row(&fields, &binding.primary_key);
    Ok(Some(shared::Entity { id: id_string, fields }))
}
```

- [ ] **Step 4: Run, verify passing**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add server/src/source/foreign_sqlite.rs server/tests/source_foreign_sqlite.rs
git commit -m "feat(server): ForeignSqliteSource::get (single + composite PK)"
```

---

## Task 18: ForeignSqliteSource — create / update / delete with read-only support

**Files:**
- Modify: `server/src/source/foreign_sqlite.rs`
- Modify: `server/tests/source_foreign_sqlite.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn foreign_create_inserts_row_and_returns_entity() {
    let (src, _a) = fixture_source_with_data().await;
    let mut fields = serde_json::Map::new();
    fields.insert("number".into(), serde_json::json!(9999));
    fields.insert("name".into(), serde_json::json!("Neu"));
    let created = src.create(&account_binding(), fields, None).await.unwrap();
    assert_eq!(created.id, "9999");

    let id = shared::source::EntityId::Single("9999".into());
    let one = src.get(&account_binding(), &id).await.unwrap().unwrap();
    assert_eq!(one.fields.get("name").and_then(|v| v.as_str()), Some("Neu"));
}

#[tokio::test]
async fn foreign_update_and_delete() {
    let (src, _a) = fixture_source_with_data().await;
    let id = shared::source::EntityId::Single("1200".into());

    let mut patch = serde_json::Map::new();
    patch.insert("name".into(), serde_json::json!("Renamed"));
    let upd = src.update(&account_binding(), &id, patch, None).await.unwrap().unwrap();
    assert_eq!(upd.fields.get("name").and_then(|v| v.as_str()), Some("Renamed"));

    let removed = src.delete(&account_binding(), &id).await.unwrap();
    assert!(removed);
    let after = src.get(&account_binding(), &id).await.unwrap();
    assert!(after.is_none());
}

#[tokio::test]
async fn foreign_read_only_binding_rejects_mutations() {
    let (src, _a) = fixture_source_with_data().await;
    let mut b = account_binding();
    b.read_only = true;

    let fields = serde_json::Map::new();
    let err = src.create(&b, fields, None).await.err().expect("should error");
    assert!(matches!(err, server::source::SourceError::ReadOnly));
}
```

- [ ] **Step 2: Run, verify fails**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: drei neue Tests "not implemented".

- [ ] **Step 3: Implement create / update / delete**

In `server/src/source/foreign_sqlite.rs`:

```rust
async fn create(
    &self,
    binding: &EntityBinding,
    fields: serde_json::Map<String, serde_json::Value>,
    _actor_user_id: Option<&str>,
) -> Result<Entity, SourceError> {
    if binding.read_only {
        return Err(SourceError::ReadOnly);
    }
    let table = match &binding.locator {
        shared::source::BindingLocator::Table { table } => table.clone(),
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    let conn = self.conn.as_ref().ok_or_else(|| SourceError::Other("init not called".into()))?;

    // logischer Spalten-Name → DB-Name
    let resolve = |logical: &str| binding.column_map.get(logical).cloned().unwrap_or_else(|| logical.to_string());

    let mut cols: Vec<String> = Vec::new();
    let mut placeholders: Vec<String> = Vec::new();
    let mut values: Vec<sea_orm::Value> = Vec::new();
    for (logical, val) in &fields {
        cols.push(quote_ident(&resolve(logical)));
        placeholders.push("?".into());
        values.push(json_to_sea_value(val));
    }

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_ident(&table),
        cols.join(", "),
        placeholders.join(", "),
    );
    let stmt = sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Sqlite, sql, values);
    use sea_orm::ConnectionTrait;
    conn.execute(stmt).await?;

    let id_string = encode_pk_from_row(&fields, &binding.primary_key);
    Ok(shared::Entity { id: id_string, fields })
}

async fn update(
    &self,
    binding: &EntityBinding,
    id: &EntityId,
    patch: serde_json::Map<String, serde_json::Value>,
    _actor_user_id: Option<&str>,
) -> Result<Option<Entity>, SourceError> {
    if binding.read_only {
        return Err(SourceError::ReadOnly);
    }
    let table = match &binding.locator {
        shared::source::BindingLocator::Table { table } => table.clone(),
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    let conn = self.conn.as_ref().ok_or_else(|| SourceError::Other("init not called".into()))?;

    let resolve = |logical: &str| binding.column_map.get(logical).cloned().unwrap_or_else(|| logical.to_string());

    if patch.is_empty() {
        return self.get(binding, id).await;
    }

    let mut set_parts: Vec<String> = Vec::new();
    let mut values: Vec<sea_orm::Value> = Vec::new();
    for (logical, val) in &patch {
        set_parts.push(format!("{} = ?", quote_ident(&resolve(logical))));
        values.push(json_to_sea_value(val));
    }

    let pk_logical = &binding.primary_key;
    let pk_values: Vec<String> = match id {
        EntityId::Single(s) => vec![s.clone()],
        EntityId::Composite(parts) => parts.clone(),
    };
    let where_clauses: Vec<String> = pk_logical.iter().map(|logical| {
        format!("{} = ?", quote_ident(&resolve(logical)))
    }).collect();
    for pv in &pk_values { values.push(sea_orm::Value::from(pv.clone())); }

    let sql = format!(
        "UPDATE {} SET {} WHERE {}",
        quote_ident(&table),
        set_parts.join(", "),
        where_clauses.join(" AND "),
    );
    let stmt = sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Sqlite, sql, values);
    use sea_orm::ConnectionTrait;
    conn.execute(stmt).await?;

    self.get(binding, id).await
}

async fn delete(
    &self,
    binding: &EntityBinding,
    id: &EntityId,
) -> Result<bool, SourceError> {
    if binding.read_only {
        return Err(SourceError::ReadOnly);
    }
    let table = match &binding.locator {
        shared::source::BindingLocator::Table { table } => table.clone(),
        other => return Err(SourceError::UnsupportedLocator(format!("{other:?}"))),
    };
    let conn = self.conn.as_ref().ok_or_else(|| SourceError::Other("init not called".into()))?;
    let resolve = |logical: &str| binding.column_map.get(logical).cloned().unwrap_or_else(|| logical.to_string());

    let pk_logical = &binding.primary_key;
    let pk_values: Vec<String> = match id {
        EntityId::Single(s) => vec![s.clone()],
        EntityId::Composite(parts) => parts.clone(),
    };
    let where_clauses: Vec<String> = pk_logical.iter().map(|logical| {
        format!("{} = ?", quote_ident(&resolve(logical)))
    }).collect();
    let mut values: Vec<sea_orm::Value> = Vec::new();
    for pv in &pk_values { values.push(sea_orm::Value::from(pv.clone())); }

    let sql = format!(
        "DELETE FROM {} WHERE {}",
        quote_ident(&table),
        where_clauses.join(" AND "),
    );
    let stmt = sea_orm::Statement::from_sql_and_values(sea_orm::DbBackend::Sqlite, sql, values);
    use sea_orm::ConnectionTrait;
    let res = conn.execute(stmt).await?;
    Ok(res.rows_affected() > 0)
}

fn json_to_sea_value(v: &serde_json::Value) -> sea_orm::Value {
    match v {
        serde_json::Value::Null => sea_orm::Value::String(None),
        serde_json::Value::Bool(b) => sea_orm::Value::Bool(Some(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { sea_orm::Value::BigInt(Some(i)) }
            else if let Some(f) = n.as_f64() { sea_orm::Value::Double(Some(f)) }
            else { sea_orm::Value::String(Some(Box::new(n.to_string()))) }
        }
        serde_json::Value::String(s) => sea_orm::Value::String(Some(Box::new(s.clone()))),
        other => sea_orm::Value::String(Some(Box::new(other.to_string()))),
    }
}
```

- [ ] **Step 4: Run, verify all foreign-sqlite tests pass**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: 9 passed.

- [ ] **Step 5: Whole-workspace regression**

```bash
cargo test --workspace
```

Expected: alle grün.

- [ ] **Step 6: Commit**

```bash
git add server/src/source/foreign_sqlite.rs server/tests/source_foreign_sqlite.rs
git commit -m "feat(server): ForeignSqliteSource create/update/delete + read-only"
```

---

## Task 19: Filter pushdown to SQL (basic predicates)

**Files:**
- Modify: `server/src/source/foreign_sqlite.rs`
- Modify: `server/tests/source_foreign_sqlite.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn foreign_list_page_filters_by_number_equals_via_sql() {
    let (src, _a) = fixture_source_with_data().await;
    let filter = shared::FilterCriteria {
        global_search: None,
        predicates: vec![shared::ColumnFilter {
            key: "number".into(),
            predicate: shared::FilterPredicate::NumberEquals { value: 1200.0 },
        }],
    };
    let page = src.list_page(&account_binding(), &server::source::PageQuery {
        page: 1, page_size: 10, sort: None, filter,
    }).await.unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items[0].fields.get("name").and_then(|v| v.as_str()), Some("Bank"));
}

#[tokio::test]
async fn foreign_list_page_filters_by_text_contains_case_insensitive() {
    let (src, _a) = fixture_source_with_data().await;
    let filter = shared::FilterCriteria {
        global_search: None,
        predicates: vec![shared::ColumnFilter {
            key: "name".into(),
            predicate: shared::FilterPredicate::TextContains {
                value: "BANK".into(),
                case_insensitive: true,
            },
        }],
    };
    let page = src.list_page(&account_binding(), &server::source::PageQuery {
        page: 1, page_size: 10, sort: None, filter,
    }).await.unwrap();
    assert_eq!(page.total_count, 1);
}
```

- [ ] **Step 2: Run, verify fails**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: total_count = 3 statt 1 (Filter wird heute ignoriert).

- [ ] **Step 3: Implement filter pushdown**

In `server/src/source/foreign_sqlite.rs`, oben einen Hilfs-Builder hinzufügen:

```rust
fn build_filter_sql(
    filter: &shared::FilterCriteria,
    binding: &EntityBinding,
) -> (String, Vec<sea_orm::Value>) {
    let resolve = |logical: &str| binding.column_map.get(logical).cloned().unwrap_or_else(|| logical.to_string());

    let mut clauses: Vec<String> = Vec::new();
    let mut values: Vec<sea_orm::Value> = Vec::new();
    for cf in &filter.predicates {
        let db_col = resolve(&cf.key);
        match &cf.predicate {
            shared::FilterPredicate::TextContains { value, case_insensitive } => {
                if *case_insensitive {
                    clauses.push(format!("LOWER({}) LIKE LOWER(?)", quote_ident(&db_col)));
                } else {
                    clauses.push(format!("{} LIKE ?", quote_ident(&db_col)));
                }
                values.push(sea_orm::Value::from(format!("%{value}%")));
            }
            shared::FilterPredicate::TextEquals { value, case_insensitive } => {
                if *case_insensitive {
                    clauses.push(format!("LOWER({}) = LOWER(?)", quote_ident(&db_col)));
                } else {
                    clauses.push(format!("{} = ?", quote_ident(&db_col)));
                }
                values.push(sea_orm::Value::from(value.clone()));
            }
            shared::FilterPredicate::NumberEquals { value } => {
                clauses.push(format!("{} = ?", quote_ident(&db_col)));
                values.push(sea_orm::Value::from(*value));
            }
            shared::FilterPredicate::NumberRange { min, max } => {
                if let Some(min) = min {
                    clauses.push(format!("{} >= ?", quote_ident(&db_col)));
                    values.push(sea_orm::Value::from(*min));
                }
                if let Some(max) = max {
                    clauses.push(format!("{} <= ?", quote_ident(&db_col)));
                    values.push(sea_orm::Value::from(*max));
                }
            }
            shared::FilterPredicate::BoolEquals { value } => {
                clauses.push(format!("{} = ?", quote_ident(&db_col)));
                values.push(sea_orm::Value::from(if *value { 1i32 } else { 0i32 }));
            }
            shared::FilterPredicate::DateRange { from, to } => {
                if let Some(from) = from {
                    clauses.push(format!("{} >= ?", quote_ident(&db_col)));
                    values.push(sea_orm::Value::from(from.clone()));
                }
                if let Some(to) = to {
                    clauses.push(format!("{} <= ?", quote_ident(&db_col)));
                    values.push(sea_orm::Value::from(to.clone()));
                }
            }
            shared::FilterPredicate::EnumIn { values: vs } => {
                if !vs.is_empty() {
                    let placeholders = vs.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                    clauses.push(format!("{} IN ({})", quote_ident(&db_col), placeholders));
                    for v in vs { values.push(sea_orm::Value::from(v.clone())); }
                }
            }
            shared::FilterPredicate::IsNull => {
                clauses.push(format!("{} IS NULL", quote_ident(&db_col)));
            }
            shared::FilterPredicate::IsNotNull => {
                clauses.push(format!("{} IS NOT NULL", quote_ident(&db_col)));
            }
        }
    }
    if let Some(search) = filter.global_search.as_deref() {
        if !search.is_empty() {
            // Globale Suche: gegen alle in column_map enthaltenen Spalten LIKE,
            // case-insensitive. Falls keine column_map: gegen alle Tabellenspalten.
            // Hier minimal: gegen die column_map.values().
            let mut search_clauses: Vec<String> = Vec::new();
            for (_, db_col) in &binding.column_map {
                search_clauses.push(format!("LOWER({}) LIKE LOWER(?)", quote_ident(db_col)));
                values.push(sea_orm::Value::from(format!("%{search}%")));
            }
            if !search_clauses.is_empty() {
                clauses.push(format!("({})", search_clauses.join(" OR ")));
            }
        }
    }
    let sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    (sql, values)
}
```

In `list_page` ersetze die `let where_sql = String::new();`-Zeile durch:

```rust
    let (where_sql, where_values) = build_filter_sql(&query.filter, binding);
```

Und die SQL-Ausführung muss `Statement::from_sql_and_values` benutzen statt `Statement::from_string`:

```rust
    let stmt_list = sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Sqlite, sql_list, where_values.clone()
    );
    let rows_json: Vec<serde_json::Value> = JsonValue::find_by_statement(stmt_list).all(conn).await?;

    let stmt_count = sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Sqlite, sql_count, where_values
    );
    let count_row: CountRow = CountRow::find_by_statement(stmt_count).one(conn).await?
        .ok_or_else(|| SourceError::Other("count returned no row".into()))?;
```

- [ ] **Step 4: Run, verify passing**

```bash
cargo test -p server --test source_foreign_sqlite
```

Expected: 11 passed.

- [ ] **Step 5: Commit**

```bash
git add server/src/source/foreign_sqlite.rs server/tests/source_foreign_sqlite.rs
git commit -m "feat(server): SQL filter pushdown in ForeignSqliteSource"
```

---

## Task 20: Final integration — workspace test pass

**Files:** none (verification only)

- [ ] **Step 1: Full workspace test run**

```bash
cargo test --workspace
```

Expected: alle grün — Bestandsschutz B0+B1 erfüllt.

- [ ] **Step 2: Clippy + format check**

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

Expected: keine Warnings/Lints, kein Format-Drift.

- [ ] **Step 3: Manual smoke — shop unverändert**

```bash
cargo run -p server -- --data-dir ./examples/shop
```

Im zweiten Terminal:

```bash
cd client && trunk serve
```

Browser → `http://127.0.0.1:8080` → Produkte-Liste lädt, edit/save funktioniert.

- [ ] **Step 4: Done — Phase 0.6 (B0+B1) abgeschlossen**

```bash
git log --oneline | head -25
```

Es sollten ca. 20 Commits seit dem Spec-Commit `16f0289` auftauchen, alle in der Reihenfolge dieser Tasks. Track A's `examples/d2v` ist jetzt baubar — separater Implementations-Plan folgt.

---

## Self-Review-Notiz

Diese Plan-Datei wurde gegen die Spec-Datei `2026-05-19-dblicious-source-architecture-design.md` geprüft:

- §3 (Architektur — Source/EntityBinding/EntityId/Registry): Tasks 2, 3, 8
- §4 (B0 — Refactoring): Tasks 4, 5, 6, 9, 10, 11, 12, 13
- §5 (B1 — foreign-sqlite): Tasks 14, 15, 16, 17, 18, 19
- §6 (Wire-Format additiv): Task 2 (EntityId), Task 8 (EntitySettings.binding)
- §7 (Konfiguration in sources.toml): Tasks 7, 9, 13
- §10 (Akzeptanz): Tasks 12, 13, 17, 18, 19, 20

Composite-PK-Encoding (Spec §3.5) — implementiert in Task 2 (`encode`/`decode`), Wire-Format-Tests pinnen die JSON-Array-Eskalation explizit.

Bewusst nicht-trivial im Plan: SeaORM's Memory-DB ist pro-Connection, daher die `sqlite:file:test_X?mode=memory&cache=shared`-Anker-Connection-Strategie in den Foreign-Tests (Tasks 16-19). Falls dieser Trick auf der Build-Maschine nicht stabil läuft (z.B. Windows-Cache-Verhalten), Plan-Adjustment: temporäres Datei-DB in `tempfile::tempdir()` statt shared-Memory.

Out-of-Plan (Spec §9 Out-of-Scope): B2 (PostgreSQL/MySQL), B3 (REST/File/Memory), B4 (shop auf echte Spalten), B5 (Designer-DDL via Source). Jeweils eigener Plan, wenn dran.
