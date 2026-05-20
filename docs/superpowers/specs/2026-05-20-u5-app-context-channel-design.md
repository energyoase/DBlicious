# U5 — App-Context-Channel Spec

Date: 2026-05-20
Status: Draft — awaiting user review
Trigger: T20 (YearSelector), alle Buchungs-Listen.
Quelle: Gap-Analyse §4.1 U5, ROADMAP §1.8 U5.

## 1. Ziel

Ein reaktiver, app-weiter Kontext-Kanal (zunächst `active_year`, später `active_tenant` etc.), der

1. im Client als Leptos-Signal lebt und alle Komponenten reaktiv versorgt,
2. bei jeder GraphQL-Anfrage als typisierter Header an den Server gegeben wird,
3. vom Server in den Resolver-Layer als `AppContext`-Struct hineingereicht und in `FilterCriteria` per Entity-Setting eingewebt werden kann.

Damit verschwindet "Welches Jahr ist aktiv?" aus jeder einzelnen Tabelle, jedem Filter und jedem Editor — sie konsumieren den Context-Channel.

## 2. Nicht-Ziele

- Multi-Tenant-Implementierung (Phase 0.7 ist eigene Phase).
- Persistenz des Context-Werts über User-Sessions hinweg (kein neues Setting-Storage; reicht heute aus, beim Refresh den Default zu nehmen).
- UI-Builder-Integration (kommt in Phase 1).

## 3. Architektur

### 3.1 `shared::AppContext`

Neuer Typ in `shared/src/lib.rs`, Wire-Format `camelCase`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_year: Option<i32>,
    // Erweiterungsstellen — fuer spaetere Phasen reserviert, defaults skip
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tenant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_currency: Option<String>,
}
```

Frei erweiterbar; `#[serde(default)]` + `skip_serializing_if` halten Wire-Format minimal.

### 3.2 Transport-Layer

Header-Name: `x-dblicious-context` mit JSON-Body von `AppContext`. Begründung: ein einziger Header transportiert beliebig viele Slots; vermeidet Header-Sprawl pro Slot (`x-active-year`, `x-active-tenant`, …).

Client-seitig wird der Header in `client/src/graphql/mod.rs::execute` zentral aus dem `AppContextSignal` befüllt — alle existierenden Queries profitieren ohne Änderung.

Server-seitig in einem axum-Layer (`server/src/context_layer.rs`) parsen → `request.extensions_mut().insert(app_context)` → in async-graphql via `Context::data::<AppContext>()` abrufbar.

### 3.3 Entity-Settings-Kopplung

Neues optionales Feld in `EntitySettings`:

```rust
/// Kontext-Filter: ordnet einem Kontext-Slot eine Spalte zu, die bei
/// jeder Listenabfrage automatisch gefiltert wird.
/// `{"active_year": "$column.year", "active_tenant": "$column.tenant_id"}`.
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub context_filter: BTreeMap<String, String>,
```

Beim `entities(...)`-Resolver merged der Server vor Sort/Page-Anwendung `context_filter` in `FilterCriteria` — pro Slot `<column> equals <context-value>`, nur wenn der Slot im Header gesetzt ist. Slots ohne Header-Wert ⇒ kein impliziter Filter.

### 3.4 Client-Komponente `YearSelector`

Neuer `components::context::YearSelector` (Dropdown im App-Header). Liest verfügbare Jahre aus einer neuen GraphQL-Query `availableYears(entity_type: String)` (delegiert an Source; Default = Range `min..=max` von `posted_at` der Buchungs-Tabelle). Schreibt in das `AppContextSignal`.

Position: oberhalb der Navigation, rechts neben dem Logo. (Pattern aus D2V — global sichtbar.)

### 3.5 Reaktivität

`AppContextSignal` ist ein `RwSignal<AppContext>`. Alle Tabellen-Komponenten reagieren bereits auf Filter-Signal-Changes; durch die Server-Side-Kopplung an `context_filter` müssen sie zusätzlich auf `AppContextSignal` lauschen und einen Re-Fetch triggern. Mechanik: ein `create_effect`, der bei jedem Context-Change `state.reload()` der aktiven `EntityTable` ruft.

## 4. Daten-Flow (eine Listenabfrage)

```
User wählt Jahr 2025 im YearSelector
  ↓ AppContextSignal.set({active_year: 2025})
  ↓ create_effect feuert reload()
EntityTable ruft fetch_entity_page(entity_type, sort, filter, page, …)
  ↓ graphql/mod.rs::execute hängt header x-dblicious-context: {"activeYear":2025}
  ↓ axum context_layer parsed → AppContext in Request-Extensions
async-graphql resolver entities(...)
  ↓ Context::data::<AppContext>() liefert {active_year: Some(2025)}
  ↓ EntitySettings.context_filter = {"active_year": "$column.year"}
  ↓ FilterCriteria erweitert um {column: "year", op: equals, value: 2025}
Source::list_page() liefert nur gefilterte Rows.
```

## 5. Fehler-/Edge-Cases

- **Kein `context_filter` gesetzt** → Server ignoriert den Slot, alle Rows zurück. Erwartet für Stammdaten-Listen.
- **Slot im Header, aber leere Tabelle** → Frontend zeigt leeren Zustand (keine Sonderbehandlung).
- **Header-Parse-Fehler** → axum-Layer loggt warn, ignoriert den Header, fährt mit leerem `AppContext` fort. Niemals 4xx, damit ein kaputter Client nicht die ganze App lahmlegt.
- **GraphiQL-Aufrufe (ohne Client-Layer)** → Header fehlt, alle Rows. OK für Entwickler-Tools.

## 6. Komponenten-Inventar

**Neu**:
- `shared/src/app_context.rs` — `AppContext` Typ
- `server/src/context_layer.rs` — axum-Layer
- `client/src/components/context/mod.rs` — `AppContextSignal`-Provider
- `client/src/components/context/year_selector.rs` — Dropdown-Component

**Erweitert**:
- `shared/src/lib.rs` — Re-Export `AppContext`
- `shared/src/settings.rs` — `EntitySettings.context_filter`
- `server/src/main.rs` — Layer registrieren
- `server/src/schema.rs::entities` — Context aus `Context::data` → `FilterCriteria` merge
- `client/src/graphql/mod.rs::execute` — Header anhängen
- `client/src/app.rs` — `AppContextSignal` bereitstellen + `YearSelector` einhängen

## 7. Tests

- `shared/tests/app_context_wire.rs` — Roundtrip-Test camelCase + skip_serializing_if.
- `server/tests/context_filter.rs` — End-to-End: Header gesetzt → gefilterte Page; Header fehlt → alle Rows; `context_filter` nicht gesetzt → ignoriert; ungültiger Header → warn + alle Rows.
- Client Unit: `YearSelector` rendert verfügbare Jahre und schreibt in Signal.

## 8. Migration / Backwards-Compat

- `AppContext` ist additiv; alte Clients senden keinen Header → Server-Verhalten unverändert.
- `EntitySettings.context_filter` hat Default `{}` → existing Settings funktionieren ohne Änderung.

## 9. Größe + Risiken

**Größe**: S-M. Drei kleine neue Dateien, drei punktuelle Erweiterungen.

**Risiken**:
- Layer-Reihenfolge in axum: muss vor dem `async-graphql`-Handler stehen, sonst sieht der Resolver kein `AppContext`.
- Reaktive Reload-Schleife: wenn ein Effekt im Reload den Context schreibt, gibt's Loops. Mitigation: Effekt liest nur Signal, schreibt nie zurück.

## 10. Spätere Erweiterungen (nicht Teil dieser Spec)

- Server-Side-Caching der `availableYears`-Query.
- Persistenz des letzten gewählten Jahres in der `users`-Tabelle (Phase 0.7-Topic).
- Mehrere parallele Kontexte (z.B. unterschiedliche Jahre für unterschiedliche Tabs) — wenn überhaupt nötig, dann als separates Tab-Context-Konzept, nicht über `AppContext` hinaus aufgebläht.

## 11. Decisions

1. **Ein Header, ein JSON-Blob** statt n Header-Slots — leichtere Erweiterung.
2. **Context-Filter pro Entity per Settings**, nicht globaler "Year=column"-Default — viele Entitäten haben kein Jahr (Stammdaten).
3. **Reaktive Reload-Trigger im Client**, nicht GraphQL-Subscriptions — heute keine Subscription-Infrastruktur.
4. **Keine Persistenz** in dieser Spec.

## 12. Referenzen

- `shared/src/lib.rs`, `shared/src/settings.rs`
- `server/src/schema.rs::entities`
- `client/src/graphql/mod.rs::execute`
- `client/src/app.rs`
- Memory [[d2v-domain-glossary]] — YearSelector ist *kein* Mandant, nur Jahresfilter.
