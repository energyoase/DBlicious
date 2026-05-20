# U5 — App-Context-Channel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Etabliert einen reaktiven, app-weiten Kontext-Kanal (`active_year`, optional `active_tenant`/`active_currency`) als Leptos-Signal im Client, der per HTTP-Header an den Server geht und dort über `EntitySettings.context_filter` automatisch in `FilterCriteria` einfliesst. Ein einfacher `<YearSelector>` ist die erste konsumierende UI.

**Architecture:** Wire-Typ `shared::AppContext` (neues Modul, camelCase, alle Felder optional/skip-if-none). Header `x-dblicious-context` mit base64-/raw-JSON-Body. Server liest den Header direkt im bestehenden `graphql_handler` (kein separater axum Tower-Layer — der Handler bekommt `HeaderMap` schon mit, das vermeidet die Layer-Order-Risiken aus Spec §9). Resolver `entities` mergt den Context vor Sort/Page in `FilterCriteria`. Client-Signal `AppContextHandle` wird in `App` provided; `client/src/graphql/mod.rs::execute` hängt den Header zentral an. `EntityTable`-Shells abonnieren das Signal und triggern ein Reload bei Änderung.

**Tech Stack:** Rust, `shared` (serde), `server` (axum + async-graphql + SeaORM), `client` (Leptos CSR/WASM, `js_sys::Date`), `cargo test --workspace`.

**Spec-Referenz:** [`docs/superpowers/specs/2026-05-20-u5-app-context-channel-design.md`](../specs/2026-05-20-u5-app-context-channel-design.md).

**Vorbedingung:** keine. Funktioniert auf jeder heutigen Source (managed-sqlite); Phase 0.6 foreign-sqlite ergänzt später ein `context_filter` für seine Bindings.

---

## File Structure

**Neu:**
- `shared/src/app_context.rs` — `AppContext`-Typ + Header-Konstante + Roundtrip-Helpers
- `shared/tests/app_context_wire.rs` — Wire-Format-Roundtrip
- `client/src/components/context/mod.rs` — `AppContextHandle` Provider
- `client/src/components/context/year_selector.rs` — `<YearSelector>` Dropdown
- `server/tests/context_filter.rs` — End-to-End-Test
- `examples/shop/entities/order/columns.toml` (Erweiterung) — `year`-Spalte für Test
- `examples/shop/entities/order/settings.toml` (Erweiterung) — `contextFilter`
- `examples/shop/entities/order/seed.toml` (Erweiterung) — Rows mit `year=2024/2025`

**Modifizieren:**
- `shared/src/lib.rs` — Re-Export `AppContext` + `APP_CONTEXT_HEADER`
- `shared/src/settings.rs` — `EntitySettings.context_filter: BTreeMap<String, String>`
- `server/src/main.rs` oder `server/src/lib.rs` — `graphql_handler` liest Header, packt `AppContext` in `Request::data`
- `server/src/schema.rs` — `entities`-Resolver liest `Context::data::<AppContext>()` und merged via Helfer
- `server/src/data.rs` — neuer Helper `apply_context_filter(criteria, ctx, settings)`
- `client/src/graphql/mod.rs::execute` — Header anhängen aus Context
- `client/src/app.rs::App` — `provide_app_context()` mounten + `<YearSelector>` einhängen
- `client/src/components/table/shell.rs::EntityTableShell` — Subscribe auf `AppContextHandle` und Reload triggern
- `client/src/lib.rs` — neues Modul `components::context` exportieren
- `client/locales/de/main.ftl`, `client/locales/en/main.ftl` — `year-selector-*` keys

---

## Architektur-Details

### `shared::AppContext`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tenant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_currency: Option<String>,
}

pub const APP_CONTEXT_HEADER: &str = "x-dblicious-context";

impl AppContext {
    pub fn is_empty(&self) -> bool {
        self.active_year.is_none()
            && self.active_tenant.is_none()
            && self.active_currency.is_none()
    }

    pub fn slot(&self, key: &str) -> Option<serde_json::Value> {
        match key {
            "active_year" => self.active_year.map(serde_json::Value::from),
            "active_tenant" => self.active_tenant.clone().map(serde_json::Value::from),
            "active_currency" => self.active_currency.clone().map(serde_json::Value::from),
            _ => None,
        }
    }
}
```

### Header-Pfad: warum nicht via Tower-Layer

Spec §3.2 schlägt einen axum-Tower-Layer (`server/src/context_layer.rs`) vor. Wir extendieren stattdessen direkt den bestehenden `graphql_handler` aus `server/src/main.rs` (oder `lib.rs`). Begründung:

- Der Handler bekommt `HeaderMap` schon als Parameter.
- Layer-Ordering (Spec §9 Risk #1) entfällt komplett.
- Eine einzige Stelle, die `Request::data(app_context)` setzt — leichter zu testen.

Die Wahl ändert nichts am Wire-Vertrag; ein späterer Refactor zu einem Layer ist trivial, wenn weitere Endpoints den Context brauchen.

### `EntitySettings.context_filter`

```rust
/// Pro Kontext-Slot → Spalten-Pfad (mit "$column."-Prefix für Forward-Compat
/// auf strukturierte Pfade). Heute: nur "$column.<name>" wird unterstützt.
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub context_filter: BTreeMap<String, String>,
```

`server/src/data.rs::apply_context_filter` mappt jeden Slot mit Header-Wert auf einen `FilterPredicate::Eq { column, value }` und appended ihn an `FilterCriteria.predicates`. Slots ohne Wert ⇒ no-op. Unbekannte Slot-Namen ⇒ warn-log + ignore.

### Reload-Mechanik im Client

`EntityTableShell` hat heute eine `LocalResource`-Closure, die auf TableState-Signale reagiert. Wir abonnieren das `AppContextHandle`-Signal **innerhalb derselben Closure** (statt einem zweiten `Effect::new`), damit es keine Reload-Loops gibt (Spec §9 Risk #2): die Closure liest das Signal, schreibt es aber nie zurück.

### Was NICHT in diesem Plan ist

- **`availableYears` GraphQL-Query** (Spec §3.4): V1 hardcodet `current_year - 4 ..= current_year + 1` in `<YearSelector>` über `js_sys::Date`. Wenn Phase 1.7.12 (Aggregation) verfügbar ist, wird ein zweiter Plan die Query nachziehen.
- **Persistenz des gewählten Jahres** (Spec §10): kommt mit Phase 0.7 Auth/User-Settings.
- **`active_tenant`/`active_currency` UI**: Wire-Slots reserviert, kein Selector.

---

## Tasks

### Task 1: `shared::AppContext` mit Wire-Roundtrip-Test

**Files:**
- Neu: `shared/src/app_context.rs`
- Modify: `shared/src/lib.rs`
- Neu (Test): `shared/tests/app_context_wire.rs`

- [ ] **Schritt 1: Failing Test schreiben**

`shared/tests/app_context_wire.rs`:

```rust
use shared::{AppContext, APP_CONTEXT_HEADER};

#[test]
fn header_name_is_lowercase_x_dblicious_context() {
    assert_eq!(APP_CONTEXT_HEADER, "x-dblicious-context");
}

#[test]
fn empty_context_serializes_to_empty_object() {
    let ctx = AppContext::default();
    let json = serde_json::to_string(&ctx).unwrap();
    assert_eq!(json, "{}");
    assert!(ctx.is_empty());
}

#[test]
fn active_year_roundtrips_as_camel_case() {
    let ctx = AppContext {
        active_year: Some(2025),
        ..AppContext::default()
    };
    let json = serde_json::to_string(&ctx).unwrap();
    assert_eq!(json, r#"{"activeYear":2025}"#);
    let back: AppContext = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ctx);
}

#[test]
fn unknown_fields_are_ignored() {
    let json = r#"{"activeYear":2024,"unknownSlot":"x"}"#;
    let ctx: AppContext = serde_json::from_str(json).unwrap();
    assert_eq!(ctx.active_year, Some(2024));
}

#[test]
fn slot_returns_json_value() {
    let ctx = AppContext { active_year: Some(2025), ..AppContext::default() };
    assert_eq!(ctx.slot("active_year"), Some(serde_json::json!(2025)));
    assert_eq!(ctx.slot("active_currency"), None);
    assert_eq!(ctx.slot("bogus"), None);
}
```

- [ ] **Schritt 2: Test laufen lassen — FAIL**

```
cargo test -p shared --test app_context_wire
```
Erwartung: Compile-Error, `AppContext` und `APP_CONTEXT_HEADER` nicht gefunden.

- [ ] **Schritt 3: `shared/src/app_context.rs` anlegen**

```rust
//! App-weiter Kontext-Kanal (siehe U5 Spec).

use serde::{Deserialize, Serialize};

pub const APP_CONTEXT_HEADER: &str = "x-dblicious-context";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tenant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_currency: Option<String>,
}

impl AppContext {
    pub fn is_empty(&self) -> bool {
        self.active_year.is_none()
            && self.active_tenant.is_none()
            && self.active_currency.is_none()
    }

    pub fn slot(&self, key: &str) -> Option<serde_json::Value> {
        match key {
            "active_year" => self.active_year.map(serde_json::Value::from),
            "active_tenant" => self.active_tenant.clone().map(serde_json::Value::from),
            "active_currency" => self.active_currency.clone().map(serde_json::Value::from),
            _ => None,
        }
    }
}
```

- [ ] **Schritt 4: `shared/src/lib.rs` exportieren**

Nach den anderen `pub mod`/`pub use`-Zeilen anhängen:

```rust
pub mod app_context;
pub use app_context::{AppContext, APP_CONTEXT_HEADER};
```

- [ ] **Schritt 5: Test laufen lassen — PASS**

```
cargo test -p shared --test app_context_wire
```
Erwartung: 5 Tests grün.

- [ ] **Schritt 6: Commit**

```
git add shared/src/app_context.rs shared/src/lib.rs shared/tests/app_context_wire.rs
git commit -m "feat(shared): AppContext wire-type + APP_CONTEXT_HEADER"
```

---

### Task 2: `EntitySettings.context_filter`

**Files:**
- Modify: `shared/src/settings.rs`
- Modify (Test): `shared/tests/app_context_wire.rs`

- [ ] **Schritt 1: Failing Test ergänzen**

In `shared/tests/app_context_wire.rs` anhängen:

```rust
use shared::EntitySettings;
use std::collections::BTreeMap;

#[test]
fn entity_settings_context_filter_defaults_empty() {
    let s = EntitySettings::default();
    assert!(s.context_filter.is_empty());
}

#[test]
fn entity_settings_context_filter_camel_case_skip_empty() {
    let mut s = EntitySettings::default();
    let json_empty = serde_json::to_string(&s).unwrap();
    assert!(!json_empty.contains("contextFilter"));

    s.context_filter.insert("active_year".into(), "$column.year".into());
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains(r#""contextFilter":{"active_year":"$column.year"}"#));

    let back: EntitySettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back.context_filter.get("active_year"), Some(&"$column.year".to_string()));
}
```

- [ ] **Schritt 2: Test laufen — FAIL**

```
cargo test -p shared --test app_context_wire
```
Erwartung: `no field 'context_filter' on type EntitySettings`.

- [ ] **Schritt 3: `EntitySettings` erweitern**

In `shared/src/settings.rs` im `EntitySettings`-Struct ergänzen:

```rust
/// Kontext-Filter: ordnet einem Kontext-Slot (z.B. "active_year") einen
/// Spalten-Pfad zu ("$column.year"). Bei Listenabfragen merged der Server
/// pro Slot mit Header-Wert einen impliziten Gleichheits-Filter.
#[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
pub context_filter: std::collections::BTreeMap<String, String>,
```

Falls `BTreeMap` schon importiert ist: `use std::collections::BTreeMap;` oben in der Datei nutzen statt voll-qualifiziert.

- [ ] **Schritt 4: Test laufen — PASS**

```
cargo test -p shared --test app_context_wire
```
Erwartung: 7 Tests grün.

- [ ] **Schritt 5: Commit**

```
git add shared/src/settings.rs shared/tests/app_context_wire.rs
git commit -m "feat(shared): EntitySettings.context_filter slot-to-column map"
```

---

### Task 3: `apply_context_filter` Server-Helper + Unit-Test

**Files:**
- Modify: `server/src/data.rs`
- Modify (Test): `server/src/data.rs` (inline `#[cfg(test)]` Modul)

- [ ] **Schritt 1: Inline-Test in `data.rs` anhängen**

Am Ende der Datei:

```rust
#[cfg(test)]
mod context_tests {
    use super::*;
    use shared::AppContext;
    use std::collections::BTreeMap;

    fn settings_with_year_filter() -> shared::EntitySettings {
        let mut s = shared::EntitySettings::default();
        s.context_filter.insert("active_year".into(), "$column.year".into());
        s
    }

    #[test]
    fn apply_context_filter_no_header_no_change() {
        let ctx = AppContext::default();
        let mut criteria = shared::FilterCriteria::default();
        apply_context_filter(&mut criteria, &ctx, &settings_with_year_filter());
        assert!(criteria.predicates.is_empty());
    }

    #[test]
    fn apply_context_filter_no_setting_no_change() {
        let ctx = AppContext { active_year: Some(2025), ..AppContext::default() };
        let mut criteria = shared::FilterCriteria::default();
        apply_context_filter(&mut criteria, &ctx, &shared::EntitySettings::default());
        assert!(criteria.predicates.is_empty());
    }

    #[test]
    fn apply_context_filter_appends_predicate() {
        let ctx = AppContext { active_year: Some(2025), ..AppContext::default() };
        let mut criteria = shared::FilterCriteria::default();
        apply_context_filter(&mut criteria, &ctx, &settings_with_year_filter());
        assert_eq!(criteria.predicates.len(), 1);
        let pred = &criteria.predicates[0];
        assert_eq!(pred.column, "year");
        // Erwartung: Equals-Operator mit Wert 2025
        let v: i64 = serde_json::from_value(pred.value.clone()).unwrap();
        assert_eq!(v, 2025);
    }
}
```

- [ ] **Schritt 2: Test laufen — FAIL**

```
CARGO_TARGET_DIR=target-test cargo test -p server data::context_tests
```
Erwartung: `apply_context_filter` nicht gefunden.

- [ ] **Schritt 3: Helper implementieren**

In `server/src/data.rs` ergänzen (oben unter den `use`-Imports `shared::AppContext` aufnehmen):

```rust
pub fn apply_context_filter(
    criteria: &mut shared::FilterCriteria,
    ctx: &shared::AppContext,
    settings: &shared::EntitySettings,
) {
    if ctx.is_empty() || settings.context_filter.is_empty() {
        return;
    }
    for (slot, path) in &settings.context_filter {
        let Some(value) = ctx.slot(slot) else { continue };
        let Some(column) = path.strip_prefix("$column.") else {
            tracing::warn!(slot = slot.as_str(), path = path.as_str(),
                "context_filter path must start with $column.");
            continue;
        };
        criteria.predicates.push(shared::ColumnFilter {
            column: column.to_string(),
            predicate: shared::FilterPredicate::Equals { value },
        });
    }
}
```

Hinweis: Konkrete Variante von `FilterPredicate` muss zur heutigen Definition in `shared/src/lib.rs` passen — falls die Variante anders heisst (z.B. `NumberEquals`/`TextEquals`), Variante via `match` auf `value` wählen (siehe Schritt 4 falls Variante anders).

- [ ] **Schritt 4: Variante an `FilterPredicate` anpassen**

Falls Schritt 3 nicht kompiliert, weil `Equals` keine Variante ist: `grep -n "enum FilterPredicate" shared/src/lib.rs` und auf die existierende Variante umstellen (z.B. `FilterPredicate::TextEquals { case_insensitive: false, value: stringified }` oder Equivalent). Der Test in Schritt 1 verifiziert nur den Spaltenpfad + numerischen Wert — wenn nötig den Test darauf anpassen.

- [ ] **Schritt 5: Test laufen — PASS**

```
CARGO_TARGET_DIR=target-test cargo test -p server data::context_tests
```
Erwartung: 3 Tests grün.

- [ ] **Schritt 6: Commit**

```
git add server/src/data.rs
git commit -m "feat(server): apply_context_filter merges AppContext into FilterCriteria"
```

---

### Task 4: `graphql_handler` parsed Header → `Request::data`

**Files:**
- Modify: `server/src/main.rs` (oder `server/src/lib.rs` — dort wo der Handler heute liegt)

- [ ] **Schritt 1: Aktuellen Handler lokalisieren**

```
grep -n "graphql_handler" server/src/main.rs server/src/lib.rs
```

Notiere die Datei und Zeile (im Folgenden `server/src/lib.rs` als Beispiel). Heutige Signatur typ. so:

```rust
async fn graphql_handler(
    State(schema): State<AppSchema>,
    headers: HeaderMap,
    Json(req): Json<async_graphql::Request>,
) -> impl IntoResponse { ... }
```

- [ ] **Schritt 2: Header-Parse + `Request::data` ergänzen**

Im Handler-Body vor dem `schema.execute(...)`-Aufruf einfügen:

```rust
let app_context = headers
    .get(shared::APP_CONTEXT_HEADER)
    .and_then(|v| v.to_str().ok())
    .and_then(|s| match serde_json::from_str::<shared::AppContext>(s) {
        Ok(ctx) => Some(ctx),
        Err(e) => {
            tracing::warn!(error = ?e, "ignoring malformed {} header",
                shared::APP_CONTEXT_HEADER);
            None
        }
    })
    .unwrap_or_default();

let req = req.data(app_context);
```

- [ ] **Schritt 3: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```
Erwartung: clean compile. Sollte `tracing` nicht da sein: `log::warn!` analog.

- [ ] **Schritt 4: Commit**

```
git add server/src/lib.rs
git commit -m "feat(server): parse x-dblicious-context header into Request::data"
```

---

### Task 5: `entities`-Resolver mergt Context

**Files:**
- Modify: `server/src/schema.rs`

- [ ] **Schritt 1: Resolver-Stelle finden**

```
grep -n "async fn entities" server/src/schema.rs
```

Im Resolver wird heute `FilterCriteria` aus der GraphQL-Eingabe gelesen und an `data::entities_page` weitergegeben. Direkt vor dem `data::entities_page`-Aufruf den Context mergen.

- [ ] **Schritt 2: Merge einbauen**

```rust
let app_context = ctx
    .data::<shared::AppContext>()
    .cloned()
    .unwrap_or_default();
let settings = crate::data::settings_for(&entity_type)?;
let mut criteria = filter.unwrap_or_default();
crate::data::apply_context_filter(&mut criteria, &app_context, &settings);
```

Anpassen falls `settings_for` einen anderen Namen hat (`grep -n "fn settings_for" server/src/data.rs`).

- [ ] **Schritt 3: Compile-Check**

```
CARGO_TARGET_DIR=target-test cargo check -p server
```

- [ ] **Schritt 4: Commit**

```
git add server/src/schema.rs
git commit -m "feat(server): entities resolver applies AppContext via context_filter"
```

---

### Task 6: End-to-End-Test `server/tests/context_filter.rs`

**Files:**
- Neu: `server/tests/context_filter.rs`
- Modify (Fixture): `examples/shop/entities/order/columns.toml`
- Modify (Fixture): `examples/shop/entities/order/settings.toml`
- Modify (Fixture): `examples/shop/entities/order/seed.toml`

- [ ] **Schritt 1: `year`-Spalte zum Order-Fixture ergänzen**

In `examples/shop/entities/order/columns.toml` eine neue Spalten-Definition anhängen (Form wie die anderen Spalten — z.B. `[[columns]] key = "year" label_key = "year" field_type = { kind = "integer" } filterable = true`).

- [ ] **Schritt 2: `contextFilter` im Settings setzen**

`examples/shop/entities/order/settings.toml` um:

```toml
[contextFilter]
active_year = "$column.year"
```

- [ ] **Schritt 3: Seed-Rows mit 2024/2025 anlegen**

In `examples/shop/entities/order/seed.toml` mindestens drei Rows mit `year = 2024` und drei mit `year = 2025` ergänzen.

- [ ] **Schritt 4: Test-Datei anlegen**

`server/tests/context_filter.rs`:

```rust
use serde_json::json;
use shared::APP_CONTEXT_HEADER;

#[tokio::test]
#[serial_test::serial]
async fn entities_filters_by_active_year_when_header_set() {
    server::fresh_test_setup().await;

    // Ohne Header: alle Rows
    let all = server::test_helpers::query_entities("Order", None).await;
    assert!(all.len() >= 6);

    // Header active_year=2025
    let ctx = json!({"activeYear": 2025});
    let filtered = server::test_helpers::query_entities_with_context(
        "Order",
        &ctx.to_string(),
    ).await;
    assert!(filtered.iter().all(|e| {
        let y = e.fields.get("year").and_then(|v| v.as_i64()).unwrap();
        y == 2025
    }));
    assert_eq!(filtered.len(), 3);
}

#[tokio::test]
#[serial_test::serial]
async fn entities_ignores_malformed_header() {
    server::fresh_test_setup().await;

    let res = server::test_helpers::query_entities_with_context(
        "Order",
        "{not valid json",
    ).await;
    assert!(res.len() >= 6, "malformed header → no filtering, all rows back");
}
```

- [ ] **Schritt 5: `test_helpers::query_entities_with_context` bereitstellen**

Falls noch nicht vorhanden, in `server/src/lib.rs` unter `#[cfg(test)] pub mod test_helpers` einen Helper anlegen, der einen GraphQL-Request mit gesetztem `APP_CONTEXT_HEADER` durch den Schema-Pfad schickt. Vorlage ist `query_entities` (gleicher Bauplan, plus `req = req.data(app_context_from_str(header))`).

- [ ] **Schritt 6: Test laufen — FAIL/PASS**

```
CARGO_TARGET_DIR=target-test cargo test -p server --test context_filter
```
Erwartung: 2 Tests grün. Bei FAIL: Fixture/Helpers nachziehen.

- [ ] **Schritt 7: Commit**

```
git add server/tests/context_filter.rs examples/shop/entities/order/columns.toml examples/shop/entities/order/settings.toml examples/shop/entities/order/seed.toml server/src/lib.rs
git commit -m "test(server): end-to-end AppContext header filters entities by year"
```

---

### Task 7: Client `AppContextHandle` Provider

**Files:**
- Neu: `client/src/components/context/mod.rs`
- Modify: `client/src/lib.rs`
- Modify: `client/src/app.rs`

- [ ] **Schritt 1: Modul anlegen**

`client/src/components/context/mod.rs`:

```rust
use leptos::prelude::*;
use shared::AppContext;

#[derive(Copy, Clone)]
pub struct AppContextHandle {
    signal: RwSignal<AppContext>,
}

impl AppContextHandle {
    pub fn read(&self) -> AppContext {
        self.signal.get()
    }

    pub fn track(&self) -> AppContext {
        // Read-with-tracking — alle abonnierten Closures laufen bei Change neu.
        self.signal.get()
    }

    pub fn set_year(&self, year: Option<i32>) {
        self.signal.update(|c| c.active_year = year);
    }

    pub fn set(&self, ctx: AppContext) {
        self.signal.set(ctx);
    }
}

pub fn provide_app_context() {
    let signal = RwSignal::new(AppContext::default());
    provide_context(AppContextHandle { signal });
}

pub fn use_app_context() -> AppContextHandle {
    use_context::<AppContextHandle>()
        .expect("AppContextHandle nicht im Leptos-Context — provide_app_context() vergessen?")
}

pub mod year_selector;
pub use year_selector::YearSelector;
```

- [ ] **Schritt 2: Modul in `client/src/lib.rs` exportieren**

Im `pub mod components { ... }`-Block oder analoger Stelle:

```rust
pub mod context;
```

(Pfad: `components/context` — passe an die heutige Struktur in `lib.rs` an.)

- [ ] **Schritt 3: In `App` provisionieren**

In `client/src/app.rs::App` direkt nach den anderen `provide_*()`-Aufrufen:

```rust
crate::components::context::provide_app_context();
```

- [ ] **Schritt 4: Compile-Check**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

- [ ] **Schritt 5: Commit**

```
git add client/src/components/context/mod.rs client/src/lib.rs client/src/app.rs
git commit -m "feat(client): AppContextHandle provider + use_app_context()"
```

---

### Task 8: GraphQL-Header-Injection im `execute`-Helper

**Files:**
- Modify: `client/src/graphql/mod.rs`

- [ ] **Schritt 1: `execute`-Stelle prüfen**

```
grep -n "pub async fn execute" client/src/graphql/mod.rs
```

Heutige Signatur typ.: baut `gloo_net::http::Request::post("/graphql")` + setzt `Content-Type` + sendet `body`.

- [ ] **Schritt 2: Header anhängen**

Innerhalb des Request-Builds, vor `.send()`, ergänzen:

```rust
let header_value = if let Some(handle) = leptos::prelude::use_context::<crate::components::context::AppContextHandle>() {
    let ctx = handle.read();
    if ctx.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&ctx).expect("AppContext serializes"))
    }
} else {
    None
};

let mut req_builder = gloo_net::http::Request::post("/graphql")
    .header("Content-Type", "application/json");

if let Some(v) = header_value.as_deref() {
    req_builder = req_builder.header(shared::APP_CONTEXT_HEADER, v);
}

let resp = req_builder.body(body)?.send().await?;
```

Hinweis: existing Code mag anders strukturiert sein — die Idee ist einen optionalen Header-Set vor `.send()` einzuschieben.

- [ ] **Schritt 3: Compile-Check**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

- [ ] **Schritt 4: Commit**

```
git add client/src/graphql/mod.rs
git commit -m "feat(client): graphql/execute attaches x-dblicious-context header"
```

---

### Task 9: `EntityTableShell` reagiert auf Context-Änderungen

**Files:**
- Modify: `client/src/components/table/shell.rs`

- [ ] **Schritt 1: Stelle finden**

```
grep -n "LocalResource" client/src/components/table/shell.rs
```

Es gibt eine `LocalResource`/`create_resource`-Definition, die auf TableState-Signale reagiert (sort/filter/page). Wir lesen das `AppContextHandle`-Signal in derselben Closure.

- [ ] **Schritt 2: Subscribe ergänzen**

Innerhalb der Resource-Closure, *vor* dem Fetch-Aufruf:

```rust
let app_ctx_handle = use_context::<crate::components::context::AppContextHandle>();
let _ctx_subscribe = app_ctx_handle.map(|h| h.track());
// _ctx_subscribe nicht weiter benutzen — der Aufruf reicht, um die
// Closure auf Context-Änderungen reaktiv zu machen.
```

Damit lädt der Shell automatisch neu, wenn der Year-Slot wechselt. Wichtig: **niemals** im Resource-Callback den Context zurückschreiben (Spec §9 Risk #2).

- [ ] **Schritt 3: Compile-Check + manuelles UI-Smoke optional**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

- [ ] **Schritt 4: Commit**

```
git add client/src/components/table/shell.rs
git commit -m "feat(client): EntityTableShell reloads on AppContext change"
```

---

### Task 10: `<YearSelector>` Komponente + i18n

**Files:**
- Neu: `client/src/components/context/year_selector.rs`
- Modify: `client/locales/de/main.ftl`, `client/locales/en/main.ftl`

- [ ] **Schritt 1: Komponente schreiben**

`client/src/components/context/year_selector.rs`:

```rust
use leptos::prelude::*;
use crate::components::context::use_app_context;
use crate::i18n::t;
use crate::styling::use_design;

#[component]
pub fn YearSelector() -> impl IntoView {
    let ctx = use_app_context();
    let design = use_design();
    let now_year = js_sys::Date::new_0().get_full_year() as i32;
    let years: Vec<i32> = ((now_year - 4)..=(now_year + 1)).collect();

    let selected = move || ctx.read().active_year;

    let on_change = move |ev: web_sys::Event| {
        let target: web_sys::HtmlSelectElement = event_target(&ev);
        let val = target.value();
        let parsed = val.parse::<i32>().ok();
        ctx.set_year(parsed);
    };

    let label_style = design.text();
    let select_style = design.surface();

    view! {
        <label style=label_style.inline.clone() class=label_style.class.clone()>
            { t("year-selector-label") }
            <select style=select_style.inline.clone()
                    class=select_style.class.clone()
                    on:change=on_change>
                <option value="">{ t("year-selector-any") }</option>
                {years.into_iter().map(|y| {
                    let is_sel = selected() == Some(y);
                    view! {
                        <option value=y.to_string() selected=is_sel>{y}</option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

fn event_target<T: wasm_bindgen::JsCast>(ev: &web_sys::Event) -> T {
    use wasm_bindgen::JsCast;
    ev.target().unwrap().dyn_into::<T>().unwrap()
}
```

- [ ] **Schritt 2: i18n-Keys ergänzen**

`client/locales/de/main.ftl`:

```
year-selector-label = Jahr
year-selector-any = Alle Jahre
```

`client/locales/en/main.ftl`:

```
year-selector-label = Year
year-selector-any = All years
```

- [ ] **Schritt 3: In App-Header einhängen**

In `client/src/app.rs::App` an der Position des bestehenden Top-Bar-Layouts:

```rust
view! {
    <header>
        // … bestehendes Logo / Nav-Toggle …
        <crate::components::context::YearSelector/>
    </header>
    // … Rest …
}
```

(Position kann je nach heutigem App-Layout variieren — wesentlich ist, dass `YearSelector` einmal global sichtbar ist.)

- [ ] **Schritt 4: Compile-Check**

```
CARGO_TARGET_DIR=../target-test cargo check --target wasm32-unknown-unknown -p client
```

- [ ] **Schritt 5: Manuelle UI-Verifikation**

```
cargo run -p server -- --data-dir ./examples/shop
# in zweitem Terminal:
cd client && trunk serve
```

Im Browser:
1. App lädt, YearSelector zeigt Optionen `Alle Jahre`, `2022`, `2023`, `2024`, `2025`, `2026`, `2027`.
2. Order-Liste lädt — alle 6 Rows sichtbar.
3. `2025` auswählen → Order-Liste reloaded, zeigt nur 3 Rows mit `year=2025`.
4. `Alle Jahre` → zurück zu 6 Rows.

Wenn das funktioniert, weiter. Sonst: DevTools-Network-Tab prüfen ob `x-dblicious-context`-Header korrekt gesetzt wird.

- [ ] **Schritt 6: Commit**

```
git add client/src/components/context/year_selector.rs client/locales/de/main.ftl client/locales/en/main.ftl client/src/app.rs
git commit -m "feat(client): YearSelector writes active_year into AppContext"
```

---

### Task 11: Workspace-Cleanup (clippy + fmt + alle Tests)

**Files:** keine direkten — Verifikationsschritt.

- [ ] **Schritt 1: Clippy**

```
CARGO_TARGET_DIR=target-test cargo clippy --workspace --no-deps -- -A clippy::doc_overindented_list_items -A clippy::too_many_arguments
```

Falls Warnings: punktuell fixen, jeweils mit Mini-Commit.

- [ ] **Schritt 2: Format**

```
cargo fmt
git diff --quiet || git commit -am "style: cargo fmt"
```

- [ ] **Schritt 3: Workspace-Tests**

```
CARGO_TARGET_DIR=target-test cargo test --workspace
```

Erwartung: alle grün, inkl. neue `app_context_wire`-Tests und `server::context_filter`-Tests.

- [ ] **Schritt 4: Self-Review-Check**

Liste durchgehen:

- Spec §3.1 `AppContext`-Wire → Task 1 ✓
- Spec §3.2 Header + Transport → Task 4 (Handler statt Tower-Layer; in Architektur-Details begründet) ✓
- Spec §3.3 EntitySettings + Resolver-Merge → Task 2, 3, 5 ✓
- Spec §3.4 YearSelector → Task 10 ✓
- Spec §3.5 Reaktivität → Task 9 ✓
- Spec §5 Edge-Cases (malformed Header) → Task 6 zweiter Test ✓
- Spec §7 Tests → Task 1 (wire), Task 3 (helper unit), Task 6 (e2e); Client-Unit für YearSelector ersetzt durch manuelle Smoke (Task 10 Schritt 5) — Begründung: kein wasm-bindgen-test setup im Repo

---

## Was später kommt

- `availableYears` GraphQL-Query, sobald Phase 1.7.12 (Aggregation) verfügbar ist
- Persistenz pro User in `users.preferences` (Phase 0.7)
- `active_tenant`-Selektor (eigene Spec, Phase 0.7)
