# U1 FK-Referenz-Picker — Design

Date: 2026-05-25
Status: Draft — awaiting user review
Welle: Track-D UI-Pattern U1 (Gap-Analyse `2026-05-24-d2v-script-first-gap-analysis.md` §5.1, höchster Hebel §7 #1).

## 0. Problem

`FieldType::Reference { entity }` zeigt heute nur die rohe **ID** (`client/src/components/field/mod.rs::render_reference`), und das Editor-Control ist ein **nicht-editierbarer Placeholder** (`ControlKind::Lookup` → `editor.placeholder.complex`). Damit ist keine FK-Auswahl in Formularen möglich und Tabellen zeigen `category-1` statt `Werkzeug`. U1 macht Reference-Felder **anzeigbar** (lesbares Label) und **editierbar** (suchbarer Picker). `picker.rs` unter `client/src/components/registries/` ist der **Implementations-Picker** (Phase 1.5) — NICHT der FK-Picker; nicht wiederverwenden.

## 1. Scope

**In dieser Welle (beide Teil-Tracks, in EINEM Plan, A vor B):**
- **A — read-time Display-Label:** Ziel-Entity deklariert ihr `display_field`; Server löst pro Reference-Zelle das Label auf und bettet es ein; Client rendert es (Fallback: ID).
- **B — Editor-Picker:** Reference-Editor wird ein suchbares Control (`fetch_entities` über die Ziel-Entity, client-gefiltert über `display_field`), das die FK-id setzt.

**Test-Bett:** `examples/shop` — `order.customer` → `customer` (existiert, `displayName`) ist der auflösbare Fall; `product.category` → `category` (KEINE category-Entity geladen) ist der hängende-Ref-Fallback.

**Bewusst NICHT (Folge-Schritte):**
- Client-Batch- und Per-Zelle-Resolution-Strategien (nur die Seams + Doku; `ServerEmbed` ist die einzige Impl dieser Welle). Per-Zelle ist Low-Prio-Einzelfall-Fallback.
- Server-seitiger Filter für die Picker-Suche (Server ignoriert Filter heute; die Picker-Suche filtert client-seitig über die gefetchte Kandidaten-Seite — für kleine Ref-Ziele ausreichend). Server-Filter = spätere Verbesserung (verwandt mit dem „Filtering aktivieren"-Gap aus der Provider-Render-Welle).
- `display_template` mit Mehrfeld-Interpolation (`"{a} ({b})"`); diese Welle: **einzelnes** `display_field`. Template ist dokumentierte Erweiterung.
- d2v-FKs (heute `integer`, z.B. `primaryAccountNr`) als `Reference` modellieren — separater Schicht-3-Config-Schritt.

## 2. Architektur

### 2.1 shared — `display_field` (settings.rs)

`EntitySettings` (`shared/src/settings.rs:121`) bekommt:
```rust
/// Schlüssel der Spalte dieser Entity, die als Anzeige-Label dient, wenn
/// ein anderes Feld via FieldType::Reference auf sie verweist (z.B.
/// customer → "displayName"). None → Reference-Anzeige fällt auf die ID
/// zurück. Einzelfeld; Mehrfeld-Template ist eine spätere Erweiterung.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub display_field: Option<String>,
```
Wire: camelCase `displayField`. `Reference { entity }` bleibt unverändert.

### 2.2 shared — Label-Transport (`EntityPage.reference_labels`)

`EntityPage` (heute `entities`, `total`, …) bekommt:
```rust
/// Pro Datensatz-id eine Map {reference-Spalten-key → aufgelöstes Label}.
/// Page-level (statt am Kern-`Entity`-Typ), um den Wire-Churn klein zu
/// halten — `Entity` wird auch für create/update genutzt. Fehlt ein
/// Eintrag, fällt der Client auf die ID zurück.
#[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
pub reference_labels: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
```
(BTreeMap für deterministische Serialisierung/Tests.) Der GraphQL-`EntityPage`-Wrapper (`server/src/schema.rs`) reicht das als `async_graphql::Json` mit; der Client decodiert es in `queries.rs` neben `entities`.

### 2.3 server — `ReferenceResolver` (Seam) + `ServerEmbedResolver`

Neuer Modul-Pfad `server/src/reference/` (oder in `data.rs`, falls klein):
```rust
pub enum ReferenceResolutionStrategy { ServerEmbed, ClientBatch, PerCell }
```
Default `ServerEmbed`. In `data.rs::entities_page_raw` (der Serve-Pfad, der die `EntityPage` baut): nach dem Sammeln der Zeilen, für die `ServerEmbed`-Strategie:
1. Aus den Spalten-Metadaten der Entity die `Reference { entity }`-Spalten bestimmen.
2. Pro Reference-Spalte die distinkten FK-ids der Seite sammeln.
3. Pro Ziel-Entity **einmal** batch-fetchen (generische `entities`-Auslese der Ziel-Tabelle, gefiltert auf die id-Menge — reuse der vorhandenen Fetch-Primitive; bei fehlendem Filter-Support: id-Menge nachladen und client-seitig der Batch zuordnen).
4. Aus der Ziel-`EntitySettings.display_field` den Label-Wert je Ziel-Row lesen.
5. `reference_labels[row_id][col_key] = label` füllen. Ziel-Entity nicht geladen / id nicht gefunden / `display_field` None → Eintrag **auslassen** (Client-Fallback).

`ClientBatch`/`PerCell` sind nur als enum-Varianten + Match-Arme-`todo!()`/Doku vorhanden (Swap-Site), **nicht** implementiert.

### 2.4 client — read-time Display

`FieldCell` (`client/src/components/table/formatters.rs`) bekommt Zugriff auf die `reference_labels`-Map der aktuellen Seite (durch die Tabelle durchgereicht, analog `fields`). Für eine Reference-Zelle: `reference_labels[row_id][col_key]` → `render_text(label)`; sonst der bisherige `render_reference(id)` (ID-Fallback). `DefaultFieldRegistry::render` Reference-Arm entsprechend erweitern bzw. die Map in den `FieldContext` aufnehmen.

### 2.5 client — Editor-Picker

Neues Control `client/src/components/field/reference_picker.rs` (oder in `field/mod.rs`), das `ControlKind::Lookup` für `FieldType::Reference` ersetzt:
- Lädt das Ziel-`display_field` aus den Settings der Ziel-Entity (vorhandener Settings/Columns-Fetch).
- Sucheingabe → `fetch_entities(target_entity)` (eine Seite), **client-seitig** über das `display_field` (case-insensitive contains) gefiltert.
- Zeigt Kandidaten als Label (`display_field`-Wert); on-select setzt die FK-id (`on_change(Value::String(id))`) und zeigt das gewählte Label.
- Styling über `use_design()` (DesignSystem), kein Hard-CSS.
- Leerer Treffer → i18n-Hinweis. Bereits gesetzte id ohne geladenes Label → id anzeigen, bis gewählt.

## 3. Datenfluss

```
A (Display):
  entities_page_raw(order) -> Zeilen mit customer=<id>
   -> ServerEmbedResolver: customer-ids sammeln -> customer batch-fetch
      -> customer.displayName lesen -> reference_labels[orderId][customer]="Max M."
   -> EntityPage{entities, reference_labels} -> GraphQL -> Client
   -> FieldCell(reference "customer"): reference_labels[id][customer] -> "Max M."
      (fehlt -> render_reference(id))

B (Picker):
  Editor für order.customer -> ReferencePicker(target=customer, display_field=displayName)
   -> Sucheingabe "max" -> fetch_entities(customer) -> client-filter displayName~"max"
   -> Auswahl -> on_change(Value::String("customer-7")) -> gespeichert
```

## 4. Fehlerbehandlung

| Lage | Verhalten |
|---|---|
| Ziel-`display_field` = None | Display: ID; Picker: ID (oder erste Textspalte als Notlabel) |
| Ziel-Entity nicht geladen (hängender Ref, z.B. category) | Display: ID; Picker für solche Felder leer/ID |
| FK-id im Ziel nicht gefunden | Label ausgelassen → Display zeigt ID |
| Picker-Suche ohne Treffer | i18n-Leer-Hinweis |
| `fetch_entities`-Fehler im Picker | i18n-Fehlerhinweis, Feld bleibt mit aktueller id |

Nie Crash; jeder Pfad degradiert auf die ID.

## 5. Tests

- **shared:** `EntitySettings` parst `displayField`; `EntityPage` serde-roundtrip mit `reference_labels`.
- **server:** Resolver füllt `reference_labels` für `order→customer` (Label = customer.displayName); lässt `product→category` (Ziel nicht geladen) aus; `display_field=None` → kein Eintrag. Ein Batch-Query je Ziel-Entity (kein N+1) — über einen Zähl-/Spy-Pfad oder mind. ein Korrektheits-Assert.
- **client:** Reference-Zelle rendert Label wenn vorhanden, sonst ID; Picker filtert Kandidaten über `display_field` und `on_change` liefert die id. (Leptos-Render soweit testbar; Browser-Augenschein manuell.)
- **Loader:** shop-`customer`-Settings tragen `displayField=displayName` (Config-Pin).

Browser-Verifikation (shop: order-Liste zeigt Kundennamen; order-Editor-Picker wählt einen Kunden) ist **manuell** — nicht automatisiert.

## 6. Betroffene Dateien (Orientierung)

- `shared/src/settings.rs` (`display_field`), `shared/src/lib.rs` oder wo `EntityPage` liegt (`reference_labels`).
- `server/src/data.rs` (`entities_page_raw` + Resolver), ggf. `server/src/reference/`, `server/src/schema.rs` (`EntityPage`-Gql-Wrapper).
- `client/src/graphql/queries.rs` (`EntityPage`-Decode + `reference_labels`).
- `client/src/components/table/formatters.rs` + `field/mod.rs` (Display-Label + Picker-Control), neu `field/reference_picker.rs`.
- `examples/shop/entities/customer/settings.json` (+ ggf. `product`/`order`) — `displayField`.
- Tests in shared/server/client + `server/tests/loader*.rs`.

## 7. Decisions

1. **Ziel-Entity definiert `display_field`** (DRY; `Reference`-Wire-Typ unverändert). Einzelfeld jetzt; Template später.
2. **Label-Transport page-level** (`EntityPage.reference_labels`), nicht am Kern-`Entity`-Typ (minimaler Wire-Churn; `Entity` wird für create/update genutzt).
3. **Pluggbare Resolution-Strategie**, `ServerEmbed` jetzt (beste UX: kein Flash, kein N+1, kein Client-Cache; Server hat `display_field` + Datenquelle). `ClientBatch` (konfigurierbar, später) + `PerCell` (Low-Prio-Einzelfall) sind Seams.
4. **Picker-Suche client-gefiltert** über die gefetchte Kandidaten-Seite (Server-Filter ignoriert heute) — ok für kleine Ref-Ziele; Server-Filter später.
5. **Test-Bett shop** (`order→customer` auflösbar, `product→category` Fallback); d2v-FK→Reference ist ein separater Schicht-3-Schritt.
6. **A vor B in einem Plan** (A liefert sichtbaren Wert + den `display_field`-Unterbau, den B mitnutzt).

## 8. Referenzen

- `docs/superpowers/specs/2026-05-24-d2v-script-first-gap-analysis.md` §5.1 (U1), §7 #1.
- `shared/src/settings.rs::EntitySettings`, `shared/src/lib.rs` (`FieldType::Reference`, `Entity`, `EntityPage`).
- `client/src/components/field/mod.rs` (`render_reference`, `ControlKind::Lookup`-Placeholder).
- `client/src/graphql/queries.rs` (`fetch_entities`, `fetch_entity_by_id`).
- `examples/shop/entities/{customer,order,product}/` — Reference-Felder + Ziel.
- Memory [[generalisierung-vier-schichten]] (U1 = Schicht-1-Control), [[built-but-unwired-pattern]] (Roundtrip/Live-Pfad verifizieren).
