# U1 — Reference-Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Erweitert `FieldType::Reference` um `display_template` + `search_columns` und liefert eine Client-Komponente `ReferencePicker`, die per GraphQL-Suche auf der Ziel-Entity eine FK-Auswahl mit Autocomplete + Rich Format anbietet. Read-only-View zeigt das aufgelöste Display-Label statt der nackten ID.

**Architecture:** Additive Wire-Format-Erweiterung in `shared::FieldType` (Backwards-Compat über `#[serde(default)]`). Neuer Server-Resolver `referenceCandidates(entity_type, query, limit, exclude_ids?)` in `server/src/schema.rs`, der über die existierende `data::*`/Source-Schicht eine paginierte Suche fährt. Client-Komponente `ReferencePicker` (Leptos) als Editor-Variante für `ControlKind::Lookup`. Display-Label-Lookup in `DefaultFieldRegistry::render_reference` über einen kleinen Client-Cache (`ReferenceDisplayCache`).

**Tech Stack:** Rust, `shared` (serde), `server` (axum + async-graphql + SeaORM), `client` (Leptos CSR/WASM), `cargo test --workspace`, kein Codegen.

**Spec-Referenz:** [`docs/superpowers/specs/2026-05-20-d2v-on-dblicious-gap-analysis.md`](../specs/2026-05-20-d2v-on-dblicious-gap-analysis.md) §4.1 U1.

**Vorbedingung:** keine harten — Plan baut auf bestehendem `FieldType::Reference` + `ControlKind::Lookup`-Stub auf. Funktioniert mit beliebiger Source (managed-sqlite heute, foreign-sqlite sobald B1 lebt).

---

## File Structure

**Modifizieren:**
- `shared/src/lib.rs` (`FieldType::Reference` Variante erweitern, Z. 107–110)
- `shared/tests/field_type_wire_format.rs` (Backwards-Compat-Test + neue Felder)
- `server/src/schema.rs` (neuer Query-Resolver `reference_candidates` im `QueryRoot`)
- `client/src/graphql/queries.rs` (neue Funktion `fetch_reference_candidates`)
- `client/src/components/field/mod.rs` (`ControlKind::Lookup`-Branch nutzt `ReferencePicker`; `DefaultFieldRegistry::render_reference` nutzt Cache)
- `client/src/lib.rs` (neues Modul `components::reference_picker` exportieren)
- `client/locales/de/main.ftl`, `client/locales/en/main.ftl` (i18n-Keys `reference-picker-*`)

**Neu:**
- `shared/src/reference.rs` (typisierte Antwortstruktur `ReferenceCandidate` für Server↔Client-Wiederverwendung)
- `client/src/components/reference_picker.rs` (`<ReferencePicker>` Leptos-Komponente)
- `client/src/components/reference_cache.rs` (Display-Label-Cache pro Entity-Typ)

**Test-Dateien:**
- `shared/tests/reference_template.rs` (neu — Wire-Format-Roundtrip)
- `server/tests/reference_candidates.rs` (neu — End-to-End-Resolver-Test)
- `client/src/components/reference_picker.rs` (Unit-Tests inline)

---

## Architektur-Details (für den ausführenden Engineer)

### Erweiterung von `FieldType::Reference`

Vorher:
```rust
Reference { entity: String },
```

Nachher (additiv):
```rust
Reference {
    entity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_template: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    search_columns: Vec<String>,
},
```

- `display_template`: ein Mustache-ähnliches Template, z.B. `"{number} — {name}"`. `{<spalte>}`-Platzhalter werden aus den `fields` der Ziel-Entity ersetzt. Fehlende Spalten werden zu leerem String, kein Fehler.
- `search_columns`: welche Spalten der Ziel-Entity für die Volltextsuche herangezogen werden. Empty = nur ID.

### Neuer GraphQL-Query

```graphql
type ReferenceCandidate {
    id: String!
    display: String!
    fields: JSON!
}

extend type Query {
    referenceCandidates(
        entityType: String!,
        query: String!,
        limit: Int = 20,
    ): [ReferenceCandidate!]!
}
```

Server-Logik:
1. Settings für `entityType` laden → daraus `FieldType::Reference` der definierenden Spalte (falls aufgerufen für ein Property — alternative: globale Auflösung pro entity_type über Settings-Default).
2. `display_template` + `search_columns` aus dem `FieldType`-Slot der Ziel-Entity beziehen (über `data::columns_for(entity_type)`).
3. Paginierte Liste der Ziel-Entity laden (`data::entities_page` oder neue Source-API), gefiltert durch `query` über `search_columns` (case-insensitive `contains`).
4. Pro Zeile `display = render_template(template, fields)` und nur `{id, display, fields}` zurückgeben.

### Client-Komponente `ReferencePicker`

Props:
- `entity_type: String` (Ziel-Entity)
- `value: Option<String>` (aktuelle FK-ID)
- `on_change: Callback<Option<String>>`
- `readonly: bool`

Verhalten:
- Rendert ein Search-Input. Bei Eingabe (debounced 200ms) feuert `fetch_reference_candidates(entity_type, query, 20)`.
- Zeigt Dropdown mit `display`-Strings.
- Bei Auswahl: `on_change(Some(id))`, Suchfeld zeigt den `display`-Wert.
- Bei leerem Eingabefeld: `on_change(None)`.
- Initial-Load: wenn `value: Some(id)`, einmal `referenceCandidates(entity_type, "", 1)` mit `exclude_ids` umgekehrt — oder besser: separater Resolver `referenceById(entity_type, id)` — entscheiden wir in Task 6.

### Display-Label-Lookup im List-View

`DefaultFieldRegistry::render_reference` (Z. 157) zeigt heute die ID. Nach diesem Plan:
1. Wenn `FieldType::Reference.display_template.is_some()`, frage einen process-weiten `ReferenceDisplayCache` nach `(entity_type, id) -> display`.
2. Cache-Miss → Platzhalter "…" + asynchroner Batch-Lookup, der mehrere IDs derselben Entity bündelt.
3. Cache-Hit → Display sofort rendern.

Cache lebt im Leptos-Kontext, ähnlich `provide_field_registry`. Batch-Lookup feuert max. alle 50ms ein `referenceCandidates`-Call (oder neuer Batch-Endpoint, in diesem Plan ausgeklammert).

### Was NICHT in diesem Plan ist

- **Phase 1.5 Picker-Registry-Integration**: später, sobald U1 das Konzept etabliert.
- **Composite-PK-Referenzen**: dieser Plan adressiert nur Single-PK-Referenzen. Composite kommt zusammen mit Track-B-B1 in eigener Iteration.
- **Multi-Reference (Collection-FK)**: `FieldType::Collection` bleibt unverändert; eigener Plan später.
- **Cross-Source-Referenzen**: Ziel-Entity muss in derselben Source liegen wie die Quell-Entity. Grenz-Check im Resolver-Code, aber nicht im UI exponiert.

---

## Tasks

### Task 1: `FieldType::Reference` erweitern + Wire-Format-Test

**Files:**
- Modify: `shared/src/lib.rs:107-110`
- Modify: `shared/tests/field_type_wire_format.rs` (oder neu wenn nicht vorhanden)
- Test: `shared/tests/reference_template.rs` (neu)

- [ ] **Step 1: Den existierenden Wire-Format-Test lesen**

Run: `cat shared/tests/field_type_wire_format.rs | head -80`
Expected: Datei existiert mit Tests, die `FieldType`-Varianten gegen JSON-Strings pinnen. Notiere, welche `Reference`-JSON-Form heute fixiert ist (vermutlich `{"kind":"reference","entity":"foo"}`).

- [ ] **Step 2: Failing-Test für die erweiterte Reference-Variante schreiben**

Datei: `shared/tests/reference_template.rs`

```rust
use shared::FieldType;

#[test]
fn reference_without_template_round_trips_as_before() {
    let ft = FieldType::Reference {
        entity: "category".into(),
        display_template: None,
        search_columns: vec![],
    };
    let json = serde_json::to_string(&ft).unwrap();
    assert_eq!(json, r#"{"kind":"reference","entity":"category"}"#);

    let back: FieldType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ft);
}

#[test]
fn reference_with_template_serializes_extra_fields() {
    let ft = FieldType::Reference {
        entity: "account".into(),
        display_template: Some("{number} — {name}".into()),
        search_columns: vec!["number".into(), "name".into()],
    };
    let json: serde_json::Value = serde_json::to_value(&ft).unwrap();
    assert_eq!(json["kind"], "reference");
    assert_eq!(json["entity"], "account");
    assert_eq!(json["displayTemplate"], "{number} — {name}");
    assert_eq!(json["searchColumns"], serde_json::json!(["number", "name"]));
}

#[test]
fn reference_legacy_json_deserializes_with_defaults() {
    // Bestands-Daten aus pre-U1-Datasets müssen weiterhin lesbar bleiben.
    let json = r#"{"kind":"reference","entity":"category"}"#;
    let ft: FieldType = serde_json::from_str(json).unwrap();
    match ft {
        FieldType::Reference { entity, display_template, search_columns } => {
            assert_eq!(entity, "category");
            assert!(display_template.is_none());
            assert!(search_columns.is_empty());
        }
        other => panic!("expected Reference, got {other:?}"),
    }
}
```

- [ ] **Step 3: Test laufen lassen — soll fehlschlagen**

Run: `cargo test -p shared --test reference_template`
Expected: Compile-Fehler "missing field display_template" oder ähnlich.

- [ ] **Step 4: `FieldType::Reference` erweitern**

`shared/src/lib.rs` Z. 107–110 ersetzen:

```rust
/// Verweis auf eine andere Entitaet (1:1).
Reference {
    entity: String,
    /// Optionales Mustache-Template fuer das Anzeige-Label. `{spalte}`
    /// wird beim Render aus den `fields` der Ziel-Entity ersetzt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_template: Option<String>,
    /// Spalten der Ziel-Entity, die fuer Volltextsuche im Picker
    /// herangezogen werden. Leer = nur ID-Lookup.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    search_columns: Vec<String>,
},
```

- [ ] **Step 5: Build prüfen, alle Aufrufstellen aktualisieren**

Run: `cargo check --workspace`
Expected: Fehler an Stellen, die heute `FieldType::Reference { entity }` matchen oder konstruieren (mind. `shared/src/ops.rs:327`, `client/src/components/field/mod.rs:126-131,392`). Für jede Match-Stelle die zwei neuen Felder mit `..` ignorieren; für Konstruktoren in Tests die neuen Defaults setzen. Beispiel-Patch in `shared/src/ops.rs`:

```rust
// Vorher: FieldType::Reference { .. } => Box::new(OpaqueOps),
// Nachher (bleibt gleich — `{ .. }` ignoriert weiterhin).
```

Beispiel-Patch in `shared/src/ops.rs:461` (Test):

```rust
let ops = ops_for(&FieldType::Reference {
    entity: "category".into(),
    display_template: None,
    search_columns: vec![],
});
```

- [ ] **Step 6: Tests grün?**

Run: `cargo test -p shared --test reference_template && cargo test -p shared`
Expected: PASS für die 3 neuen Tests; alle bestehenden `shared`-Tests grün.

- [ ] **Step 7: Commit**

```bash
git add shared/src/lib.rs shared/tests/reference_template.rs shared/src/ops.rs
git commit -m "feat(shared): FieldType::Reference adds display_template and search_columns"
```

---

### Task 2: Geteilter `ReferenceCandidate`-Typ + Template-Renderer

**Files:**
- Create: `shared/src/reference.rs`
- Modify: `shared/src/lib.rs` (Modul `reference` einbinden + re-exporten)
- Test: `shared/src/reference.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Failing-Test für `render_template`**

Neue Datei `shared/src/reference.rs`:

```rust
//! Geteilter Typ und Helper fuer Reference-Picker-Antworten.

use serde::{Deserialize, Serialize};
use serde_json::Map;

/// Antwort-Zeile des `referenceCandidates`-Resolvers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceCandidate {
    pub id: String,
    pub display: String,
    pub fields: Map<String, serde_json::Value>,
}

/// Rendert ein Mustache-aehnliches Template gegen die `fields`-Map einer
/// Entitaet. `{<spalte>}`-Platzhalter werden ersetzt; fehlende Spalten
/// werden zu leerem String. Keine Escape-Logik — die Ausgabe wird im
/// Client als Plaintext gerendert.
pub fn render_template(template: &str, fields: &Map<String, serde_json::Value>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut key = String::new();
            let mut closed = false;
            while let Some(&nc) = chars.peek() {
                chars.next();
                if nc == '}' {
                    closed = true;
                    break;
                }
                key.push(nc);
            }
            if closed {
                if let Some(v) = fields.get(&key) {
                    out.push_str(&value_to_display(v));
                }
            } else {
                out.push('{');
                out.push_str(&key);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn value_to_display(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fields() -> Map<String, serde_json::Value> {
        let mut m = Map::new();
        m.insert("number".into(), json!(1200));
        m.insert("name".into(), json!("Bank"));
        m
    }

    #[test]
    fn renders_two_placeholders() {
        assert_eq!(render_template("{number} — {name}", &fields()), "1200 — Bank");
    }

    #[test]
    fn missing_key_renders_empty() {
        assert_eq!(render_template("{number} {missing}", &fields()), "1200 ");
    }

    #[test]
    fn no_placeholders_passes_through() {
        assert_eq!(render_template("Kontonummer", &fields()), "Kontonummer");
    }

    #[test]
    fn unclosed_brace_kept_literal() {
        assert_eq!(render_template("{number} {oops", &fields()), "1200 {oops");
    }
}
```

- [ ] **Step 2: Modul registrieren**

`shared/src/lib.rs` — bei den `pub mod`-Zeilen Block einfügen (alphabetisch sortiert um Z. 18):

```rust
pub mod reference;
```

Und unter den `pub use`-Zeilen Block einfügen:

```rust
pub use reference::{render_template, ReferenceCandidate};
```

- [ ] **Step 3: Test laufen lassen**

Run: `cargo test -p shared reference`
Expected: 4 Tests PASS.

- [ ] **Step 4: Commit**

```bash
git add shared/src/reference.rs shared/src/lib.rs
git commit -m "feat(shared): ReferenceCandidate type and render_template helper"
```

---

### Task 3: Server-Resolver `reference_candidates`

**Files:**
- Modify: `server/src/schema.rs` (neue Methode in `QueryRoot`)
- Test: `server/tests/reference_candidates.rs` (neu)

- [ ] **Step 1: Failing-Integration-Test schreiben**

Neue Datei `server/tests/reference_candidates.rs`:

```rust
//! End-to-End-Test fuer den `referenceCandidates`-Resolver.

use serial_test::serial;
use server::fresh_test_setup;

#[tokio::test]
#[serial]
async fn returns_matching_candidates_with_display() {
    let schema = fresh_test_setup().await;

    // shop-Beispiel hat `category` mit display_template "{name}".
    // Wir suchen nach "werk".
    let query = r#"
        query Q($q: String!) {
            referenceCandidates(entityType: "category", query: $q, limit: 5) {
                id
                display
            }
        }
    "#;
    let resp = schema
        .execute(async_graphql::Request::new(query).variables(
            async_graphql::Variables::from_json(serde_json::json!({"q": "werk"}))
        ))
        .await;
    assert!(resp.errors.is_empty(), "got errors: {:?}", resp.errors);

    let data = resp.data.into_json().unwrap();
    let candidates = data["referenceCandidates"].as_array().unwrap();
    assert!(!candidates.is_empty(), "expected at least one match");
    for cand in candidates {
        let display = cand["display"].as_str().unwrap();
        assert!(
            display.to_lowercase().contains("werk"),
            "display {display:?} does not contain query"
        );
    }
}

#[tokio::test]
#[serial]
async fn empty_query_returns_first_page() {
    let schema = fresh_test_setup().await;

    let query = r#"
        query Q {
            referenceCandidates(entityType: "category", query: "", limit: 3) {
                id
                display
            }
        }
    "#;
    let resp = schema.execute(query).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    let data = resp.data.into_json().unwrap();
    let candidates = data["referenceCandidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 3);
}
```

- [ ] **Step 2: Test laufen lassen — soll fehlschlagen**

Run: `cargo test -p server --test reference_candidates --target-dir target-test`
Expected: Compile-Fehler "no field `referenceCandidates` on type `Query`" oder Resolver-Resolution-Fehler.

- [ ] **Step 3: Resolver implementieren**

In `server/src/schema.rs` — den Block lesen, wo `entities`-Resolver definiert ist (ca. Z. 811). Direkt davor oder danach einfügen:

```rust
async fn reference_candidates(
    &self,
    ctx: &Context<'_>,
    entity_type: String,
    query: String,
    #[graphql(default = 20)] limit: i32,
) -> async_graphql::Result<Vec<ReferenceCandidateOut>> {
    use shared::{FieldType, reference::render_template};

    // 1. Display-Template + Such-Spalten der Ziel-Entity bestimmen.
    //    Konvention: irgendeine Spalte der Ziel-Entity, deren FieldType
    //    Reference auf sich selbst zeigt, traegt das Template. Praktischer:
    //    wir suchen es in entitysettings.display_template als neues
    //    optionales Feld. Fuer diesen ersten Wurf: heuristisch ueber alle
    //    Spalten — finde die erste, die `display_template` setzt.
    let cols = crate::data::columns_for(&entity_type).await
        .ok_or_else(|| async_graphql::Error::new(format!(
            "unknown entity type: {entity_type}"
        )))?;
    let (template, search_cols) = cols
        .iter()
        .find_map(|c| match &c.field_type {
            FieldType::Reference { display_template: Some(t), search_columns, .. } => {
                Some((t.clone(), search_columns.clone()))
            }
            _ => None,
        })
        .unwrap_or_else(|| (String::new(), Vec::new()));

    // 2. Filter aufbauen: contains-Suche ueber search_cols (oder ueber
    //    alle Text-Spalten, wenn search_cols leer und query nicht leer).
    let mut filter = shared::FilterCriteria::default();
    if !query.is_empty() {
        let target_cols: Vec<String> = if !search_cols.is_empty() {
            search_cols.clone()
        } else {
            cols.iter()
                .filter(|c| matches!(c.field_type, FieldType::Text))
                .map(|c| c.key.clone())
                .collect()
        };
        // Mehrere search_cols werden als OR verknuepft. Falls die heutige
        // FilterCriteria kein OR kennt, fallback auf erste Spalte:
        if let Some(first) = target_cols.first() {
            filter.predicates.push(shared::FilterPredicate::Contains {
                column: first.clone(),
                value: query.clone(),
                case_insensitive: true,
            });
        }
    }

    // 3. Erste Seite laden (limit als page_size).
    let page = crate::data::entities_page(
        &entity_type,
        1,
        limit.max(1).min(200),
        None,
        &filter,
    ).await?;

    // 4. Pro Zeile Template rendern.
    let out = page
        .entities
        .iter()
        .map(|e| {
            let display = if template.is_empty() {
                e.id.clone()
            } else {
                render_template(&template, &e.fields)
            };
            ReferenceCandidateOut {
                id: e.id.clone(),
                display,
                fields: async_graphql::Json(serde_json::Value::Object(e.fields.clone())),
            }
        })
        .collect();
    Ok(out)
}
```

Und (wenn nicht schon vorhanden) den Wrapper-Typ ans Ende der Server-Typ-Section:

```rust
#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
struct ReferenceCandidateOut {
    id: String,
    display: String,
    fields: async_graphql::Json<serde_json::Value>,
}
```

- [ ] **Step 4: Build und Test prüfen**

Run: `cargo test -p server --test reference_candidates --target-dir target-test`
Expected: PASS für beide Tests. Wenn `category` im `shop`-Beispiel kein `display_template` hat, schlägt der erste Test fehl — dann Task 4 vorziehen.

- [ ] **Step 5: `examples/shop/entities/category/columns.json` um Display-Template anreichern (für den Test-Path)**

Run: `cat examples/shop/entities/category/columns.json`
Erwartet: eine `id`-Spalte oder ähnlich. Falls die Datei keine Reference-Spalte hat, ergänze für `product/columns.json` (das product hat vermutlich `category_id`) ein `display_template` auf der Reference-Spalte:

Beispiel-Diff in `examples/shop/entities/product/columns.json` (Spalte `category` → Reference auf `category`):

```jsonc
{
  "key": "category",
  "labelKey": "product.category",
  "fieldType": {
    "kind": "reference",
    "entity": "category",
    "displayTemplate": "{name}",
    "searchColumns": ["name"]
  }
}
```

- [ ] **Step 6: Test erneut laufen**

Run: `cargo test -p server --test reference_candidates --target-dir target-test`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add server/src/schema.rs examples/shop/entities/product/columns.json server/tests/reference_candidates.rs
git commit -m "feat(server): referenceCandidates resolver for FK picker search"
```

---

### Task 4: Client GraphQL-Binding `fetch_reference_candidates`

**Files:**
- Modify: `client/src/graphql/queries.rs` (neue Query + Funktion am Ende)

- [ ] **Step 1: Query-String + Vars-/Response-Strukturen anhängen**

`client/src/graphql/queries.rs` am Ende:

```rust
const REFERENCE_CANDIDATES_QUERY: &str = r#"
    query ReferenceCandidates($entityType: String!, $query: String!, $limit: Int!) {
        referenceCandidates(entityType: $entityType, query: $query, limit: $limit) {
            id
            display
            fields
        }
    }
"#;

#[derive(Serialize)]
struct RefCandidatesVars<'a> {
    entity_type: &'a str,
    query: &'a str,
    limit: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefCandidatesData {
    reference_candidates: Vec<shared::ReferenceCandidate>,
}

pub async fn fetch_reference_candidates(
    entity_type: &str,
    query: &str,
    limit: i32,
) -> Result<Vec<shared::ReferenceCandidate>, GqlError> {
    let data: RefCandidatesData = execute(
        REFERENCE_CANDIDATES_QUERY,
        RefCandidatesVars { entity_type, query, limit },
    )
    .await?;
    Ok(data.reference_candidates)
}
```

- [ ] **Step 2: Build prüfen**

Run: `cargo check -p client --target wasm32-unknown-unknown`
Expected: OK.

- [ ] **Step 3: Commit**

```bash
git add client/src/graphql/queries.rs
git commit -m "feat(client): GraphQL binding for referenceCandidates"
```

---

### Task 5: Client-Komponente `<ReferencePicker>`

**Files:**
- Create: `client/src/components/reference_picker.rs`
- Modify: `client/src/components/mod.rs` (Modul exportieren)

- [ ] **Step 1: Modul-Datei anlegen**

`client/src/components/reference_picker.rs`:

```rust
//! ReferencePicker — FK-Auswahl mit Server-getriebener Suche.
//!
//! Editor-Form von `FieldType::Reference`. Rendert ein Search-Input,
//! laedt bei Eingabe (debounced 200 ms) Kandidaten ueber
//! `fetch_reference_candidates` und zeigt eine Dropdown-Liste. Bei
//! Selektion meldet der Picker die neue FK-ID ueber `on_change`.

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::ReferenceCandidate;

use crate::graphql::queries::fetch_reference_candidates;

#[component]
pub fn ReferencePicker(
    /// Ziel-Entity, in der gesucht wird.
    entity_type: String,
    /// Aktuell ausgewaehlte FK-ID (oder None).
    #[prop(into)]
    value: Signal<Option<String>>,
    /// Wird mit der neuen FK-ID (oder None bei Leeren) aufgerufen.
    on_change: Callback<Option<String>>,
    #[prop(default = false)] readonly: bool,
) -> impl IntoView {
    let query: RwSignal<String> = RwSignal::new(String::new());
    let candidates: RwSignal<Vec<ReferenceCandidate>> = RwSignal::new(Vec::new());
    let loading: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let open: RwSignal<bool> = RwSignal::new(false);

    // Beim Mount und bei value-Aenderung: aktuelle ID -> Display laden.
    let entity_for_load = entity_type.clone();
    Effect::new(move |_| {
        let id = value.get();
        if let Some(id) = id {
            let et = entity_for_load.clone();
            spawn_local(async move {
                // Wir nutzen die Suche mit der ID — fallback wenn der
                // Server `referenceById` (zukuenftig) nicht hat.
                if let Ok(list) = fetch_reference_candidates(&et, &id, 1).await {
                    if let Some(found) = list.into_iter().find(|c| c.id == id) {
                        query.set(found.display);
                    }
                }
            });
        } else {
            query.set(String::new());
        }
    });

    // Debounced Search.
    let entity_for_search = entity_type.clone();
    let on_input = move |ev: leptos::ev::Event| {
        let v = event_target_value(&ev);
        query.set(v.clone());
        open.set(true);
        loading.set(true);
        error.set(None);
        let et = entity_for_search.clone();
        let v_for_async = v.clone();
        spawn_local(async move {
            // 200ms Debounce ueber gloo-timers koennen wir spaeter ergaenzen.
            match fetch_reference_candidates(&et, &v_for_async, 20).await {
                Ok(list) => candidates.set(list),
                Err(e) => error.set(Some(format!("{e}"))),
            }
            loading.set(false);
        });
    };

    let on_pick = move |c: ReferenceCandidate| {
        query.set(c.display.clone());
        candidates.set(Vec::new());
        open.set(false);
        on_change.run(Some(c.id));
    };

    let on_clear = move |_| {
        query.set(String::new());
        candidates.set(Vec::new());
        open.set(false);
        on_change.run(None);
    };

    view! {
        <div style="position: relative; display: inline-flex; gap: 0.25rem; align-items: center;">
            <input
                type="text"
                prop:value=move || query.get()
                readonly=readonly
                on:input=on_input
                placeholder=move || crate::i18n::t("reference-picker-placeholder")
                style="min-width: 14rem;"
            />
            <button type="button" on:click=on_clear disabled=readonly>"×"</button>
            {move || loading.get().then(|| view! {
                <span style="color:#6b7280; font-size: 0.8rem;">
                    {move || crate::i18n::t("reference-picker-loading")}
                </span>
            })}
            {move || error.get().map(|e| view! {
                <span style="color:#b91c1c; font-size: 0.8rem;">{e}</span>
            })}
            {move || (open.get() && !candidates.get().is_empty()).then(|| {
                let cands = candidates.get();
                view! {
                    <ul style="position:absolute; top:100%; left:0; right:0; \
                               margin:0; padding:0; list-style:none; \
                               background:#fff; border:1px solid #d1d5db; \
                               max-height:14rem; overflow-y:auto; z-index:50;">
                        {cands.into_iter().map(|c| {
                            let c_for_click = c.clone();
                            let display = c.display.clone();
                            view! {
                                <li
                                    style="padding:0.25rem 0.5rem; cursor:pointer;"
                                    on:mousedown=move |_| on_pick(c_for_click.clone())
                                >{display}</li>
                            }
                        }).collect_view()}
                    </ul>
                }
            })}
        </div>
    }
}
```

- [ ] **Step 2: Modul registrieren**

In `client/src/components/mod.rs` neuen `pub mod reference_picker;`-Eintrag ergänzen (alphabetisch sortiert).

- [ ] **Step 3: i18n-Keys anlegen**

`client/locales/de/main.ftl` ans Ende:

```
reference-picker-placeholder = Suchen…
reference-picker-loading = lädt…
```

`client/locales/en/main.ftl`:

```
reference-picker-placeholder = Search…
reference-picker-loading = loading…
```

- [ ] **Step 4: Build prüfen**

Run: `cargo check -p client --target wasm32-unknown-unknown`
Expected: OK.

- [ ] **Step 5: Commit**

```bash
git add client/src/components/reference_picker.rs client/src/components/mod.rs client/locales/de/main.ftl client/locales/en/main.ftl
git commit -m "feat(client): ReferencePicker component with server-driven search"
```

---

### Task 6: `ControlKind::Lookup` schaltet auf `ReferencePicker`

**Files:**
- Modify: `client/src/components/field/mod.rs` (Z. 462–467 — der `ControlKind::Lookup`-Branch)

- [ ] **Step 1: Branch lesen und Plan-Patch vorbereiten**

Run: `cat -n client/src/components/field/mod.rs | sed -n '462,467p'`
Expected: Branch zeigt heute den "complex"-Placeholder.

- [ ] **Step 2: Branch durch `ReferencePicker` ersetzen**

`client/src/components/field/mod.rs` Z. 462–467 ersetzen:

```rust
ControlKind::Lookup => {
    let entity = match &meta.field_type {
        FieldType::Reference { entity, .. } => entity.clone(),
        _ => String::new(),
    };
    if entity.is_empty() {
        view! {
            <span style="color: #6b7280; font-style: italic;">
                {move || crate::i18n::t("editor.placeholder.complex")}
            </span>
        }
        .into_any()
    } else {
        let initial: Option<String> = match &value {
            Value::String(s) if !s.is_empty() => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        };
        let value_signal = RwSignal::new(initial);
        let on_pick = Callback::new(move |v: Option<String>| {
            value_signal.set(v.clone());
            let json = v.map(Value::String).unwrap_or(Value::Null);
            on_change_value.run(json);
        });
        view! {
            <crate::components::reference_picker::ReferencePicker
                entity_type=entity
                value=value_signal.into()
                on_change=on_pick
                readonly=readonly
            />
        }
        .into_any()
    }
}
```

- [ ] **Step 3: Build prüfen**

Run: `cargo check -p client --target wasm32-unknown-unknown`
Expected: OK.

- [ ] **Step 4: Manuell smoke-testen**

Run: zwei Terminals
- `cargo run -p server -- --data-dir ./examples/shop`
- `cd client && trunk serve`

Browser: http://localhost:8080 → Produkt-Liste → ein Produkt editieren → Kategorie-Feld sollte jetzt einen Picker mit Suche zeigen (statt "complex"-Placeholder). Beim Tippen "werk" erscheinen passende Kategorien.

Bei Abweichung: Konsole/Server-Log prüfen, Resolver-Fehler verifizieren.

- [ ] **Step 5: Commit**

```bash
git add client/src/components/field/mod.rs
git commit -m "feat(client): ControlKind::Lookup uses ReferencePicker"
```

---

### Task 7: Display-Label-Cache + List-View-Auflösung

**Files:**
- Create: `client/src/components/reference_cache.rs`
- Modify: `client/src/components/mod.rs`
- Modify: `client/src/components/field/mod.rs` (`render_reference`-Aufruf-Stelle Z. 126–131)
- Modify: `client/src/app.rs` (Cache-Provider im App-Root)

- [ ] **Step 1: Cache-Modul anlegen**

`client/src/components/reference_cache.rs`:

```rust
//! Display-Label-Cache fuer Reference-Felder in Listen-Views.
//!
//! Pro `(entity_type, id)`-Paar wird das aufgeloeste Display gehalten.
//! Cache-Misses triggern asynchrone Lookups ueber
//! `fetch_reference_candidates(entity_type, id, 1)` mit Single-ID-Suche.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use parking_lot::Mutex;

use crate::graphql::queries::fetch_reference_candidates;

#[derive(Default)]
struct CacheInner {
    map: HashMap<(String, String), String>,
    pending: HashSet<(String, String)>,
}

#[derive(Clone, Default)]
pub struct ReferenceDisplayCache {
    inner: Arc<Mutex<CacheInner>>,
    /// Signal-Tick, damit Consumer reaktiv re-rendern.
    pub tick: RwSignal<u64>,
}

impl ReferenceDisplayCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Liefert das Display synchron, wenn schon im Cache. Sonst startet
    /// einen Lookup im Hintergrund und gibt `None` zurueck.
    pub fn lookup(&self, entity_type: &str, id: &str) -> Option<String> {
        let key = (entity_type.to_string(), id.to_string());
        let mut g = self.inner.lock();
        if let Some(v) = g.map.get(&key) {
            return Some(v.clone());
        }
        if g.pending.insert(key.clone()) {
            drop(g);
            let this = self.clone();
            let et = key.0.clone();
            let id = key.1.clone();
            spawn_local(async move {
                let result = fetch_reference_candidates(&et, &id, 1).await;
                let mut g = this.inner.lock();
                g.pending.remove(&key);
                if let Ok(list) = result {
                    if let Some(found) = list.into_iter().find(|c| c.id == id) {
                        g.map.insert(key, found.display);
                        this.tick.update(|v| *v = v.wrapping_add(1));
                    }
                }
            });
        }
        None
    }
}

pub fn provide_reference_cache() {
    provide_context(ReferenceDisplayCache::new());
}

pub fn use_reference_cache() -> ReferenceDisplayCache {
    use_context::<ReferenceDisplayCache>()
        .expect("kein ReferenceDisplayCache im Context (provide_reference_cache fehlt?)")
}
```

- [ ] **Step 2: `parking_lot` als Client-Dependency hinzufügen (wenn nicht schon vorhanden)**

Run: `grep -n "parking_lot" client/Cargo.toml`
Wenn nicht vorhanden: in `client/Cargo.toml` unter `[dependencies]` ergänzen:
```toml
parking_lot = "0.12"
```

- [ ] **Step 3: Modul registrieren + Provider im App-Root**

`client/src/components/mod.rs`: `pub mod reference_cache;`

`client/src/app.rs`: in `App`-Komponente, neben `provide_design_system()` und `provide_field_registry()`:

```rust
crate::components::reference_cache::provide_reference_cache();
```

- [ ] **Step 4: `render_reference` umstellen**

`client/src/components/field/mod.rs` Z. 126–131 — der `Reference`-Match-Arm in `DefaultFieldRegistry::render` muss jetzt das Entity-Type und das Template kennen. Dazu den Helper anpassen:

```rust
// vorher:
(FieldType::Reference { .. }, Value::String(s)) if !s.is_empty() => {
    render_reference(s)
}
(FieldType::Reference { .. }, Value::Number(n)) => render_reference(&n.to_string()),

// nachher:
(FieldType::Reference { entity, display_template, .. }, Value::String(s)) if !s.is_empty() => {
    render_reference_with_template(entity, display_template.as_deref(), s)
}
(FieldType::Reference { entity, display_template, .. }, Value::Number(n)) => {
    render_reference_with_template(entity, display_template.as_deref(), &n.to_string())
}
```

Und neuen Helper am Ende der Datei (oder neben `render_reference`):

```rust
fn render_reference_with_template(
    entity_type: &str,
    template: Option<&str>,
    id: &str,
) -> AnyView {
    if template.is_none() {
        return render_reference(id);
    }
    let cache = crate::components::reference_cache::use_reference_cache();
    let entity = entity_type.to_string();
    let id_owned = id.to_string();
    let id_for_view = id.to_string();
    view! {
        <span>
            {move || {
                // Re-render bei tick.
                cache.tick.get();
                match cache.lookup(&entity, &id_owned) {
                    Some(display) => display,
                    None => id_for_view.clone(),
                }
            }}
        </span>
    }
    .into_any()
}
```

- [ ] **Step 5: Build prüfen**

Run: `cargo check -p client --target wasm32-unknown-unknown`
Expected: OK.

- [ ] **Step 6: Smoke-Test im Browser**

Run: Server + Trunk wie in Task 6.

Browser: Produkt-Liste sollte in der `category`-Spalte nicht mehr `category-1` (Mono-Style) zeigen, sondern den `name` der Kategorie. Beim ersten Render kurzer "ID-Flash", dann erscheint das Display.

- [ ] **Step 7: Commit**

```bash
git add client/src/components/reference_cache.rs client/src/components/mod.rs client/src/components/field/mod.rs client/src/app.rs client/Cargo.toml
git commit -m "feat(client): reference display cache resolves FK IDs in list view"
```

---

### Task 8: Workspace-Test-Run + Cleanup

- [ ] **Step 1: Alle Tests grün**

Run: `cargo test --workspace --target-dir target-test`
Expected: alle Tests PASS. Bei Fehlschlag: zurück zur betroffenen Task, Schritt 4/6 prüfen.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --target-dir target-test -- -D warnings`
Expected: keine Warnungen.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`

- [ ] **Step 4: Commit (falls fmt etwas geändert hat)**

```bash
git add -A
git diff --cached --quiet || git commit -m "chore: cargo fmt after U1 reference picker"
```

- [ ] **Step 5: Final-Status**

Run: `git log --oneline -10`
Expected: 7 neue Commits (Task 1–7) plus optional fmt.

---

## Self-Review-Notiz

**Spec-Coverage** (gegen `2026-05-20-d2v-on-dblicious-gap-analysis.md` §4.1 U1):
- ✅ `FieldType::Reference { entity, display_template, search_columns }` → Task 1
- ✅ Client-Component mit paginierter Suche → Task 5
- ✅ Read-only-Display-Auflösung → Task 7
- ✅ Verbindet sich mit `ControlKind::Lookup` → Task 6
- 🚩 **Nicht abgedeckt** (mit Absicht): Phase-1.5-Picker-Registry-Integration, Composite-PK, Multi-Reference. Sind in §4.1 explizit ausgeklammert.

**Type-Konsistenz**:
- `FieldType::Reference`-Felder einheitlich `entity` / `display_template` / `search_columns` in allen Aufrufstellen.
- `ReferenceCandidate` Felder `id` / `display` / `fields` durchgängig (camelCase auf Wire).
- `fetch_reference_candidates(&str, &str, i32)` Signatur identisch in queries.rs und reference_picker.rs.

**Risiken**:
- Server-Resolver heuristik "first column with Reference + display_template" trifft an mehrdeutigen Schemata daneben. Akzeptabel als V1 — wenn der Use Case auftaucht (Account hat zwei Reference-Templates auf sich selbst), bekommt der Resolver einen `property`-Parameter.
- Cache hat keinen LRU/Cap heute — bei sehr großen Listen wächst der Speicher. Acceptable in V1; Cap kann später ergänzt werden.
- `fetch_reference_candidates(entity_type, id, 1)` zur Display-Auflösung ist nicht effizient (Volltextsuche statt direktem `entityById`). Optimierung: Batch-Resolver oder eigener `referenceById`-Endpoint — Backlog.

**Placeholder-Scan**: keine TBD/TODO/„später implementieren" im Plan.

---

## Plan-Abschluss

Plan complete and saved to `docs/superpowers/plans/2026-05-20-u1-reference-picker.md`. Two execution options:

**1. Subagent-Driven (recommended)** — ich dispatche pro Task einen frischen Subagent, mit Review-Punkt zwischen den Tasks. Fast Iteration, kontextisoliert.

**2. Inline Execution** — Tasks in dieser Session ausführen, Batch mit Checkpoints für Review.

Which approach?
