# Provider-Slot-Scripts in den Render-Pfad verdrahten (Formatter) — Design

Date: 2026-05-25
Status: Draft — awaiting user review
Welle: Framework-Folgewelle der D2V-script-first-Umsetzung
(`2026-05-24-d2v-script-first-gap-analysis.md` §5.2, „Provider-Render-Pfad").
Vorlauf: `2026-05-25-d2v-valuetype-script-first-pilot` (Config-Teil committet
`8557119`); dieser Spec ist der Framework-Teil, der das ValueType-Formatter-
Script end-to-end lauffähig macht.

## 0. Problem

Die Gap-Analyse (§6) nahm an, dass Formatter/Validator/Filter „vollständig
script-first" gehen. Verifiziert gegen den heutigen Client-Code stimmt das
**nicht**:

- `client/src/script/provider_lookup.rs::lookup_provider` (Q0009 Phase 5.2)
  ist gebaut + getestet, hat aber **null Aufrufer** außerhalb der eigenen
  Datei. Phase 5.3 verdrahtete nur Component-Scripts (`ScriptRenderer` /
  `UiNode::Script`).
- `FieldCell` (`client/src/components/table/formatters.rs`) rendert jede
  Zelle direkt über `DefaultFieldRegistry::render` (FieldType-Match) — es
  konsultiert **kein** `formatter_id`-Script.
- `ScriptEngine::run(ast, host, ctx)` bekommt **den Zellenwert nicht** — ein
  Formatter könnte nicht lesen, was er formatieren soll (der vorhandene Test
  rechnet nur `40+2`).
- `ctx.t` (i18n) ist im Client-Engine **nicht** registriert; `ScriptCtx` hat
  nur `user_id`/`tenant_id`/`locale`/`now`.

Ein Spalten-Formatter-Script ist also heute nicht in den Render-Pfad
eingehängt. Diese Welle schließt das **für den Formatter-Slot**.

## 1. Scope

**In dieser Welle:**
- Engine-Input-API: `ScriptEngine::run` reicht pro Aufruf einen Wert + die
  Zeile durch (symmetrisch `shared`-Trait + Rhai-Impl client **und** server).
- `ctx.t`-Host-Funktion (Client), gated durch `ReadI18n`.
- `FieldCell` ruft bei `formatter_id == "script:<id>"` den Formatter-Slot über
  `lookup_provider` auf; sonst bisheriger Default-Pfad.
- `examples/d2v`: ValueType-Formatter-Script + Translatables, end-to-end
  lauffähig (lokalisiertes „Soll"/„Haben").

**Bewusst NICHT (Task #53 / spätere Wellen):**
- Filter/Validator/Computed/RowAction in ihre Pfade verdrahten. **Filter**
  braucht zusätzlich „Filtering überhaupt aktivieren" (heute `RemoteSource` +
  Server ignoriert Filter; `LocalSource` nicht in `EntityListPage`
  eingehängt; `ops_for_named` ist statischer Lookup, kein Script-Run).
- Server-SSR-Render des Formatters (Phase 6). In dieser Welle rendert der
  Formatter client-seitig; die Server-Engine-Signatur wird nur **symmetrisch**
  mitgezogen (Stub-Pfad).
- Codegen-/Laufzeit-Optimierung (Formatter inlinen/simplifizieren im
  Production-Binary-Pfad). Entscheidung: client-seitig sieht das Script den
  **vollen Wert** und kommt selbst zum String; die optimierte Form ist eine
  spätere Stufe.

## 2. Architektur

### 2.1 Engine-Input-API (`shared` + client + server)

`ScriptEngine::run` bekommt einen zusätzlichen Parameter `inputs`:

```rust
/// Pro-Aufruf-Eingaben für einen Provider-Slot-Run. Bewusst getrennt von
/// `ScriptCtx` (ambient: user/tenant/locale/now), damit `ScriptCtx` über
/// alle Script-Kinds gültig bleibt — ein Component-Script hat keinen
/// Zellenwert.
#[derive(Debug, Clone, Default)]
pub struct ScriptInputs {
    /// Der Zellenwert wie der Client ihn hält (nach Server-Grenz-Decode,
    /// z.B. directionalEnum 1 -> "SOLL"). NICHT vorformatiert.
    pub value: serde_json::Value,
    /// Die gesamte Zeile (Entity.fields) für kreuzfeld-Formatter.
    pub fields: serde_json::Map<String, serde_json::Value>,
}
```

Signatur (Trait in `shared::script::engine`):
`fn run(&self, ast: &Self::Ast, inputs: ScriptInputs, host: Arc<dyn HostApi>, ctx: ScriptCtx) -> Result<ScriptValue, ScriptError>`.

Die Rhai-Impl bindet vor `eval_ast_with_scope` zwei Scope-Variablen:
`scope.push_constant("value", <value als Dynamic>)` und
`scope.push_constant("fields", <fields als Dynamic/Map>)`. JSON↔Rhai-Dynamic
über die bestehende `ScriptValue`/`serde`-Brücke. Konstanten, weil das Script
seine Eingaben nicht mutieren soll.

`ScriptCtx` bleibt **unverändert**. Die Wert-Daten leben in `ScriptInputs`,
nicht in `ScriptCtx`.

### 2.2 `ctx.t`-Host-Funktion (Client)

Im Engine-Scope wird ein `ctx`-Objekt bereitgestellt mit Methoden:
- `ctx.t(key)` → `String` (Fluent-Lookup), gated `ReadI18n`.
- `ctx.t(key, args_map)` → `String` (mit Fluent-Argumenten), gated `ReadI18n`.
- `ctx.locale()`, `ctx.user_id()`, `ctx.tenant_id()`, `ctx.now()` (aus
  `ScriptCtx`, kein zusätzliches Token nötig — ambient read-only).

Gating: `ctx.t` läuft durch das Sandbox-Capability-Gate; fehlt `ReadI18n` im
Manifest, ist der Fehler **unmaskable** (`CapabilityDenied`, per try/catch
nicht fangbar — wie die übrigen Host-fns aus Q0009).

Client-Auflösung: `ctx.t(key)` ruft `crate::i18n::t(key)` (bzw. die
Args-Variante). Das nutzt die gemergten Fluent-Bundles — inkl. der per
`install_translatable_bundle` aus der DB nachgeladenen Entity-Translatables.
Damit findet `ctx.t("field.datev_entry.value_type.soll")` die d2v-Labels,
sobald sie geseedet + gefetcht sind.

Server-Impl (Symmetrie): die Server-Engine bekommt dieselbe `ctx.t`-Signatur;
Auflösung gegen die Server-Translatables. In dieser Welle rendert der
Formatter client-seitig, der Server-Pfad ist Symmetrie-/SSR-Vorbereitung.

### 2.3 FieldCell-Wiring (Client)

`FieldCell` (heute: `field_type`, `key`, `value`, `fields`) bekommt zusätzlich
die Spalten-`formatter_id` (bzw. löst sie via
`resolve_implementation_id(col, defaults, "formatter")` auf) und Zugriff auf
`ScriptRegistry` + `HostApi` + `ScriptCtx` aus dem Leptos-Context.

Ablauf beim Zellen-Render:
1. Formatter-ID auflösen. Kein `script:`-Prefix → bisheriger Pfad
   (`DefaultFieldRegistry::render`). **Keine Verhaltensänderung** für alle
   heutigen Spalten.
2. `"script:<id>"` → `lookup_provider(formatter_id, ProviderSlot::Formatter,
   ScriptInputs { value, fields }, &registry, host, ctx)`.
   - `LookupResult::Ok { value }` → `script_value_to_display(&value)` als
     Zellentext rendern.
   - `LookupResult::Fallback { .. }` / `NotAScriptId` →
     `DefaultFieldRegistry::render` (Default-Pfad). Fallback ist bereits in
     die Audit-Queue gelegt (`push_fallback`).

`lookup_provider` wird um den `inputs: ScriptInputs`-Parameter erweitert und
reicht ihn an `engine.run` durch.

### 2.4 d2v-Config + erstes Script

- `examples/d2v/translatables/entries.json` + `values.json`: Keys
  `field.datev_entry.value_type.soll` / `.haben` für de/en/fr
  (de: „Soll"/„Haben"; en: „Debit"/„Credit"; fr: „Débit"/„Crédit").
- `examples/d2v/entities/datev_entry/columns.json`: valueType-Spalte bekommt
  `"formatterId": "script:d2v_value_type_label"`.
- `examples/d2v/scripts/d2v_value_type_label.rhai`:

  ```rhai
  // value ist der decodete wire_name ("SOLL"/"HABEN"); ctx.t lokalisiert.
  if value == "SOLL" { ctx.t("field.datev_entry.value_type.soll") }
  else if value == "HABEN" { ctx.t("field.datev_entry.value_type.haben") }
  else { "" }
  ```

- `examples/d2v/scripts/d2v_value_type_label.manifest.json`:

  ```json
  {
    "kind": { "kind": "provider", "slot": "formatter" },
    "manifest": {
      "manifestVersion": 1,
      "tier": "reader",
      "capabilities": [ { "kind": "computeOnly" }, { "kind": "readI18n" } ]
    }
  }
  ```

## 3. Datenfluss

```
DB(int 1) --[Server directional_enum::decode]--> "SOLL"
  --> Entity.fields["valueType"] = "SOLL"
  --> FieldCell(value="SOLL", fields={...}, formatter_id="script:d2v_value_type_label")
  --> lookup_provider(Formatter, inputs{value:"SOLL", fields})
  --> engine.run: scope.value="SOLL" --> ctx.t("field.datev_entry.value_type.soll")
  --> "Soll" --> Zelle
```

## 4. Fehlerbehandlung

Jeder Script-Fehler degradiert sauber auf den Default-Renderer (zeigt den
wire_name „SOLL"), nie Crash:

| Lage | Verhalten |
|---|---|
| Script fehlt / Draft / Locked | Fallback + Audit-Log (`FallbackReason::Missing/NotActive`) |
| Slot-Mismatch (z.B. Validator-Script in Formatter-Position) | Fallback + Audit-Log |
| Compile-Fehler | Fallback + Audit-Log (`CompileFailed`) |
| Runtime-Fehler / Timeout / Memory | Fallback + Audit-Log (`RuntimeError`) |
| `ctx.t` ohne `ReadI18n` | `CapabilityDenied` (unmaskable) → Fallback + Audit-Log |
| ID ohne `script:`-Prefix | `NotAScriptId` → Default-Pfad (kein Audit) |

## 5. Tests

- **Engine (shared/client/server):** `run` mit `ScriptInputs` bindet `value`
  und `fields` in den Scope; ein Script `value` gibt den Input zurück.
- **`ctx.t`:** Host-fn liefert die Übersetzung für einen bekannten Key; ein
  Script ohne `ReadI18n` im Manifest → `CapabilityDenied` (per try/catch nicht
  fangbar).
- **Symmetrie-Test:** der bestehende Server/Client-Symmetrie-Test wird um den
  `inputs`-Parameter + `ctx.t` erweitert (gleiche Signatur beidseitig).
- **`lookup_provider`:** Formatter-Script mit `value`-Input → erwarteter
  Display-String; bestehende Fallback-Tests bleiben grün.
- **FieldCell:** Spalte mit `formatter_id="script:<id>"` + Active-Script →
  Zelle zeigt Script-Output; fehlendes/Draft-Script → Default-Render
  (wire_name). (Leptos-Render-Test soweit ohne Browser möglich; reiner
  Browser-Augenschein wird manuell gemacht und als solcher benannt.)
- **d2v-Loader:** `d2v_value_type_label` seedet mit `state=Active`, Slot
  Formatter, Manifest-Capabilities `[ComputeOnly, ReadI18n]`.

Browser-Verifikation (Dev-Server gegen echte d2v.db, Spalte ValueType zeigt
lokalisiertes „Soll"/„Haben") ist **manuell** — nicht automatisiert
verifizierbar, wird explizit so berichtet.

## 6. Betroffene Dateien (Orientierung, nicht abschließend)

- `shared/src/script/engine.rs` (oder wo `ScriptEngine` + `ScriptCtx` liegen):
  `ScriptInputs`, `run`-Signatur.
- `client/src/script/engine/rhai.rs` + `server/src/script/engine/rhai.rs`:
  Scope-Binding `value`/`fields`, `ctx.t`-Registrierung (client real, server
  symmetrisch), Gating.
- `client/src/script/host/ctx.rs`: `t`-Methode/Brücke zu `i18n::t`.
- `client/src/script/provider_lookup.rs`: `inputs`-Parameter durchreichen.
- `client/src/components/table/formatters.rs` (`FieldCell`) +
  ggf. `field/mod.rs`: Formatter-ID-Resolution + Script-Aufruf + Fallback.
- `client/src/components/registries/resolve.rs`: bereits vorhandene
  `resolve_implementation_id`-Nutzung.
- `server/tests/script_symmetry.rs`: Symmetrie-Test erweitern.
- `examples/d2v/...`: translatables, columns.json, scripts/.

## 7. Decisions

1. **Approach A** (Scope-Globals `value`/`fields` + saubere Trennung):
   per-Aufruf-Input in `ScriptInputs`, nicht in `ScriptCtx`. `ScriptCtx`
   bleibt ambient. Begründung: `ScriptCtx` gilt für alle Script-Kinds; ein
   Component-Script hat keinen Zellenwert.
2. **i18n als `ctx.t(key)`** (Methode am ctx-Objekt), nicht als globales
   `t()`. Begründung: `t` ist locale-ambient, und `ctx` trägt die locale schon
   — natürliche Heimat; gated durch `ReadI18n`.
3. **Script sieht den vollen Wert** (`value` + `fields`, roh statt
   vorformatiert) und kommt selbst zum String-Ergebnis. Laufzeit-/Codegen-
   Optimierung (Formatter inlinen) ist eine spätere Stufe.
4. **Symmetrische Engine-Änderung** (client + server), auch wenn der Formatter
   in dieser Welle nur client-seitig rendert — der Trait lebt in `shared`, und
   der Symmetrie-Test schützt die Wire-/API-Gleichheit.
5. **Nur Formatter** in dieser Welle. Filter (inkl. „Filtering aktivieren") +
   Validator/Computed/RowAction = Task #53.
6. **Fallback-First:** jeder Script-Fehler degradiert auf den Default-Renderer
   (wire_name), nie Crash. Audit-Queue-Logging existiert bereits.

## 8. Referenzen

- `docs/superpowers/specs/2026-05-24-d2v-script-first-gap-analysis.md` §5.2/§6.
- `docs/superpowers/specs/2026-05-23-q0009-skript-sprache-design.md` — Sandbox,
  Capabilities, Provider-Slots, unmaskable-Fehler.
- `client/src/script/provider_lookup.rs` — `lookup_provider`, `LookupResult`,
  `script_value_to_display` (heute 0 Aufrufer).
- `client/src/components/table/formatters.rs` — `FieldCell`.
- `client/src/script/host/ctx.rs`, `shared::script::engine::ScriptCtx`.
- `client/src/i18n/mod.rs` — `t`, `install_translatable_bundle` (DB-Merge).
- `examples/d2v/entities/datev_entry/columns.json` — valueType (directionalEnum
  seit `8557119`).
- Memory [[generalisierung-vier-schichten]] — Schicht-1-Framework-Mechanismus.
