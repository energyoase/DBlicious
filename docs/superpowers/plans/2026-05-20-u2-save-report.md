# U2 — Save-Report (Preview-vor-Commit) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Jede Mutation (`create`/`update`/`delete`) kann optional erst eine **Preview** liefern, die zeigt, welche Eigentliche Entity geändert wird (alt/neu pro Feld), welche abgeleiteten Rows entstehen (z.B. computed AccountEntries via `derive_handler`) und welche Warnings der Server ausgibt. Nach Bestätigung wird die Preview committed; ohne Bestätigung verfällt sie nach TTL. Editor-Flow toggled über `EntitySettings.preview_on_save`.

**Architecture:** Wire-Typen `SavePreview`, `EntityDiff`, `DerivedRow`, `FieldDiff`, `DiffKind`, `PreviewCounts` in neuem Modul `shared::preview`. Server hält in-memory `DashMap<PreviewId, PreviewSession>` mit 5-Min-TTL (Pattern aus U6 Run-Store). Fünf neue GraphQL-Mutationen: `previewCreateEntity`, `previewUpdateEntity`, `previewDeleteEntity`, `commitPreview`, `cancelPreview`. `derive_handler`-Registry mit Default-Noop-Handler; T5/T6 registriert sich später. Client: `<SavePreviewModal>` mit drei Sections (Primary-Diff / Derived-per-Entity-Type / Warnings) in einer ledger-document-Ästhetik (serif body, warm accent left-border, tabular-nums in Diff-Werten). Editor-Save-Pfad branched auf `EntitySettings.preview_on_save`.

**Tech Stack:** Rust, `shared` (serde), `server` (axum + async-graphql + `dashmap`), `client` (Leptos), `cargo test --workspace`.

**Spec-Referenz:** [`docs/superpowers/specs/2026-05-20-u2-save-report-design.md`](../specs/2026-05-20-u2-save-report-design.md).

**Vorbedingung:** keine. U6 (`ActionRunStore`-Pattern) ist parallele Spec, nicht Voraussetzung.

**Frontend-design Entscheidungen** (aus skill-output für die Modal-Ästhetik):
1. **Ledger-document Aesthetic**: serif body, dezenter warmer Akzent (`#b45309` Burnt Sienna) als 3px left-border auf den Section-Headers, keine generischen Modal-Schatten.
2. **Tabular-nums in Diff-Werten**: jede Number-Cell bekommt `font-variant-numeric: tabular-nums` — Spaltenausrichtung über Diff-Tabellen hinweg.
3. **Counts-Pill mit Mono-Font**: `+2 ~1 −0` Header-Pill in JetBrains-Mono-ähnlicher Mono-Font für scan-fähige Magnitude.
4. **Section-Header in Small-Caps**: `text-transform: uppercase; letter-spacing: 0.14em; font-size: 0.85em` — editoriale Hierarchie ohne Size-Sprünge.
5. **Reason-Chip Cyan-tönig** (`#ecfeff/#67e8f9`): semantisch von der akzent-orange-betonten Value-Spalte getrennt — wo ein Derived-Row durch eine Berechnung entstand, sagt der Chip *warum*, nicht *wie viel*.

---

## File Structure

**Neu:**
- `shared/src/preview.rs` — `SavePreview`, `EntityDiff`, `DerivedRow`, `FieldDiff`, `DiffKind`, `PreviewCounts`, `SavePreviewResult`
- `shared/tests/save_preview_wire.rs` — Roundtrip
- `server/src/preview/mod.rs`
- `server/src/preview/store.rs` — `PreviewStore` (DashMap + TTL)
- `server/src/preview/handler.rs` — Registry (`derive_handler`) + Default-Noop
- `server/src/preview/diff.rs` — `build_diff`, `counts_for`
- `server/tests/preview_flow.rs` — End-to-End
- `server/tests/preview_handlers.rs` — Handler-Registry-Test
- `client/src/components/preview/mod.rs`
- `client/src/components/preview/save_preview_modal.rs`
- `client/src/components/preview/diff_table.rs` — generische Diff-Tabelle (Primary + Derived)

**Modifizieren:**
- `shared/src/lib.rs` — Re-Export Preview-Typen
- `shared/src/settings.rs` — `EntitySettings.preview_on_save: bool`, `derive_handler: Option<String>`
- `server/src/schema.rs` — neue Mutationen
- `server/src/mutation.rs` (oder wo Persistenz heute lebt) — `persist_create_impl`, `persist_update_impl`, `persist_delete_impl` extrahieren, sodass sowohl direkter Save als auch `commitPreview` denselben Code-Pfad rufen
- `client/src/graphql/queries.rs` — `preview_create_entity`, `preview_update_entity`, `preview_delete_entity`, `commit_preview`, `cancel_preview`
- `client/src/routes/editor.rs` (oder `client/src/components/field/editor.rs` — verifizieren) — Branch auf `preview_on_save`
- `client/src/styling/mod.rs` + `inline.rs` — neue DesignSystem-Accessoren
- `client/src/lib.rs` — neues Modul `components::preview`
- `client/locales/de/main.ftl`, `client/locales/en/main.ftl` — `preview-*` keys

---

## Architektur-Details

### `shared::preview` Wire-Typen

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavePreview {
    pub preview_id: String,
    pub primary: EntityDiff,
    #[serde(default)]
    pub derived: BTreeMap<String, Vec<DerivedRow>>,
    #[serde(default)]
    pub warnings: Vec<shared::ValidationMessage>,
    pub counts: PreviewCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityDiff {
    pub entity_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub kind: DiffKind,
    pub fields: Vec<FieldDiff>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiffKind { Create, Update, Delete }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiff {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DerivedRow {
    pub kind: DiffKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub fields: serde_json::Map<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_key: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCounts { pub creates: u32, pub updates: u32, pub deletes: u32 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavePreviewResult {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<SavePreview>,
    pub validation: shared::ValidationResult,
}
```

### Server-Persistenz-Pfad: einheitliches `persist_*`

Heute haben `createEntity`/`updateEntity`/`deleteEntity` jeweils eigenen Save-Code im Mutation-Resolver. Wir extrahieren pro Op einen *internal* Helper:

```rust
async fn persist_create_impl(input: shared::EntityCreate) -> Result<shared::EntityChangeResult, AppError>;
async fn persist_update_impl(input: shared::EntityUpdate) -> Result<shared::EntityChangeResult, AppError>;
async fn persist_delete_impl(input: shared::EntityDelete) -> Result<shared::EntityChangeResult, AppError>;
```

Sowohl der direkte Save-Resolver als auch `commitPreview` rufen denselben Helper. Damit ist garantiert: die Persistenz-Logik läuft auf einer einzigen Code-Bahn (Spec §9 Risk: "Zwei Persistenz-Pfade").

### `PreviewStore`

```rust
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct PreviewSession {
    pub id: String,
    pub entity_type: String,
    pub kind: PreviewKind,
    pub input: serde_json::Value,   // EntityCreate / EntityUpdate / EntityDelete als JSON
    pub diff: shared::SavePreview,
    pub user_id: Option<String>,    // aus AuthSession; null in GraphiQL
    pub expires_at: Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum PreviewKind { Create, Update, Delete }

#[derive(Clone, Default)]
pub struct PreviewStore {
    sessions: Arc<DashMap<String, PreviewSession>>,
}

impl PreviewStore {
    pub fn new() -> Self { Self::default() }
    pub fn insert(&self, s: PreviewSession) { self.sessions.insert(s.id.clone(), s); }
    pub fn take(&self, id: &str) -> Option<PreviewSession> {
        self.sessions.remove(id).map(|(_, v)| v)
    }
    pub fn purge_expired(&self) {
        let now = Instant::now();
        self.sessions.retain(|_, s| s.expires_at > now);
    }
}
```

Background-Tokio-Task läuft alle 60 s und ruft `purge_expired()`.

### Derive-Handler-Registry

```rust
use shared::{DerivedRow, Entity};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub type DeriveFn = fn(entity: &Entity, prev: Option<&Entity>) -> BTreeMap<String, Vec<DerivedRow>>;

static REGISTRY: OnceLock<std::sync::RwLock<std::collections::HashMap<String, DeriveFn>>> = OnceLock::new();

pub fn registry() -> &'static std::sync::RwLock<std::collections::HashMap<String, DeriveFn>> {
    REGISTRY.get_or_init(Default::default)
}

pub fn register(id: &str, f: DeriveFn) {
    registry().write().unwrap().insert(id.to_string(), f);
}

pub fn compute(handler_id: Option<&str>, entity: &Entity, prev: Option<&Entity>) -> BTreeMap<String, Vec<DerivedRow>> {
    let Some(id) = handler_id else { return BTreeMap::new(); };
    let r = registry().read().unwrap();
    match r.get(id).copied() {
        Some(f) => f(entity, prev),
        None => {
            tracing::warn!(handler_id = id, "unknown derive_handler");
            BTreeMap::new()
        }
    }
}

/// Default-Noop für Tests: liefert leere Derived-Rows.
pub fn noop(_e: &Entity, _p: Option<&Entity>) -> BTreeMap<String, Vec<DerivedRow>> {
    BTreeMap::new()
}

/// Echo: liefert eine künstliche Derived-Row für Tests.
pub fn echo(e: &Entity, _p: Option<&Entity>) -> BTreeMap<String, Vec<DerivedRow>> {
    let mut map = BTreeMap::new();
    let mut row_fields = serde_json::Map::new();
    row_fields.insert("source_id".into(), serde_json::json!(e.id.clone()));
    map.insert("EchoChild".into(), vec![DerivedRow {
        kind: shared::DiffKind::Create,
        id: None,
        fields: row_fields,
        reason_key: Some("echo-reason".into()),
    }]);
    map
}
```

### Diff-Builder

`server/src/preview/diff.rs::build_diff(prev: Option<&Entity>, next: Option<&Entity>) -> EntityDiff`. Pro Feld vergleicht `prev.fields[key]` vs `next.fields[key]`; Unterschiede landen in `FieldDiff { old, new }`.

`counts_for(primary: &EntityDiff, derived: &BTreeMap<String, Vec<DerivedRow>>) -> PreviewCounts` zählt Creates/Updates/Deletes.

### `<SavePreviewModal>` Aufbau

- **Header-Bar**: Titel ("Vorschau speichern") + Counts-Pill (`+2 ~1 −0`, Mono-Font). Left-Border 3px solid `#b45309`.
- **Section: Primary** — Header "Eigentliche Änderung" (small-caps). `<DiffTable rows=primary.fields layout=Primary/>`. Old-Wert durchgestrichen + 60% Opacity; New-Wert fett.
- **Section: Derived (pro Entity-Type)** — Header "Abgeleitete Rows · `<entity_type>`". `<DiffTable rows=derived[<type>] layout=Derived/>`. Spalten: alle Keys, plus rechts ein "Grund"-Spalt mit dem Cyan-Chip (`reason_key` lokalisiert).
- **Section: Warnings** — gelbe Boxen mit `ValidationMessage`-Severity ≥ Warning.
- **Footer**: `[Abbrechen]` `[Speichern]`. Cancel → `cancelPreview`; Speichern → `commitPreview`.

### Was NICHT in diesem Plan ist

- Bulk-Preview (Spec §10): `previewBulk(entity_type, payload[])` — eigene Folge-Spec.
- Live-Preview (typing → Live-Diff): Stretch.
- Cross-User-Preview (Snapshot-Persistenz): out of scope.

---

## Tasks

### Task 1: `shared::preview` Wire + Roundtrip

**Files:**
- Neu: `shared/src/preview.rs`
- Modify: `shared/src/lib.rs`
- Neu: `shared/tests/save_preview_wire.rs`

- [ ] **Schritt 1: Failing Test**

`shared/tests/save_preview_wire.rs`:

```rust
use shared::{
    DiffKind, EntityDiff, FieldDiff, PreviewCounts, SavePreview, DerivedRow,
};
use std::collections::BTreeMap;

#[test]
fn primary_diff_roundtrip() {
    let d = EntityDiff {
        entity_type: "Order".into(),
        id: Some("1".into()),
        kind: DiffKind::Update,
        fields: vec![
            FieldDiff { key: "status".into(), old: Some(serde_json::json!("draft")), new: Some(serde_json::json!("paid")) },
        ],
    };
    let json = serde_json::to_string(&d).unwrap();
    let back: EntityDiff = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
}

#[test]
fn diff_kind_camel_case() {
    let json = serde_json::to_string(&DiffKind::Update).unwrap();
    assert_eq!(json, "\"update\"");
}

#[test]
fn save_preview_minimal_roundtrip() {
    let p = SavePreview {
        preview_id: "uuid".into(),
        primary: EntityDiff {
            entity_type: "X".into(), id: None,
            kind: DiffKind::Create, fields: vec![],
        },
        derived: BTreeMap::new(),
        warnings: vec![],
        counts: PreviewCounts { creates: 1, updates: 0, deletes: 0 },
    };
    let json = serde_json::to_string(&p).unwrap();
    let back: SavePreview = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
    assert!(json.contains(r#""previewId":"uuid""#));
}

#[test]
fn derived_row_has_reason_chip() {
    let mut fields = serde_json::Map::new();
    fields.insert("amount".into(), serde_json::json!(19.0));
    let r = DerivedRow {
        kind: DiffKind::Create,
        id: None,
        fields,
        reason_key: Some("vat-split".into()),
    };
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains(r#""reasonKey":"vat-split""#));
}

#[test]
fn field_diff_skips_none() {
    let d = FieldDiff { key: "x".into(), old: None, new: Some(serde_json::json!(1)) };
    let json = serde_json::to_string(&d).unwrap();
    assert!(!json.contains("\"old\""));
    assert!(json.contains(r#""new":1"#));
}
```

- [ ] **Schritt 2: FAIL**

- [ ] **Schritt 3: Modul anlegen**

`shared/src/preview.rs` mit den Typen aus "Architektur-Details" (komplett, inkl. `SavePreviewResult`).

- [ ] **Schritt 4: `lib.rs` exportieren**

```rust
pub mod preview;
pub use preview::{
    DerivedRow, DiffKind, EntityDiff, FieldDiff, PreviewCounts, SavePreview, SavePreviewResult,
};
```

- [ ] **Schritt 5: PASS + Commit**

```
cargo test -p shared --test save_preview_wire
git add shared/src/preview.rs shared/src/lib.rs shared/tests/save_preview_wire.rs
git commit -m "feat(shared): SavePreview + EntityDiff wire types"
```

---

### Task 2: `EntitySettings.preview_on_save` + `derive_handler`

**Files:**
- Modify: `shared/src/settings.rs`
- Modify (Test): `shared/tests/save_preview_wire.rs`

- [ ] **Schritt 1: Tests**

In `shared/tests/save_preview_wire.rs` anhängen:

```rust
use shared::EntitySettings;

#[test]
fn entity_settings_preview_defaults_off() {
    let s = EntitySettings::default();
    assert!(!s.preview_on_save);
    assert!(s.derive_handler.is_none());
}

#[test]
fn entity_settings_preview_camel_case() {
    let mut s = EntitySettings::default();
    s.preview_on_save = true;
    s.derive_handler = Some("vat-split".into());
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains(r#""previewOnSave":true"#));
    assert!(json.contains(r#""deriveHandler":"vat-split""#));
    let back: EntitySettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back.preview_on_save, true);
    assert_eq!(back.derive_handler, Some("vat-split".to_string()));
}
```

- [ ] **Schritt 2: FAIL**

- [ ] **Schritt 3: Felder ergänzen**

In `shared/src/settings.rs` im `EntitySettings`-Struct:

```rust
#[serde(default)]
pub preview_on_save: bool,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub derive_handler: Option<String>,
```

- [ ] **Schritt 4: PASS + Commit**

```
cargo test -p shared --test save_preview_wire
git add shared/src/settings.rs shared/tests/save_preview_wire.rs
git commit -m "feat(shared): EntitySettings.preview_on_save + derive_handler"
```

---

### Task 3: Server `PreviewStore` + TTL-Purge

**Files:**
- Neu: `server/src/preview/mod.rs`
- Neu: `server/src/preview/store.rs`
- Modify: `server/src/lib.rs`

- [ ] **Schritt 1: Modul-Scaffold**

`server/src/preview/mod.rs`:

```rust
pub mod store;
pub mod handler;
pub mod diff;

pub use store::{PreviewStore, PreviewSession, PreviewKind};
```

`server/src/preview/store.rs` mit Code aus "Architektur-Details".

- [ ] **Schritt 2: In `lib.rs` einhängen**

```rust
pub mod preview;
```

- [ ] **Schritt 3: Background-Purge-Task starten**

In `server/src/lib.rs::boot` (oder wo Hintergrund-Tasks gestartet werden) — beim Server-Start einen Tokio-Task spawnen, der alle 60 s `store.purge_expired()` ruft. Den Store-Arc als Tower-Layer-State exposen.

Konkret:

```rust
// in boot() / startup-path:
let preview_store = crate::preview::PreviewStore::new();
{
    let s = preview_store.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop { tick.tick().await; s.purge_expired(); }
    });
}
// preview_store in App-State legen oder als global OnceLock setzen:
crate::preview::store::set_global(preview_store);
```

In `server/src/preview/store.rs` einen globalen Slot ergänzen:

```rust
use std::sync::OnceLock;
static GLOBAL: OnceLock<PreviewStore> = OnceLock::new();
pub fn set_global(s: PreviewStore) { let _ = GLOBAL.set(s); }
pub fn global() -> &'static PreviewStore {
    GLOBAL.get_or_init(PreviewStore::new)
}
```

- [ ] **Schritt 4: Inline-Unit-Test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn dummy_session(id: &str, ttl: Duration) -> PreviewSession {
        PreviewSession {
            id: id.into(), entity_type: "Order".into(),
            kind: PreviewKind::Create,
            input: serde_json::json!({}),
            diff: shared::SavePreview {
                preview_id: id.into(),
                primary: shared::EntityDiff {
                    entity_type: "Order".into(), id: None,
                    kind: shared::DiffKind::Create, fields: vec![],
                },
                derived: Default::default(),
                warnings: vec![],
                counts: shared::PreviewCounts::default(),
            },
            user_id: None,
            expires_at: Instant::now() + ttl,
        }
    }

    #[test]
    fn insert_take() {
        let s = PreviewStore::new();
        s.insert(dummy_session("p1", Duration::from_secs(60)));
        let got = s.take("p1");
        assert!(got.is_some());
        assert!(s.take("p1").is_none());
    }

    #[tokio::test]
    async fn purge_removes_expired() {
        let s = PreviewStore::new();
        s.insert(dummy_session("p1", Duration::from_millis(10)));
        tokio::time::sleep(Duration::from_millis(50)).await;
        s.purge_expired();
        assert!(s.take("p1").is_none());
    }
}
```

- [ ] **Schritt 5: Test laufen + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server preview::store::tests
git add server/src/preview/ server/src/lib.rs
git commit -m "feat(server): PreviewStore with TTL purge"
```

---

### Task 4: `derive_handler`-Registry + Default/Echo

**Files:**
- Neu: `server/src/preview/handler.rs`
- Neu: `server/tests/preview_handlers.rs`

- [ ] **Schritt 1: Handler-Modul**

`server/src/preview/handler.rs` mit dem Code aus "Architektur-Details".

- [ ] **Schritt 2: Echo beim Server-Start registrieren (nur in Tests/Examples)**

In `server/src/lib.rs::boot`:

```rust
#[cfg(any(test, debug_assertions))]
crate::preview::handler::register("echo", crate::preview::handler::echo);
```

- [ ] **Schritt 3: Test schreiben**

`server/tests/preview_handlers.rs`:

```rust
use std::collections::BTreeMap;

#[tokio::test]
#[serial_test::serial]
async fn noop_handler_returns_empty() {
    server::fresh_test_setup().await;
    let entity = shared::Entity { id: "1".into(), fields: BTreeMap::new() };
    let map = server::preview::handler::compute(Some("noop"), &entity, None);
    assert!(map.is_empty());
}

#[tokio::test]
#[serial_test::serial]
async fn echo_handler_returns_one_derived() {
    server::fresh_test_setup().await;
    let entity = shared::Entity { id: "42".into(), fields: BTreeMap::new() };
    let map = server::preview::handler::compute(Some("echo"), &entity, None);
    let rows = map.get("EchoChild").expect("EchoChild");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].reason_key.as_deref(), Some("echo-reason"));
}

#[tokio::test]
#[serial_test::serial]
async fn unknown_handler_returns_empty() {
    server::fresh_test_setup().await;
    let entity = shared::Entity { id: "1".into(), fields: BTreeMap::new() };
    let map = server::preview::handler::compute(Some("does-not-exist"), &entity, None);
    assert!(map.is_empty());
}
```

(Noop ist nicht standardmäßig registriert — fügen wir Schritt-2 noch hinzu: `register("noop", noop)`.)

- [ ] **Schritt 4: Noop registrieren**

In `boot()`:

```rust
crate::preview::handler::register("noop", crate::preview::handler::noop);
```

- [ ] **Schritt 5: Tests laufen + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test preview_handlers
git add server/src/preview/handler.rs server/tests/preview_handlers.rs server/src/lib.rs
git commit -m "feat(server): derive_handler registry + noop/echo defaults"
```

---

### Task 5: Persistenz-Refactor: `persist_*_impl`-Helper

**Files:**
- Modify: `server/src/schema.rs` (oder wo `createEntity`/`updateEntity`/`deleteEntity` Resolver leben)
- Modify: `server/src/mutation.rs` (falls existent)

- [ ] **Schritt 1: Aktuelle Save-Pfade lokalisieren**

```
grep -n "create_entity\|createEntity\|update_entity\|updateEntity\|delete_entity\|deleteEntity" server/src/schema.rs server/src/mutation.rs
```

- [ ] **Schritt 2: Refactor: Pro Op einen internen Helper extrahieren**

Beispiel (`schema.rs`):

```rust
async fn persist_create_impl(input: shared::EntityCreate) -> async_graphql::Result<shared::EntityChangeResult> {
    // ehemaliger Body von create_entity-Resolver
    // …
}

#[Object]
impl MutationRoot {
    async fn create_entity(&self, _ctx: &Context<'_>, input: async_graphql::Json<shared::EntityCreate>)
        -> async_graphql::Result<async_graphql::Json<shared::EntityChangeResult>>
    {
        let r = persist_create_impl(input.0).await?;
        Ok(async_graphql::Json(r))
    }
    // dito update_entity, delete_entity
}
```

Wichtig: `persist_*_impl` darf nicht selbst nochmal `Json` wrappen; das macht der äußere Resolver.

- [ ] **Schritt 3: Bestehende Tests laufen lassen**

```
CARGO_TARGET_DIR=target-test cargo test -p server
```

Erwartung: keine Regressionen — der Refactor ist nur Code-Bewegung, kein Verhalten geändert.

- [ ] **Schritt 4: Commit**

```
git add server/src/schema.rs server/src/mutation.rs
git commit -m "refactor(server): extract persist_create/update/delete_impl helpers"
```

---

### Task 6: GraphQL-Mutationen `previewCreate/Update/Delete/commitPreview/cancelPreview`

**Files:**
- Modify: `server/src/schema.rs`
- Neu: `server/src/preview/diff.rs` (Diff-Builder)

- [ ] **Schritt 1: Diff-Builder schreiben**

`server/src/preview/diff.rs`:

```rust
use shared::{DerivedRow, DiffKind, Entity, EntityDiff, FieldDiff, PreviewCounts, SavePreview};
use std::collections::BTreeMap;

pub fn build_entity_diff(entity_type: &str, kind: DiffKind, prev: Option<&Entity>, next: Option<&Entity>) -> EntityDiff {
    let mut fields = Vec::new();
    match (prev, next) {
        (None, Some(n)) => {
            for (k, v) in &n.fields {
                fields.push(FieldDiff { key: k.clone(), old: None, new: Some(v.clone()) });
            }
            EntityDiff { entity_type: entity_type.into(), id: Some(n.id.clone()), kind, fields }
        }
        (Some(p), None) => {
            for (k, v) in &p.fields {
                fields.push(FieldDiff { key: k.clone(), old: Some(v.clone()), new: None });
            }
            EntityDiff { entity_type: entity_type.into(), id: Some(p.id.clone()), kind, fields }
        }
        (Some(p), Some(n)) => {
            let mut all_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            all_keys.extend(p.fields.keys().cloned());
            all_keys.extend(n.fields.keys().cloned());
            for k in all_keys {
                let old = p.fields.get(&k).cloned();
                let new = n.fields.get(&k).cloned();
                if old != new {
                    fields.push(FieldDiff { key: k, old, new });
                }
            }
            EntityDiff { entity_type: entity_type.into(), id: Some(n.id.clone()), kind, fields }
        }
        (None, None) => unreachable!("build_entity_diff needs at least one entity"),
    }
}

pub fn counts_for(primary: &EntityDiff, derived: &BTreeMap<String, Vec<DerivedRow>>) -> PreviewCounts {
    let mut c = PreviewCounts::default();
    match primary.kind {
        DiffKind::Create => c.creates += 1,
        DiffKind::Update => c.updates += 1,
        DiffKind::Delete => c.deletes += 1,
    }
    for rows in derived.values() {
        for row in rows {
            match row.kind {
                DiffKind::Create => c.creates += 1,
                DiffKind::Update => c.updates += 1,
                DiffKind::Delete => c.deletes += 1,
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use serde_json::json;

    fn entity(id: &str, fields: Vec<(&str, serde_json::Value)>) -> Entity {
        let map: BTreeMap<String, serde_json::Value> = fields.into_iter().map(|(k, v)| (k.into(), v)).collect();
        Entity { id: id.into(), fields: map }
    }

    #[test]
    fn create_diff_includes_all_new_fields() {
        let next = entity("1", vec![("a", json!(1)), ("b", json!("x"))]);
        let d = build_entity_diff("X", DiffKind::Create, None, Some(&next));
        assert_eq!(d.fields.len(), 2);
    }

    #[test]
    fn update_diff_only_changed_fields() {
        let prev = entity("1", vec![("a", json!(1)), ("b", json!("x"))]);
        let next = entity("1", vec![("a", json!(2)), ("b", json!("x"))]);
        let d = build_entity_diff("X", DiffKind::Update, Some(&prev), Some(&next));
        assert_eq!(d.fields.len(), 1);
        assert_eq!(d.fields[0].key, "a");
    }

    #[test]
    fn counts_for_primary_plus_derived() {
        let primary = EntityDiff { entity_type: "X".into(), id: None, kind: DiffKind::Update, fields: vec![] };
        let mut derived: BTreeMap<String, Vec<DerivedRow>> = BTreeMap::new();
        derived.insert("Y".into(), vec![DerivedRow {
            kind: DiffKind::Create, id: None,
            fields: serde_json::Map::new(), reason_key: None,
        }]);
        let c = counts_for(&primary, &derived);
        assert_eq!(c, PreviewCounts { creates: 1, updates: 1, deletes: 0 });
    }
}
```

- [ ] **Schritt 2: Mutationen ergänzen**

In `server/src/schema.rs::MutationRoot`:

```rust
async fn preview_create_entity(
    &self, ctx: &Context<'_>,
    input: async_graphql::Json<shared::EntityCreate>,
) -> async_graphql::Result<async_graphql::Json<shared::SavePreviewResult>> {
    use shared::DiffKind;
    let entity_type = input.0.entity_type.clone();
    let settings = crate::data::settings_for(&entity_type)?;

    // Validate (existing validation path) — wenn fail, return Result mit ok=false + validation
    let validation = crate::data::validate_create(&input.0).await?;
    if validation.has_errors() {
        return Ok(async_graphql::Json(shared::SavePreviewResult {
            ok: false, preview: None, validation,
        }));
    }

    // Synthetisches "next"-Entity bauen (ohne tatsächlich zu speichern)
    let synthesized = crate::data::synthesize_create_entity(&input.0)?;
    let primary = crate::preview::diff::build_entity_diff(
        &entity_type, DiffKind::Create, None, Some(&synthesized),
    );
    let derived = crate::preview::handler::compute(
        settings.derive_handler.as_deref(),
        &synthesized,
        None,
    );
    let counts = crate::preview::diff::counts_for(&primary, &derived);

    let preview_id = uuid::Uuid::new_v4().to_string();
    let preview = shared::SavePreview {
        preview_id: preview_id.clone(),
        primary, derived, warnings: validation.messages.clone(), counts,
    };

    let user_id = crate::auth::current_user_id(ctx).ok();
    crate::preview::store::global().insert(crate::preview::PreviewSession {
        id: preview_id.clone(),
        entity_type,
        kind: crate::preview::PreviewKind::Create,
        input: serde_json::to_value(&input.0)?,
        diff: preview.clone(),
        user_id,
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(5 * 60),
    });

    Ok(async_graphql::Json(shared::SavePreviewResult {
        ok: true, preview: Some(preview), validation,
    }))
}

async fn preview_update_entity(
    &self, ctx: &Context<'_>,
    input: async_graphql::Json<shared::EntityUpdate>,
) -> async_graphql::Result<async_graphql::Json<shared::SavePreviewResult>> {
    // analog: prev aus DB lesen, next = apply(prev, input.fields), Diff bauen
    todo!("analog zu preview_create_entity, mit prev-Lookup + Apply-Diff")
}

async fn preview_delete_entity(
    &self, ctx: &Context<'_>,
    input: async_graphql::Json<shared::EntityDelete>,
) -> async_graphql::Result<async_graphql::Json<shared::SavePreviewResult>> {
    // prev aus DB; next = None; Kind=Delete
    todo!("analog")
}

async fn commit_preview(
    &self, ctx: &Context<'_>,
    preview_id: String,
) -> async_graphql::Result<async_graphql::Json<shared::EntityChangeResult>> {
    let session = crate::preview::store::global().take(&preview_id)
        .ok_or_else(|| async_graphql::Error::new("preview expired or unknown"))?;

    // Permission-Check: user_id muss == current_user_id (oder beide None)
    let me = crate::auth::current_user_id(ctx).ok();
    if session.user_id != me {
        return Err(async_graphql::Error::new("not your preview"));
    }

    let result = match session.kind {
        crate::preview::PreviewKind::Create => {
            let input: shared::EntityCreate = serde_json::from_value(session.input)?;
            persist_create_impl(input).await?
        }
        crate::preview::PreviewKind::Update => {
            let input: shared::EntityUpdate = serde_json::from_value(session.input)?;
            persist_update_impl(input).await?
        }
        crate::preview::PreviewKind::Delete => {
            let input: shared::EntityDelete = serde_json::from_value(session.input)?;
            persist_delete_impl(input).await?
        }
    };
    Ok(async_graphql::Json(result))
}

async fn cancel_preview(
    &self, _ctx: &Context<'_>,
    preview_id: String,
) -> async_graphql::Result<bool> {
    crate::preview::store::global().take(&preview_id);
    Ok(true)
}
```

- [ ] **Schritt 3: Update + Delete-Preview ausimplementieren**

```rust
async fn preview_update_entity(
    &self, ctx: &Context<'_>,
    input: async_graphql::Json<shared::EntityUpdate>,
) -> async_graphql::Result<async_graphql::Json<shared::SavePreviewResult>> {
    use shared::DiffKind;
    let entity_type = input.0.entity_type.clone();
    let settings = crate::data::settings_for(&entity_type)?;

    let validation = crate::data::validate_update(&input.0).await?;
    if validation.has_errors() {
        return Ok(async_graphql::Json(shared::SavePreviewResult {
            ok: false, preview: None, validation,
        }));
    }

    let prev = crate::data::entity_by_id(&entity_type, &input.0.id).await?
        .ok_or_else(|| async_graphql::Error::new("entity not found"))?;
    let next = crate::data::synthesize_update_entity(&prev, &input.0)?;

    let primary = crate::preview::diff::build_entity_diff(
        &entity_type, DiffKind::Update, Some(&prev), Some(&next),
    );
    let derived = crate::preview::handler::compute(
        settings.derive_handler.as_deref(),
        &next, Some(&prev),
    );
    let counts = crate::preview::diff::counts_for(&primary, &derived);

    let preview_id = uuid::Uuid::new_v4().to_string();
    let preview = shared::SavePreview {
        preview_id: preview_id.clone(),
        primary, derived, warnings: validation.messages.clone(), counts,
    };
    let user_id = crate::auth::current_user_id(ctx).ok();
    crate::preview::store::global().insert(crate::preview::PreviewSession {
        id: preview_id.clone(), entity_type,
        kind: crate::preview::PreviewKind::Update,
        input: serde_json::to_value(&input.0)?,
        diff: preview.clone(), user_id,
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(5 * 60),
    });
    Ok(async_graphql::Json(shared::SavePreviewResult {
        ok: true, preview: Some(preview), validation,
    }))
}

async fn preview_delete_entity(
    &self, ctx: &Context<'_>,
    input: async_graphql::Json<shared::EntityDelete>,
) -> async_graphql::Result<async_graphql::Json<shared::SavePreviewResult>> {
    use shared::DiffKind;
    let entity_type = input.0.entity_type.clone();
    let settings = crate::data::settings_for(&entity_type)?;
    let validation = shared::ValidationResult::default(); // Delete hat heute keine Field-Validation
    let prev = crate::data::entity_by_id(&entity_type, &input.0.id).await?
        .ok_or_else(|| async_graphql::Error::new("entity not found"))?;
    let primary = crate::preview::diff::build_entity_diff(
        &entity_type, DiffKind::Delete, Some(&prev), None,
    );
    let derived = crate::preview::handler::compute(
        settings.derive_handler.as_deref(),
        &prev, None,
    );
    let counts = crate::preview::diff::counts_for(&primary, &derived);

    let preview_id = uuid::Uuid::new_v4().to_string();
    let preview = shared::SavePreview {
        preview_id: preview_id.clone(),
        primary, derived, warnings: vec![], counts,
    };
    let user_id = crate::auth::current_user_id(ctx).ok();
    crate::preview::store::global().insert(crate::preview::PreviewSession {
        id: preview_id.clone(), entity_type,
        kind: crate::preview::PreviewKind::Delete,
        input: serde_json::to_value(&input.0)?,
        diff: preview.clone(), user_id,
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(5 * 60),
    });
    Ok(async_graphql::Json(shared::SavePreviewResult {
        ok: true, preview: Some(preview), validation,
    }))
}
```

Hinweis: `synthesize_create_entity`, `synthesize_update_entity`, `validate_create`, `validate_update`, `entity_by_id`, `current_user_id` müssen in `data.rs`/`auth.rs` existieren oder als kleine Helper neu angelegt werden. Heutige Save-Pfade haben äquivalente Logik — extrahieren statt neu schreiben.

- [ ] **Schritt 4: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 5: Commit**

```
git add server/src/preview/diff.rs server/src/schema.rs
git commit -m "feat(server): preview_*/commit_preview/cancel_preview mutations"
```

---

### Task 7: End-to-End-Test `server/tests/preview_flow.rs`

**Files:**
- Neu: `server/tests/preview_flow.rs`

- [ ] **Schritt 1: Tests schreiben**

```rust
use serde_json::json;

#[tokio::test]
#[serial_test::serial]
async fn preview_then_commit_persists() {
    server::fresh_test_setup().await;

    let create = json!({
        "entityType": "Order",
        "fields": {
            "customer_name": "Alice",
            "status": "draft",
            "total": 100.0
        }
    });
    let body = json!({
        "query": "mutation Pv($i: JSON!) { previewCreateEntity(input: $i) }",
        "variables": { "i": create.clone() }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let preview = &resp["data"]["previewCreateEntity"]["preview"];
    let preview_id = preview["previewId"].as_str().unwrap().to_string();
    let counts = &preview["counts"];
    assert_eq!(counts["creates"], 1);

    // Commit
    let body = json!({
        "query": "mutation C($id: String!) { commitPreview(previewId: $id) }",
        "variables": { "id": preview_id.clone() }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let result = &resp["data"]["commitPreview"];
    assert_eq!(result["ok"], true);
}

#[tokio::test]
#[serial_test::serial]
async fn commit_with_expired_preview_errors() {
    server::fresh_test_setup().await;
    let body = json!({
        "query": "mutation C($id: String!) { commitPreview(previewId: $id) }",
        "variables": { "id": "nonexistent" }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    assert!(resp["errors"].is_array());
}

#[tokio::test]
#[serial_test::serial]
async fn cancel_preview_removes_it() {
    server::fresh_test_setup().await;

    let create = json!({
        "entityType": "Order",
        "fields": {"customer_name": "X", "status": "draft", "total": 0.0}
    });
    let body = json!({
        "query": "mutation Pv($i: JSON!) { previewCreateEntity(input: $i) }",
        "variables": { "i": create }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let id = resp["data"]["previewCreateEntity"]["preview"]["previewId"].as_str().unwrap().to_string();

    let body = json!({
        "query": "mutation X($id: String!) { cancelPreview(previewId: $id) }",
        "variables": { "id": id.clone() }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    assert_eq!(resp["data"]["cancelPreview"], true);

    // Folge-Commit muss scheitern
    let body = json!({
        "query": "mutation C($id: String!) { commitPreview(previewId: $id) }",
        "variables": { "id": id }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    assert!(resp["errors"].is_array());
}

#[tokio::test]
#[serial_test::serial]
async fn preview_with_echo_handler_produces_derived() {
    server::fresh_test_setup().await;

    // Setup: setze derive_handler="echo" für Order via settings
    server::test_helpers::set_entity_setting_derive_handler("Order", "echo").await;

    let create = json!({
        "entityType": "Order",
        "fields": {"customer_name": "X", "status": "draft", "total": 0.0}
    });
    let body = json!({
        "query": "mutation Pv($i: JSON!) { previewCreateEntity(input: $i) }",
        "variables": { "i": create }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let preview = &resp["data"]["previewCreateEntity"]["preview"];
    let derived = preview["derived"]["EchoChild"].as_array().unwrap();
    assert_eq!(derived.len(), 1);
    assert_eq!(derived[0]["reasonKey"], "echo-reason");
}
```

- [ ] **Schritt 2: Helper bereitstellen**

`set_entity_setting_derive_handler` in `server::test_helpers` — überschreibt das in-memory geladene `ExampleSet` für die Order-Entity (oder direkt das `EntitySettings`-Map in `example::current()`).

- [ ] **Schritt 3: Tests laufen + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test preview_flow
git add server/tests/preview_flow.rs server/src/lib.rs
git commit -m "test(server): preview flow end-to-end (preview+commit+cancel+derive)"
```

---

### Task 8: Client GraphQL-Helpers

**Files:**
- Modify: `client/src/graphql/queries.rs`

- [ ] **Schritt 1: Funktionen**

```rust
pub async fn preview_create_entity(input: shared::EntityCreate) -> Result<shared::SavePreviewResult, GraphqlError> {
    #[derive(serde::Deserialize)]
    struct Resp { #[serde(rename = "previewCreateEntity")] r: shared::SavePreviewResult }
    let vars = serde_json::json!({"i": input});
    let r: Resp = crate::graphql::execute(
        "mutation P($i: JSON!) { previewCreateEntity(input: $i) }", &vars,
    ).await?;
    Ok(r.r)
}

pub async fn preview_update_entity(input: shared::EntityUpdate) -> Result<shared::SavePreviewResult, GraphqlError> {
    #[derive(serde::Deserialize)]
    struct Resp { #[serde(rename = "previewUpdateEntity")] r: shared::SavePreviewResult }
    let vars = serde_json::json!({"i": input});
    let r: Resp = crate::graphql::execute(
        "mutation P($i: JSON!) { previewUpdateEntity(input: $i) }", &vars,
    ).await?;
    Ok(r.r)
}

pub async fn preview_delete_entity(input: shared::EntityDelete) -> Result<shared::SavePreviewResult, GraphqlError> {
    #[derive(serde::Deserialize)]
    struct Resp { #[serde(rename = "previewDeleteEntity")] r: shared::SavePreviewResult }
    let vars = serde_json::json!({"i": input});
    let r: Resp = crate::graphql::execute(
        "mutation P($i: JSON!) { previewDeleteEntity(input: $i) }", &vars,
    ).await?;
    Ok(r.r)
}

pub async fn commit_preview(preview_id: &str) -> Result<shared::EntityChangeResult, GraphqlError> {
    #[derive(serde::Deserialize)]
    struct Resp { #[serde(rename = "commitPreview")] r: shared::EntityChangeResult }
    let vars = serde_json::json!({"id": preview_id});
    let r: Resp = crate::graphql::execute(
        "mutation C($id: String!) { commitPreview(previewId: $id) }", &vars,
    ).await?;
    Ok(r.r)
}

pub async fn cancel_preview(preview_id: &str) -> Result<bool, GraphqlError> {
    #[derive(serde::Deserialize)]
    struct Resp { #[serde(rename = "cancelPreview")] r: bool }
    let vars = serde_json::json!({"id": preview_id});
    let r: Resp = crate::graphql::execute(
        "mutation X($id: String!) { cancelPreview(previewId: $id) }", &vars,
    ).await?;
    Ok(r.r)
}
```

- [ ] **Schritt 2: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/graphql/queries.rs
git commit -m "feat(client): preview/commit/cancel GraphQL helpers"
```

---

### Task 9: DesignSystem-Accessoren für Modal-Ästhetik

**Files:**
- Modify: `client/src/styling/mod.rs` (Trait)
- Modify: `client/src/styling/inline.rs` (InlineDesign-Impl)

- [ ] **Schritt 1: Trait erweitern**

```rust
pub trait DesignSystem: 'static {
    // … bestehende Methoden …
    fn modal_overlay(&self) -> Style;
    fn modal_panel(&self) -> Style;
    fn counts_pill(&self) -> Style;
    fn section_header(&self) -> Style;
    fn diff_row_old(&self) -> Style;
    fn diff_row_new(&self) -> Style;
    fn derived_table(&self) -> Style;
    fn reason_chip(&self) -> Style;
    fn warning_box(&self) -> Style;
    fn numeric_cell(&self) -> Style;
}
```

- [ ] **Schritt 2: InlineDesign-Impl**

```rust
impl DesignSystem for InlineDesign {
    fn modal_overlay(&self) -> Style {
        Style { inline: "position:fixed;inset:0;background:rgba(20,15,10,0.45);display:flex;align-items:center;justify-content:center;z-index:1000;".into(), class: String::new() }
    }
    fn modal_panel(&self) -> Style {
        Style {
            inline: "background:#fbf7f0;border-left:3px solid #b45309;font-family:Georgia,'Times New Roman',serif;max-width:760px;width:90%;max-height:80vh;overflow:auto;box-shadow:0 12px 32px rgba(0,0,0,0.18);padding:24px;".into(),
            class: String::new(),
        }
    }
    fn counts_pill(&self) -> Style {
        Style {
            inline: "font-family:'JetBrains Mono','Cascadia Mono',monospace;background:#1f2937;color:#fafaf9;padding:2px 10px;border-radius:999px;font-size:13px;letter-spacing:0.08em;".into(),
            class: String::new(),
        }
    }
    fn section_header(&self) -> Style {
        Style {
            inline: "text-transform:uppercase;letter-spacing:0.14em;font-size:0.85em;color:#78350f;margin:16px 0 4px 0;border-bottom:1px solid #d6c7a8;padding-bottom:2px;".into(),
            class: String::new(),
        }
    }
    fn diff_row_old(&self) -> Style {
        Style { inline: "text-decoration:line-through;opacity:0.6;font-variant-numeric:tabular-nums;".into(), class: String::new() }
    }
    fn diff_row_new(&self) -> Style {
        Style { inline: "font-weight:600;color:#7c2d12;font-variant-numeric:tabular-nums;".into(), class: String::new() }
    }
    fn derived_table(&self) -> Style {
        Style { inline: "width:100%;border-collapse:collapse;font-size:13px;".into(), class: String::new() }
    }
    fn reason_chip(&self) -> Style {
        Style { inline: "background:#ecfeff;color:#0e7490;padding:2px 8px;border-radius:4px;font-size:11px;letter-spacing:0.04em;".into(), class: String::new() }
    }
    fn warning_box(&self) -> Style {
        Style { inline: "background:#fef9c3;border-left:3px solid #ca8a04;padding:8px 12px;margin:8px 0;font-size:13px;".into(), class: String::new() }
    }
    fn numeric_cell(&self) -> Style {
        Style { inline: "font-variant-numeric:tabular-nums;text-align:right;".into(), class: String::new() }
    }
}
```

- [ ] **Schritt 3: Compile-Check + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/styling/
git commit -m "feat(client): DesignSystem accessors for SavePreviewModal"
```

---

### Task 10: `<DiffTable>` Komponente

**Files:**
- Neu: `client/src/components/preview/diff_table.rs`
- Neu: `client/src/components/preview/mod.rs`

- [ ] **Schritt 1: Komponente**

`client/src/components/preview/mod.rs`:

```rust
pub mod diff_table;
pub mod save_preview_modal;

pub use save_preview_modal::SavePreviewModal;
```

`client/src/components/preview/diff_table.rs`:

```rust
use leptos::prelude::*;
use shared::{DerivedRow, FieldDiff};
use crate::styling::use_design;
use crate::i18n::t;

#[component]
pub fn PrimaryDiffTable(fields: Vec<FieldDiff>) -> impl IntoView {
    let design = use_design();
    view! {
        <table style=design.derived_table().inline.clone()>
            <thead>
                <tr>
                    <th>{ t("diff-field") }</th>
                    <th>{ t("diff-old") }</th>
                    <th>{ t("diff-new") }</th>
                </tr>
            </thead>
            <tbody>
                {fields.into_iter().map(|f| {
                    let old_s = render_value(&f.old);
                    let new_s = render_value(&f.new);
                    let is_numeric = is_number(&f.old) || is_number(&f.new);
                    let num_style = if is_numeric { design.numeric_cell().inline.clone() } else { String::new() };
                    view! {
                        <tr>
                            <td>{f.key.clone()}</td>
                            <td style=format!("{}{}", design.diff_row_old().inline, num_style)>{old_s}</td>
                            <td style=format!("{}{}", design.diff_row_new().inline, num_style)>{new_s}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
pub fn DerivedDiffTable(rows: Vec<DerivedRow>) -> impl IntoView {
    let design = use_design();
    if rows.is_empty() {
        return view! { <p>{ t("diff-no-derived") }</p> }.into_any();
    }

    // Spalten-Keys aus erster Row
    let columns: Vec<String> = rows[0].fields.keys().cloned().collect();
    let has_reason = rows.iter().any(|r| r.reason_key.is_some());

    view! {
        <table style=design.derived_table().inline.clone()>
            <thead>
                <tr>
                    {columns.iter().map(|c| view! { <th>{c.clone()}</th> }).collect_view()}
                    { has_reason.then(|| view! { <th>{ t("diff-reason") }</th> }) }
                </tr>
            </thead>
            <tbody>
                {rows.into_iter().map(|r| {
                    let cols = columns.clone();
                    view! {
                        <tr>
                            {cols.iter().map(|c| {
                                let v = r.fields.get(c).cloned().unwrap_or(serde_json::Value::Null);
                                let is_num = matches!(v, serde_json::Value::Number(_));
                                let style = if is_num { design.numeric_cell().inline.clone() } else { String::new() };
                                view! { <td style=style>{render_value_owned(&v)}</td> }
                            }).collect_view()}
                            { r.reason_key.as_ref().map(|rk| view! {
                                <td><span style=design.reason_chip().inline.clone()>{ t(rk) }</span></td>
                            }) }
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }.into_any()
}

fn render_value(v: &Option<serde_json::Value>) -> String {
    v.as_ref().map(render_value_owned).unwrap_or_else(|| "—".into())
}

fn render_value_owned(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "—".into(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn is_number(v: &Option<serde_json::Value>) -> bool {
    matches!(v, Some(serde_json::Value::Number(_)))
}
```

- [ ] **Schritt 2: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/preview/
git commit -m "feat(client): PrimaryDiffTable + DerivedDiffTable components"
```

---

### Task 11: `<SavePreviewModal>` Komponente

**Files:**
- Neu: `client/src/components/preview/save_preview_modal.rs`
- Modify: `client/src/lib.rs`
- Modify: `client/locales/`

- [ ] **Schritt 1: Modal**

```rust
use leptos::prelude::*;
use shared::SavePreview;
use crate::components::preview::diff_table::{PrimaryDiffTable, DerivedDiffTable};
use crate::graphql::queries;
use crate::styling::use_design;
use crate::i18n::t;

#[component]
pub fn SavePreviewModal(
    preview: SavePreview,
    on_committed: Callback<shared::EntityChangeResult>,
    on_cancelled: Callback<()>,
) -> impl IntoView {
    let design = use_design();
    let preview_clone_for_commit = preview.clone();
    let preview_clone_for_cancel = preview.clone();
    let preview_for_render = preview.clone();
    let (busy, set_busy) = create_signal(false);
    let (error, set_error) = create_signal::<Option<String>>(None);

    let on_commit_click = move |_| {
        let id = preview_clone_for_commit.preview_id.clone();
        let on_committed = on_committed.clone();
        let set_busy = set_busy.clone();
        let set_error = set_error.clone();
        set_busy.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            match queries::commit_preview(&id).await {
                Ok(r) => { on_committed.call(r); }
                Err(e) => set_error.set(Some(format!("{e:?}"))),
            }
            set_busy.set(false);
        });
    };

    let on_cancel_click = move |_| {
        let id = preview_clone_for_cancel.preview_id.clone();
        let on_cancelled = on_cancelled.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = queries::cancel_preview(&id).await;
            on_cancelled.call(());
        });
    };

    let counts_text = format!(
        "+{} ~{} −{}",
        preview_for_render.counts.creates,
        preview_for_render.counts.updates,
        preview_for_render.counts.deletes,
    );

    view! {
        <div style=design.modal_overlay().inline.clone()>
            <div style=design.modal_panel().inline.clone()>
                <header style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px;">
                    <h2 style="margin:0;font-size:1.2em;">{ t("preview-title") }</h2>
                    <span style=design.counts_pill().inline.clone()>{counts_text}</span>
                </header>

                <section>
                    <h3 style=design.section_header().inline.clone()>{ t("preview-primary") }</h3>
                    <PrimaryDiffTable fields=preview_for_render.primary.fields.clone()/>
                </section>

                {preview_for_render.derived.iter().map(|(entity_type, rows)| {
                    let label = format!("{} · {}", t("preview-derived"), entity_type);
                    let rows = rows.clone();
                    view! {
                        <section>
                            <h3 style=design.section_header().inline.clone()>{label}</h3>
                            <DerivedDiffTable rows=rows/>
                        </section>
                    }
                }).collect_view()}

                {(!preview_for_render.warnings.is_empty()).then(|| view! {
                    <section>
                        <h3 style=design.section_header().inline.clone()>{ t("preview-warnings") }</h3>
                        {preview_for_render.warnings.iter().map(|m| view! {
                            <div style=design.warning_box().inline.clone()>{m.message.clone()}</div>
                        }).collect_view()}
                    </section>
                })}

                {move || error.get().map(|e| view! {
                    <div style=design.warning_box().inline.clone()>{e}</div>
                })}

                <footer style="display:flex;gap:8px;justify-content:flex-end;margin-top:16px;">
                    <button on:click=on_cancel_click disabled=move || busy.get()>{ t("preview-cancel") }</button>
                    <button on:click=on_commit_click disabled=move || busy.get()>{ t("preview-commit") }</button>
                </footer>
            </div>
        </div>
    }
}
```

- [ ] **Schritt 2: Modul registrieren**

In `client/src/lib.rs`:

```rust
pub mod components { pub mod preview; /* ... */ }
```

- [ ] **Schritt 3: i18n**

`client/locales/de/main.ftl`:
```
preview-title = Vorschau speichern
preview-primary = Eigentliche Änderung
preview-derived = Abgeleitete Rows
preview-warnings = Hinweise
preview-cancel = Abbrechen
preview-commit = Speichern
diff-field = Feld
diff-old = Alt
diff-new = Neu
diff-reason = Grund
diff-no-derived = Keine abgeleiteten Rows.
echo-reason = Echo-Demo
```

`client/locales/en/main.ftl`:
```
preview-title = Save preview
preview-primary = Primary change
preview-derived = Derived rows
preview-warnings = Notices
preview-cancel = Cancel
preview-commit = Save
diff-field = Field
diff-old = Old
diff-new = New
diff-reason = Reason
diff-no-derived = No derived rows.
echo-reason = Echo demo
```

- [ ] **Schritt 4: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/components/preview/save_preview_modal.rs client/src/lib.rs client/locales/
git commit -m "feat(client): SavePreviewModal with ledger-document aesthetic"
```

---

### Task 12: Editor-Save-Pfad Branch auf `preview_on_save`

**Files:**
- Modify: `client/src/routes/editor.rs` (oder wo der Editor-Save-Pfad heute lebt — Spec sagt `client/src/components/field/mod.rs`, real-Pfad mit grep verifizieren)

- [ ] **Schritt 1: Save-Stelle finden**

```
grep -rn "on_save\|mutate.*Entity\|fetch_entity_create\|fetch_entity_update" client/src/
```

- [ ] **Schritt 2: Branch implementieren**

Im Save-Handler:

```rust
let settings = use_settings_for(&entity_type);
let preview_on_save = settings.read().preview_on_save;

if preview_on_save {
    let preview_result = match operation {
        Op::Create(input) => queries::preview_create_entity(input).await?,
        Op::Update(input) => queries::preview_update_entity(input).await?,
        Op::Delete(input) => queries::preview_delete_entity(input).await?,
    };
    if !preview_result.ok {
        // Validation-Errors anzeigen wie heute
        show_validation_errors(preview_result.validation);
        return;
    }
    if let Some(preview) = preview_result.preview {
        set_active_preview.set(Some(preview));
        // Modal wird über separates Signal in der Route gemountet
    }
} else {
    // bisheriger direkter Save-Pfad
    match operation {
        Op::Create(i) => queries::create_entity(i).await?,
        // …
    }
}
```

- [ ] **Schritt 3: Modal mounten**

In `EntityListPage` (oder dem Route-View, das den Editor öffnet):

```rust
{move || active_preview.get().map(|preview| view! {
    <SavePreviewModal
        preview=preview
        on_committed=Callback::new(move |_result| {
            set_active_preview.set(None);
            reload_table_signal.set(reload_table_signal.get() + 1);
        })
        on_cancelled=Callback::new(move |_| set_active_preview.set(None))
    />
})}
```

- [ ] **Schritt 4: Compile + Commit**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
git add client/src/
git commit -m "feat(client): editor branches to SavePreviewModal when preview_on_save=true"
```

---

### Task 13: Concurrent-Commit-Race-Test + Manuelle UI-Verifikation

**Files:**
- Modify: `server/tests/preview_flow.rs`

- [ ] **Schritt 1: Race-Test**

In `preview_flow.rs` ergänzen:

```rust
#[tokio::test]
#[serial_test::serial]
async fn commit_after_other_write_returns_warning_or_error() {
    server::fresh_test_setup().await;
    // create Order, get preview for update
    let order_id = server::test_helpers::create_order_and_get_id("Alice", "draft", 100.0).await;
    let update = json!({
        "entityType": "Order", "id": order_id.clone(),
        "fields": {"status": "paid"},
        "expectedHash": null
    });
    let body = json!({
        "query": "mutation P($i: JSON!) { previewUpdateEntity(input: $i) }",
        "variables": { "i": update }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    let preview_id = resp["data"]["previewUpdateEntity"]["preview"]["previewId"].as_str().unwrap().to_string();

    // Zwischenzeitlich von "außen" anderen Update setzen
    server::test_helpers::direct_update_order_status(&order_id, "cancelled").await;

    // Commit — re-validation soll Konflikt erkennen
    let body = json!({
        "query": "mutation C($id: String!) { commitPreview(previewId: $id) }",
        "variables": { "id": preview_id }
    });
    let resp: serde_json::Value = server::test_helpers::run_graphql(&body).await;
    // Erwartung: entweder ok=false in EntityChangeResult oder GraphQL-Error
    let ok = resp["data"]["commitPreview"]["ok"].as_bool();
    let has_errors = resp["errors"].is_array();
    assert!(ok == Some(false) || has_errors, "expected conflict response: {resp}");
}
```

- [ ] **Schritt 2: Helper bereitstellen**

`create_order_and_get_id`, `direct_update_order_status` in `test_helpers`.

- [ ] **Schritt 3: Test + Commit**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test preview_flow commit_after_other_write
git add server/tests/preview_flow.rs server/src/lib.rs
git commit -m "test(server): preview commit detects concurrent conflict"
```

- [ ] **Schritt 4: Manuelle UI-Verifikation**

```
cargo run -p server -- --data-dir ./examples/shop &
cd client && trunk serve
```

In `examples/shop/entities/order/settings.toml` `previewOnSave = true` und (optional) `deriveHandler = "echo"`.

Im Browser: Order anlegen oder bearbeiten → SavePreviewModal öffnet sich mit Diff-Tabelle, "echo"-Derived-Row. "Speichern" → Modal schließt, Tabelle reloaded. "Abbrechen" → Modal schließt, kein Save.

---

### Task 14: Workspace-Cleanup + Self-Review

- [ ] **Schritt 1: Clippy + fmt + Tests**

```
CARGO_TARGET_DIR=target-test cargo clippy --workspace --no-deps -- -A clippy::doc_overindented_list_items -A clippy::too_many_arguments
cargo fmt
CARGO_TARGET_DIR=target-test cargo test --workspace
```

- [ ] **Schritt 2: Self-Review**

- Spec §3.1 PreviewSession Server-State → Task 3 ✓
- Spec §3.2 SavePreview Wire → Task 1 ✓
- Spec §3.3 GraphQL Mutations → Task 6 ✓
- Spec §3.4 derive_handler → Task 4 ✓
- Spec §3.5 Modal → Tasks 9-12 ✓
- Spec §3.6 EntitySettings → Task 2 ✓
- Spec §4 Daten-Flow → Tasks 6+12 ✓
- Spec §5 Edge-Cases: expired (Task 7), Validation-Fail (Task 6 ok=false), Race (Task 13), Multi-Tab (separate PreviewIds — Task 6) ✓
- Spec §7 Tests → Task 1 (wire), Task 7 (e2e), Task 4 (handlers), Task 13 (race) ✓
- Spec §8 Backwards-Compat → preview_on_save default false (Task 2) ✓
- Spec §9 Risks: Welt-Verändernde-Preview (synthesize_*-Helper schreiben nicht in DB — explizit dokumentiert in Architektur-Details), Latenz (handler.compute synchron — wenn >3s ⇒ U6 statt U2, in Doku), Zwei-Pfade (Task 5 Refactor löst das), Concurrency (user_id-Check in commit_preview, Task 6) ✓

- [ ] **Schritt 3: Final Commit**

```
git diff --quiet || git commit -am "style: cargo fmt"
```

---

## Was später kommt

- Bulk-Preview (`previewBulk(entity_type, payload[])`)
- Live-Preview (typing-Diff ohne Save-Klick)
- Cross-User-Preview-Persistierung
- Konkrete `derive_handler` für T5/T6 (GenerateAccountEntries)
