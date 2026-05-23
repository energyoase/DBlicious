# Q0009 — Skript-Sprache für Reports, Custom-Komponenten und Capability-Provider — Spec

Date: 2026-05-23
Status: Draft — awaiting user review
Trigger: User-Anfrage „ich will eine scriptsprache haben, mit der man reports/designs selber schreiben kann" — Brainstorm-Session „DB: Script-Sprache".
Verwandt: [[u3-report-view-design]] (Report-Component, deklarativ, nimmt optional Skripte als Datenquelle), [[2026-05-20-u2-save-report-design]] (Save-Preview, könnte Skript-Validatoren konsumieren), VISION.md §3 (WASM-Plugin-Sandbox, Phase 2), VISION.md §4 (Codegen, Phase 4).

## 1. Ziel

Eine eingebettete Skript-Sprache, mit der **User über die deklarativen
Konfigurationen hinaus** eigene Logik in DBlicious bringen können — ohne
dass dafür Rust-Code im Core geändert werden muss. Vier Use-Cases (in der
Reihenfolge ihrer heutigen Priorität):

1. **Reports / Auswertungen** — User schreibt ein Skript, das Daten liest,
   aggregiert und in einer Tabelle/Liste/Diagramm darstellt. Ad-hoc oder
   gespeichert.
2. **Eigene Komponenten / Views** — Skript liefert einen `UiNode`-Subtree,
   wird im Builder als erstklassiger Knoten neben Table/Report angezeigt.
3. **Custom-Behavior im deklarativen Builder** — Formatter, Filter, Computed
   Columns, Validatoren, Row-Actions als Skript, die in die existierenden
   Registries eingehängt werden.
4. **Workflow-Aktionen** — Button-Click oder Background-Job, der Skripte
   mit Seiteneffekten ausführt (Use-Case 4 hängt an U6, eigener Spec).

Skripte laufen **symmetrisch auf Server und Client** mit identischer
Host-API, gleicher Sandbox-Schicht und gleichem Capability-Modell. Der
Server bleibt die Autorität für Rechte und Persistenz.

## 2. Nicht-Ziele

- **WASM-Engine** (Phase 2) — nur die Trait-Schnittstelle wird verankert,
  Implementation in eigenem Spec.
- **Codegen / Lift-and-Lock-Pipeline** (Phase 4) — dieser Spec liefert nur
  die statische Analysierbarkeit der Skripte; der Build-Step (Skript →
  Rust-Funktion bzw. Skript → Leptos-Komponente) steht in einem separaten
  Spec.
- **Lua als native Engine** — verworfen. Begründung: doppelte
  Host-Function-Pflege, schwierige Sandbox, kein Codegen-Pfad. Lua-Bedarf
  wird über die WASM-Engine bedient.
- **Skript-Editor / IDE-Support** (Syntax-Highlighting, Auto-Complete,
  Linter-UI) — eigener Spec im Builder-Track.
- **Aggregations-Layer für Reports** (Phase 1.7.12) — Voraussetzung für U3
  Performance, läuft parallel; Skripte konsumieren die Aggregation-API,
  definieren sie nicht.
- **Cluster-Cache-Invalidierung** (Multi-Server-Deployment) — out-of-scope.
- **Distributed Tracing** über Skript-Grenzen hinweg — `audit.log` reicht
  für jetzt.
- **Skript-Marketplace / Sharing** — out-of-scope.

## 3. Architektur-Überblick

```
┌─────────────────────────────────────────────────────────────────┐
│                           shared/                               │
│  • Script {source, manifest, kind, state, version}              │
│  • ScriptManifest {capabilities, ui_primitives, tier}           │
│  • CapabilityToken (enum: ReadEntity, WriteEntity, ComputeOnly, │
│                     ReadI18n, EmitUiNode, …)                    │
│  • UiNode::Script {…}  (neue Variante neben Table/Report)       │
│  • ScriptEngine-Trait (engine-agnostisch)                       │
└────────────────────────┬──────────────────────────┬─────────────┘
                         │                          │
        ┌────────────────▼──────────────┐  ┌────────▼──────────────┐
        │           server/             │  │       client/         │
        │  • script::engine             │  │  • script::engine     │
        │     (Rhai + Host-Functions)   │  │     (Rhai-WASM build) │
        │  • script::host::*            │  │  • script::host::*    │
        │     db, i18n, ui, audit       │  │     db (GraphQL), …   │
        │  • script::sandbox            │  │  • script::sandbox    │
        │     (Tier, Quota, Timeout,    │  │     (Tier, Timeout,   │
        │      Token-Audit)             │  │      Token-Audit)     │
        │  • Persistenz: scripts table  │  │  Sources: ScriptSource│
        │     (SeaORM)                  │  │     (DataSource impl) │
        └───────────────────────────────┘  └───────────────────────┘
                         │                          │
                         └──── symmetrische ────────┘
                              Host-Function-API
```

Drei Kernpunkte:

1. **`shared` definiert Wire-Format und Modell.** Wie heute mit
   `FieldType`/`ColumnMeta`/`Entity` — plain-serde, beide Crates lesen die
   gleichen Typen.
2. **Engine läuft beidseitig mit identischer Host-Function-Signatur.**
   Server liefert echte DB-Calls; Client liefert GraphQL-Calls — Skript
   ruft auf beiden Seiten dieselben Functions auf. Symmetrie ist
   **Spec-Constraint**, nicht Implementations-Detail. Sandbox enforcet
   Capability-Tokens beidseitig.
3. **Sandbox ist eigene Schicht zwischen Engine und Host.** Tier-Regeln,
   Timeouts, Memory-Limits, Token-Audit. Engine ist austauschbar — Rhai
   jetzt, WASM-Host später; Sandbox-API bleibt stabil.

## 4. Skript-Modell

```rust
pub struct Script {
    pub id: ScriptId,                    // ULID
    pub kind: ScriptKind,
    pub manifest: ScriptManifest,
    pub source: String,                  // Rhai-Quelle
    pub version: u32,                    // monoton; bei Save +1
    pub state: ScriptState,
    pub last_error: Option<ScriptError>, // gesetzt nur im Draft-State
    pub created_by: UserId,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

pub enum ScriptKind {
    Provider { slot: ProviderSlot },     // Formatter | Filter | Computed | Validator | RowAction
    Component { entry: String },         // Funktionsname für render()
    Wasm { wasm_bytes: Vec<u8>, entry: String }, // reserviert für Phase 2, heute unreachable
}

pub enum ScriptState {
    Draft,    // Save akzeptiert, kompiliert nicht / Manifest invalid — nicht ausführbar
    Active,   // Kompiliert, validiert, läuft
    Locked,   // Codegen hat übernommen, read-only; Edit erzeugt neuen Draft-Branch
}

pub struct ScriptManifest {
    pub manifest_version: u8,            // beginnt bei 1; spätere Versionen dürfen ergänzen
    pub tier: ScriptTier,
    pub capabilities: Vec<CapabilityToken>,
    pub ui_primitives: Vec<UiPrimitive>,  // nur für Component-Skripte
    pub timeout_ms: Option<u32>,          // gedeckelt durch Tier-Maximum
    pub memory_kb: Option<u32>,
    pub lift_capable: bool,               // beim Save analysiert
}
```

### 4.1 Capability-Tiers

| Tier | Wer | Capabilities (Default-Set, deckelt Manifest) | Limits |
|---|---|---|---|
| **Reader** | jeder eingeloggte User | `ReadOwnEntities`, `ReadI18n`, `ComputeOnly`, `EmitUiNode(Leaf)` | 100 ms, 4 MB |
| **Author** | Rolle „Power-User" | + `ReadAllEntitiesWhereAllowed`, `EmitUiNode(Composite)`, `ReadAuditLog(Own)` | 500 ms, 16 MB |
| **Developer** | Rolle „Customizer" | + `WriteEntity(Validated)`, `EmitWorkflowAction`, `LoadOtherScript` | 5 s, 64 MB |
| **Admin** | System-Admin | alle Tokens, inkl. `WriteAuditLog`, `RegisterHostFunction` | 30 s, 256 MB |

Regeln:

1. **Manifest ist Pflicht und maximal-restriktiv.** Skripte deklarieren
   *exakt* die Tokens, die sie brauchen — nicht mehr. Sandbox lehnt
   `WriteEntity`-Calls ab, wenn das Manifest nur `ReadOwnEntities` listet.
2. **Tier ist Maximum-Deckel, nicht Default.** Ein Author-Skript darf nur
   Tokens deklarieren, die im Author-Set vorkommen.
3. **Capability-Tokens sind statisch.** Keine „kann ich später
   `WriteEntity` anfragen" — alles im Manifest oder gar nicht.
4. **`LoadOtherScript`** (Tier ≥ Developer) erlaubt Komposition. Aufgerufenes
   Skript erbt **keine** Capabilities — sein eigenes Manifest entscheidet.
5. **Manifest-Validierung passiert beim Save**, nicht beim Run. Invalide
   Skripte landen mit `state = Draft` in der DB.

### 4.2 Save-Pipeline + Draft-State

| Save-Pfad | Ergebnis |
|---|---|
| Source OK + Manifest OK + Tier OK | `state = Active`, `last_error = None` |
| Source ParseFailed | `state = Draft`, `last_error = ParseFailed{…}` |
| ManifestInvalid (Token unbekannt, Schema-Fehler, etc.) | `state = Draft`, `last_error = ManifestInvalid{…}` |
| TierExceeded (Author-Skript will Developer-Tokens) | `state = Draft`, `last_error = TierExceeded{…}` |
| User darf das Skript gar nicht editieren (ACL) | **Hartes Reject** vor dem Save |

Runtime-Verhalten von Draft-Skripten:

- Lookup auf `formatter_id` / Component-Id, der auf ein Draft zeigt →
  Fallback auf Default (statische Registry; bei Komponenten ein leerer
  UiNode mit Inline-Hinweis „Skript ist im Draft: ${last_error.msg}").
- Builder-UI markiert Draft-Skripte sichtbar (Badge „Entwurf, Fehler: …").
- `script_versions` bekommt **alle** Saves — Drafts inklusive.

### 4.3 Lifecycle

```
   ┌─────────────────────────────────────────────────────┐
   ▼                                                     │
┌─────────┐ save  ┌───────────┐  save+valid  ┌─────────┐ │
│  -none- │──────▶│   Draft   │─────────────▶│  Active │─┘
└─────────┘       │ (mit      │              │         │
                  │  Fehlern  │◀─────────────│         │
                  │  parsbar) │ save+invalid │         │
                  └───────────┘              └────┬────┘
                                                  │ codegen (Phase 4)
                                                  ▼
                                            ┌──────────┐
                                            │  Locked  │
                                            └──────────┘
```

## 5. Engine & Sandbox

### 5.1 Engine-Layer (`script::engine`)

- **Rhai-Engine pro Run.** AST wird einmalig kompiliert und in einem Cache
  per `(script_id, version)` gehalten. Cache-Invalidierung beim Save.
- **`Engine::new_raw()`** statt `Engine::new()` — Rhai ohne Standard-Module.
  Eingebettet werden nur explizit erlaubte Module: `Arithmetic`, `Logic`,
  `BasicString`, `BasicArray`, `BasicMap`.
- **Operation-Counter** (`engine.set_max_operations(n)`) als harte Obergrenze
  — deterministisch, plattformunabhängig (wichtig für Server/Client-
  Symmetrie). Wall-Clock-Timeout läuft daneben als Fail-Safe.
- **Symbol-Disable** für `eval`, `import`, dynamische Module-Resolver,
  `print`, `debug`. Symbol-Disable wird in `engine::configure_strict()`
  zentral gesetzt und durch Tests gepinnt.

### 5.2 Sandbox-Layer (`script::sandbox`)

- **CapabilityCheck als Host-Function-Wrapper.** Jeder Host-Call läuft durch
  ein Makro `gate!(CapabilityToken::ReadEntity, { …real call… })`. Token nicht
  im Manifest → `ScriptError::CapabilityDenied`.
- **AuditTrail** schreibt jede Token-Nutzung in einen Per-Run-Buffer; am Run-
  Ende geflusht. Server → `script_audit_log` (SeaORM). Client → Telemetry-
  Event, mit nächstem Heartbeat hoch.
- **PanicCatch** ist Pflicht: `std::panic::catch_unwind` umfasst jeden Run.
  Panic wird zu `ScriptError::InternalPanic` — Hosting-Prozess crasht nie.
- **Unmaskable Errors**: `CapabilityDenied`, `UiPrimitiveDenied`, `Timeout`,
  `MemoryExceeded` sind **nicht** per Rhai-`try`/`catch` fangbar. Engine
  setzt das durch ein „unmaskable error"-Flag durch.

### 5.3 Symmetrie-Constraint (Spec-Regel)

> Wenn eine Host-Function im Server existiert, muss sie im Client mit
> identischer Signatur und identischem `CapabilityToken` existieren — oder
> explizit als `#[server_only]` markiert sein. Letzteres ist nur für
> `WriteEntity`, `RegisterHostFunction`, `ScheduleJob` erlaubt; alles andere
> ist symmetrisch.

Durchgesetzt durch einen `shared`-seitigen `HostApiRegistry`-Trait
(Compile-Time-Check), den beide Crates implementieren müssen.

### 5.4 Engine-Austausch (Forward-Compat)

Sandbox-Layer kennt das Wort „Rhai" nirgendwo außer im `engine::rhai`-Modul.
Für die WASM-Variante (Phase 2) wird `engine::wasm` danebengelegt; gleiche
`ScriptEngine`-Trait-Schnittstelle. Sandbox bleibt unverändert.

```rust
pub trait ScriptEngine {
    type Ast: Clone + Send + Sync;
    fn compile(&self, source: &str, manifest: &ScriptManifest) -> Result<Self::Ast>;
    fn run(&self, ast: &Self::Ast, host: &dyn HostApi, ctx: ScriptCtx) -> Result<ScriptValue>;
}
```

## 6. Zwei Skript-Klassen

### 6.1 Provider-Skript

Kleine Pure-Function, in eine existierende Registry eingehängt.

```rhai
// formatter:discount-tier   tier=Reader
//   capabilities: [ReadOwnEntities]
fn format(value, row, ctx) {
    if row.total >= 1000 { return ctx.t("tier.gold")   + " " + ctx.fmt_money(value); }
    if row.total >=  500 { return ctx.t("tier.silver") + " " + ctx.fmt_money(value); }
    ctx.fmt_money(value)
}
```

Eigenschaften:

- Aufruf-Kontext: synchron, in einem Render-Loop oder Validierungs-Pass.
- Rückgabewert: strikt typisiert nach Slot — `format()` → `String`,
  `validate()` → `Result<(), ValidationError>`, `compute()` → typisierter Wert,
  `filter()` → `bool`, `action()` → `WorkflowResult`.
- Lifetime: Engine wird wiederverwendet, AST aus Cache, Run < 1 ms.
- Capability-Set: klein per Default (Reader-Tier reicht meistens).

Registry-Einsprung (Pseudo-Code):

```rust
match column.formatter_id {
    Some(id) if id.starts_with("script:") => script_registry.run_provider(id, args)?,
    Some(id) => static_registry.format(id, args),
    None => default_format(args),
}
```

### 6.2 Component-Skript

Eigener `UiNode::Script`-Knoten im Builder-Tree, erstklassig neben Table/
Report.

```rhai
// component:sales-dashboard   tier=Author
//   capabilities: [ReadAllEntitiesWhereAllowed, ReadI18n, EmitUiNode(Composite)]
//   ui_primitives: [vstack, hstack, text, table, chart]
fn render(ctx) {
    let sales = db.entities("order")
                  .where("status", "==", "paid")
                  .since(ctx.month_start());
    let by_region = sales.group_by("region").sum("total");

    ui.vstack([
        ui.text(ctx.t("dashboard.title"), #{ size: "h1" }),
        ui.hstack([
            ui.chart(#{ data: by_region, kind: "bar", x: "region", y: "sum" }),
            ui.table(#{ data: sales, columns: ["id", "customer", "total"] })
        ])
    ])
}
```

Eigenschaften:

- Aufruf-Kontext: beim Rendern des UiNode. Server pre-rendert für SSR/Export;
  Client re-rendert bei Daten-/State-Änderung.
- Rückgabewert: ein `UiTree`-Subtree (kein roher HTML/CSS-Output). Sandbox
  prüft, dass nur deklarierte `ui_primitives` vorkommen.
- Lifetime: typisch 10–500 ms; Timeout per Tier.
- State: zustandsbehaftet erlaubt via `ctx.state` (Map, pro-Run persistiert
  im Client-Signal, im Server pro Session).
- Komposition: `ctx.invoke("script:discount-tier", value, row)` — wenn
  `LoadOtherScript` im Manifest steht.

### 6.3 Vergleich

| Aspekt | Provider | Component |
|---|---|---|
| Eintrittspunkt | `fn format/validate/...` | `fn render` |
| Rückgabe | typisierter Slot-Wert | `UiTree`-Subtree |
| Aufrufer | Registry-Lookup im Render-Loop | `UiNode::Script`-Renderer |
| Default-Tier | Reader | Author |
| Timeout-Default | 100 ms | 500 ms |
| Codegen-Pfad | Transpiliert zu Rust-Fn in Registry | Transpiliert zu Leptos-Komponente |

### 6.4 Lift-and-Lock — die gleitende Achse

Beide Klassen liefern denselben Sandbox-Output (Rhai-AST + Manifest +
bekannte Rückgabe-Form), daher kann der Builder einen Skript-Knoten
**promoten**:

1. **Provider → Provider+Locked.** Build-Step transpiliert das
   Provider-Skript zu einer Rust-Funktion in der Registry. Source bleibt in
   der DB, zur Laufzeit wird die kompilierte Version aufgerufen.
2. **Component → Component+Locked.** Build-Step transpiliert zu einer
   Leptos-Komponente mit eigenem `UiNode`-Typ. UiTree-Nodes, die früher
   `Script { id: "sales-dashboard" }` waren, werden zu
   `SalesDashboard { args }` migriert.
3. **Locked → Hot Patch.** Edit eines Locked-Skripts erzeugt einen neuen
   Draft-Branch; alte Locked-Version bleibt produktiv bis zum nächsten
   Build.

Phase-4-Codegen-Detail ist separater Spec, aber dieser Spec **garantiert**
die Eigenschaften, die Lift voraussetzt: Rhai-AST + statisches Manifest +
symmetrische Host-API + `lift_capable: bool` im Manifest (statisch berechnet
beim Save — Skripte mit dynamischen `db.entities(var)`-Calls sind nicht
lift-fähig).

## 7. Host-API

Vier Module, symmetrisch auf Server und Client. Capability-Tokens stehen
jeweils am Modul-Eingang.

### 7.1 `db` — Daten lesen und schreiben

```rhai
let products = db.entities("product")
                 .where("category_id", "==", cat)
                 .where("price", ">=", 100.0)
                 .order_by("name")
                 .limit(50)
                 .fetch();          // → Array<Map>

let one = db.entity("product", id).fetch();
let count = db.entities("order").where("status", "open").count();

// Schreibend, gated durch WriteEntity-Token
db.entity("product", id).patch(#{ price: 199.0 });
```

Regeln:

- **Builder-Pattern**, lazy. Erst `.fetch()`/`.count()` löst aus.
- **Rechte-Durchsetzung serverseitig.** Client-`db.*` ist ein dünner
  GraphQL-Wrapper — die gleichen Queries, die ein Anwender im UI absetzt.
  Wenn der Server nichts zurückgibt, sieht das Skript leere Listen.
- **Keine Raw-SQL.** Builder mappt auf `entities`-Tabelle + indizierte
  JSON-Felder. Server validiert `where`-Predikate gegen ColumnMeta (Typ,
  Operator-Whitelist pro FieldType).
- **`patch()` ist immer mediated**: läuft durch die normale Save-Pipeline
  (Validatoren, Audit, U2-Preview falls aktiviert).

### 7.2 `ui` — UiTree-Konstruktion (nur Component-Skripte)

```rhai
ui.vstack([
    ui.text("Hallo", #{ size: "h2" }),
    ui.table(#{ data: rows, columns: cols, sortable: true }),
    ui.chart(#{ data: by_region, kind: "bar" }),
    ui.if(condition, ui.text("ja"), ui.text("nein")),
    ui.for_each(items, |item| ui.text(item.name)),
])
```

Regeln:

- Rückgabe ist immer ein `UiNode`-Subtree.
- Primitives sind whitelisted im Manifest — nicht-deklarierte Primitive
  führen zu `ScriptError::UiPrimitiveDenied`.
- Keine direkten Style-Strings. Argumente sind semantische Tokens (`size:
  "h2"`, `tone: "accent"`), die durch die `DesignSystem`-Trait laufen.
- `ui.action(label, fn)` für Buttons ist Token-gated
  (`EmitWorkflowAction`). Der Fn-Body läuft beim Klick im selben Sandbox-
  Kontext.

### 7.3 `i18n` — Übersetzungen und Formatierung

```rhai
ctx.t("dashboard.title")
ctx.t("orders.count", #{ count: 42 })
ctx.fmt_money(amount, "EUR")
ctx.fmt_date(date, "long")
ctx.fmt_number(n, #{ digits: 2 })
```

Regeln:

- Fluent-Keys aus den FTL-Bundles. Keine Inline-Strings für UI-Text.
- Formatter delegieren auf Plattform: Client → `Intl`, Server → `icu` /
  `formatx`.
- `ReadI18n` ist Default für jeden Tier.

### 7.4 `ctx` & `audit` — Telemetrie und Kontext

```rhai
ctx.user_id
ctx.tenant_id
ctx.now()
ctx.locale
ctx.state                                 // Map, pro-Run persistierbar (nur Component)
ctx.invoke("script:foo", args)            // nur mit LoadOtherScript

audit.log("custom.event", #{ amount: 100 })   // Tier ≥ Developer
```

### 7.5 Verbots-Liste (Sicherheits-Anker)

In **allen** Tiers deaktiviert:

- `eval()`, dynamische String-zu-Code-Ausführung
- `import` / Module-Resolver (Sub-Skripte nur über `ctx.invoke`)
- Direkter Heap-Zugriff (`Dynamic::cast` auf interne Typen)
- `print`, `debug` (Output geht ausschließlich durch `audit.log`)

Symbol-Disable wird in `engine::configure_strict()` zentral gesetzt;
Sandbox-Tests verifizieren, dass die Symbole tatsächlich weg sind.

## 8. Server/Client-Symmetrie

Identische Quelle, identisches Manifest, identische Host-Function-Signaturen
— unterschiedliche Autorität:

| Aspekt | Server | Client |
|---|---|---|
| **Engine** | `rhai` (native build) | `rhai` (no-std-Profil, WASM-Target) |
| **`db.fetch()`** | direkter SeaORM-Call, ACL-Filter in SQL | GraphQL-Call gegen denselben Server-Resolver |
| **`db.patch()`** | echte Mutation durch Save-Pipeline | GraphQL-Mutation; Server validiert nochmal |
| **`ui.*`** | rendert zu UiTree für SSR / PDF / Email | rendert zu UiTree für DOM via Leptos |
| **`audit.log`** | persistiert direkt in `script_audit_log` | gepuffert, mit nächstem Heartbeat hoch |
| **Sandbox-Verstoß** | hartes Fail, 5xx an Client | hartes Fail, Toast für User, Event an Server |

Server-Run und Client-Run sind **nicht** automatisch redundant. Welche
Seite läuft, hängt am Ausführungs-Kontext:

```
Provider-Skript
├─ Validator   → server-only (Save-Pipeline, Wahrheit)
├─ Formatter   → client (Rendering, kein Round-Trip nötig)
└─ Computed    → server bei DB-Persistenz, client bei Live-Preview

Component-Skript
├─ Edit-/Live-Preview im Builder         → client
├─ Normale Anzeige in laufender App      → client
├─ Print/PDF-Export, Email-Versand       → server (SSR)
└─ Scheduled Job (Background)            → server
```

`shared::HostApiRegistry` definiert pro Function eine `#[server_only]`-
Markierung. Client-Build entfernt diese Funktionen aus dem Engine-
Namensraum; Skript-Versuch terminiert mit `ScriptError::ServerOnlyFunction`.

## 9. Persistenz

Neue Tabellen in `server/src/entity/`:

```rust
// scripts
struct Script {
    id: ScriptId,
    kind: ScriptKind,
    manifest: Json<ScriptManifest>,
    source: String,
    version: u32,
    state: ScriptState,
    last_error: Option<Json<ScriptError>>,
    created_by: UserId,
    created_at: DateTime,
    updated_at: DateTime,
}

// script_versions  (Append-Only)
struct ScriptVersion {
    script_id: ScriptId,
    version: u32,                 // PK zusammen mit script_id
    source: String,
    manifest: Json<ScriptManifest>,
    state_at_save: ScriptState,
    last_error: Option<Json<ScriptError>>,
    created_by: UserId,
    created_at: DateTime,
}

// script_audit_log
struct ScriptAuditEntry {
    id: ULID,
    script_id: ScriptId,
    script_version: u32,
    run_id: ULID,
    user_id: UserId,
    started_at: DateTime,
    finished_at: DateTime,
    outcome: AuditOutcome,        // Ok | Denied | Timeout | Panic
    tokens_used: Json<Vec<TokenUse>>,
    custom_events: Json<Vec<CustomEvent>>,
}
```

### 9.1 Loader-Integration (Dev-Mode)

Wie bei Entities heute (`examples/shop/entities/<type>/columns.{json,toml}`):
ein Beispiel-Datenverzeichnis darf

- `scripts/<id>.rhai` — Skript-Quelle
- `scripts/<id>.manifest.{json,toml}` — Manifest

enthalten. `loader.rs::load(dir)` liest beides; `example::install` ergänzt
den prozessweiten Slot; `db::init` berücksichtigt das beim Seed. Skripte
sind damit versionierbar (Git) im Beispiel-Layout *und* in der DB
editierbar zur Laufzeit — gleiche Symmetrie wie für ColumnMeta heute.

### 9.2 Versions-Pinning

UiNode-Referenzen auf Skripte verwenden `script_id` + optional
`version_pin`. Default „neueste Active-Version"; Pinning für
Reproduzierbarkeit (z. B. Reports im Archiv).

## 10. Fehlerklassen

```rust
pub enum ScriptError {
    // Compile-Time (Save akzeptiert mit state=Draft)
    ParseFailed     { line: u32, col: u32, msg: String },
    ManifestInvalid { reason: ManifestError },
    TierExceeded    { declared: ScriptTier, user: ScriptTier },

    // Run-Time
    CapabilityDenied    { token: CapabilityToken },
    UiPrimitiveDenied   { primitive: String },
    ServerOnlyFunction  { name: String },
    Timeout             { limit_ms: u32 },
    MemoryExceeded      { limit_kb: u32 },
    InternalPanic       { backtrace: String },
    HostError           { source: HostError },

    // Validation-spezifisch (Provider, Slot=Validator)
    ValidationFailed    { field: Option<String>, msg_key: String, args: FluentArgs },
}
```

Regeln:

- Compile-/Manifest-/Tier-Fehler **verhindern nicht den Save**; sie
  verhindern die Ausführung (`state = Draft`).
- Run-Time-Fehler werden zu Audit-Entries (`outcome = Denied | Timeout |
  Panic`) und über i18n-Message-Keys für den User lesbar.
- Skript darf `try`/`catch` auf Sandbox-Fehler benutzen, *außer* auf
  `CapabilityDenied` / `UiPrimitiveDenied` / `Timeout` / `MemoryExceeded`
  — diese sind unmaskable.

## 11. Forward-Compat-Garantien

| Garantie | Geprüft durch |
|---|---|
| Sandbox kennt Rhai nur in `script::engine::rhai`-Modul | Modul-Boundary-Test |
| `HostApi`-Trait engine-agnostisch | Compile-Time-Check, kein `rhai::*` im Trait-Body |
| Manifest-Schema versioniert + abwärtskompatibel | Schema-Round-Trip-Test pro Version |
| `ScriptKind::Wasm`-Variante reserviert | Enum-Diskriminanten gepinnt |
| UiNode-Referenzen auf Skripte statisch analysierbar | `lift_capable`-Flag wird beim Save berechnet und persistiert |
| Capability-Tokens statisch + komplette Token-Liste in `shared` | `CapabilityToken`-Enum + Exhaustiveness-Test |

### 11.1 Codegen-Profile (Phase 4, separater Spec)

- Skripte sind Capability-Manifest-getrieben. Manifest ist statisch lesbar
  zur Build-Zeit; Profil-Filter `include_capabilities = [...]` schneidet
  Skripte mit anderen Tokens raus.
- UiNode-Referenzen auf Skripte sind statisch traversierbar. Profile prunen
  den UiTree — alle nicht-erreichbaren Skripte fliegen mit raus.
- `embed_runtime_engine`-Flag entscheidet, ob die Rhai-Engine in das Final-
  Binary kommt oder ob alle Skripte zu Rust transpiliert werden müssen.
  Default-Profil `admin` = `true`, spezialisierte Profile (z. B.
  `score-display`) = `false`.

### 11.2 Lua via WASM

Nicht als native Engine. Wer Lua-Skripting braucht: Lua-zu-WASM-Compiler
(`lua-rs` o.ä.) und das Skript wird als `ScriptKind::Wasm`-Variante
eingebettet.

## 12. Testing-Strategie

Drei Test-Ebenen, alle in `cargo test --workspace`:

1. **Wire-Format-Tests** (`shared/tests/script_wire_format.rs`)
   - Pinnt `Script`, `ScriptManifest`, `CapabilityToken`, `ScriptError`
     Serde-Output (camelCase, tagged unions wie `FieldType`).
   - Round-Trip-Test pro `manifest_version`.
   - Lehnt sich an `shared/tests/field_type_wire_format.rs` an.

2. **Engine + Sandbox-Tests** (`server/tests/script_engine.rs`,
   `client/tests/script_engine.rs`)
   - Pos.-Tests: Bekannte Skripte (Fixtures unter
     `tests/fixtures/scripts/`) liefern erwartete Outputs für jeden
     Provider-Slot und für zwei Component-Skripte.
   - Neg.-Tests pro Sandbox-Constraint: `eval()` → `ParseFailed`;
     nicht-deklariertes Token → `CapabilityDenied`; `while true {}` →
     `Timeout`; 1 GB Array → `MemoryExceeded`; Panic im Host →
     `InternalPanic`. Jeder Constraint hat einen eigenen Negative-Test.
   - Symmetrie-Tests: gleiches Skript läuft mit gleichem Mock-Host auf
     Server und Client, Outputs müssen byte-identisch sein.

3. **Lift-and-Lock-Integration-Tests** (`server/tests/script_lift.rs`)
   - Provider-Skript transpiliert → Output-Funktion liefert dasselbe
     Ergebnis wie der interpretierte Run (für N gepinnte Test-Cases).
   - `lift_capable = false` schlägt Lift mit klarem Grund fehl.
   - Vorbereitend für Phase 4; pinnt die Lift-Annahmen.

Test-Hosts: `MockHostApi` in `shared/src/script/testing.rs`, deterministisches
In-Memory-DB-Mock + UI-Tree-Recorder. Jeder Konsument (Server, Client, später
WASM) testet gegen denselben Host.

Coverage-Ziel: 100 % der `CapabilityToken`-Varianten haben mindestens einen
Positive- und einen Negative-Test. Enforced durch einen Exhaustiveness-
Match-Test, der jede Variante in einer Liste enumerieren muss.

## 13. Abhängigkeiten

| Abhängigkeit | Status | Auswirkung wenn fehlend |
|---|---|---|
| Phase 1.5 Resolution-Kette | offen | Provider-Skripte funktionieren auch ohne, aber das Resolution-Pattern für `formatter_id` wäre uneinheitlich |
| Phase 1.6 Designer-Persistenz | offen | Component-Skripte können nicht aus dem Builder gespeichert werden — workaround: nur Loader-Format |
| Phase 1.7.12 Aggregations-Layer | offen | `db.entities().group_by().sum()` läuft, aber unperformant |
| U3 Report-Component | Spec | Komplementär; Report-View nutzt Skripte als Datenquelle |
| ADR zur WASM-Sandbox (Phase 2) | spätere Hängung | Kein Blocker |

## 14. Spec-Boundaries

**Was IN diesem Spec ist:**

- Datenmodell (`Script`, `ScriptManifest`, `CapabilityToken`, `ScriptError`,
  `UiNode::Script`) in `shared/`
- Engine-Layer (Rhai), Sandbox-Layer, Host-Function-Module in Server *und*
  Client
- Persistenz: `scripts`, `script_versions`, `script_audit_log` (SeaORM)
- Loader-Format-Erweiterung für `examples/<set>/scripts/`
- Tier-System (Reader/Author/Developer/Admin) inkl. Validierung beim Save
- Draft-State + `last_error` für unfertige Skripte
- Lift-Capability-Analyse (`lift_capable: bool` im Manifest)
- Test-Suite (Wire-Format, Engine + Sandbox, Lift-Integration)

**Was NICHT in diesem Spec ist:**

- WASM-Engine (Phase 2, eigener Spec) — nur Hook-Trait-Schnittstelle hier
- Codegen-Profile (Phase 4, eigener Spec) — nur Garantien hier
- Codegen selbst (Skript → Rust, Component → Leptos) — eigener Spec
- Skript-Editor / IDE-Support — eigener Spec
- Long-running-Workflows / Background-Jobs — bedient U6
- Aggregations-Layer (Phase 1.7.12) — eigener Spec
- Cluster-Cache-Invalidierung — out-of-scope
- Distributed Tracing über Skript-Grenzen — out-of-scope
- Skript-Marketplace / Sharing — out-of-scope
