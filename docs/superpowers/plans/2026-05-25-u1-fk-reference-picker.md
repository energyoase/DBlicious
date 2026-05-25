# U1 FK-Referenz-Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reference-Felder zeigen ein lesbares Label statt der ID (server-aufgelöst, page-level) und werden über einen suchbaren Picker editierbar — demonstriert an `examples/shop` `order→customer`.

**Architecture:** Track A (Display): Ziel-Entity deklariert `display_field` in `EntitySettings`; ein pluggbarer `ServerEmbedResolver` füllt `EntityPage.reference_labels` (page-level Map `{id → {col → label}}`) beim Servieren; Client rendert das Label (Fallback ID). Track B (Picker): ein neues Client-Control ersetzt den `ControlKind::Lookup`-Placeholder, sucht via `fetch_entities` über die Ziel-Entity (client-gefiltert über `display_field`) und setzt die FK-id. A vor B.

**Tech Stack:** Rust, async-graphql (Server `SimpleObject`/`Json`), Leptos/WASM (Client), serde_json. Tests isoliert: `--target-dir target-u1<x> -j 2`, Exit via `; echo "EXIT=$?"`, nie `| tail`; Commits mit explizitem Pathspec.

**Spec:** `docs/superpowers/specs/2026-05-25-u1-fk-reference-picker-design.md`.

**Konventionen (Repo):** Parallel-Sessions → isolierte target-dirs, nie shared `target`/`target-test`; Korruptions-Fehler (`invalid metadata`/`LNK1102`/spurious type errors) sind environment → fresh `--target-dir`. Commit-Trailer: `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`. DesignSystem (`use_design()`) für Client-Styling, kein Hard-CSS. `cargo test -p <crate>` (nicht nur `--lib` — kompiliert `tests/`).

---

## File Structure

- **Modify** `shared/src/settings.rs` → `EntitySettings.display_field: Option<String>`.
- **Modify** `shared/src/lib.rs` → `EntityPage.reference_labels` (page-level Map).
- **Modify** `shared/tests/` → wire-tests für beide.
- **Create** `server/src/reference.rs` → `ReferenceResolutionStrategy` enum + `resolve_reference_labels(...)` (ServerEmbed-Impl).
- **Modify** `server/src/lib.rs` → `pub mod reference;`.
- **Modify** `server/src/data.rs` → `entities_page_raw` ruft den Resolver für den Page-Slice; `EntityPage`-Konstruktion bekommt `reference_labels`.
- **Modify** `server/src/schema.rs` → Gql-`EntityPage.reference_labels: Json<serde_json::Value>`; der `entities`-Resolver reicht es durch.
- **Modify** `client/src/graphql/queries.rs` → `ENTITIES_QUERY` + Decode-Structs + `EntityPageResult.reference_labels`.
- **Modify** `client/src/components/table/` (Tabelle → `FieldCell`) + `client/src/components/field/mod.rs` → `reference_labels` durchreichen, Reference-Display-Arm, Lookup-Control ersetzen.
- **Create** `client/src/components/field/reference_picker.rs` → der Picker.
- **Modify** `examples/shop/entities/customer/settings.json` → `displayField`.
- **Test** `server/tests/`, `client/...`, `server/tests/loader.rs`.

---

## Track A — Read-time Display-Label

### Task A1: shared — `display_field` + `EntityPage.reference_labels`

**Files:**
- Modify: `shared/src/settings.rs:121-156` (`EntitySettings`)
- Modify: `shared/src/lib.rs:308-315` (`EntityPage`)
- Test: `shared/tests/settings_wire.rs` (neu oder bestehende settings-Testdatei), `shared/tests/` für EntityPage

- [ ] **Step 1: Failing test — `displayField` + `referenceLabels` wire**

Neue Datei `shared/tests/reference_wire_format.rs`:
```rust
use shared::{EntityPage, EntitySettings};

#[test]
fn entity_settings_parses_display_field() {
    let s: EntitySettings =
        serde_json::from_value(serde_json::json!({ "displayField": "displayName" })).unwrap();
    assert_eq!(s.display_field.as_deref(), Some("displayName"));
}

#[test]
fn entity_page_reference_labels_roundtrip() {
    let mut labels = std::collections::BTreeMap::new();
    let mut row = std::collections::BTreeMap::new();
    row.insert("customer".to_string(), "Max M.".to_string());
    labels.insert("order-1".to_string(), row);
    let page = EntityPage {
        items: vec![],
        total_count: 0,
        page: 1,
        page_size: 50,
        reference_labels: labels,
    };
    let json = serde_json::to_string(&page).unwrap();
    assert!(json.contains("referenceLabels"));
    let back: EntityPage = serde_json::from_str(&json).unwrap();
    assert_eq!(back.reference_labels["order-1"]["customer"], "Max M.");
}

#[test]
fn entity_page_without_reference_labels_defaults_empty() {
    // Rückwärtskompatibel: alte Payload ohne das Feld.
    let page: EntityPage =
        serde_json::from_value(serde_json::json!({
            "items": [], "totalCount": 0, "page": 1, "pageSize": 50
        })).unwrap();
    assert!(page.reference_labels.is_empty());
}
```

- [ ] **Step 2: Run, expect FAIL** (`display_field`/`reference_labels` Felder fehlen):
`cargo test -p shared --test reference_wire_format --target-dir target-u1a -j 2 2>&1 | rg "test result|error|FAILED"; echo "EXIT=${PIPESTATUS[0]}"`

- [ ] **Step 3: `display_field` zu `EntitySettings`** (`shared/src/settings.rs`, nach `default_filter`, vor `properties`):
```rust
    /// Schlüssel der Spalte dieser Entity, die als Anzeige-Label dient, wenn
    /// ein anderes Feld via `FieldType::Reference` auf sie verweist (z.B.
    /// customer → "displayName"). None → Reference-Anzeige fällt auf die ID
    /// zurück. Einzelfeld; Mehrfeld-Template ist eine spätere Erweiterung.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_field: Option<String>,
```

- [ ] **Step 4: `reference_labels` zu `EntityPage`** (`shared/src/lib.rs`, im Struct nach `page_size`):
```rust
    /// Pro Datensatz-id eine Map {reference-Spalten-key → aufgelöstes Label}.
    /// Page-level, damit der Kern-`Entity`-Typ (auch für create/update) frei
    /// bleibt. Fehlt ein Eintrag, fällt der Client auf die ID zurück.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub reference_labels:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
```
(Prüfen: `EntityPage` hat `#[serde(rename_all = "camelCase")]` → `referenceLabels`/`totalCount` etc. — ja, Zeile 309.) **Alle bestehenden `EntityPage { .. }`-Konstruktionen** im Workspace bekommen `reference_labels: Default::default()` (rg findet sie: `server/src/data.rs:278`, evtl. Tests). In dieser Task nur kompilierbar machen; die echte Füllung kommt in A2.

- [ ] **Step 5: Run, expect PASS** + `cargo build -p shared --target-dir target-u1a -j 2` grün.
`cargo test -p shared --test reference_wire_format --target-dir target-u1a -j 2 2>&1 | rg "test result|FAILED"; echo "EXIT=${PIPESTATUS[0]}"` → 3 passed.

- [ ] **Step 6: Commit**
```bash
git commit -m "feat(shared): EntitySettings.display_field + EntityPage.reference_labels

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>" -- shared/src/settings.rs shared/src/lib.rs shared/tests/reference_wire_format.rs
```
(Plus die Aufrufer-Dateien mit `reference_labels: Default::default()`.)

### Task A2: server — `ServerEmbedResolver` + Gql-Durchreichung

**Files:**
- Create: `server/src/reference.rs`
- Modify: `server/src/lib.rs` (`pub mod reference;`)
- Modify: `server/src/data.rs:238-284` (`entities_page_raw`)
- Modify: `server/src/schema.rs:50-54` (Gql-`EntityPage`) + der `entities`-Resolver (`schema.rs:839`)
- Test: `server/tests/reference_resolver.rs`

- [ ] **Step 1: Failing test — Resolver embedded Label für order→customer, lässt category aus**

`server/tests/reference_resolver.rs`:
```rust
//! ServerEmbed-Resolver: Reference-Labels für eine Entity-Seite.
use server::lib_test_prelude::*; // siehe vorhandene server-Test-Setups; sonst direkter Pfad

#[tokio::test]
#[serial_test::serial]
async fn server_embeds_customer_label_for_order_page() {
    server::fresh_test_setup_with("examples/shop").await; // an vorhandenes shop-Setup anpassen
    // order seed referenziert customer-<x>; customer hat displayField=displayName.
    let page = server::data::entities_page_raw("order", 1, 50, None, Default::default()).await;
    // mind. ein order mit gesetztem customer:
    let some = page.items.iter().find(|e| e.fields.get("customer").is_some()).unwrap();
    let label = page.reference_labels.get(&some.id).and_then(|m| m.get("customer"));
    assert!(label.is_some(), "customer-Label muss aufgelöst sein");
    // product→category ist hängend (keine category-Entity) → kein Label
    let page2 = server::data::entities_page_raw("product", 1, 50, None, Default::default()).await;
    let any_cat = page2.reference_labels.values().any(|m| m.contains_key("category"));
    assert!(!any_cat, "hängender category-Ref darf kein Label haben");
}
```
ZUERST das echte shop-Test-Setup + die `entities_page_raw`-Sichtbarkeit prüfen (`server/src/lib.rs` Test-Entrypoint, `setup_for_tests`); Test exakt daran anpassen. Falls `entities_page_raw` nicht `pub`: über den GraphQL-`entities`-Resolver testen oder Sichtbarkeit auf `pub(crate)`+Test-Helper.

- [ ] **Step 2: Run, expect FAIL** (reference_labels leer):
`cargo test -p server --test reference_resolver --target-dir target-u1a2 -j 2 2>&1 | rg "test result|FAILED|error"; echo "EXIT=${PIPESTATUS[0]}"`

- [ ] **Step 3: `server/src/reference.rs`**:
```rust
//! Reference-Label-Resolution (U1). Pluggbare Strategie; heute nur
//! `ServerEmbed`: beim Servieren einer Entity-Seite das Anzeige-Label der
//! referenzierten Zielzeile (deren `EntitySettings.display_field`) je
//! Reference-Spalte einbetten.
use std::collections::BTreeMap;
use shared::FieldType;

/// Wahl der Resolution-Strategie. Default `ServerEmbed`. `ClientBatch` +
/// `PerCell` sind dokumentierte Folge-Strategien (Seam) — heute nicht gebaut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferenceResolutionStrategy {
    #[default]
    ServerEmbed,
    ClientBatch,
    PerCell,
}

/// Liefert {row_id → {col_key → label}} für die gegebenen Zeilen.
/// `fetch_target` lädt alle Zeilen einer Ziel-Entity (id → fields-Map);
/// `display_field_of` liefert das display_field einer Ziel-Entity.
pub fn resolve_server_embed(
    rows: &[shared::Entity],
    columns: &[shared::ColumnMeta],
    fetch_target: &mut dyn FnMut(&str) -> BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    display_field_of: &dyn Fn(&str) -> Option<String>,
) -> BTreeMap<String, BTreeMap<String, String>> {
    // Reference-Spalten der Quelle bestimmen.
    let refs: Vec<(&str, &str)> = columns
        .iter()
        .filter_map(|c| match &c.field_type {
            FieldType::Reference { entity } => Some((c.key.as_str(), entity.as_str())),
            _ => None,
        })
        .collect();
    if refs.is_empty() {
        return BTreeMap::new();
    }
    // Pro Ziel-Entity einmal laden (Batch) + display_field cachen.
    let mut target_cache: BTreeMap<String, BTreeMap<String, serde_json::Map<String, serde_json::Value>>> = BTreeMap::new();
    let mut df_cache: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for row in rows {
        for (col, target) in &refs {
            let Some(fk) = row.fields.get(*col).and_then(|v| v.as_str()) else { continue };
            let target_rows = target_cache
                .entry((*target).to_string())
                .or_insert_with(|| fetch_target(target));
            let df = df_cache
                .entry((*target).to_string())
                .or_insert_with(|| display_field_of(target));
            let Some(df) = df.as_deref() else { continue };
            let Some(trow) = target_rows.get(fk) else { continue };
            let Some(label) = trow.get(df).and_then(|v| v.as_str()) else { continue };
            out.entry(row.id.clone()).or_default().insert((*col).to_string(), label.to_string());
        }
    }
    out
}
```
Inline-Unit-Tests dazu (reine Funktion, ohne DB): ein `rows`+`columns`+Fake-`fetch_target`/`display_field_of` → erwartete Map; hängendes Ziel (fetch liefert leere Map) → kein Eintrag; df=None → kein Eintrag.

- [ ] **Step 4: In `data.rs::entities_page_raw` verdrahten.** Nach `let slice = ...;` (Zeile ~272-276), vor dem `shared::EntityPage { .. }`:
```rust
    let reference_labels = crate::reference::resolve_server_embed(
        &slice,
        &columns,
        &mut |target| {
            // Batch: alle Zeilen der Ziel-Entity als id→fields.
            // (reuse der generischen Auslese; synchron via block-in-place
            //  ist hier nicht nötig — entities_page_raw ist async, also
            //  die Ziel-Rows vorab laden: siehe unten.)
            unimplemented!("siehe Hinweis")
        },
        &|target| crate::data::display_field_for(target),
    );
```
HINWEIS zur async-Falle: `resolve_server_embed` ist synchron, aber das Ziel-Laden ist async (DB). Lösung: **vor** dem Aufruf die distinkten Ziel-Entities aus den Reference-Spalten bestimmen, ihre Rows async laden (`entity::entities::find().filter(EntityType.eq(target)).all()` → id→fields-Map), in eine `BTreeMap<String, BTreeMap<id, fields>>` legen, und `fetch_target` als Closure über diese vorab-geladene Map bauen (kein async im Closure). `display_field_for(target)` = sync Lookup über `settings_for(target).and_then(|s| s.display_field)` (sync-Variante; `settings_for_async` existiert — eine sync `settings_for` aus dem installierten Set nutzen, vgl. `columns_for`). Den genauen sync-Settings-Zugriff am vorhandenen `crate::example::current()`-Pfad orientieren (wie `shared_columns_for`).
Dann `reference_labels` in die `EntityPage`-Konstruktion (Zeile 278) aufnehmen.

- [ ] **Step 5: Gql-Durchreichung.** `schema.rs:50` `EntityPage` SimpleObject bekommt:
```rust
    pub reference_labels: async_graphql::Json<serde_json::Value>,
```
Im `entities`-Resolver (schema.rs:839), wo die shared-`EntityPage` in die Gql-`EntityPage` gemappt wird: `reference_labels: Json(serde_json::to_value(&shared_page.reference_labels).unwrap_or_default())`. (Den Mapping-Punkt lesen + einfügen.)

- [ ] **Step 6: Run tests** (Resolver-Inline + Integration + bestehende server-Tests grün):
`cargo test -p server --test reference_resolver --target-dir target-u1a2 -j 2 2>&1 | rg "test result|FAILED|^error"; echo "EXIT=${PIPESTATUS[0]}"` → ok.
`cargo test -p server --lib reference --target-dir target-u1a2 -j 2 2>&1 | rg "test result|FAILED"; echo "EXIT=${PIPESTATUS[0]}"` → Inline-Tests ok.

- [ ] **Step 7: Commit** (Pathspec: reference.rs, lib.rs, data.rs, schema.rs, test).

### Task A3: client — Decode + Reference-Display

**Files:**
- Modify: `client/src/graphql/queries.rs` (`ENTITIES_QUERY` ~ vor Zeile 177; Decode-Structs `EntitiesData`/`RawEntityPage`; `EntityPageResult`)
- Modify: `client/src/components/table/` (Tabelle reicht `reference_labels` an `FieldCell`) + `client/src/components/field/mod.rs` (`FieldContext` + Reference-Arm)
- Test: client inline

- [ ] **Step 1: `ENTITIES_QUERY` + Decode um `referenceLabels`.** Den `ENTITIES_QUERY`-String (Konstante, die `fetch_entities` nutzt) um das Feld `referenceLabels` erweitern; die Response-Decode-Structs (das `entities`-Objekt) um `reference_labels: serde_json::Value` (camelCase via serde) ergänzen; `EntityPageResult` (Rückgabe von `fetch_entities`) bekommt `pub reference_labels: std::collections::BTreeMap<String, std::collections::BTreeMap<String,String>>`, befüllt aus dem decodeten Value (`serde_json::from_value(...).unwrap_or_default()`). Den `EntityPageResult`-Konstruktionspunkt (Zeile 203) erweitern.

- [ ] **Step 2: `reference_labels` an `FieldCell` durchreichen.** Die Tabelle (`components/table/`, wo `EntityPageResult` → Rows → `FieldCell` läuft; die `RemoteSource`/`DataSource` liefert die Page) reicht die Page-`reference_labels` + die row-id an `FieldCell` (neue Prop `#[prop(default = None)] reference_label: Option<String>` — das bereits aufgelöste Label für GENAU diese Zelle, vom Aufrufer als `reference_labels.get(row_id).and_then(|m| m.get(col_key)).cloned()` ermittelt; hält `FieldCell` einfach). Der Tabellen-Row-Aufrufer (analog dem `formatter_id`-Threading aus der Provider-Welle, `table_view.rs`/`view.rs`) berechnet das pro Zelle.

- [ ] **Step 3: Reference-Display-Arm.** In `client/src/components/field/mod.rs::DefaultFieldRegistry::render`, der Reference-Arm (`(FieldType::Reference { .. }, Value::String(s)) if !s.is_empty() => render_reference(s)`, Zeile 139): wenn ein `reference_label` (über `FieldContext`/Prop) vorhanden ist → `render_text(label)`; sonst `render_reference(id)` (Fallback). `FieldContext` um `reference_label: Option<&str>` erweitern und `FieldCell` setzt es.

- [ ] **Step 4: Build + tests** (kein Regress; Reference ohne Label = ID wie bisher):
`cargo build -p client --target-dir target-u1a3 -j 2 2>&1 | tail -3; echo "EXIT=${PIPESTATUS[0]}"` → Finished.
`cargo test -p client --target-dir target-u1a3 -j 2 2>&1 | rg "test result|FAILED|^error"; echo "EXIT=${PIPESTATUS[0]}"` → ok. Inline-Test, dass der Reference-Arm bei gesetztem Label `render_text` wählt (soweit ohne Leptos-Render prüfbar; sonst Build + Browser in Schluss-Task).

- [ ] **Step 5: Commit.**

### Task A4: shop-Config + Loader-Pin

**Files:**
- Modify: `examples/shop/entities/customer/settings.json`
- Test: `server/tests/loader.rs`

- [ ] **Step 1: `displayField` an customer.** `examples/shop/entities/customer/settings.json`: `"displayField": "displayName"` ergänzen (Schema gegen die echte settings.json prüfen; das Feld liegt top-level neben `entityType`/`access`). (Optional `order`/`product` unverändert.)

- [ ] **Step 2: Loader-Pin.** In `server/tests/loader.rs` (oder dem shop-Loader-Test) einen Test: `example::load("examples/shop")` → `customer`-Settings → `display_field == Some("displayName")`. Den exakten Set-/Settings-Zugriffspfad am vorhandenen Loader-Test orientieren.

- [ ] **Step 3: Run + Commit.**
`cargo test -p server --test loader --target-dir target-u1a4 -j 2 2>&1 | rg "test result|FAILED|displayField|customer"; echo "EXIT=${PIPESTATUS[0]}"` → ok.

---

## Track B — Editor-Picker

### Task B1: client — `ReferencePicker` ersetzt Lookup-Placeholder

**Files:**
- Create: `client/src/components/field/reference_picker.rs`
- Modify: `client/src/components/field/mod.rs` (Lookup-Arm → Picker; `mod reference_picker;`)
- Test: client inline (Filter-Logik rein) + Browser (manuell)

- [ ] **Step 1: Reine Filter-Funktion + Test.** In `reference_picker.rs` eine pure Hilfsfunktion:
```rust
/// Filtert Kandidaten-Zeilen client-seitig über das display_field
/// (case-insensitive contains). Leerer Query → alle.
pub fn filter_candidates<'a>(
    rows: &'a [shared::Entity],
    display_field: &str,
    query: &str,
) -> Vec<&'a shared::Entity> {
    let q = query.trim().to_lowercase();
    rows.iter()
        .filter(|e| {
            q.is_empty()
                || e.fields
                    .get(display_field)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase().contains(&q))
                    .unwrap_or(false)
        })
        .collect()
}
```
Inline-Test: 3 Kandidaten mit `displayName`, Query "max" → nur passende; leerer Query → alle; fehlendes display_field-Feld → nicht gematcht.

- [ ] **Step 2: Run, verify PASS** (pure fn):
`cargo test -p client --lib reference_picker --target-dir target-u1b -j 2 2>&1 | rg "test result|FAILED"; echo "EXIT=${PIPESTATUS[0]}"`

- [ ] **Step 3: Picker-Komponente.** `#[component] fn ReferencePicker(target_entity: String, current_id: Option<String>, on_change: Callback<Value>)`:
  - Ein `LocalResource`/`Resource` lädt einmal `target_entity`s Settings → `display_field` (über den vorhandenen Settings-Fetch; Fallback erste Textspalte via `fetch_columns`) UND eine Kandidaten-Seite via `fetch_entities(target_entity, 1, <reasonable page_size z.B. 200>)`.
  - Sucheingabe (`RwSignal<String>`) → `filter_candidates(&rows, &display_field, &query)`.
  - Liste der gefilterten Kandidaten als anklickbare Optionen (Label = `display_field`-Wert); Klick → `on_change.run(Value::String(id))` + Anzeige des gewählten Labels.
  - Styling über `use_design()`; leere Trefferliste → `i18n::t("picker.no_results")` (Key in beiden FTL ergänzen, de/en).
  - Bereits gesetzte `current_id` ohne geladenes Label → id zeigen.

- [ ] **Step 4: Lookup-Arm ersetzen.** In `field/mod.rs` (der Editor-`control_view`-`match`, `ControlKind::Lookup`-Arm, ~Zeile 477-482): statt des `editor.placeholder.complex`-Spans den `ReferencePicker` rendern, mit `target_entity` aus `meta.field_type` (`FieldType::Reference { entity }`), `current_id` aus dem aktuellen Wert, `on_change` = `on_change_value`.

- [ ] **Step 5: Build (host + wasm) + tests:**
`cargo build -p client --target wasm32-unknown-unknown --target-dir target-u1bw -j 2 2>&1 | tail -3; echo "EXIT=${PIPESTATUS[0]}"` → Finished.
`cargo test -p client --target-dir target-u1b -j 2 2>&1 | rg "test result|FAILED|^error"; echo "EXIT=${PIPESTATUS[0]}"` → ok.

- [ ] **Step 6: Commit.**

---

## Verifikation (Schluss)

- [ ] **Workspace grün (isoliert):** `cargo test --workspace --target-dir target-u1final -j 2 2>&1 | rg "test result: FAILED|^error|FAILED|test result: ok" | tail -40; echo "EXIT=${PIPESTATUS[0]}"` → nur ok, EXIT=0.
- [ ] **Roundtrip-Bewusstsein (built-but-unwired-Lehre):** sicherstellen, dass `reference_labels` wirklich über GraphQL beim Client ankommt (A2/A3) — ein Test, der den `entities`-Resolver-Output (Gql `referenceLabels`) prüft ODER `EntityPageResult.reference_labels` aus einem echten Query-Decode. Nicht nur die shared-/data-Ebene.
- [ ] **Manueller Browser-Augenschein** (nicht automatisierbar, explizit so berichten): `cargo run -p server -- --data-dir ./examples/shop` + `trunk serve`; order-Liste → Spalte „Customer" zeigt den Kundennamen (nicht die id); order-Editor → Reference-Feld ist ein suchbarer Picker, Auswahl setzt den Kunden; `product→category` zeigt weiterhin die id (Fallback).
- [ ] **finishing-a-development-branch** aufrufen. Branch `dev` (shared) — Push/Promote separate User-Entscheidung.

---

## Self-Review

**Spec-Coverage:** §2.1 display_field → A1; §2.2 reference_labels → A1+A2(gql)+A3(decode); §2.3 ServerEmbed-Resolver+Seam → A2; §2.4 Client-Display → A3; §2.5 Picker → B1; §4 Fehler/Fallback → A3(ID-Fallback)+B1(leer/Fehler); §5 Tests → je Task + Schluss-Verifikation; §7 Decisions (page-level, target-defined, ServerEmbed-only, client-filter, shop) → durchgängig abgebildet. ✅

**Placeholder-Scan:** Zwei Stellen mit „am vorhandenen X orientieren / Pfad prüfen" (server-Test-Setup, sync-Settings-Zugriff, ENTITIES_QUERY-Stelle, Tabellen-Plumbing) — das sind reale, gegen den Code zu verifizierende Anker (Sichtbarkeiten/Setups, die ich nicht raten will), jeweils mit konkretem Datei-/Funktionsbezug, kein Hand-wave. Die neuen Kern-Logiken (resolve_server_embed, filter_candidates, Wire-Typen, Tests) sind voll ausgeschrieben. Die async-Falle in A2-Step-4 ist explizit aufgelöst (Ziel-Rows vorab async laden, Closure synchron).

**Typ-Konsistenz:** `EntityPage.reference_labels: BTreeMap<String, BTreeMap<String,String>>` einheitlich in A1/A2/A3; `display_field: Option<String>` einheitlich A1/A2/A4; `filter_candidates`-Signatur B1 konsistent genutzt. `EntityPage`-Felder (`items`/`total_count`/`page`/`page_size`) korrekt (nicht „entities"/„total"). ✅

**Bewusste Abweichung von der Spec-Wortwahl:** Spec §2.2 schrieb beispielhaft `entities`; reales Feld ist `items` — Plan nutzt durchgängig `items`.
