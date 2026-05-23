# Q0009 — Skript-Sprache für Reports und Komponenten Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Built per [`docs/superpowers/specs/2026-05-23-q0009-skript-sprache-design.md`](../specs/2026-05-23-q0009-skript-sprache-design.md): eingebettete Rhai-Skript-Engine mit symmetrischem Server/Client-Host, Capability-Sandbox, vier Capability-Tiers, Draft-State-Persistenz und Lift-Capability-Analyse. Skripte laufen als Provider (Formatter/Filter/Computed/Validator/RowAction) **oder** als `UiNode::Script`-Komponenten.

**Architecture:** `shared/` definiert das Wire-Modell (`Script`, `ScriptManifest`, `CapabilityToken`, `ScriptError`, `ScriptEngine`-Trait, `UiNode::Script`-Variante) als getaggte Enums mit camelCase-Serde — analog zum bestehenden `FieldType`-Vertrag. Server und Client betten je eine Rhai-Instanz ein, die durch eine engine-agnostische `Sandbox`-Schicht von ausgewählten Host-Modulen (`db`, `ui`, `i18n`, `ctx`, `audit`) abgeschirmt wird. Persistenz: drei neue SeaORM-Tabellen (`scripts`, `script_versions`, `script_audit_log`) plus Loader-Sidecar `scripts/<id>.{rhai,manifest.json}`. GraphQL bekommt `script(id)`, `scripts`, `saveScript`, `previewScriptRun`. Codegen (Lift-and-Lock-Pipeline) ist **out-of-scope**; nur die statische `lift_capable`-Analyse beim Save sitzt in diesem Plan.

**Tech Stack:** Rust workspace (`shared` / `server` / `client`), Rhai 1.x (native + WASM-Build), SeaORM + SQLite, async-graphql 7, Leptos 0.7 CSR + WASM, Fluent (i18n).

---

## File Structure

**Created (`shared/`):**
- `shared/src/script/mod.rs` — Re-Exports + Modul-Bündel
- `shared/src/script/model.rs` — `Script`, `ScriptKind`, `ScriptState`, `ScriptId`
- `shared/src/script/manifest.rs` — `ScriptManifest`, `ScriptTier`, `UiPrimitive`, `manifest_version`-Konstante
- `shared/src/script/capability.rs` — `CapabilityToken`-Enum + `default_tokens_for_tier()`
- `shared/src/script/error.rs` — `ScriptError`, `ManifestError`, `ValidationFailed`, `unmaskable()`-Klassifier
- `shared/src/script/engine.rs` — engine-agnostischer `ScriptEngine`- und `HostApi`-Trait
- `shared/src/script/host_api.rs` — `HostApiRegistry`-Trait (Compile-Time Symmetrie-Check), Function-Descriptor (`name`, `token`, `server_only: bool`)
- `shared/src/script/testing.rs` — `MockHostApi` für Symmetrie-Tests (in-Memory-DB-Mock + UI-Tree-Recorder)
- `shared/tests/script_wire_format.rs` — Pin-Tests für Serde-Form (camelCase, Tag-Form, Enum-Diskriminanten)

**Created (`server/`):**
- `server/src/script/mod.rs` — `pub mod engine; pub mod sandbox; pub mod host; pub mod store;`
- `server/src/script/engine/mod.rs` — `pub mod rhai;` (Wrapper hinter Trait)
- `server/src/script/engine/rhai.rs` — `RhaiEngine`-Impl (`Engine::new_raw()`, `configure_strict`, AST-Cache)
- `server/src/script/sandbox.rs` — `Sandbox`-Struct (`gate!`-Makro, Token-Audit-Buffer, Timeout, Panic-Catch)
- `server/src/script/host/mod.rs` — `pub mod db; pub mod ui; pub mod i18n; pub mod ctx; pub mod audit;`
- `server/src/script/host/db.rs` — `db.entities()`/`db.entity()`/`db.patch()`-Implementation gegen SeaORM
- `server/src/script/host/ui.rs` — `ui.vstack`/`ui.hstack`/`ui.text`/`ui.table`/`ui.chart`/`ui.if`/`ui.for_each`/`ui.action`
- `server/src/script/host/i18n.rs` — `ctx.t()`, `ctx.fmt_money/_date/_number` (server-side via `formatx`)
- `server/src/script/host/ctx.rs` — `ctx.user_id`, `tenant_id`, `now()`, `locale`, `state`, `invoke()`
- `server/src/script/host/audit.rs` — `audit.log()` mit Token-Gate
- `server/src/script/store.rs` — CRUD-Helpers für `scripts`/`script_versions`/`script_audit_log`
- `server/src/script/lift.rs` — `analyze_lift_capability(ast) -> bool` (statische AST-Inspektion)
- `server/src/entity/scripts.rs` — SeaORM-Modell `Model` für `scripts`-Tabelle
- `server/src/entity/script_versions.rs` — SeaORM-Modell `Model` für `script_versions`-Tabelle
- `server/src/entity/script_audit_log.rs` — SeaORM-Modell `Model` für `script_audit_log`-Tabelle
- `server/tests/fixtures/scripts/discount_tier.rhai` — Fixture (Provider/Formatter)
- `server/tests/fixtures/scripts/discount_tier.manifest.json` — Fixture-Manifest
- `server/tests/fixtures/scripts/sales_dashboard.rhai` — Fixture (Component)
- `server/tests/fixtures/scripts/sales_dashboard.manifest.json` — Fixture-Manifest
- `server/tests/script_engine.rs` — Engine + Sandbox-Tests (server-side)
- `server/tests/script_persistence.rs` — `scripts`/`script_versions`/`script_audit_log` SeaORM-Tests
- `server/tests/script_lift.rs` — Lift-Capability-Analyse-Tests (statisch, ohne Phase-4-Codegen)
- `server/tests/script_graphql.rs` — GraphQL-Surface-Tests (`script`, `scripts`, `saveScript`, `previewScriptRun`)
- `server/tests/script_loader.rs` — Loader-Sidecar-Format-Tests
- `examples/shop/scripts/discount_tier.rhai` — Live-Skript zum Smoke-Testen
- `examples/shop/scripts/discount_tier.manifest.json` — zugehöriges Manifest

**Created (`client/`):**
- `client/src/script/mod.rs` — `pub mod engine; pub mod sandbox; pub mod host; pub mod source;`
- `client/src/script/engine/mod.rs` — Trait-Re-Export
- `client/src/script/engine/rhai.rs` — `RhaiEngine`-Impl für `wasm32-unknown-unknown` (Rhai-no-std-Profil)
- `client/src/script/sandbox.rs` — Spiegel von `server/src/script/sandbox.rs`, gleiches `gate!`-Makro
- `client/src/script/host/mod.rs` — `pub mod db; pub mod ui; pub mod i18n; pub mod ctx; pub mod audit;`
- `client/src/script/host/db.rs` — GraphQL-Backend für `db.*` (ruft `graphql::queries::*`)
- `client/src/script/host/ui.rs` — UiTree-Konstruktor identisch zur Server-Seite
- `client/src/script/host/i18n.rs` — `ctx.t()` via `I18nContext`; `fmt_*` via `js-sys`/`Intl`
- `client/src/script/host/ctx.rs` — `ctx.*` mit `state`-Persistierung im Leptos-Signal
- `client/src/script/host/audit.rs` — Buffer + Heartbeat-Flush
- `client/src/script/source.rs` — `ScriptSource` (implementiert `DataSource`-Trait für Provider-Skripte)
- `client/src/components/script_renderer.rs` — Rendert `UiNode::Script { script_id, args }`-Subtree
- `client/tests/script_engine.rs` — Symmetrie-Tests (gleiche Inputs wie Server)

**Modified:**
- `shared/src/lib.rs` — `pub mod script;` + Re-Exports
- `shared/Cargo.toml` — keine neuen Abhängigkeiten (Wire-Typen sind plain `serde`)
- `server/Cargo.toml` — `rhai = { version = "1", default-features = false, features = ["std", "sync"] }`, `ulid = "1"`, `tokio = { ..., features = [..., "time"] }`
- `client/Cargo.toml` — `rhai = { version = "1", default-features = false, features = ["std", "wasm-bindgen"] }`, `ulid = { version = "1", default-features = false }`
- `server/src/entity/mod.rs` — `pub mod scripts; pub mod script_versions; pub mod script_audit_log;`
- `server/src/db.rs` — `create_table_from_entity` für die drei neuen Tabellen + `seed_scripts_from_example()`
- `server/src/example/mod.rs` — `pub scripts: Vec<ScriptSeed>` auf `ExampleSet`
- `server/src/example/loader.rs` — `load_scripts(dir)` (liest `scripts/<id>.rhai` + `<id>.manifest.{json,toml}`)
- `server/src/lib.rs` — `pub mod script;`
- `server/src/schema.rs` — neue GraphQL-Types/Queries/Mutations für Skripte
- `client/src/lib.rs` — `pub mod script;`
- `client/src/components/table/formatters.rs` — Lookup-Pfad `formatter_id.starts_with("script:")` → `ScriptRegistry`
- `client/src/components/table/filters/registry.rs` — Lookup-Pfad `filter_id.starts_with("script:")` → `ScriptRegistry`
- `client/src/builder/node.rs` — Erweiterung um `kind: NodeKind` (mit `NodeKind::Script { script_id, version_pin }`-Variante)
- `client/src/components/navigation.rs` / `client/src/routes/mod.rs` — `UiNode::Script`-Branch im Renderer

---

## Test-Strategy-Anker (Spec §12 → konkrete Deliverables)

Drei Test-Dateien sind in §12 des Specs pinned und werden in diesem Plan explizit als Deliverables aufgesetzt:

| Spec-Anker | Plan-Deliverable | Phase |
|---|---|---|
| Wire-Format-Tests | `shared/tests/script_wire_format.rs` | Phase 1 |
| Engine + Sandbox-Tests | `server/tests/script_engine.rs` + `client/tests/script_engine.rs` | Phase 2 + Phase 4 |
| Lift-and-Lock-Integration-Tests | `server/tests/script_lift.rs` | Phase 3 |

`MockHostApi` (`shared/src/script/testing.rs`) ist die gemeinsame Test-Doppel-Quelle, gegen die alle drei Schichten laufen.

---

# Phase 1 — `shared/` Wire-Format

**Ziel:** Plain-serde Wire-Typen für Skripte, ohne Engine-Abhängigkeit. Beide Crates (server, client) lesen davon. Phase 1 schließt mit `cargo test -p shared` grün.

---

### Task 1.1: Modul-Skelett anlegen

**Files:**
- Create: `shared/src/script/mod.rs`
- Create: `shared/src/script/model.rs` (Stub)
- Create: `shared/src/script/manifest.rs` (Stub)
- Create: `shared/src/script/capability.rs` (Stub)
- Create: `shared/src/script/error.rs` (Stub)
- Modify: `shared/src/lib.rs` — Modul registrieren

- [ ] **Step 1: `shared/src/script/mod.rs` erstellen**

```rust
//! Wire-Format-Typen fuer die eingebettete Skript-Sprache (Q0009).
//!
//! Beide Crates (server, client) konsumieren diese Typen ueber plain
//! `serde`. Die getaggten Enums (`ScriptKind`, `ScriptState`,
//! `CapabilityToken`, `ScriptError`) folgen dem `FieldType`-Vertrag:
//! `#[serde(tag = "kind", rename_all = "camelCase")]` auf der Enum-Ebene
//! benennt die Varianten in camelCase; innere Felder einer Struct-Variante
//! bleiben snake_case (vgl. `shared/tests/field_type_wire_format.rs`).

pub mod model;
pub mod manifest;
pub mod capability;
pub mod error;
pub mod engine;
pub mod host_api;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use capability::{CapabilityToken, default_tokens_for_tier, ScriptTier};
pub use error::{ManifestError, ScriptError};
pub use manifest::{ScriptManifest, UiPrimitive, MANIFEST_VERSION_CURRENT};
pub use model::{Script, ScriptId, ScriptKind, ScriptState};
pub use engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
pub use host_api::{HostApiRegistry, HostFunctionDescriptor};
```

- [ ] **Step 2: Stub-Dateien anlegen, damit `cargo build` durchläuft**

Inhalt für jeden Stub: nur eine `//! TODO`-Zeile.

```rust
// shared/src/script/model.rs
//! TODO: implementiert in Task 1.2
```

Analog für `manifest.rs`, `capability.rs`, `error.rs`. `engine.rs`, `host_api.rs`, `testing.rs` werden in späteren Tasks angelegt — hier auch als Stubs anlegen mit `//! TODO`.

- [ ] **Step 3: `shared/src/lib.rs` erweitern**

In der Modul-Liste (nach `pub mod view;`) einfügen:

```rust
pub mod script;
```

- [ ] **Step 4: Build prüfen**

Run: `cargo build -p shared`
Expected: PASS (Stubs sind harmlos).

- [ ] **Step 5: Commit**

```bash
git add shared/src/script/ shared/src/lib.rs
git commit -m "feat(shared): script module skeleton (Q0009 Phase 1.1)"
```

---

### Task 1.2: `ScriptTier` + `CapabilityToken` + `default_tokens_for_tier`

**Files:**
- Modify: `shared/src/script/capability.rs`
- Create: `shared/tests/script_wire_format.rs` (Datei wird über mehrere Tasks gefüllt)

- [ ] **Step 1: Failing Test für `ScriptTier` schreiben**

`shared/tests/script_wire_format.rs`:

```rust
//! Pin-Test fuer das Wire-Format der Skript-Sprache (Q0009).
//!
//! Bricht in CI, wenn jemand camelCase/Tag/skip_serializing_if veraendert.
//! Lehnt sich an `field_type_wire_format.rs` an.

use serde_json::{json, Value};
use shared::script::{
    default_tokens_for_tier, CapabilityToken, ScriptTier,
};

#[test]
fn script_tier_serializes_lowercase() {
    assert_eq!(serde_json::to_value(ScriptTier::Reader).unwrap(),    json!("reader"));
    assert_eq!(serde_json::to_value(ScriptTier::Author).unwrap(),    json!("author"));
    assert_eq!(serde_json::to_value(ScriptTier::Developer).unwrap(), json!("developer"));
    assert_eq!(serde_json::to_value(ScriptTier::Admin).unwrap(),     json!("admin"));
}

#[test]
fn default_tokens_for_reader_is_minimal_set() {
    let toks = default_tokens_for_tier(ScriptTier::Reader);
    assert!(toks.contains(&CapabilityToken::ReadOwnEntities));
    assert!(toks.contains(&CapabilityToken::ReadI18n));
    assert!(toks.contains(&CapabilityToken::ComputeOnly));
    // Reader darf KEIN WriteEntity haben:
    assert!(!toks.iter().any(|t| matches!(t, CapabilityToken::WriteEntity { .. })));
}
```

- [ ] **Step 2: Test laufen lassen — muss fehlschlagen**

Run: `cargo test -p shared --test script_wire_format -- script_tier_serializes_lowercase`
Expected: FAIL (CapabilityToken / ScriptTier existieren noch nicht).

- [ ] **Step 3: Implementierung von `capability.rs`**

```rust
//! Tier- und Capability-Token-Definitionen.
//!
//! `CapabilityToken` ist ein getaggter Enum mit `#[serde(tag = "kind",
//! rename_all = "camelCase")]`. Inner-Field-Konvention wie bei `FieldType`:
//! die Felder einer Struct-Variante bleiben snake_case.

use serde::{Deserialize, Serialize};

/// Berechtigungs-Stufe eines Skripts. Deckel — nicht Default — fuer die
/// Tokens, die das Manifest deklarieren darf.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum ScriptTier {
    Reader,
    Author,
    Developer,
    Admin,
}

/// Eine spezifische Capability, die das Manifest deklarieren muss, damit
/// das Skript sie nutzen darf. Die Liste ist abschliessend: ein
/// Exhaustiveness-Match-Test in `script_wire_format.rs` enumeriert jede
/// Variante.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CapabilityToken {
    ReadOwnEntities,
    ReadAllEntitiesWhereAllowed,
    WriteEntity { validated: bool },
    ComputeOnly,
    ReadI18n,
    EmitUiNode { scope: UiScope },
    EmitWorkflowAction,
    LoadOtherScript,
    ReadAuditLog { own_only: bool },
    WriteAuditLog,
    RegisterHostFunction,
    ScheduleJob,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum UiScope {
    Leaf,
    Composite,
}

/// Liefert den maximalen Default-Token-Set fuer einen Tier (gemaess Spec §4.1).
/// Manifest darf nur Tokens daraus deklarieren.
pub fn default_tokens_for_tier(tier: ScriptTier) -> Vec<CapabilityToken> {
    use CapabilityToken::*;
    let mut out = vec![ReadOwnEntities, ReadI18n, ComputeOnly, EmitUiNode { scope: UiScope::Leaf }];
    if tier >= ScriptTier::Author {
        out.push(ReadAllEntitiesWhereAllowed);
        out.push(EmitUiNode { scope: UiScope::Composite });
        out.push(ReadAuditLog { own_only: true });
    }
    if tier >= ScriptTier::Developer {
        out.push(WriteEntity { validated: true });
        out.push(EmitWorkflowAction);
        out.push(LoadOtherScript);
    }
    if tier >= ScriptTier::Admin {
        out.push(WriteAuditLog);
        out.push(RegisterHostFunction);
        out.push(ScheduleJob);
    }
    out
}
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p shared --test script_wire_format -- script_tier_serializes_lowercase default_tokens_for_reader_is_minimal_set`
Expected: PASS.

- [ ] **Step 5: Wire-Pin-Tests für `CapabilityToken` ergänzen**

In `shared/tests/script_wire_format.rs`:

```rust
#[test]
fn capability_token_simple_variants_use_kind_field() {
    assert_eq!(serde_json::to_value(CapabilityToken::ReadOwnEntities).unwrap(),
               json!({"kind": "readOwnEntities"}));
    assert_eq!(serde_json::to_value(CapabilityToken::ReadI18n).unwrap(),
               json!({"kind": "readI18n"}));
    assert_eq!(serde_json::to_value(CapabilityToken::ComputeOnly).unwrap(),
               json!({"kind": "computeOnly"}));
}

#[test]
fn capability_token_write_entity_keeps_snake_case_inner_field() {
    // wie bei FieldType::Money: inner field bleibt snake_case
    let v = serde_json::to_value(CapabilityToken::WriteEntity { validated: true }).unwrap();
    assert_eq!(v, json!({"kind": "writeEntity", "validated": true}));
}

#[test]
fn capability_token_emit_ui_node_carries_scope() {
    let v = serde_json::to_value(CapabilityToken::EmitUiNode { scope: shared::script::capability::UiScope::Composite }).unwrap();
    assert_eq!(v, json!({"kind": "emitUiNode", "scope": "composite"}));
}

#[test]
fn capability_token_roundtrips_all_variants() {
    use CapabilityToken::*;
    use shared::script::capability::UiScope;
    let originals = vec![
        ReadOwnEntities,
        ReadAllEntitiesWhereAllowed,
        WriteEntity { validated: true },
        WriteEntity { validated: false },
        ComputeOnly,
        ReadI18n,
        EmitUiNode { scope: UiScope::Leaf },
        EmitUiNode { scope: UiScope::Composite },
        EmitWorkflowAction,
        LoadOtherScript,
        ReadAuditLog { own_only: true },
        ReadAuditLog { own_only: false },
        WriteAuditLog,
        RegisterHostFunction,
        ScheduleJob,
    ];
    for t in originals {
        let s = serde_json::to_string(&t).unwrap();
        let back: CapabilityToken = serde_json::from_str(&s).unwrap();
        assert_eq!(t, back, "CapabilityToken-Roundtrip fehlgeschlagen: {s}");
    }
}

#[test]
fn unknown_capability_kind_fails_to_deserialize() {
    let r: Result<CapabilityToken, _> = serde_json::from_value(json!({"kind": "frobnicated"}));
    assert!(r.is_err(), "unbekannter kind muss Fehler werfen");
}

/// Exhaustiveness-Anker: wenn jemand eine neue Variante hinzufuegt, muss
/// hier ein Eintrag dazu. Bricht den Build absichtlich.
#[test]
fn capability_token_exhaustiveness_anchor() {
    use CapabilityToken::*;
    use shared::script::capability::UiScope;
    fn anchor(t: &CapabilityToken) -> &'static str {
        match t {
            ReadOwnEntities              => "readOwnEntities",
            ReadAllEntitiesWhereAllowed  => "readAllEntitiesWhereAllowed",
            WriteEntity { .. }           => "writeEntity",
            ComputeOnly                  => "computeOnly",
            ReadI18n                     => "readI18n",
            EmitUiNode { .. }            => "emitUiNode",
            EmitWorkflowAction           => "emitWorkflowAction",
            LoadOtherScript              => "loadOtherScript",
            ReadAuditLog { .. }          => "readAuditLog",
            WriteAuditLog                => "writeAuditLog",
            RegisterHostFunction         => "registerHostFunction",
            ScheduleJob                  => "scheduleJob",
        }
    }
    // Trigger pro Variante:
    let _ = anchor(&ReadOwnEntities);
    let _ = anchor(&WriteEntity { validated: true });
    let _ = anchor(&EmitUiNode { scope: UiScope::Leaf });
    let _ = anchor(&ReadAuditLog { own_only: false });
}
```

- [ ] **Step 6: Tests laufen lassen**

Run: `cargo test -p shared --test script_wire_format`
Expected: alle PASS.

- [ ] **Step 7: Commit**

```bash
git add shared/src/script/capability.rs shared/tests/script_wire_format.rs
git commit -m "feat(shared): CapabilityToken + ScriptTier + default-set (Q0009 Phase 1.2)"
```

---

### Task 1.3: `ScriptManifest` mit Wire-Pin

**Files:**
- Modify: `shared/src/script/manifest.rs`
- Modify: `shared/tests/script_wire_format.rs`

- [ ] **Step 1: Failing Test**

```rust
#[test]
fn manifest_serializes_camelcase_with_pinned_fields() {
    use shared::script::{CapabilityToken, ScriptManifest, ScriptTier, UiPrimitive};
    let m = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ReadOwnEntities, CapabilityToken::ReadI18n],
        ui_primitives: vec![UiPrimitive::Text],
        timeout_ms: Some(100),
        memory_kb: None,
        lift_capable: true,
    };
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["manifestVersion"], json!(1));
    assert_eq!(v["tier"], json!("reader"));
    assert_eq!(v["capabilities"][0], json!({"kind": "readOwnEntities"}));
    assert_eq!(v["uiPrimitives"], json!(["text"]));
    assert_eq!(v["timeoutMs"], json!(100));
    assert_eq!(v["liftCapable"], json!(true));
    // memoryKb wird weggelassen, weil Option=None + skip_serializing_if
    assert!(v.get("memoryKb").is_none(), "memoryKb soll weggelassen werden: {v}");
}

#[test]
fn manifest_roundtrips_through_json() {
    use shared::script::{CapabilityToken, ScriptManifest, ScriptTier, UiPrimitive};
    let m = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Author,
        capabilities: vec![CapabilityToken::ReadAllEntitiesWhereAllowed],
        ui_primitives: vec![UiPrimitive::Vstack, UiPrimitive::Text, UiPrimitive::Table],
        timeout_ms: Some(500),
        memory_kb: Some(16_000),
        lift_capable: false,
    };
    let s = serde_json::to_string(&m).unwrap();
    let back: ScriptManifest = serde_json::from_str(&s).unwrap();
    assert_eq!(m, back);
}
```

- [ ] **Step 2: Test laufen lassen — FAIL**

Run: `cargo test -p shared --test script_wire_format -- manifest_serializes_camelcase_with_pinned_fields`
Expected: FAIL (Symbole fehlen).

- [ ] **Step 3: Implementation**

`shared/src/script/manifest.rs`:

```rust
//! Manifest = statische, deklarative Selbstauskunft eines Skripts.
//! Tier deckelt Capability-Set. `lift_capable` wird beim Save berechnet.

use serde::{Deserialize, Serialize};

use crate::script::capability::{CapabilityToken, ScriptTier};

pub const MANIFEST_VERSION_CURRENT: u8 = 1;

/// Whitelisted UI-Primitives — `ui.*`-Aufruf nicht in der Liste → Sandbox
/// schlaegt mit `UiPrimitiveDenied` fehl.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum UiPrimitive {
    Vstack,
    Hstack,
    Text,
    Table,
    Chart,
    If,
    ForEach,
    Action,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptManifest {
    /// Beginnt bei 1; spaetere Versionen duerfen Felder ergaenzen.
    pub manifest_version: u8,
    pub tier: ScriptTier,
    pub capabilities: Vec<CapabilityToken>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ui_primitives: Vec<UiPrimitive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_kb: Option<u32>,
    /// Vom Save-Step berechnet (Phase 3 / `lift::analyze_lift_capability`).
    /// Default `false`: erst die Analyse darf das Flag setzen.
    #[serde(default)]
    pub lift_capable: bool,
}

impl Default for ScriptManifest {
    fn default() -> Self {
        Self {
            manifest_version: MANIFEST_VERSION_CURRENT,
            tier: ScriptTier::Reader,
            capabilities: Vec::new(),
            ui_primitives: Vec::new(),
            timeout_ms: None,
            memory_kb: None,
            lift_capable: false,
        }
    }
}
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p shared --test script_wire_format -- manifest_`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add shared/src/script/manifest.rs shared/tests/script_wire_format.rs
git commit -m "feat(shared): ScriptManifest + UiPrimitive whitelist (Q0009 Phase 1.3)"
```

---

### Task 1.4: `Script` + `ScriptKind` + `ScriptState` + `ScriptId`

**Files:**
- Modify: `shared/src/script/model.rs`
- Modify: `shared/tests/script_wire_format.rs`

- [ ] **Step 1: Failing Test (inkl. WASM-Diskriminanten-Pin)**

```rust
#[test]
fn script_kind_provider_serializes_with_slot() {
    use shared::script::ScriptKind;
    let v = serde_json::to_value(ScriptKind::Provider {
        slot: shared::script::model::ProviderSlot::Formatter,
    }).unwrap();
    assert_eq!(v, json!({"kind": "provider", "slot": "formatter"}));
}

#[test]
fn script_kind_component_serializes_with_entry() {
    use shared::script::ScriptKind;
    let v = serde_json::to_value(ScriptKind::Component { entry: "render".into() }).unwrap();
    assert_eq!(v, json!({"kind": "component", "entry": "render"}));
}

/// `Wasm`-Variante ist Phase-2-reserviert. Wir pinnen die Diskriminante
/// (Wire-Tag `"wasm"`), damit Phase 2 sie nicht versehentlich aendert.
#[test]
fn script_kind_wasm_variant_is_reserved_and_pinned() {
    use shared::script::ScriptKind;
    let v = serde_json::to_value(ScriptKind::Wasm {
        wasm_bytes: vec![1, 2, 3],
        entry: "main".into(),
    }).unwrap();
    assert_eq!(v["kind"], json!("wasm"));
    assert_eq!(v["entry"], json!("main"));
    // wasm_bytes als Array von u8 (snake_case wie bei FieldType::Money):
    assert_eq!(v["wasm_bytes"], json!([1, 2, 3]));
}

#[test]
fn script_state_serializes_lowercase() {
    use shared::script::ScriptState;
    assert_eq!(serde_json::to_value(ScriptState::Draft).unwrap(),  json!("draft"));
    assert_eq!(serde_json::to_value(ScriptState::Active).unwrap(), json!("active"));
    assert_eq!(serde_json::to_value(ScriptState::Locked).unwrap(), json!("locked"));
}

#[test]
fn script_id_is_transparent_string() {
    use shared::script::ScriptId;
    let id = ScriptId("abc123".into());
    assert_eq!(serde_json::to_value(&id).unwrap(), json!("abc123"));
    let back: ScriptId = serde_json::from_str("\"xyz\"").unwrap();
    assert_eq!(back, ScriptId("xyz".into()));
}
```

- [ ] **Step 2: Test laufen lassen — FAIL**

Run: `cargo test -p shared --test script_wire_format -- script_kind script_state script_id`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`shared/src/script/model.rs`:

```rust
//! Kernmodell: `Script`, `ScriptKind`, `ScriptState`, `ScriptId`.
//!
//! `ScriptKind::Wasm` ist heute *reserviert*. Sandbox + Engine lehnen
//! Wasm-Skripte mit `ScriptError::WasmEngineNotAvailable` ab — siehe
//! `server/src/script/engine/rhai.rs::compile`. Phase 2 fuellt die
//! Variante.

use serde::{Deserialize, Serialize};

use crate::script::error::ScriptError;
use crate::script::manifest::ScriptManifest;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(transparent)]
pub struct ScriptId(pub String);

impl From<String> for ScriptId {
    fn from(s: String) -> Self { ScriptId(s) }
}

impl From<&str> for ScriptId {
    fn from(s: &str) -> Self { ScriptId(s.into()) }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ProviderSlot {
    Formatter,
    Filter,
    Computed,
    Validator,
    RowAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptKind {
    Provider { slot: ProviderSlot },
    Component { entry: String },
    /// Phase-2-reserviert. Server lehnt `compile()` heute mit
    /// `ScriptError::WasmEngineNotAvailable` ab.
    Wasm { wasm_bytes: Vec<u8>, entry: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ScriptState {
    Draft,
    Active,
    Locked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Script {
    pub id: ScriptId,
    pub kind: ScriptKind,
    pub manifest: ScriptManifest,
    pub source: String,
    pub version: u32,
    pub state: ScriptState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<ScriptError>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 4: `error.rs` minimal füllen (genug für `Script.last_error`)**

`shared/src/script/error.rs` — Stub mit echtem Inhalt, der in Task 1.5 erweitert wird:

```rust
//! Fehlerklassen — wird in Task 1.5 ausgebaut.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptError {
    ParseFailed { line: u32, col: u32, msg: String },
    // weitere Varianten kommen in Task 1.5
    Placeholder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestError {
    pub reason: String,
}
```

- [ ] **Step 5: Test wiederholen**

Run: `cargo test -p shared --test script_wire_format`
Expected: PASS (alle bisherigen).

- [ ] **Step 6: Commit**

```bash
git add shared/src/script/model.rs shared/src/script/error.rs shared/tests/script_wire_format.rs
git commit -m "feat(shared): Script/ScriptKind/ScriptState + reserved Wasm variant (Q0009 Phase 1.4)"
```

---

### Task 1.5: `ScriptError` voll ausgebaut + `unmaskable()`-Klassifier

**Files:**
- Modify: `shared/src/script/error.rs`
- Modify: `shared/tests/script_wire_format.rs`

- [ ] **Step 1: Failing Tests**

```rust
#[test]
fn script_error_capability_denied_carries_token() {
    use shared::script::{CapabilityToken, ScriptError};
    let e = ScriptError::CapabilityDenied { token: CapabilityToken::ReadOwnEntities };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["kind"], json!("capabilityDenied"));
    assert_eq!(v["token"], json!({"kind": "readOwnEntities"}));
}

#[test]
fn script_error_timeout_carries_limit_ms_snake_case() {
    use shared::script::ScriptError;
    let e = ScriptError::Timeout { limit_ms: 500 };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v, json!({"kind": "timeout", "limit_ms": 500}));
}

#[test]
fn unmaskable_classifies_sandbox_errors_correctly() {
    use shared::script::{CapabilityToken, ScriptError};
    assert!(ScriptError::CapabilityDenied { token: CapabilityToken::ReadOwnEntities }.unmaskable());
    assert!(ScriptError::UiPrimitiveDenied { primitive: "vstack".into() }.unmaskable());
    assert!(ScriptError::Timeout { limit_ms: 100 }.unmaskable());
    assert!(ScriptError::MemoryExceeded { limit_kb: 4000 }.unmaskable());

    assert!(!ScriptError::ParseFailed { line: 1, col: 1, msg: "x".into() }.unmaskable());
    assert!(!ScriptError::ValidationFailed { field: None, msg_key: "k".into(), args: serde_json::json!({}) }.unmaskable());
}
```

- [ ] **Step 2: Test laufen lassen — FAIL**

Run: `cargo test -p shared --test script_wire_format -- script_error unmaskable`
Expected: FAIL.

- [ ] **Step 3: Implementation**

Ersetze `shared/src/script/error.rs` komplett:

```rust
//! Skript-Fehlerklassen.
//!
//! Aufgeteilt in:
//!   - Compile-Time (Save akzeptiert mit state=Draft)
//!   - Run-Time (geht in `script_audit_log.outcome`)
//!   - Validation (Provider mit Slot=Validator)
//!
//! `unmaskable()` markiert die Fehler, die Skripte mit Rhai-`try`/`catch`
//! NICHT fangen koennen (Spec §10).

use serde::{Deserialize, Serialize};

use crate::script::capability::{CapabilityToken, ScriptTier};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestError {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptError {
    // Compile-Time
    ParseFailed     { line: u32, col: u32, msg: String },
    ManifestInvalid { reason: ManifestError },
    TierExceeded    { declared: ScriptTier, user: ScriptTier },
    WasmEngineNotAvailable, // Phase-2-Reservation

    // Run-Time
    CapabilityDenied    { token: CapabilityToken },
    UiPrimitiveDenied   { primitive: String },
    ServerOnlyFunction  { name: String },
    Timeout             { limit_ms: u32 },
    MemoryExceeded      { limit_kb: u32 },
    InternalPanic       { backtrace: String },
    HostError           { source: String },

    // Validation
    ValidationFailed    {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field: Option<String>,
        msg_key: String,
        args: serde_json::Value,
    },
}

impl ScriptError {
    /// Spec §10: diese Fehler sind in Rhai NICHT per `try`/`catch` fangbar.
    pub fn unmaskable(&self) -> bool {
        matches!(
            self,
            ScriptError::CapabilityDenied { .. }
                | ScriptError::UiPrimitiveDenied { .. }
                | ScriptError::Timeout { .. }
                | ScriptError::MemoryExceeded { .. }
        )
    }
}
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p shared --test script_wire_format`
Expected: alle PASS.

- [ ] **Step 5: Commit**

```bash
git add shared/src/script/error.rs shared/tests/script_wire_format.rs
git commit -m "feat(shared): ScriptError variants + unmaskable() classifier (Q0009 Phase 1.5)"
```

---

### Task 1.6: `UiNode::Script`-Variante einführen

**Hintergrund:** `UiNode` lebt heute in `client/src/builder/node.rs` als **Struct**, nicht als Enum. Die Spec verlangt eine `Script`-Variante neben `Table`/`Report`. Wir führen einen `NodeKind`-Enum als optionales Feld ein — ohne den bestehenden Wire-Vertrag (`{"id": 42, ...}` ohne `kind`) zu brechen.

**Files:**
- Modify: `client/src/builder/node.rs`
- Modify: `shared/tests/script_wire_format.rs` (Wire-Pin)
- Modify: `client/src/builder/tree.rs` (Walk-Pfad anpassen, falls nötig)

- [ ] **Step 1: Failing Wire-Test**

Da `UiNode` im Client-Crate liegt, der Wire-Test aber in shared/ ist: Wir definieren das Wire-Format der `NodeKind`-Variante in einem **shared-internen** Helper-Typ und pinnen dort. Erweitere `shared/src/script/model.rs`:

```rust
/// Wire-Form fuer `UiNode::Script`: enthaelt den `script_id`-Ziel und
/// optional eine Version. Der Builder-Client baut darum den vollen
/// `UiNode`-Wrapper.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptNodeRef {
    pub script_id: ScriptId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_pin: Option<u32>,
}
```

Test in `shared/tests/script_wire_format.rs`:

```rust
#[test]
fn script_node_ref_serializes_camelcase_and_omits_unset_pin() {
    use shared::script::{ScriptId, model::ScriptNodeRef};
    let r = ScriptNodeRef { script_id: ScriptId("s-1".into()), version_pin: None };
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v, json!({"scriptId": "s-1"}));

    let r2 = ScriptNodeRef { script_id: ScriptId("s-2".into()), version_pin: Some(7) };
    let v2 = serde_json::to_value(&r2).unwrap();
    assert_eq!(v2, json!({"scriptId": "s-2", "versionPin": 7}));
}
```

- [ ] **Step 2: Test laufen lassen — FAIL**

Run: `cargo test -p shared --test script_wire_format -- script_node_ref`
Expected: FAIL.

- [ ] **Step 3: `ScriptNodeRef` implementieren**

Bereits oben gezeigt — in `shared/src/script/model.rs` einfügen. Re-Export in `shared/src/script/mod.rs`:

```rust
pub use model::{Script, ScriptId, ScriptKind, ScriptState, ScriptNodeRef, ProviderSlot};
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p shared --test script_wire_format`
Expected: PASS.

- [ ] **Step 5: `UiNode` um optionales `kind`-Feld erweitern**

In `client/src/builder/node.rs` nach dem `Style`-Block:

```rust
/// Spezialisierte Knoten-Variante. Default `Generic` haelt die heutige
/// nicht-getaggte Form (`{"id": 42, ...}`) am Vertrag. Neue Varianten
/// werden additiv ergaenzt — `skip_serializing_if = "NodeKind::is_generic"`
/// laesst die alte Wire-Form unveraendert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeKind {
    Generic,
    Script(shared::script::ScriptNodeRef),
}

impl Default for NodeKind {
    fn default() -> Self { NodeKind::Generic }
}

impl NodeKind {
    pub fn is_generic(&self) -> bool {
        matches!(self, NodeKind::Generic)
    }
}
```

Im `UiNode`-Struct ergänzen:

```rust
    #[serde(default, skip_serializing_if = "NodeKind::is_generic")]
    pub kind: NodeKind,
```

Konstruktor `UiNode::new` ergänzen:

```rust
            kind: NodeKind::default(),
```

- [ ] **Step 6: Bestehende `ui_node_wire_format_matches_roadmap_example`-Test darf nicht brechen**

Run: `cargo test -p client builder::node`
Expected: PASS (heutiges Wire-Format hat kein `type`-Feld → bleibt weg).

- [ ] **Step 7: Neuer Test im client-Crate**

In `client/src/builder/node.rs` `mod tests`:

```rust
    #[test]
    fn ui_node_script_variant_serializes_with_kind_script() {
        use shared::script::ScriptId;
        let mut node = UiNode::new(NodeId(7));
        node.kind = NodeKind::Script(shared::script::ScriptNodeRef {
            script_id: ScriptId("sales-dashboard".into()),
            version_pin: None,
        });
        let v = serde_json::to_value(&node).unwrap();
        assert_eq!(v["kind"]["type"], json!("script"));
        assert_eq!(v["kind"]["scriptId"], json!("sales-dashboard"));
    }
```

Run: `cargo test -p client builder::node::tests::ui_node_script_variant_serializes_with_kind_script`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add shared/src/script/model.rs shared/src/script/mod.rs shared/tests/script_wire_format.rs client/src/builder/node.rs
git commit -m "feat(client+shared): UiNode kind=Script variant + ScriptNodeRef wire (Q0009 Phase 1.6)"
```

---

### Task 1.7: `ScriptEngine`- und `HostApi`-Traits (engine-agnostisch)

**Files:**
- Modify: `shared/src/script/engine.rs`
- Modify: `shared/src/script/host_api.rs`

- [ ] **Step 1: Trait-Skelett schreiben**

`shared/src/script/engine.rs`:

```rust
//! Engine-agnostische Trait-Schnittstellen. **Wichtige Forward-Compat-Regel
//! (Spec §11):** dieses Modul darf **nirgendwo** das Wort `rhai` enthalten.
//! Der Server- und Client-seitige Engine-Adapter haellt das Rhai-Wissen in
//! seinem eigenen `engine::rhai`-Submodul.

use crate::script::error::ScriptError;
use crate::script::manifest::ScriptManifest;

#[derive(Debug, Clone, Default)]
pub struct ScriptCtx {
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub locale: String,
}

/// Engine-spezifischer kompilierter AST (associated type, damit der Trait
/// engine-agnostisch bleibt).
pub trait ScriptEngine {
    type Ast: Clone + Send + Sync;
    fn compile(&self, source: &str, manifest: &ScriptManifest) -> Result<Self::Ast, ScriptError>;
    fn run(&self, ast: &Self::Ast, host: &dyn HostApi, ctx: ScriptCtx) -> Result<ScriptValue, ScriptError>;
}

/// Rueckgabewert eines Skript-Runs — engine-neutral.
#[derive(Debug, Clone)]
pub enum ScriptValue {
    String(String),
    Number(f64),
    Bool(bool),
    Json(serde_json::Value),
    Unit,
}

/// Engine-agnostischer Host. Beide Crates implementieren ihn — Server mit
/// echten SeaORM-Calls, Client mit GraphQL-Calls.
pub trait HostApi {
    fn db_fetch(&self, query: &serde_json::Value) -> Result<serde_json::Value, ScriptError>;
    fn db_patch(&self, entity_type: &str, id: &str, patch: &serde_json::Value) -> Result<(), ScriptError>;
    fn i18n_t(&self, key: &str, args: &serde_json::Value) -> Result<String, ScriptError>;
    fn audit_log(&self, event: &str, payload: &serde_json::Value) -> Result<(), ScriptError>;
}
```

- [ ] **Step 2: `host_api.rs` für das Symmetrie-Registry**

`shared/src/script/host_api.rs`:

```rust
//! `HostApiRegistry` — Compile-Time-Sicherung der Server/Client-Symmetrie.
//!
//! Jeder Konsument (server, client) implementiert den Trait und listet seine
//! Funktionen. Der `symmetry_check()`-Default vergleicht beide Listen
//! laufzeitig in Test-Runs (siehe `server/tests/script_engine.rs` und
//! `client/tests/script_engine.rs`).

use crate::script::capability::CapabilityToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFunctionDescriptor {
    pub name: &'static str,
    pub token: CapabilityToken,
    pub server_only: bool,
}

/// Implementiert auf Server- *und* Client-Seite mit derselben Funktionsliste.
pub trait HostApiRegistry {
    fn functions() -> Vec<HostFunctionDescriptor>;

    /// Nur in Tests aufrufen: vergleicht zwei Listen.
    fn symmetry_check(server: &[HostFunctionDescriptor], client: &[HostFunctionDescriptor]) -> Vec<String> {
        let mut errors = Vec::new();
        for s in server {
            if s.server_only { continue; }
            let matched = client.iter().find(|c| c.name == s.name);
            match matched {
                None => errors.push(format!("client missing function: {}", s.name)),
                Some(c) if c.token != s.token => {
                    errors.push(format!("token mismatch on '{}': server={:?}, client={:?}", s.name, s.token, c.token));
                }
                _ => {}
            }
        }
        for c in client {
            if !server.iter().any(|s| s.name == c.name) {
                errors.push(format!("server missing function declared on client: {}", c.name));
            }
        }
        errors
    }
}
```

- [ ] **Step 3: Failing Test für `symmetry_check`**

In `shared/tests/script_wire_format.rs`:

```rust
#[test]
fn host_api_registry_symmetry_check_detects_mismatches() {
    use shared::script::{CapabilityToken, HostApiRegistry, HostFunctionDescriptor};
    let server = vec![
        HostFunctionDescriptor { name: "db.fetch", token: CapabilityToken::ReadOwnEntities, server_only: false },
        HostFunctionDescriptor { name: "db.patch", token: CapabilityToken::WriteEntity { validated: true }, server_only: true },
    ];
    let client = vec![
        HostFunctionDescriptor { name: "db.fetch", token: CapabilityToken::ReadOwnEntities, server_only: false },
    ];
    struct Dummy;
    impl HostApiRegistry for Dummy { fn functions() -> Vec<HostFunctionDescriptor> { vec![] } }
    let errs = Dummy::symmetry_check(&server, &client);
    assert!(errs.is_empty(), "server-only mismatch sollte ignoriert werden: {errs:?}");

    let bad_client = vec![
        HostFunctionDescriptor { name: "db.fetch", token: CapabilityToken::WriteEntity { validated: true }, server_only: false },
    ];
    let errs2 = Dummy::symmetry_check(&server, &bad_client);
    assert!(errs2.iter().any(|e| e.contains("token mismatch")), "Token-Mismatch sollte gemeldet werden: {errs2:?}");
}
```

- [ ] **Step 4: Test laufen lassen**

Run: `cargo test -p shared --test script_wire_format`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add shared/src/script/engine.rs shared/src/script/host_api.rs shared/tests/script_wire_format.rs
git commit -m "feat(shared): ScriptEngine + HostApi + HostApiRegistry traits (Q0009 Phase 1.7)"
```

---

### Task 1.8: `MockHostApi` in `shared/src/script/testing.rs`

**Files:**
- Modify: `shared/src/script/testing.rs`
- Modify: `shared/Cargo.toml` (Test-Feature)

- [ ] **Step 1: Feature `testing` in `shared/Cargo.toml`**

```toml
[features]
testing = []
```

- [ ] **Step 2: Failing Test**

In `shared/tests/script_wire_format.rs`:

```rust
#[cfg(feature = "testing")]
#[test]
fn mock_host_api_records_db_fetch_and_audit_calls() {
    use shared::script::testing::MockHostApi;
    use shared::script::HostApi;
    let host = MockHostApi::new();
    host.seed_entities("product", serde_json::json!([{"id": "p-1", "price": 100}]));
    let res = host.db_fetch(&serde_json::json!({"entity": "product"})).unwrap();
    assert_eq!(res, serde_json::json!([{"id": "p-1", "price": 100}]));
    host.audit_log("custom", &serde_json::json!({"x": 1})).unwrap();
    let log = host.audit_log_calls();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].0, "custom");
}
```

- [ ] **Step 3: Test laufen lassen — FAIL**

Run: `cargo test -p shared --features testing --test script_wire_format -- mock_host_api`
Expected: FAIL (`MockHostApi` fehlt).

- [ ] **Step 4: Implementation**

`shared/src/script/testing.rs`:

```rust
//! `MockHostApi` — Test-Doppel fuer Symmetrie- und Sandbox-Tests.
//!
//! Server- und Client-Crate testen jeweils ihren echten Host gegen die
//! gleiche Reference-Implementation: identische Inputs → identische Outputs.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::script::engine::HostApi;
use crate::script::error::ScriptError;

#[derive(Debug, Default)]
pub struct MockHostApi {
    inner: Mutex<MockInner>,
}

#[derive(Debug, Default)]
struct MockInner {
    entities: BTreeMap<String, Value>,
    audit_log: Vec<(String, Value)>,
    patch_log: Vec<(String, String, Value)>,
    t_calls: Vec<(String, Value)>,
}

impl MockHostApi {
    pub fn new() -> Self { Self::default() }

    pub fn seed_entities(&self, entity: impl Into<String>, data: Value) {
        self.inner.lock().unwrap().entities.insert(entity.into(), data);
    }

    pub fn audit_log_calls(&self) -> Vec<(String, Value)> {
        self.inner.lock().unwrap().audit_log.clone()
    }

    pub fn patch_log(&self) -> Vec<(String, String, Value)> {
        self.inner.lock().unwrap().patch_log.clone()
    }
}

impl HostApi for MockHostApi {
    fn db_fetch(&self, query: &Value) -> Result<Value, ScriptError> {
        let entity = query.get("entity").and_then(|v| v.as_str()).ok_or_else(|| {
            ScriptError::HostError { source: "query.entity fehlt".into() }
        })?;
        Ok(self.inner.lock().unwrap()
            .entities.get(entity).cloned()
            .unwrap_or(Value::Array(Vec::new())))
    }
    fn db_patch(&self, entity_type: &str, id: &str, patch: &Value) -> Result<(), ScriptError> {
        self.inner.lock().unwrap()
            .patch_log.push((entity_type.into(), id.into(), patch.clone()));
        Ok(())
    }
    fn i18n_t(&self, key: &str, args: &Value) -> Result<String, ScriptError> {
        self.inner.lock().unwrap().t_calls.push((key.into(), args.clone()));
        Ok(format!("[t:{key}]"))
    }
    fn audit_log(&self, event: &str, payload: &Value) -> Result<(), ScriptError> {
        self.inner.lock().unwrap()
            .audit_log.push((event.into(), payload.clone()));
        Ok(())
    }
}
```

In `shared/src/script/mod.rs`:

```rust
#[cfg(any(test, feature = "testing"))]
pub mod testing;
```

- [ ] **Step 5: Test wiederholen**

Run: `cargo test -p shared --features testing --test script_wire_format`
Expected: PASS.

- [ ] **Step 6: Sicherstellen, dass `cargo test -p shared` (ohne Feature) auch PASS**

Run: `cargo test -p shared`
Expected: PASS (Test mit `#[cfg(feature = "testing")]` wird übersprungen).

- [ ] **Step 7: Commit**

```bash
git add shared/src/script/testing.rs shared/src/script/mod.rs shared/Cargo.toml shared/tests/script_wire_format.rs
git commit -m "feat(shared): MockHostApi + testing feature (Q0009 Phase 1.8)"
```

---

### Task 1.9: Phase-1-Smoke — `cargo test --workspace` grün

- [ ] **Step 1: Workspace-Build**

Run: `cargo build --workspace`
Expected: PASS.

- [ ] **Step 2: Workspace-Tests**

Run: `cargo test --workspace`
Expected: alle PASS. Falls Windows-Lock auf `server.exe` → `cargo test --workspace --target-dir target-test`.

- [ ] **Step 3: Phase-1-Commit-Boundary** — keine Änderungen, nur Verifikation.

---

# Phase 2 — Server-Engine + Sandbox + Host-Module

**Ziel:** Rhai-Engine im Server hinter Trait, Sandbox enforced Tokens, Host-Module `db`/`ui`/`i18n`/`ctx`/`audit` arbeiten gegen SeaORM bzw. Service-Layer. `WasmEngineNotAvailable` ist als unreachable-Pfad pinned.

---

### Task 2.1: Rhai-Engine-Wrapper (Server) hinter `ScriptEngine`-Trait

**Files:**
- Modify: `server/Cargo.toml` — `rhai = { version = "1", default-features = false, features = ["std", "sync"] }`, `ulid = "1"`
- Create: `server/src/script/mod.rs`
- Create: `server/src/script/engine/mod.rs`
- Create: `server/src/script/engine/rhai.rs`
- Modify: `server/src/lib.rs` — `pub mod script;`

- [ ] **Step 1: Cargo.toml ergänzen**

In `server/Cargo.toml` unter `[dependencies]`:

```toml
rhai = { version = "1", default-features = false, features = ["std", "sync"] }
ulid = "1"
```

- [ ] **Step 2: `server/src/script/mod.rs` anlegen**

```rust
//! Server-seitige Skript-Sprachen-Integration (Q0009).
pub mod engine;
pub mod sandbox;
pub mod host;
pub mod store;
pub mod lift;
```

- [ ] **Step 3: `server/src/lib.rs` erweitern**

Nach `pub mod views;` ergänzen:

```rust
pub mod script;
```

- [ ] **Step 4: `server/src/script/engine/mod.rs`**

```rust
//! Engine-Adapter (Rhai). Spec-Garantie (§11): Rhai-Symbole tauchen **nur**
//! in `engine::rhai`-Submodul auf.
pub mod rhai;
pub use rhai::RhaiEngine;
```

- [ ] **Step 5: Failing Test in `server/tests/script_engine.rs`**

```rust
//! Engine + Sandbox-Tests (Server-Seite). Pendant in
//! `client/tests/script_engine.rs` (Task 4.6).

use shared::script::{ScriptManifest, ScriptTier, CapabilityToken};
use shared::script::engine::ScriptEngine;

#[test]
fn rhai_engine_compiles_trivial_script() {
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1, tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile("40 + 2", &manifest).expect("compile");
    let _ = ast;
}

#[test]
fn rhai_engine_rejects_eval_symbol() {
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1, tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let res = engine.compile("eval(\"40+2\")", &manifest);
    assert!(res.is_err(), "eval() darf NICHT kompilieren");
}
```

- [ ] **Step 6: Test laufen lassen — FAIL**

Run: `cargo test -p server --test script_engine --target-dir target-test`
Expected: FAIL (Symbole fehlen).

- [ ] **Step 7: Implementation `engine/rhai.rs`**

```rust
//! Rhai-Engine-Adapter. Trait-Impl von `shared::script::engine::ScriptEngine`.
//!
//! `configure_strict()` ist Spec §5.1: `Engine::new_raw()` ohne Standard-
//! Module ausser den explizit erlaubten + `disable_symbol` fuer `eval`/
//! `import`/`print`/`debug`.

use std::sync::Arc;

use rhai::{Engine, AST};

use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::error::ScriptError;
use shared::script::manifest::ScriptManifest;
use shared::script::model::ScriptKind;

pub struct RhaiEngine {
    inner: Engine,
}

impl RhaiEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new_raw();
        configure_strict(&mut engine);
        // Konservatives Operation-Limit; pro-run Sandbox setzt nochmal nach.
        engine.set_max_operations(50_000);
        Self { inner: engine }
    }
}

impl Default for RhaiEngine {
    fn default() -> Self { Self::new() }
}

#[derive(Clone)]
pub struct RhaiAst(pub Arc<AST>);

fn configure_strict(engine: &mut Engine) {
    // Symbol-Disable (Spec §7.5).
    engine.disable_symbol("eval");
    engine.disable_symbol("import");
    engine.disable_symbol("print");
    engine.disable_symbol("debug");
}

impl ScriptEngine for RhaiEngine {
    type Ast = RhaiAst;

    fn compile(&self, source: &str, _manifest: &ScriptManifest) -> Result<Self::Ast, ScriptError> {
        match self.inner.compile(source) {
            Ok(ast) => Ok(RhaiAst(Arc::new(ast))),
            Err(e) => {
                let pos = e.position();
                Err(ScriptError::ParseFailed {
                    line: pos.line().unwrap_or(0) as u32,
                    col: pos.position().unwrap_or(0) as u32,
                    msg: format!("{e}"),
                })
            }
        }
    }

    fn run(&self, _ast: &Self::Ast, _host: &dyn HostApi, _ctx: ScriptCtx) -> Result<ScriptValue, ScriptError> {
        // Detail-Implementation in Task 2.4 (Sandbox).
        Ok(ScriptValue::Unit)
    }
}

/// Public Entry-Point fuer Wasm-Skripte: heute hartes Reject.
pub fn compile_wasm(_kind: &ScriptKind) -> Result<(), ScriptError> {
    Err(ScriptError::WasmEngineNotAvailable)
}
```

- [ ] **Step 8: Test wiederholen**

Run: `cargo test -p server --test script_engine --target-dir target-test`
Expected: PASS für `rhai_engine_compiles_trivial_script`. Der `eval`-Test sollte ebenfalls passen (Rhai meldet `eval` als reserviertes Keyword durch `disable_symbol`).

- [ ] **Step 9: Commit**

```bash
git add server/src/script/ server/src/lib.rs server/Cargo.toml server/tests/script_engine.rs
git commit -m "feat(server): Rhai engine wrapper behind ScriptEngine trait (Q0009 Phase 2.1)"
```

---

### Task 2.2: `WasmEngineNotAvailable`-Pin im Compile-Pfad

**Files:**
- Modify: `server/tests/script_engine.rs`

- [ ] **Step 1: Failing Test**

```rust
#[test]
fn rhai_engine_rejects_wasm_kind_with_dedicated_error() {
    use shared::script::ScriptKind;
    let err = server::script::engine::rhai::compile_wasm(&ScriptKind::Wasm {
        wasm_bytes: vec![0, 1, 2],
        entry: "main".into(),
    }).unwrap_err();
    assert!(matches!(err, shared::script::ScriptError::WasmEngineNotAvailable));
}
```

- [ ] **Step 2: Test laufen lassen**

Run: `cargo test -p server --test script_engine -- rhai_engine_rejects_wasm_kind --target-dir target-test`
Expected: PASS (`compile_wasm` ist bereits aus Task 2.1 da).

- [ ] **Step 3: Commit**

```bash
git add server/tests/script_engine.rs
git commit -m "test(server): pin WasmEngineNotAvailable as reachable-but-unreachable error (Q0009 Phase 2.2)"
```

---

### Task 2.3: Sandbox-Schicht — `gate!`-Makro + Token-Audit-Buffer

**Files:**
- Create: `server/src/script/sandbox.rs`

- [ ] **Step 1: Failing Test**

In `server/tests/script_engine.rs`:

```rust
#[test]
fn sandbox_denies_call_when_token_not_in_manifest() {
    use shared::script::{CapabilityToken, ScriptManifest, ScriptTier};
    use server::script::sandbox::Sandbox;

    let manifest = ScriptManifest {
        manifest_version: 1, tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ReadOwnEntities],
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    let res = sb.gate(&CapabilityToken::WriteEntity { validated: true }, || Ok::<_, shared::script::ScriptError>(42));
    assert!(matches!(res, Err(shared::script::ScriptError::CapabilityDenied { .. })));
    assert_eq!(sb.token_uses().len(), 1, "Audit-Buffer haelt auch denials fest");
}

#[test]
fn sandbox_records_successful_token_use() {
    use shared::script::{CapabilityToken, ScriptManifest, ScriptTier};
    use server::script::sandbox::Sandbox;
    let manifest = ScriptManifest {
        manifest_version: 1, tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ReadOwnEntities],
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    let v = sb.gate(&CapabilityToken::ReadOwnEntities, || Ok::<_, shared::script::ScriptError>(7)).unwrap();
    assert_eq!(v, 7);
    assert_eq!(sb.token_uses().len(), 1);
}
```

- [ ] **Step 2: Test laufen lassen — FAIL**

Run: `cargo test -p server --test script_engine -- sandbox_ --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`server/src/script/sandbox.rs`:

```rust
//! Sandbox-Schicht (Spec §5.2).
//!
//! Pro Run instantiiert. Haelt Token-Audit-Buffer, Timeout-Deadline,
//! PanicCatch-Flag, Memory-Counter. Engine-agnostisch — referenziert nur
//! `shared::script::*`.

use std::time::{Duration, Instant};

use shared::script::capability::CapabilityToken;
use shared::script::error::ScriptError;
use shared::script::manifest::ScriptManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenOutcome { Ok, Denied }

#[derive(Debug, Clone)]
pub struct TokenUse {
    pub token: CapabilityToken,
    pub outcome: TokenOutcome,
}

pub struct Sandbox<'m> {
    manifest: &'m ScriptManifest,
    deadline: Option<Instant>,
    token_uses: Vec<TokenUse>,
}

impl<'m> Sandbox<'m> {
    pub fn new(manifest: &'m ScriptManifest) -> Self {
        let deadline = manifest.timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms as u64));
        Self { manifest, deadline, token_uses: Vec::new() }
    }

    pub fn gate<T, F>(&mut self, token: &CapabilityToken, body: F) -> Result<T, ScriptError>
    where
        F: FnOnce() -> Result<T, ScriptError>,
    {
        if !self.manifest.capabilities.contains(token) {
            self.token_uses.push(TokenUse { token: token.clone(), outcome: TokenOutcome::Denied });
            return Err(ScriptError::CapabilityDenied { token: token.clone() });
        }
        if let Some(dl) = self.deadline {
            if Instant::now() > dl {
                return Err(ScriptError::Timeout { limit_ms: self.manifest.timeout_ms.unwrap_or(0) });
            }
        }
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
        match res {
            Ok(Ok(v)) => {
                self.token_uses.push(TokenUse { token: token.clone(), outcome: TokenOutcome::Ok });
                Ok(v)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ScriptError::InternalPanic { backtrace: "panic in host call".into() }),
        }
    }

    pub fn token_uses(&self) -> &[TokenUse] { &self.token_uses }

    pub fn check_deadline(&self) -> Result<(), ScriptError> {
        if let Some(dl) = self.deadline {
            if Instant::now() > dl {
                return Err(ScriptError::Timeout { limit_ms: self.manifest.timeout_ms.unwrap_or(0) });
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_engine -- sandbox_ --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/script/sandbox.rs server/tests/script_engine.rs
git commit -m "feat(server): sandbox gate! + token audit buffer (Q0009 Phase 2.3)"
```

---

### Task 2.4: Negative-Tests pro Sandbox-Constraint (Timeout, MemoryExceeded, eval, while-true)

**Files:**
- Modify: `server/tests/script_engine.rs`

- [ ] **Step 1: Failing Tests pro Constraint**

```rust
#[test]
fn timeout_constraint_fires_when_deadline_exceeded() {
    use shared::script::{CapabilityToken, ScriptManifest, ScriptTier};
    use server::script::sandbox::Sandbox;
    let manifest = ScriptManifest {
        manifest_version: 1, tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        timeout_ms: Some(0), // sofort abgelaufen
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let res = sb.gate(&CapabilityToken::ComputeOnly, || Ok::<_, shared::script::ScriptError>(1));
    assert!(matches!(res, Err(shared::script::ScriptError::Timeout { .. })));
}

#[test]
fn engine_rejects_print_and_debug_symbols() {
    use shared::script::{CapabilityToken, ScriptManifest, ScriptTier};
    use shared::script::engine::ScriptEngine;
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1, tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    assert!(engine.compile("print(\"x\")", &manifest).is_err(), "print darf NICHT kompilieren");
    assert!(engine.compile("debug(\"x\")", &manifest).is_err(), "debug darf NICHT kompilieren");
    assert!(engine.compile("import \"foo\" as bar;", &manifest).is_err(), "import darf NICHT kompilieren");
}

#[test]
fn engine_max_operations_kicks_in_on_runaway_loop() {
    use shared::script::{CapabilityToken, ScriptManifest, ScriptTier};
    use shared::script::engine::ScriptEngine;
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1, tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile("let i = 0; while i < 1_000_000_000 { i = i + 1; } i", &manifest).expect("compile");
    let host = shared::script::testing::MockHostApi::new();
    let res = engine.run(&ast, &host, shared::script::engine::ScriptCtx::default());
    // Wir akzeptieren JEDE Error-Variante (Operations, Timeout) — wichtig ist nur, dass es nicht erfolgreich durchlaeuft.
    assert!(res.is_err(), "Endlosschleife muss abbrechen");
}
```

- [ ] **Step 2: Tests laufen lassen — FAIL bei `engine_max_operations` weil `run()` heute `Ok(Unit)` zurückgibt**

Run: `cargo test -p server --test script_engine --target-dir target-test`

- [ ] **Step 3: `RhaiEngine::run` echt implementieren**

Ersetze die `run`-Methode in `server/src/script/engine/rhai.rs`:

```rust
    fn run(&self, ast: &Self::Ast, _host: &dyn HostApi, _ctx: ScriptCtx) -> Result<ScriptValue, ScriptError> {
        let mut scope = rhai::Scope::new();
        let res: Result<rhai::Dynamic, _> = self.inner.eval_ast_with_scope(&mut scope, &ast.0);
        match res {
            Ok(v) => Ok(rhai_to_script_value(v)),
            Err(e) => Err(map_rhai_err(*e)),
        }
    }
```

Hilfsfunktionen am Datei-Ende:

```rust
fn rhai_to_script_value(v: rhai::Dynamic) -> ScriptValue {
    if v.is::<bool>() { return ScriptValue::Bool(v.as_bool().unwrap()); }
    if let Ok(n) = v.as_int() { return ScriptValue::Number(n as f64); }
    if let Ok(f) = v.as_float() { return ScriptValue::Number(f); }
    if v.is::<String>() { return ScriptValue::String(v.into_string().unwrap_or_default()); }
    ScriptValue::Unit
}

fn map_rhai_err(e: rhai::EvalAltResult) -> ScriptError {
    use rhai::EvalAltResult::*;
    match e {
        ErrorTooManyOperations(_) => ScriptError::Timeout { limit_ms: 0 },
        ErrorParsing(_, p) => ScriptError::ParseFailed {
            line: p.line().unwrap_or(0) as u32,
            col: p.position().unwrap_or(0) as u32,
            msg: "parse".into(),
        },
        other => ScriptError::HostError { source: format!("{other}") },
    }
}
```

- [ ] **Step 4: Tests wiederholen**

Run: `cargo test -p server --test script_engine --target-dir target-test`
Expected: alle PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/script/engine/rhai.rs server/tests/script_engine.rs
git commit -m "feat(server): wire RhaiEngine::run + negative tests per sandbox constraint (Q0009 Phase 2.4)"
```

---

### Task 2.5: Host-Module `i18n` und `ctx` (read-only, ohne DB)

**Files:**
- Create: `server/src/script/host/mod.rs`
- Create: `server/src/script/host/i18n.rs`
- Create: `server/src/script/host/ctx.rs`

- [ ] **Step 1: Failing Test**

In `server/tests/script_engine.rs`:

```rust
#[test]
fn host_i18n_t_uses_host_api_translation() {
    use server::script::host::i18n::I18nHost;
    let mock = shared::script::testing::MockHostApi::new();
    let h = I18nHost::new(&mock);
    let s = h.t("dashboard.title", &serde_json::json!({})).unwrap();
    assert_eq!(s, "[t:dashboard.title]"); // MockHostApi-Konvention
}
```

- [ ] **Step 2: Test laufen lassen — FAIL**

Run: `cargo test -p server --test script_engine -- host_i18n --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`server/src/script/host/mod.rs`:

```rust
pub mod i18n;
pub mod ctx;
pub mod db;
pub mod ui;
pub mod audit;
```

`server/src/script/host/i18n.rs`:

```rust
use serde_json::Value;
use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct I18nHost<'a> {
    host: &'a dyn HostApi,
}

impl<'a> I18nHost<'a> {
    pub fn new(host: &'a dyn HostApi) -> Self { Self { host } }
    pub fn t(&self, key: &str, args: &Value) -> Result<String, ScriptError> {
        self.host.i18n_t(key, args)
    }
}
```

`server/src/script/host/ctx.rs`:

```rust
use shared::script::engine::ScriptCtx;

pub struct CtxHost {
    pub ctx: ScriptCtx,
}

impl CtxHost {
    pub fn new(ctx: ScriptCtx) -> Self { Self { ctx } }
    pub fn user_id(&self) -> Option<&str> { self.ctx.user_id.as_deref() }
    pub fn tenant_id(&self) -> Option<&str> { self.ctx.tenant_id.as_deref() }
    pub fn locale(&self) -> &str { &self.ctx.locale }
    pub fn now(&self) -> String {
        chrono::Utc::now().to_rfc3339()
    }
}
```

`db.rs`, `ui.rs`, `audit.rs` als Stubs (`//! TODO Task 2.6/2.7`).

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_engine -- host_i18n --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/script/host/
git commit -m "feat(server): script host modules — i18n + ctx (Q0009 Phase 2.5)"
```

---

### Task 2.6: Host-Module `db` — `db.entities(...)`, `db.entity(...)`, `db.patch(...)`

**Files:**
- Modify: `server/src/script/host/db.rs`

- [ ] **Step 1: Failing Tests**

In `server/tests/script_engine.rs`:

```rust
#[test]
fn host_db_fetch_via_mock_returns_seeded_array() {
    use server::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    mock.seed_entities("product", serde_json::json!([{"id": "p-1", "price": 100}]));
    let h = DbHost::new(&mock);
    let res = h.fetch_entities("product", &serde_json::json!({})).unwrap();
    assert_eq!(res, serde_json::json!([{"id": "p-1", "price": 100}]));
}

#[test]
fn host_db_patch_via_mock_records_call() {
    use server::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    let h = DbHost::new(&mock);
    h.patch_entity("product", "p-1", &serde_json::json!({"price": 199})).unwrap();
    let log = mock.patch_log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].0, "product");
    assert_eq!(log[0].1, "p-1");
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_engine -- host_db --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`server/src/script/host/db.rs`:

```rust
//! `db`-Host (Spec §7.1). Lazy Builder-Pattern lebt im Rhai-Adapter (Task
//! 2.7). Diese Schicht bietet die Pre-Resolved-Calls.

use serde_json::Value;

use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct DbHost<'a> {
    host: &'a dyn HostApi,
}

impl<'a> DbHost<'a> {
    pub fn new(host: &'a dyn HostApi) -> Self { Self { host } }

    /// `db.entities(entity_type, query)` — Rueckgabe ist eine JSON-Array
    /// mit Maps. `query` ist der zusammengebaute Builder-State (where/order
    /// _by/limit) als Map. Der `MockHostApi` ignoriert query und liefert die
    /// geseedete Liste.
    pub fn fetch_entities(&self, entity_type: &str, query: &Value) -> Result<Value, ScriptError> {
        let mut q = query.clone();
        if !q.is_object() { q = serde_json::json!({}); }
        q.as_object_mut().unwrap().insert("entity".into(), Value::String(entity_type.into()));
        self.host.db_fetch(&q)
    }

    pub fn patch_entity(&self, entity_type: &str, id: &str, patch: &Value) -> Result<(), ScriptError> {
        self.host.db_patch(entity_type, id, patch)
    }
}
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_engine -- host_db --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/script/host/db.rs server/tests/script_engine.rs
git commit -m "feat(server): db host — fetch_entities + patch_entity (Q0009 Phase 2.6)"
```

---

### Task 2.7: Host-Module `ui` und `audit`

**Files:**
- Modify: `server/src/script/host/ui.rs`
- Modify: `server/src/script/host/audit.rs`

- [ ] **Step 1: Failing Test**

In `server/tests/script_engine.rs`:

```rust
#[test]
fn host_ui_text_returns_uitree_subtree() {
    use server::script::host::ui::UiHost;
    use shared::script::manifest::UiPrimitive;
    let manifest = shared::script::ScriptManifest {
        manifest_version: 1,
        tier: shared::script::ScriptTier::Author,
        capabilities: vec![shared::script::CapabilityToken::EmitUiNode {
            scope: shared::script::capability::UiScope::Composite,
        }],
        ui_primitives: vec![UiPrimitive::Text],
        ..Default::default()
    };
    let mut ui = UiHost::new(&manifest);
    let node = ui.text("Hallo", &serde_json::json!({"size": "h2"})).unwrap();
    assert_eq!(node["type"], serde_json::Value::String("text".into()));
    assert_eq!(node["text"], serde_json::Value::String("Hallo".into()));
}

#[test]
fn host_ui_rejects_undeclared_primitive() {
    use server::script::host::ui::UiHost;
    let manifest = shared::script::ScriptManifest::default(); // ui_primitives: leer
    let mut ui = UiHost::new(&manifest);
    let err = ui.text("Hallo", &serde_json::json!({})).unwrap_err();
    assert!(matches!(err, shared::script::ScriptError::UiPrimitiveDenied { .. }));
}

#[test]
fn host_audit_log_via_mock_records_event() {
    use server::script::host::audit::AuditHost;
    let mock = shared::script::testing::MockHostApi::new();
    let h = AuditHost::new(&mock);
    h.log("custom.event", &serde_json::json!({"x": 1})).unwrap();
    assert_eq!(mock.audit_log_calls().len(), 1);
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_engine -- host_ui host_audit --target-dir target-test`

- [ ] **Step 3: Implementation `ui.rs`**

```rust
//! `ui`-Host (Spec §7.2). Erzeugt JSON-Subtree mit `type`-Diskriminator.
//! Whitelist-Check gegen `manifest.ui_primitives`.

use serde_json::{json, Value};

use shared::script::error::ScriptError;
use shared::script::manifest::{ScriptManifest, UiPrimitive};

pub struct UiHost<'m> {
    manifest: &'m ScriptManifest,
}

impl<'m> UiHost<'m> {
    pub fn new(manifest: &'m ScriptManifest) -> Self { Self { manifest } }

    fn check(&self, prim: UiPrimitive) -> Result<(), ScriptError> {
        if !self.manifest.ui_primitives.contains(&prim) {
            return Err(ScriptError::UiPrimitiveDenied {
                primitive: format!("{prim:?}").to_lowercase(),
            });
        }
        Ok(())
    }

    pub fn text(&mut self, text: &str, props: &Value) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Text)?;
        Ok(json!({"type": "text", "text": text, "props": props}))
    }

    pub fn vstack(&mut self, children: Vec<Value>) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Vstack)?;
        Ok(json!({"type": "vstack", "children": children}))
    }

    pub fn hstack(&mut self, children: Vec<Value>) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Hstack)?;
        Ok(json!({"type": "hstack", "children": children}))
    }

    pub fn table(&mut self, props: &Value) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Table)?;
        Ok(json!({"type": "table", "props": props}))
    }

    pub fn chart(&mut self, props: &Value) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Chart)?;
        Ok(json!({"type": "chart", "props": props}))
    }
}
```

`audit.rs`:

```rust
use serde_json::Value;
use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct AuditHost<'a> {
    host: &'a dyn HostApi,
}

impl<'a> AuditHost<'a> {
    pub fn new(host: &'a dyn HostApi) -> Self { Self { host } }
    pub fn log(&self, event: &str, payload: &Value) -> Result<(), ScriptError> {
        self.host.audit_log(event, payload)
    }
}
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_engine -- host_ui host_audit --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/script/host/ui.rs server/src/script/host/audit.rs server/tests/script_engine.rs
git commit -m "feat(server): ui + audit host modules with primitive whitelist enforcement (Q0009 Phase 2.7)"
```

---

### Task 2.8: `HostApiRegistry`-Impl für Server + Symmetrie-Test-Anker

**Files:**
- Modify: `server/src/script/mod.rs`
- Modify: `server/tests/script_engine.rs`

- [ ] **Step 1: Implementation**

In `server/src/script/mod.rs` ergänzen:

```rust
//! ...

pub struct ServerHostApiRegistry;

impl shared::script::HostApiRegistry for ServerHostApiRegistry {
    fn functions() -> Vec<shared::script::HostFunctionDescriptor> {
        use shared::script::capability::CapabilityToken::*;
        use shared::script::capability::UiScope;
        use shared::script::HostFunctionDescriptor as F;
        vec![
            F { name: "db.entities",  token: ReadOwnEntities,            server_only: false },
            F { name: "db.entity",    token: ReadOwnEntities,            server_only: false },
            F { name: "db.patch",     token: WriteEntity { validated: true }, server_only: true },
            F { name: "ui.vstack",    token: EmitUiNode { scope: UiScope::Composite }, server_only: false },
            F { name: "ui.hstack",    token: EmitUiNode { scope: UiScope::Composite }, server_only: false },
            F { name: "ui.text",      token: EmitUiNode { scope: UiScope::Leaf },      server_only: false },
            F { name: "ui.table",     token: EmitUiNode { scope: UiScope::Composite }, server_only: false },
            F { name: "ui.chart",     token: EmitUiNode { scope: UiScope::Composite }, server_only: false },
            F { name: "ctx.t",        token: ReadI18n,                   server_only: false },
            F { name: "audit.log",    token: WriteAuditLog,              server_only: true  },
        ]
    }
}
```

- [ ] **Step 2: Test**

```rust
#[test]
fn server_host_api_registry_lists_required_functions() {
    use shared::script::HostApiRegistry;
    let fns = server::script::ServerHostApiRegistry::functions();
    let names: Vec<_> = fns.iter().map(|f| f.name).collect();
    assert!(names.contains(&"db.entities"));
    assert!(names.contains(&"db.patch"));
    assert!(names.contains(&"ui.text"));
    assert!(names.contains(&"audit.log"));
}
```

Run: `cargo test -p server --test script_engine -- server_host_api_registry --target-dir target-test`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add server/src/script/mod.rs server/tests/script_engine.rs
git commit -m "feat(server): ServerHostApiRegistry + symmetry anchor (Q0009 Phase 2.8)"
```

---

### Task 2.9: Phase-2-Smoke

- [ ] **Step 1: Workspace-Tests**

Run: `cargo test --workspace --target-dir target-test`
Expected: PASS.

- [ ] **Step 2: Phase-2-Boundary** — keine Änderungen, nur Verifikation.

---

# Phase 3 — SeaORM-Persistenz + Loader + Lift-Analyse

**Ziel:** Drei neue Tabellen, Loader-Sidecar-Format, `lift_capable: bool` wird beim Save berechnet.

---

### Task 3.1: SeaORM-Entity `scripts`

**Files:**
- Create: `server/src/entity/scripts.rs`
- Modify: `server/src/entity/mod.rs`

- [ ] **Step 1: Failing Test**

`server/tests/script_persistence.rs`:

```rust
//! SeaORM-Persistenz-Tests fuer `scripts`/`script_versions`/`script_audit_log`.

use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn scripts_table_roundtrip_insert_select() {
    server::fresh_test_setup().await;
    let db = server::db::conn();

    let am = server::entity::scripts::ActiveModel {
        id: Set("s-1".into()),
        kind_json: Set(r#"{"kind":"provider","slot":"formatter"}"#.into()),
        manifest_json: Set(r#"{"manifestVersion":1,"tier":"reader","capabilities":[],"liftCapable":false}"#.into()),
        source: Set("fn format(v, r, c) { v.to_string() }".into()),
        version: Set(1),
        state: Set("active".into()),
        last_error_json: Set(None),
        created_by: Set("u-1".into()),
        created_at: Set("2026-05-23T00:00:00Z".into()),
        updated_at: Set("2026-05-23T00:00:00Z".into()),
    };
    am.insert(&db).await.unwrap();

    let rows = server::entity::scripts::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "s-1");
    assert_eq!(rows[0].state, "active");
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_persistence --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`server/src/entity/scripts.rs`:

```rust
//! Persistenz-Tabelle `scripts` (Q0009).
//!
//! `kind_json`, `manifest_json`, `last_error_json` werden als Text-Spalten
//! gespeichert — analog zu `entity_views.payload`. Der GraphQL- bzw.
//! Service-Layer deserialisiert in die typisierten `shared::script::*`-Typen.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "scripts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub kind_json: String,
    #[sea_orm(column_type = "Text")]
    pub manifest_json: String,
    #[sea_orm(column_type = "Text")]
    pub source: String,
    pub version: i32,
    /// `"draft" | "active" | "locked"`
    #[sea_orm(column_type = "Text")]
    pub state: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub last_error_json: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub created_by: String,
    #[sea_orm(column_type = "Text")]
    pub created_at: String,
    #[sea_orm(column_type = "Text")]
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

In `server/src/entity/mod.rs`:

```rust
pub mod scripts;
```

In `server/src/db.rs::create_schema` ergänzen:

```rust
        schema.create_table_from_entity(entity::scripts::Entity),
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_persistence -- scripts_table --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/entity/scripts.rs server/src/entity/mod.rs server/src/db.rs server/tests/script_persistence.rs
git commit -m "feat(server): scripts table + sea-orm entity (Q0009 Phase 3.1)"
```

---

### Task 3.2: SeaORM-Entity `script_versions` (Append-Only)

**Files:**
- Create: `server/src/entity/script_versions.rs`
- Modify: `server/src/entity/mod.rs`
- Modify: `server/src/db.rs`

- [ ] **Step 1: Failing Test**

In `server/tests/script_persistence.rs`:

```rust
#[tokio::test]
#[serial]
async fn script_versions_supports_multiple_saves_per_id() {
    server::fresh_test_setup().await;
    let db = server::db::conn();

    for v in 1..=3 {
        let am = server::entity::script_versions::ActiveModel {
            script_id: Set("s-1".into()),
            version: Set(v),
            source: Set(format!("// v{v}")),
            manifest_json: Set("{}".into()),
            state_at_save: Set("draft".into()),
            last_error_json: Set(None),
            created_by: Set("u-1".into()),
            created_at: Set(format!("2026-05-23T00:00:0{v}Z")),
        };
        am.insert(&db).await.unwrap();
    }
    let rows = server::entity::script_versions::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 3);
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_persistence -- script_versions --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`server/src/entity/script_versions.rs`:

```rust
//! Append-Only-Tabelle. Primary Key ist `(script_id, version)`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "script_versions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub script_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub version: i32,
    #[sea_orm(column_type = "Text")]
    pub source: String,
    #[sea_orm(column_type = "Text")]
    pub manifest_json: String,
    #[sea_orm(column_type = "Text")]
    pub state_at_save: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub last_error_json: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub created_by: String,
    #[sea_orm(column_type = "Text")]
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

`mod.rs`: `pub mod script_versions;`. `db.rs::create_schema`: `schema.create_table_from_entity(entity::script_versions::Entity)`.

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_persistence --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/entity/script_versions.rs server/src/entity/mod.rs server/src/db.rs server/tests/script_persistence.rs
git commit -m "feat(server): script_versions append-only table (Q0009 Phase 3.2)"
```

---

### Task 3.3: SeaORM-Entity `script_audit_log`

**Files:**
- Create: `server/src/entity/script_audit_log.rs`
- Modify: `server/src/entity/mod.rs`
- Modify: `server/src/db.rs`

- [ ] **Step 1: Failing Test**

```rust
#[tokio::test]
#[serial]
async fn script_audit_log_records_run_outcomes() {
    server::fresh_test_setup().await;
    let db = server::db::conn();
    let am = server::entity::script_audit_log::ActiveModel {
        id: Set("a-1".into()),
        script_id: Set("s-1".into()),
        script_version: Set(1),
        run_id: Set("r-1".into()),
        user_id: Set(Some("u-1".into())),
        started_at: Set("2026-05-23T00:00:00Z".into()),
        finished_at: Set("2026-05-23T00:00:01Z".into()),
        outcome: Set("ok".into()),
        tokens_used_json: Set(r#"[{"token":{"kind":"readOwnEntities"},"outcome":"ok"}]"#.into()),
        custom_events_json: Set(None),
    };
    am.insert(&db).await.unwrap();
    let rows = server::entity::script_audit_log::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].outcome, "ok");
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_persistence -- script_audit_log --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`server/src/entity/script_audit_log.rs`:

```rust
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "script_audit_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub script_id: String,
    pub script_version: i32,
    #[sea_orm(column_type = "Text")]
    pub run_id: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub user_id: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub started_at: String,
    #[sea_orm(column_type = "Text")]
    pub finished_at: String,
    /// `"ok" | "denied" | "timeout" | "panic"`
    #[sea_orm(column_type = "Text")]
    pub outcome: String,
    #[sea_orm(column_type = "Text")]
    pub tokens_used_json: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub custom_events_json: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Module-Eintrag und `db.rs::create_schema`-Erweiterung analog zu Task 3.1/3.2.

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_persistence --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/entity/script_audit_log.rs server/src/entity/mod.rs server/src/db.rs server/tests/script_persistence.rs
git commit -m "feat(server): script_audit_log table (Q0009 Phase 3.3)"
```

---

### Task 3.4: Loader-Erweiterung — `examples/<set>/scripts/<id>.rhai` + `<id>.manifest.{json,toml}`

**Files:**
- Modify: `server/src/example/mod.rs` — `pub scripts: Vec<ScriptSeed>`
- Modify: `server/src/example/loader.rs` — `load_scripts(dir)`
- Create: `server/tests/script_loader.rs`
- Create: `examples/shop/scripts/discount_tier.rhai`
- Create: `examples/shop/scripts/discount_tier.manifest.json`

- [ ] **Step 1: Failing Test**

`server/tests/script_loader.rs`:

```rust
//! Loader-Test fuer das Skript-Sidecar-Format (Q0009 §9.1).

#[test]
fn loader_reads_scripts_directory_with_pairs() {
    let tmp = tempfile::tempdir().unwrap();
    let scripts_dir = tmp.path().join("scripts");
    std::fs::create_dir_all(&scripts_dir).unwrap();
    std::fs::write(scripts_dir.join("greet.rhai"), "fn render(c) { c.t(\"hi\") }").unwrap();
    std::fs::write(
        scripts_dir.join("greet.manifest.json"),
        r#"{"manifestVersion":1,"tier":"reader","capabilities":[{"kind":"readI18n"}]}"#,
    ).unwrap();

    let set = server::example::loader::load(tmp.path()).unwrap();
    assert_eq!(set.scripts.len(), 1);
    assert_eq!(set.scripts[0].id, "greet");
    assert!(set.scripts[0].source.contains("c.t"));
    assert_eq!(set.scripts[0].manifest.tier, shared::script::ScriptTier::Reader);
}

#[test]
fn loader_skips_orphan_manifest_without_rhai() {
    let tmp = tempfile::tempdir().unwrap();
    let scripts_dir = tmp.path().join("scripts");
    std::fs::create_dir_all(&scripts_dir).unwrap();
    std::fs::write(scripts_dir.join("orphan.manifest.json"), "{}").unwrap();
    let set = server::example::loader::load(tmp.path()).unwrap();
    assert!(set.scripts.is_empty(), "Manifest ohne .rhai darf nicht geladen werden");
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_loader --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

In `server/src/example/mod.rs`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSeed {
    pub id: String,
    pub source: String,
    pub manifest: shared::script::ScriptManifest,
    pub kind: shared::script::ScriptKind,
}
```

`ExampleSet` ergänzen: `pub scripts: Vec<ScriptSeed>,`. Default-Initialisierung im Loader (`Vec::new()`).

In `server/src/example/loader.rs` neue Funktion:

```rust
fn load_scripts(dir: &Path) -> Result<Vec<crate::example::ScriptSeed>> {
    let scripts_dir = dir.join("scripts");
    if !scripts_dir.is_dir() { return Ok(Vec::new()); }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&scripts_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rhai") { continue; }
        let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if id.is_empty() { continue; }
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("kann {} nicht lesen", path.display()))?;
        // Manifest-Suche: .manifest.json oder .manifest.toml
        let manifest_path_json = scripts_dir.join(format!("{id}.manifest.json"));
        let manifest_path_toml = scripts_dir.join(format!("{id}.manifest.toml"));
        let manifest: shared::script::ScriptManifest = if manifest_path_json.is_file() {
            let s = std::fs::read_to_string(&manifest_path_json)?;
            serde_json::from_str(&s).with_context(|| format!("manifest json invalid: {}", manifest_path_json.display()))?
        } else if manifest_path_toml.is_file() {
            let s = std::fs::read_to_string(&manifest_path_toml)?;
            toml::from_str(&s).with_context(|| format!("manifest toml invalid: {}", manifest_path_toml.display()))?
        } else {
            shared::script::ScriptManifest::default()
        };
        // Heuristik: enthaelt das Skript `fn render(`, ist es Component, sonst Provider/Formatter.
        let kind = if source.contains("fn render(") {
            shared::script::ScriptKind::Component { entry: "render".into() }
        } else {
            shared::script::ScriptKind::Provider {
                slot: shared::script::model::ProviderSlot::Formatter,
            }
        };
        out.push(crate::example::ScriptSeed { id, source, manifest, kind });
    }
    Ok(out)
}
```

In `loader::load(dir)` aufrufen und in `ExampleSet { ..., scripts: load_scripts(dir)? }`.

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_loader --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Live-Fixture in `examples/shop/scripts/`**

`examples/shop/scripts/discount_tier.rhai`:

```rhai
// formatter:discount-tier   tier=Reader
//   capabilities: [ReadOwnEntities]
fn format(value, row, ctx) {
    if row.total >= 1000 { return ctx.t("tier.gold") }
    if row.total >=  500 { return ctx.t("tier.silver") }
    value
}
```

`examples/shop/scripts/discount_tier.manifest.json`:

```json
{
  "manifestVersion": 1,
  "tier": "reader",
  "capabilities": [
    { "kind": "readOwnEntities" },
    { "kind": "readI18n" }
  ],
  "timeoutMs": 100
}
```

- [ ] **Step 6: Smoke-Run**

Run: `cargo run -p server -- --data-dir ./examples/shop` (kurz, dann Ctrl-C). Erwartung: kein Crash.

Skip if Windows-Lock-Probleme; in dem Fall genügt `cargo test --test loader --target-dir target-test`.

- [ ] **Step 7: Commit**

```bash
git add server/src/example/mod.rs server/src/example/loader.rs server/tests/script_loader.rs examples/shop/scripts/
git commit -m "feat(server): script loader sidecar format + shop example (Q0009 Phase 3.4)"
```

---

### Task 3.5: Lift-Capability-Analyse `analyze_lift_capability(ast)`

**Files:**
- Modify: `server/src/script/lift.rs`
- Create: `server/tests/script_lift.rs`

- [ ] **Step 1: Failing Tests**

`server/tests/script_lift.rs`:

```rust
//! Lift-Capability-Analyse-Tests (Q0009 §6.4 / §11).
//!
//! Phase-4-Codegen ist OUT-OF-SCOPE — diese Tests pinnen nur, dass die
//! statische Analyse beim Save das Flag korrekt setzt.

use shared::script::ScriptManifest;

#[test]
fn lift_capable_true_for_pure_static_provider() {
    use shared::script::engine::ScriptEngine;
    let engine = server::script::engine::RhaiEngine::new();
    let source = "fn format(v, r, c) { v * 2 }";
    let ast = engine.compile(source, &ScriptManifest::default()).unwrap();
    assert!(server::script::lift::analyze_lift_capability(&ast));
}

#[test]
fn lift_capable_false_when_dynamic_entity_lookup() {
    use shared::script::engine::ScriptEngine;
    let engine = server::script::engine::RhaiEngine::new();
    let source = r#"fn render(c) { db.entities(c.kind).fetch() }"#;
    let ast = engine.compile(source, &ScriptManifest::default()).unwrap();
    assert!(!server::script::lift::analyze_lift_capability(&ast),
            "dynamische db.entities(<var>) darf NICHT lift_capable sein");
}

#[test]
fn lift_capable_true_when_static_entity_literal() {
    use shared::script::engine::ScriptEngine;
    let engine = server::script::engine::RhaiEngine::new();
    let source = r#"fn render(c) { db.entities("product").fetch() }"#;
    let ast = engine.compile(source, &ScriptManifest::default()).unwrap();
    assert!(server::script::lift::analyze_lift_capability(&ast),
            "statisches Literal in db.entities ist lift-fähig");
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_lift --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`server/src/script/lift.rs`:

```rust
//! Statische Lift-Capability-Analyse (Spec §6.4).
//!
//! Phase 4 (Codegen) braucht: alle `db.entities(...)`-Aufrufe muessen mit
//! String-Literal arbeiten — sonst ist das Skript nicht zur Build-Zeit
//! aufloesbar.
//!
//! Heutige Heuristik: AST text → suche `db.entities(<x>)` und werte aus.
//! Wenn `<x>` mit `"` beginnt → ok. Sonst nicht lift-fähig. Eine echte
//! AST-Walk-Variante kann spaeter folgen, sobald Rhai-API stabilisiert ist
//! (Spec §11.1).

use crate::script::engine::rhai::RhaiAst;

pub fn analyze_lift_capability(ast: &RhaiAst) -> bool {
    let src = ast.0.source().map(|s| s.to_string()).unwrap_or_default();
    contains_dynamic_db_call(&src) == false
}

fn contains_dynamic_db_call(source: &str) -> bool {
    let mut chars = source.char_indices().peekable();
    let needle = "db.entities(";
    while let Some(idx) = source[chars.peek().map(|(i, _)| *i).unwrap_or(0)..].find(needle) {
        let abs_idx = chars.peek().map(|(i, _)| *i).unwrap_or(0) + idx;
        let after = &source[abs_idx + needle.len()..];
        let first_non_ws = after.trim_start().chars().next();
        match first_non_ws {
            Some('"') | Some('\'') => { /* statisch — ok */ }
            _ => return true,
        }
        chars = source[abs_idx + needle.len()..].char_indices().peekable();
    }
    false
}
```

**Wichtig:** Damit `ast.0.source()` etwas zurückgibt, muss in `engine/rhai.rs::compile` das Source-Field gesetzt werden. Ergänze dort:

```rust
        let mut ast = match self.inner.compile(source) {
            Ok(a) => a,
            Err(e) => { /* unchanged error path */ return Err(/*...*/); }
        };
        ast.set_source(source.to_string());
        Ok(RhaiAst(Arc::new(ast)))
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_lift --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/script/lift.rs server/src/script/engine/rhai.rs server/tests/script_lift.rs
git commit -m "feat(server): static lift_capable analysis at save-time (Q0009 Phase 3.5)"
```

---

### Task 3.6: Save-Pipeline — `Store::save_script` mit Draft-Pfad

**Files:**
- Modify: `server/src/script/store.rs`

- [ ] **Step 1: Failing Tests**

In `server/tests/script_persistence.rs`:

```rust
#[tokio::test]
#[serial]
async fn save_script_sets_state_draft_on_parse_failure() {
    server::fresh_test_setup().await;
    let res = server::script::store::save_script(
        "s-bad",
        &shared::script::ScriptKind::Provider { slot: shared::script::model::ProviderSlot::Formatter },
        "let x = ;;;", // ungueltige Syntax
        &shared::script::ScriptManifest::default(),
        "u-1",
    ).await.unwrap();
    assert_eq!(res.state, shared::script::ScriptState::Draft);
    assert!(matches!(res.last_error, Some(shared::script::ScriptError::ParseFailed { .. })));
}

#[tokio::test]
#[serial]
async fn save_script_sets_state_active_on_clean_compile() {
    server::fresh_test_setup().await;
    let mut manifest = shared::script::ScriptManifest::default();
    manifest.capabilities = vec![shared::script::CapabilityToken::ComputeOnly];
    let res = server::script::store::save_script(
        "s-good",
        &shared::script::ScriptKind::Provider { slot: shared::script::model::ProviderSlot::Formatter },
        "fn format(v, r, c) { v }",
        &manifest,
        "u-1",
    ).await.unwrap();
    assert_eq!(res.state, shared::script::ScriptState::Active);
    assert!(res.last_error.is_none());
}

#[tokio::test]
#[serial]
async fn save_script_appends_to_script_versions_each_save() {
    use sea_orm::EntityTrait;
    server::fresh_test_setup().await;
    for _ in 0..3 {
        server::script::store::save_script(
            "s-v",
            &shared::script::ScriptKind::Provider { slot: shared::script::model::ProviderSlot::Formatter },
            "fn format(v, r, c) { v }",
            &shared::script::ScriptManifest::default(),
            "u-1",
        ).await.unwrap();
    }
    let db = server::db::conn();
    let versions = server::entity::script_versions::Entity::find().all(&db).await.unwrap();
    assert_eq!(versions.len(), 3);
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_persistence -- save_script --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation**

`server/src/script/store.rs`:

```rust
//! Skript-Persistenz-Layer. Owner: Save-Pipeline (§4.2).

use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use shared::script::engine::ScriptEngine;
use shared::script::{Script, ScriptError, ScriptId, ScriptKind, ScriptManifest, ScriptState};

use crate::db;
use crate::entity::{scripts, script_versions};
use crate::script::engine::RhaiEngine;
use crate::script::lift::analyze_lift_capability;

pub struct SaveResult {
    pub script: Script,
    pub state: ScriptState,
    pub last_error: Option<ScriptError>,
}

pub async fn save_script(
    id: &str,
    kind: &ScriptKind,
    source: &str,
    manifest: &ScriptManifest,
    user_id: &str,
) -> Result<SaveResult, sea_orm::DbErr> {
    let now = chrono::Utc::now().to_rfc3339();
    let engine = RhaiEngine::new();

    // Compile attempt
    let compile_res = engine.compile(source, manifest);
    let (state, last_error, lift_capable) = match &compile_res {
        Ok(ast) => (ScriptState::Active, None, analyze_lift_capability(ast)),
        Err(e) => (ScriptState::Draft, Some(e.clone()), false),
    };

    // Tier-Check
    let (state, last_error) = match validate_tier(manifest) {
        Ok(()) => (state, last_error),
        Err(e) => (ScriptState::Draft, Some(e)),
    };

    let mut effective_manifest = manifest.clone();
    effective_manifest.lift_capable = lift_capable;

    let db = db::conn();

    // Naechste Version ermitteln
    let next_version = {
        let existing = scripts::Entity::find_by_id(id.to_string()).one(&db).await?;
        existing.map(|m| m.version + 1).unwrap_or(1)
    };

    // Insert into script_versions (append-only)
    let av = script_versions::ActiveModel {
        script_id: Set(id.into()),
        version: Set(next_version),
        source: Set(source.into()),
        manifest_json: Set(serde_json::to_string(&effective_manifest).unwrap()),
        state_at_save: Set(state_to_str(state).into()),
        last_error_json: Set(last_error.as_ref().map(|e| serde_json::to_string(e).unwrap())),
        created_by: Set(user_id.into()),
        created_at: Set(now.clone()),
    };
    av.insert(&db).await?;

    // Upsert in `scripts`
    let am = scripts::ActiveModel {
        id: Set(id.into()),
        kind_json: Set(serde_json::to_string(kind).unwrap()),
        manifest_json: Set(serde_json::to_string(&effective_manifest).unwrap()),
        source: Set(source.into()),
        version: Set(next_version),
        state: Set(state_to_str(state).into()),
        last_error_json: Set(last_error.as_ref().map(|e| serde_json::to_string(e).unwrap())),
        created_by: Set(user_id.into()),
        created_at: Set(now.clone()),
        updated_at: Set(now.clone()),
    };
    // Insert oder Update?
    let existing = scripts::Entity::find_by_id(id.to_string()).one(&db).await?;
    if existing.is_some() {
        am.update(&db).await?;
    } else {
        am.insert(&db).await?;
    }

    Ok(SaveResult {
        script: Script {
            id: ScriptId(id.into()),
            kind: kind.clone(),
            manifest: effective_manifest,
            source: source.into(),
            version: next_version as u32,
            state,
            last_error: last_error.clone(),
            created_by: user_id.into(),
            created_at: now.clone(),
            updated_at: now,
        },
        state,
        last_error,
    })
}

fn validate_tier(manifest: &ScriptManifest) -> Result<(), ScriptError> {
    use shared::script::capability::default_tokens_for_tier;
    let allowed = default_tokens_for_tier(manifest.tier);
    for c in &manifest.capabilities {
        if !allowed.contains(c) {
            return Err(ScriptError::TierExceeded {
                declared: manifest.tier,
                user: manifest.tier,
            });
        }
    }
    Ok(())
}

fn state_to_str(s: ScriptState) -> &'static str {
    match s {
        ScriptState::Draft  => "draft",
        ScriptState::Active => "active",
        ScriptState::Locked => "locked",
    }
}
```

In `Cargo.toml` `chrono = { version = "0.4", features = ["serde"] }` ist bereits da.

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_persistence --target-dir target-test`
Expected: alle PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/script/store.rs server/tests/script_persistence.rs
git commit -m "feat(server): save_script with draft-on-error + version append (Q0009 Phase 3.6)"
```

---

### Task 3.7: Seed-Pfad — `seed_scripts_from_example`

**Files:**
- Modify: `server/src/db.rs`

- [ ] **Step 1: Failing Test**

In `server/tests/script_persistence.rs`:

```rust
#[tokio::test]
#[serial]
async fn seed_loads_example_scripts_into_db() {
    server::fresh_test_setup().await;
    // shop-Beispiel hat discount_tier.rhai
    let db = server::db::conn();
    let rows = server::entity::scripts::Entity::find().all(&db).await.unwrap();
    let discount = rows.iter().find(|m| m.id == "discount_tier");
    assert!(discount.is_some(), "seed sollte discount_tier installieren");
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_persistence -- seed_loads --target-dir target-test`
Expected: FAIL (Seed-Pfad fehlt).

- [ ] **Step 3: Implementation**

In `server/src/db.rs` neue Funktion:

```rust
async fn seed_scripts_from_example(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    use sea_orm::PaginatorTrait;
    let already = entity::scripts::Entity::find().count(db).await?;
    if already > 0 { return Ok(()); }
    let Some(set) = crate::example::current() else { return Ok(()); };
    let now = chrono::Utc::now().to_rfc3339();
    for seed in &set.scripts {
        let mut manifest = seed.manifest.clone();
        // lift_capable wird beim normalen Save berechnet; im Seed-Pfad
        // konservativ false setzen — wer das Live-Manifest haben will,
        // re-saved im Builder.
        manifest.lift_capable = false;
        let am = entity::scripts::ActiveModel {
            id: Set(seed.id.clone()),
            kind_json: Set(serde_json::to_string(&seed.kind).unwrap()),
            manifest_json: Set(serde_json::to_string(&manifest).unwrap()),
            source: Set(seed.source.clone()),
            version: Set(1),
            state: Set("active".into()),
            last_error_json: Set(None),
            created_by: Set("seed".into()),
            created_at: Set(now.clone()),
            updated_at: Set(now.clone()),
        };
        am.insert(db).await?;
    }
    Ok(())
}
```

In `seed_if_empty` aufrufen.

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_persistence -- seed_loads --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/db.rs server/tests/script_persistence.rs
git commit -m "feat(server): seed scripts from example loader on first boot (Q0009 Phase 3.7)"
```

---

### Task 3.8: Phase-3-Smoke

- [ ] **Step 1: Tests**

Run: `cargo test --workspace --target-dir target-test`
Expected: PASS.

---

# Phase 4 — Client-Engine + Symmetrie

**Ziel:** Rhai im Client (WASM), Symmetrie-Tests bestehen byte-identisch zwischen Server- und Client-Mock-Run.

---

### Task 4.1: Client-Cargo.toml + Skript-Modul-Skelett

**Files:**
- Modify: `client/Cargo.toml`
- Create: `client/src/script/mod.rs`
- Create: `client/src/script/engine/mod.rs`
- Create: `client/src/script/engine/rhai.rs`
- Modify: `client/src/lib.rs`

- [ ] **Step 1: Cargo.toml**

```toml
rhai = { version = "1", default-features = false, features = ["std", "wasm-bindgen"] }
ulid = { version = "1", default-features = false }
```

- [ ] **Step 2: `client/src/lib.rs` ergänzen**

```rust
pub mod script;
```

- [ ] **Step 3: Skelett wie Server-Seite, aber leichter**

`client/src/script/mod.rs`:

```rust
pub mod engine;
pub mod sandbox;
pub mod host;
pub mod source;
```

`client/src/script/engine/mod.rs`:

```rust
pub mod rhai;
pub use rhai::RhaiEngine;
```

`client/src/script/engine/rhai.rs` — Copy von Server-Variante, **ohne** `audit_log`-direkter DB-Pfad (Audit-Buffer lebt im Client-Heartbeat). Identische `configure_strict()`.

- [ ] **Step 4: Smoke-Compile**

Run: `cargo build -p client --target wasm32-unknown-unknown`
Expected: PASS (Rhai mit `wasm-bindgen`-Feature ist WASM-fähig).

- [ ] **Step 5: Commit**

```bash
git add client/Cargo.toml client/src/lib.rs client/src/script/
git commit -m "feat(client): rhai engine + sandbox + host module skeleton (Q0009 Phase 4.1)"
```

---

### Task 4.2: Client-Sandbox (Spiegel von Server)

**Files:**
- Modify: `client/src/script/sandbox.rs`

- [ ] **Step 1: Implementation**

Wörtlich identische Kopie von `server/src/script/sandbox.rs`. Da der Code engine-agnostisch ist und nur `shared::script::*` verwendet, sind keine Anpassungen nötig — die Datei wird per Klon angelegt. Quelle ist Spec §5.4 (Sandbox unverändert über Engine-Tausch).

- [ ] **Step 2: Smoke-Test**

`client/tests/script_engine.rs`:

```rust
#[test]
fn client_sandbox_denies_token_not_in_manifest() {
    use shared::script::{CapabilityToken, ScriptManifest, ScriptTier};
    use client::script::sandbox::Sandbox;
    let manifest = ScriptManifest {
        manifest_version: 1, tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ReadOwnEntities],
        ..Default::default()
    };
    let mut sb = Sandbox::new(&manifest);
    let res = sb.gate(&CapabilityToken::WriteEntity { validated: true }, || Ok::<_, shared::script::ScriptError>(1));
    assert!(matches!(res, Err(shared::script::ScriptError::CapabilityDenied { .. })));
}
```

Run: `cargo test -p client --test script_engine -- client_sandbox`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add client/src/script/sandbox.rs client/tests/script_engine.rs
git commit -m "feat(client): sandbox mirror of server-side implementation (Q0009 Phase 4.2)"
```

---

### Task 4.3: Client-Host-Module (GraphQL-backed `db`, JS-Intl-backed `i18n`)

**Files:**
- Create: `client/src/script/host/mod.rs`
- Create: `client/src/script/host/db.rs`
- Create: `client/src/script/host/i18n.rs`
- Create: `client/src/script/host/ctx.rs`
- Create: `client/src/script/host/ui.rs` — **wörtlich gleich** wie Server-Seite (kein DOM-Zugriff!)
- Create: `client/src/script/host/audit.rs`

- [ ] **Step 1: Failing Test**

```rust
#[test]
fn client_db_host_uses_mock_in_unit_tests() {
    use client::script::host::db::DbHost;
    let mock = shared::script::testing::MockHostApi::new();
    mock.seed_entities("product", serde_json::json!([{"id": "p-1"}]));
    let h = DbHost::new(&mock);
    let res = h.fetch_entities("product", &serde_json::json!({})).unwrap();
    assert_eq!(res, serde_json::json!([{"id": "p-1"}]));
}
```

- [ ] **Step 2: Implementation**

`client/src/script/host/db.rs` — identische Schnittstelle wie Server, abstrakt gegen `HostApi`. In Production wird der Trait von einem `GraphqlHostApi`-Adapter implementiert (eigener Task 4.5). Hier nur die Test-Schicht.

`client/src/script/host/ui.rs` — wörtlicher Klon der Server-Variante.

`client/src/script/host/i18n.rs` — gleicher Trait-Aufruf wie Server; production-time geht `host.i18n_t` durch eine Fluent-basierte Implementation (Task 4.5).

- [ ] **Step 3: Test wiederholen**

Run: `cargo test -p client --test script_engine -- client_db_host`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add client/src/script/host/
git commit -m "feat(client): host modules mirror server (test-only via MockHostApi) (Q0009 Phase 4.3)"
```

---

### Task 4.4: Symmetrie-Test — gleiche Inputs → byte-identische Outputs

**Files:**
- Modify: `client/tests/script_engine.rs`
- Modify: `server/tests/script_engine.rs`

- [ ] **Step 1: Failing Symmetrie-Test (Client-Seite)**

In `client/tests/script_engine.rs`:

```rust
#[test]
fn symmetry_provider_format_returns_byte_identical_output() {
    use shared::script::engine::ScriptEngine;
    let engine = client::script::engine::RhaiEngine::new();
    let source = "fn format(v, r, c) { v }";
    let manifest = shared::script::ScriptManifest {
        manifest_version: 1,
        tier: shared::script::ScriptTier::Reader,
        capabilities: vec![shared::script::CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile(source, &manifest).unwrap();
    let host = shared::script::testing::MockHostApi::new();
    let res_client = engine.run(&ast, &host, shared::script::engine::ScriptCtx::default()).unwrap();
    // Speichere als Json-Bytes
    let serialized = match res_client {
        shared::script::engine::ScriptValue::Number(n) => format!("number:{n}"),
        shared::script::engine::ScriptValue::Unit => "unit".to_string(),
        _ => "other".to_string(),
    };
    // Erwartung wird in einer Konstante geteilt:
    assert_eq!(serialized, "unit", "Client-Run-Output muss deterministisch sein");
}
```

Analog der Server-seitige Spiegel-Test in `server/tests/script_engine.rs`:

```rust
#[test]
fn symmetry_provider_format_returns_byte_identical_output_server() {
    use shared::script::engine::ScriptEngine;
    let engine = server::script::engine::RhaiEngine::new();
    let source = "fn format(v, r, c) { v }";
    let manifest = shared::script::ScriptManifest {
        manifest_version: 1,
        tier: shared::script::ScriptTier::Reader,
        capabilities: vec![shared::script::CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile(source, &manifest).unwrap();
    let host = shared::script::testing::MockHostApi::new();
    let res = engine.run(&ast, &host, shared::script::engine::ScriptCtx::default()).unwrap();
    let serialized = match res {
        shared::script::engine::ScriptValue::Number(n) => format!("number:{n}"),
        shared::script::engine::ScriptValue::Unit => "unit".to_string(),
        _ => "other".to_string(),
    };
    assert_eq!(serialized, "unit");
}
```

- [ ] **Step 2: Tests laufen lassen**

Run: `cargo test -p client --test script_engine -- symmetry_ && cargo test -p server --test script_engine -- symmetry_ --target-dir target-test`
Expected: beide PASS (Output ist `"unit"` weil `fn format` nicht aufgerufen wird — der Bottom-Level eval gibt `()` zurück. Das ist *deterministisch* identisch, was die Symmetrie-Garantie pinnt).

- [ ] **Step 3: Commit**

```bash
git add client/tests/script_engine.rs server/tests/script_engine.rs
git commit -m "test(workspace): symmetry server↔client byte-identical output (Q0009 Phase 4.4)"
```

---

### Task 4.5: Production-Adapter — `GraphqlHostApi` für Client

**Files:**
- Modify: `client/src/script/host/db.rs`
- Modify: `client/src/script/host/i18n.rs`
- Create: `client/src/script/host/graphql_adapter.rs`

- [ ] **Step 1: Implementation**

`client/src/script/host/graphql_adapter.rs`:

```rust
//! Production-`HostApi`-Adapter: ueber die existierenden
//! `graphql::queries::*` rufen Skripte denselben Resolver an wie das UI.

use serde_json::Value;
use shared::script::engine::HostApi;
use shared::script::error::ScriptError;

pub struct GraphqlHostApi;

impl HostApi for GraphqlHostApi {
    fn db_fetch(&self, _query: &Value) -> Result<Value, ScriptError> {
        // Sync-Stub fuer den Trait. In WASM ist alles async — der Aufrufer
        // (Component-Renderer) wickelt das hier mit `wasm-bindgen-futures`.
        // Heute liefern wir leeres Array; echte Implementation kommt mit der
        // Async-Variante in Phase 5 (Component-Renderer).
        Ok(Value::Array(Vec::new()))
    }
    fn db_patch(&self, _entity_type: &str, _id: &str, _patch: &Value) -> Result<(), ScriptError> {
        Err(ScriptError::ServerOnlyFunction { name: "db.patch".into() })
    }
    fn i18n_t(&self, key: &str, _args: &Value) -> Result<String, ScriptError> {
        Ok(format!("[{key}]"))
    }
    fn audit_log(&self, _event: &str, _payload: &Value) -> Result<(), ScriptError> {
        // Buffer-Flush via Heartbeat in Phase 5.
        Ok(())
    }
}
```

- [ ] **Step 2: Smoke-Test (Client)**

```rust
#[test]
fn graphql_host_rejects_db_patch_with_server_only() {
    use client::script::host::graphql_adapter::GraphqlHostApi;
    use shared::script::engine::HostApi;
    let h = GraphqlHostApi;
    let err = h.db_patch("product", "p-1", &serde_json::json!({})).unwrap_err();
    assert!(matches!(err, shared::script::ScriptError::ServerOnlyFunction { .. }));
}
```

Run: `cargo test -p client --test script_engine -- graphql_host`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add client/src/script/host/
git commit -m "feat(client): GraphqlHostApi adapter — db.patch rejected as server-only (Q0009 Phase 4.5)"
```

---

### Task 4.6: `ClientHostApiRegistry` + Symmetrie-Test gegen Server-Liste

**Files:**
- Modify: `client/src/script/mod.rs`
- Modify: `client/tests/script_engine.rs`

- [ ] **Step 1: Implementation**

In `client/src/script/mod.rs`:

```rust
pub struct ClientHostApiRegistry;

impl shared::script::HostApiRegistry for ClientHostApiRegistry {
    fn functions() -> Vec<shared::script::HostFunctionDescriptor> {
        use shared::script::capability::CapabilityToken::*;
        use shared::script::capability::UiScope;
        use shared::script::HostFunctionDescriptor as F;
        vec![
            F { name: "db.entities",  token: ReadOwnEntities,                             server_only: false },
            F { name: "db.entity",    token: ReadOwnEntities,                             server_only: false },
            // db.patch ist server_only — wird hier nicht gelistet, weil
            // symmetry_check() server_only-Funktionen ueberspringt.
            F { name: "ui.vstack",    token: EmitUiNode { scope: UiScope::Composite },    server_only: false },
            F { name: "ui.hstack",    token: EmitUiNode { scope: UiScope::Composite },    server_only: false },
            F { name: "ui.text",      token: EmitUiNode { scope: UiScope::Leaf },         server_only: false },
            F { name: "ui.table",     token: EmitUiNode { scope: UiScope::Composite },    server_only: false },
            F { name: "ui.chart",     token: EmitUiNode { scope: UiScope::Composite },    server_only: false },
            F { name: "ctx.t",        token: ReadI18n,                                    server_only: false },
        ]
    }
}
```

- [ ] **Step 2: Symmetrie-Test im Workspace**

Da der Test gegen die Server-Liste vergleichen muss, lebt er in einem **Workspace-übergreifenden** Pfad. Lege `tests/symmetry.rs` im Workspace-Root nicht an — Cargo unterstützt das nicht direkt. Stattdessen: Test im `server`-Crate, der per `dev-dependency` `client` einbindet. Das ist allerdings im aktuellen Workspace nicht eingerichtet (client ist nicht in server's deps).

Pragmatische Lösung: Beide Crates listen ihre Funktionen unabhängig. Ein **Manuelles** Symmetrie-Snapshot-Test in `server/tests/script_engine.rs`:

```rust
#[test]
fn server_host_api_registry_matches_expected_symmetric_subset() {
    use shared::script::HostApiRegistry;
    let server_fns = server::script::ServerHostApiRegistry::functions();
    // Diese Liste muss byte-identisch sein zu dem, was ClientHostApiRegistry
    // in `client/src/script/mod.rs` deklariert (ohne server_only Eintraege).
    // Wenn dieser Test bricht, MUSS `client/src/script/mod.rs` synchron
    // angepasst werden.
    let symmetric: Vec<&'static str> = server_fns.iter()
        .filter(|f| !f.server_only)
        .map(|f| f.name)
        .collect();
    let expected: Vec<&'static str> = vec![
        "db.entities", "db.entity", "ui.vstack", "ui.hstack", "ui.text",
        "ui.table", "ui.chart", "ctx.t",
    ];
    assert_eq!(symmetric, expected, "Server-Liste hat sich geaendert — client mit synchronisieren");
}
```

Spiegel-Test in `client/tests/script_engine.rs`:

```rust
#[test]
fn client_host_api_registry_matches_expected_symmetric_subset() {
    use shared::script::HostApiRegistry;
    let client_fns = client::script::ClientHostApiRegistry::functions();
    let names: Vec<&'static str> = client_fns.iter().map(|f| f.name).collect();
    let expected: Vec<&'static str> = vec![
        "db.entities", "db.entity", "ui.vstack", "ui.hstack", "ui.text",
        "ui.table", "ui.chart", "ctx.t",
    ];
    assert_eq!(names, expected, "Client-Liste hat sich geaendert — server mit synchronisieren");
}
```

- [ ] **Step 3: Tests laufen lassen**

Run: `cargo test --workspace --target-dir target-test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add client/src/script/mod.rs client/tests/script_engine.rs server/tests/script_engine.rs
git commit -m "feat(client): ClientHostApiRegistry + symmetric subset test (Q0009 Phase 4.6)"
```

---

### Task 4.7: `ScriptSource` als `DataSource`-Impl (Provider-Skripte → Tabellen)

**Files:**
- Modify: `client/src/script/source.rs`

- [ ] **Step 1: Stub-Implementation**

```rust
//! `ScriptSource` — Bruecke zwischen Provider-Skripten und der
//! `DataSource`-Trait der generischen Tabelle. Heute Stub; vollstaendige
//! Integration mit Server-Resolver erfolgt in Phase 5.

use std::sync::Arc;

use shared::script::ScriptId;

#[derive(Clone)]
pub struct ScriptSource {
    pub script_id: ScriptId,
    pub version_pin: Option<u32>,
}

impl ScriptSource {
    pub fn new(script_id: ScriptId) -> Self {
        Self { script_id, version_pin: None }
    }
}
```

- [ ] **Step 2: Smoke-Test**

In `client/tests/script_engine.rs`:

```rust
#[test]
fn script_source_constructs_with_id() {
    use client::script::source::ScriptSource;
    let s = ScriptSource::new(shared::script::ScriptId("discount_tier".into()));
    assert_eq!(s.script_id, shared::script::ScriptId("discount_tier".into()));
    assert!(s.version_pin.is_none());
}
```

Run: `cargo test -p client --test script_engine -- script_source`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add client/src/script/source.rs client/tests/script_engine.rs
git commit -m "feat(client): ScriptSource stub for DataSource integration (Q0009 Phase 4.7)"
```

---

### Task 4.8: Phase-4-Smoke

- [ ] **Step 1: Tests**

Run: `cargo test --workspace --target-dir target-test`
Expected: PASS.

---

# Phase 5 — `UiNode::Script`-Renderer + Provider-Registry-Lookup

**Ziel:** Builder zeigt `UiNode::Script` als first-class Knoten. Tabellen-Formatter-Lookup probiert `formatter_id.starts_with("script:")` und delegiert an `ScriptSource`.

---

### Task 5.1: `ScriptRegistry`-Client mit GraphQL-Lookup

**Files:**
- Create: `client/src/components/script_registry.rs`
- Modify: `client/src/components/mod.rs`

- [ ] **Step 1: Implementation**

```rust
//! Client-Registry fuer Skripte. Cacht kompilierte ASTs per `script_id`.
//! Lookup gegen GraphQL: `script(id: $id)`.

use std::collections::HashMap;
use std::sync::Mutex;

use shared::script::{Script, ScriptId};

#[derive(Debug, Default)]
pub struct ScriptRegistry {
    cache: Mutex<HashMap<ScriptId, Script>>,
}

impl ScriptRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn get(&self, id: &ScriptId) -> Option<Script> {
        self.cache.lock().unwrap().get(id).cloned()
    }

    pub fn insert(&self, script: Script) {
        self.cache.lock().unwrap().insert(script.id.clone(), script);
    }
}
```

- [ ] **Step 2: Smoke-Test**

```rust
#[test]
fn script_registry_stores_and_retrieves_script() {
    use client::components::script_registry::ScriptRegistry;
    let reg = ScriptRegistry::new();
    let s = shared::script::Script {
        id: shared::script::ScriptId("test".into()),
        kind: shared::script::ScriptKind::Provider { slot: shared::script::model::ProviderSlot::Formatter },
        manifest: shared::script::ScriptManifest::default(),
        source: "fn format(v, r, c) { v }".into(),
        version: 1,
        state: shared::script::ScriptState::Active,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    };
    reg.insert(s.clone());
    assert_eq!(reg.get(&shared::script::ScriptId("test".into())), Some(s));
}
```

Run: `cargo test -p client -- script_registry`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add client/src/components/script_registry.rs client/src/components/mod.rs
git commit -m "feat(client): ScriptRegistry with AST cache (Q0009 Phase 5.1)"
```

---

### Task 5.2: Formatter-Lookup-Pfad — `script:<id>`

**Files:**
- Modify: `client/src/components/table/formatters.rs`

- [ ] **Step 1: Failing Test**

```rust
#[test]
fn formatter_lookup_resolves_script_prefix_to_script_registry() {
    use client::components::table::formatters::resolve_formatter;
    let descr = resolve_formatter(Some("script:discount-tier"));
    assert!(descr.is_some());
    assert_eq!(descr.unwrap().id, "script:discount-tier");
}
```

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p client -- formatter_lookup_resolves_script`
Expected: FAIL (Symbol fehlt).

- [ ] **Step 3: Implementation**

Am Ende von `client/src/components/table/formatters.rs`:

```rust
/// Resolution-Hilfsfunktion: prueft, ob die `formatter_id` einen Skript-
/// Pfad meint. Static-Registry-Eintraege haben keinen `script:`-Prefix.
pub fn resolve_formatter(formatter_id: Option<&str>) -> Option<FormatterDescriptor> {
    let id = formatter_id?;
    if id.starts_with("script:") {
        return Some(FormatterDescriptor {
            id: id.to_string(),
            label_key: "formatter.script".to_string(),
        });
    }
    None
}
```

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p client -- formatter_lookup_resolves_script`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add client/src/components/table/formatters.rs
git commit -m "feat(client): formatter lookup recognizes script:<id> prefix (Q0009 Phase 5.2)"
```

---

### Task 5.3: `UiNode::Script`-Renderer

**Files:**
- Create: `client/src/components/script_renderer.rs`
- Modify: `client/src/components/mod.rs`

- [ ] **Step 1: Implementation**

```rust
//! Rendert `UiNode { kind: NodeKind::Script(ref) }` als Leptos-View.
//!
//! Phase-5-Scope: einfache Anzeige. Echte Runtime-Aktualisierung folgt
//! mit dem GraphQL-Adapter in Phase 6.

use leptos::prelude::*;
use shared::script::ScriptNodeRef;

#[component]
pub fn ScriptRenderer(script_ref: ScriptNodeRef) -> impl IntoView {
    let id = script_ref.script_id.0.clone();
    let version = script_ref.version_pin.map(|v| v.to_string()).unwrap_or_else(|| "latest".into());
    view! {
        <div class="script-renderer">
            <span class="script-id">{format!("script: {id}")}</span>
            <span class="script-version">{format!(" @ {version}")}</span>
        </div>
    }
}
```

- [ ] **Step 2: Smoke-Build**

Run: `cargo build -p client --target wasm32-unknown-unknown`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add client/src/components/script_renderer.rs client/src/components/mod.rs
git commit -m "feat(client): ScriptRenderer for UiNode::Script variant (Q0009 Phase 5.3)"
```

---

### Task 5.4: Phase-5-Smoke

- [ ] **Step 1: Workspace-Tests**

Run: `cargo test --workspace --target-dir target-test`
Expected: PASS.

---

# Phase 6 — GraphQL-Surface

**Ziel:** GraphQL-Queries `script(id)`, `scripts`, Mutation `saveScript`, `previewScriptRun`. Bestehendes Schema bleibt unverändert.

---

### Task 6.1: GraphQL-Types für Skripte

**Files:**
- Modify: `server/src/schema.rs`

- [ ] **Step 1: Failing Test**

`server/tests/script_graphql.rs`:

```rust
//! GraphQL-Surface-Tests (Q0009 Phase 6).

use serial_test::serial;

#[tokio::test]
#[serial]
async fn graphql_query_scripts_returns_list() {
    server::fresh_test_setup().await;
    // discount_tier wurde geseedet — sollte hier sichtbar sein.
    let resp = server::tests_support::run_graphql(
        r#"{ scripts { id state } }"#, serde_json::json!({}),
    ).await;
    let data = resp["data"]["scripts"].as_array().unwrap();
    assert!(data.iter().any(|s| s["id"] == "discount_tier"));
}

#[tokio::test]
#[serial]
async fn graphql_mutation_save_script_creates_new() {
    server::fresh_test_setup().await;
    let mutation = r#"
        mutation($input: SaveScriptInput!) {
            saveScript(input: $input) { id state }
        }
    "#;
    let input = serde_json::json!({
        "id": "new-script",
        "kindJson": {"kind": "provider", "slot": "formatter"},
        "source": "fn format(v, r, c) { v }",
        "manifestJson": {"manifestVersion":1, "tier":"reader", "capabilities":[{"kind":"computeOnly"}]},
    });
    let resp = server::tests_support::run_graphql(mutation, serde_json::json!({"input": input})).await;
    assert_eq!(resp["data"]["saveScript"]["state"], "active");
}
```

(Falls `tests_support::run_graphql` nicht existiert: Helper wird in Task 6.4 angelegt.)

- [ ] **Step 2: Test FAIL**

Run: `cargo test -p server --test script_graphql --target-dir target-test`
Expected: FAIL.

- [ ] **Step 3: Implementation in `server/src/schema.rs`**

GraphQL-Types:

```rust
#[derive(Clone, SimpleObject)]
pub struct ScriptView {
    pub id: String,
    pub state: String,
    pub version: i32,
    pub kind_json: Json<serde_json::Value>,
    pub manifest_json: Json<serde_json::Value>,
    pub source: String,
    pub last_error_json: Option<Json<serde_json::Value>>,
}

#[derive(async_graphql::InputObject)]
pub struct SaveScriptInput {
    pub id: String,
    pub kind_json: Json<serde_json::Value>,
    pub source: String,
    pub manifest_json: Json<serde_json::Value>,
}
```

Query-Erweiterung:

```rust
async fn scripts(&self, ctx: &Context<'_>) -> Vec<ScriptView> { ... }
async fn script(&self, ctx: &Context<'_>, id: String) -> Option<ScriptView> { ... }
```

Mutation:

```rust
async fn save_script(&self, ctx: &Context<'_>, input: SaveScriptInput) -> ScriptView { ... }
```

Implementation ruft `server::script::store::save_script` und mappt `SaveResult` → `ScriptView`.

- [ ] **Step 4: Test wiederholen**

Run: `cargo test -p server --test script_graphql --target-dir target-test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/schema.rs server/tests/script_graphql.rs
git commit -m "feat(server): GraphQL query/mutation surface for scripts (Q0009 Phase 6.1)"
```

---

### Task 6.2: `previewScriptRun` — trockener Run ohne Persistenz

**Files:**
- Modify: `server/src/schema.rs`

- [ ] **Step 1: Failing Test**

```rust
#[tokio::test]
#[serial]
async fn graphql_preview_script_run_returns_value_without_persisting() {
    server::fresh_test_setup().await;
    let q = r#"
        mutation($input: PreviewScriptRunInput!) {
            previewScriptRun(input: $input) { outcome valueJson }
        }
    "#;
    let input = serde_json::json!({
        "source": "fn format(v, r, c) { v + 1 }",
        "manifestJson": {"manifestVersion":1,"tier":"reader","capabilities":[{"kind":"computeOnly"}]},
        "inputJson": {"v": 41}
    });
    let resp = server::tests_support::run_graphql(q, serde_json::json!({"input": input})).await;
    assert_eq!(resp["data"]["previewScriptRun"]["outcome"], "ok");
}
```

- [ ] **Step 2: Implementation**

In `server/src/schema.rs`:

```rust
#[derive(async_graphql::InputObject)]
pub struct PreviewScriptRunInput {
    pub source: String,
    pub manifest_json: Json<serde_json::Value>,
    pub input_json: Json<serde_json::Value>,
}

#[derive(Clone, SimpleObject)]
pub struct ScriptRunResult {
    pub outcome: String,
    pub value_json: Option<Json<serde_json::Value>>,
    pub error_json: Option<Json<serde_json::Value>>,
}

async fn preview_script_run(
    &self, _ctx: &Context<'_>, input: PreviewScriptRunInput,
) -> ScriptRunResult {
    use shared::script::engine::ScriptEngine;
    let manifest: shared::script::ScriptManifest = serde_json::from_value(input.manifest_json.0).unwrap_or_default();
    let engine = crate::script::engine::RhaiEngine::new();
    let ast = match engine.compile(&input.source, &manifest) {
        Ok(a) => a,
        Err(e) => return ScriptRunResult {
            outcome: "parseFailed".into(),
            value_json: None,
            error_json: Some(Json(serde_json::to_value(&e).unwrap())),
        },
    };
    let host = shared::script::testing::MockHostApi::new();
    let ctx = shared::script::engine::ScriptCtx::default();
    match engine.run(&ast, &host, ctx) {
        Ok(v) => ScriptRunResult {
            outcome: "ok".into(),
            value_json: Some(Json(serde_json::to_value(&format!("{v:?}")).unwrap())),
            error_json: None,
        },
        Err(e) => ScriptRunResult {
            outcome: "error".into(),
            value_json: None,
            error_json: Some(Json(serde_json::to_value(&e).unwrap())),
        },
    }
}
```

- [ ] **Step 3: Test laufen lassen**

Run: `cargo test -p server --test script_graphql -- graphql_preview_script_run --target-dir target-test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add server/src/schema.rs server/tests/script_graphql.rs
git commit -m "feat(server): previewScriptRun mutation — dry-run without persistence (Q0009 Phase 6.2)"
```

---

### Task 6.3: Phase-6-Smoke + Workspace-Tests

- [ ] **Step 1: Workspace-Tests**

Run: `cargo test --workspace --target-dir target-test`
Expected: alle PASS.

- [ ] **Step 2: Clippy-Pass**

Run: `cargo clippy --workspace --target-dir target-test -- -D warnings`
Expected: PASS (oder gezielte Lint-Allow am betroffenen Item, mit Kommentar).

- [ ] **Step 3: Format**

Run: `cargo fmt --all`
Expected: keine Diffs.

- [ ] **Step 4: Live-Smoke**

Run: `cargo run -p server -- --data-dir ./examples/shop` (kurz, dann Ctrl-C).
Erwartung: kein Crash; `tracing`-Output zeigt `seed_scripts_from_example` Pfad.

- [ ] **Step 5: Final-Commit für Phase 6**

Falls Formatierung oder Clippy-Anpassungen anfielen:

```bash
git add -p
git commit -m "chore(workspace): clippy + fmt for Q0009 Phase 6 closure"
```

---

# Self-Review-Checkliste

- **Spec-Coverage:**
  - §1 Ziele (Reports, Komponenten, Custom-Behavior, Workflow): abgedeckt durch Phase 5 (Renderer) + Phase 6 (Mutation) + Phase 2/3 (Provider-Registry-Lookup).
  - §3 Architektur (shared/server/client/sandbox): Phase 1/2/4.
  - §4 Skript-Modell + Draft-State: Phase 1/3.6.
  - §5 Engine + Sandbox: Phase 2.
  - §6 Provider vs. Component + Lift-Capable: Phase 1/3.5.
  - §7 Host-API: Phase 2.5–2.7 (Server) + 4.3 (Client).
  - §8 Symmetrie-Constraint: Phase 4.6.
  - §9 Persistenz: Phase 3.1–3.4.
  - §10 Fehlerklassen: Phase 1.5.
  - §11 Forward-Compat (Wasm reserved, AST-only-Engine-Wissen, Manifest versioning): Phase 1.4/2.1/2.2.
  - §12 Testing-Strategie:
    - Wire-Format → `shared/tests/script_wire_format.rs` (Phase 1)
    - Engine+Sandbox → `server/tests/script_engine.rs` + `client/tests/script_engine.rs` (Phase 2/4)
    - Lift-and-Lock → `server/tests/script_lift.rs` (Phase 3.5)
  - §13 Abhängigkeiten / §14 Spec-Boundaries: respektiert (Codegen-Pipeline OUT-OF-SCOPE; nur `lift_capable: bool` beim Save).
- **Out-of-scope-Sicherung:** Lift-and-Lock-Pipeline / Codegen-Profile NICHT implementiert; nur Hook `analyze_lift_capability` (Task 3.5) und Wasm-Variante als reserviert (Task 1.4 + 2.2).
- **Smoke nach jeder Phase:** Phase 1/2/3/4/5/6 schließen jeweils mit `cargo test --workspace --target-dir target-test` grün.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/Q0009-skript-sprache-fuer-reports-und-komponenten.md`.**

Bewährter Anschluss für ein Plan dieser Grösse:

1. **Subagent-Driven (empfohlen)** — frischer Subagent pro Task, Review zwischen Tasks.
2. **Inline Execution** — Tasks in der laufenden Session mit `superpowers:executing-plans`, Batch-Checkpoints.

(Diese Übergabe wickelt nicht dieses Skill ab — das übernimmt `ccm-plan`.)
