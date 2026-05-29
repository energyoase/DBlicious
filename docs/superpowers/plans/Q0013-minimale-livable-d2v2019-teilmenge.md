# Q0013 — Minimale livable d2v2019-Teilmenge (Stufe 1) Implementierungsplan

> **Für agentische Worker:** ERFORDERLICHE SUB-SKILL: Nutze
> `superpowers:subagent-driven-development` (empfohlen) oder
> `superpowers:executing-plans`, um diesen Plan Task-für-Task zu
> implementieren. Schritte verwenden Checkbox-Syntax (`- [ ]`) zum
> Tracking.

**Ziel:** Den Stufe-1-Schnitt aus Q0013 (read/display/validate, U1-frei) im
laufenden `examples/d2v/`-Setup konkret machen — durch zwei
script-first Piloten ohne neuen Framework-Code: P1 (Soll/Haben-Balance-
Validator) und P3 (DatevEntryStack-Filter). Der bereits live verdrahtete
`d2v_value_type_label`-Formatter bleibt unverändert.

**Architektur:** Beide Piloten folgen 1:1 dem Referenzmuster
`examples/d2v/scripts/d2v_value_type_label.{rhai,manifest.json}` — eine
`.rhai`-Quelle plus ein `.manifest.json` neben ihr. Der bestehende
`server::example::loader::load_scripts`-Pfad lädt sie ohne Code-Änderung.
P3 wird über das vorhandene `filter_id`-Feld auf `datev_entry.stackId`
verdrahtet (Resolution-Kette Phase 1.5, analog zu `formatter_id`). P1
liefert das Skript + Manifest + ladbaren Provider mit Slot=Validator;
**Runtime-Aufruf-Verdrahtung in der Spalte ist heute nicht möglich**
(siehe §0 „Verifizierte Lücke"), daher wird P1 als „ladbar +
algorithmisch beweisbar via Engine-Driven-Test" geliefert und die
Verdrahtung explizit als Folge-Item nach Stufe 2 geschoben.

**Tech Stack:** Rhai (Provider-Skripte, Tier `reader`,
Capability `computeOnly` + `readI18n`), `server::example::loader`
(Datei-Lader), `server::script::engine::RhaiEngine` (Engine-Driven-Tests),
`shared::script::{ProviderSlot, ScriptKind, ScriptManifest}` (Manifest-
Modell), `examples/d2v/entities/datev_entry/columns.json` (Verdrahtung
P3 via `filter_id`).

---

## 0. Scope-Anchor

| Stufe-1-Akzeptanz-Bullet (Q0013-Spec §2.2) | Plan-Task |
|---|---|
| #1 Navigieren & Listen aller §2.1-Entities | bereits erfüllt durch existierendes `examples/d2v/` — Plan ändert daran nichts; **Verifikation** in Task 6 (manueller Smoke / `cargo test --workspace`) |
| #2 Sortieren & Filtern pro `sortable`/`filterable`-Spalte | bereits UI-verdrahtet (CLAUDE.md / Spec A1) — kein neuer Task; Verifikation in Task 6 |
| #3 SOLL/HABEN lesbar via `script:d2v_value_type_label` | bereits live (Spec §1, `datev_entry/columns.json` Z. 6) — kein neuer Task; Regressions-Test bleibt `loader_d2v::d2v_value_type_formatter_script_loads_active` |
| #4 Buchungs-Plausibilität als Script-Validator | **Task 1 + Task 2** (P1 Balance-Validator, RED-first) |
| #5 Read-time-Vorschau Kleinwert pro Zeile | NICHT in diesem Plan (optionaler P4/P5 lt. D5 Mindestset; Stufe 2 für Aggregation). Punkt 5 ist durch das aktiv-formatierte `valueType` schon ansatzweise abgedeckt; volle Saldo-Vorschau = Stufe 2. |
| #6 Stack-Fokus via Filter-Script | **Task 3 + Task 4 + Task 5** (P3 Stack-Filter, RED-first, Verdrahtung in `columns.json`) |
| §3 D5 „Mindest-Pilot-Satz P1 + P3 + bestehender ValueType-Formatter" | Tasks 1–5 |
| §3 P2 (IBAN) — bindungs-gated | **Task 7** (Entscheidung dokumentieren); Entscheid hier: **(c) defer** — siehe §1 unten |
| §6 D7 — Q0012-Abgrenzung | §8 „Out of Scope" |

---

## 1. Verifizierte Lücke (Spec-vs-Code-Spot-Check 2026-05-30)

Beim Grounding ist eine Abweichung zur Spec-Annahme A3 aufgefallen,
die das Design eines Tasks beeinflusst:

- **Kein `validator_id`/`validatorId`-Feld in `shared`.** `ColumnMeta`
  exponiert heute genau drei Provider-Slot-Binding-IDs:
  `formatter_id`, `filter_id`, `editor_id` (`shared/src/lib.rs` Z. 283–289).
  `EntitySettings::field_type_defaults` deckt dieselben drei plus
  `action_ids` ab (`shared/src/settings.rs` Z. 25–46). **Keinen
  `validator_id`-Schlüssel — weder pro Spalte noch pro Entity** —
  obwohl der Engine-Layer `ProviderSlot::Validator` kennt
  (`shared/src/script/model.rs` Z. 30–37) und der Provider-Lookup ihn
  unterstützt (`server/src/script/provider_lookup.rs` Z. 155).
- **Konsequenz für P1:** Die spec-getreue Verdrahtung („settings →
  validatorId") existiert nicht als Datenfeld. A3 nennt das einen
  „Pilot-Detail-Fragen-Punkt"; in der Realität ist es eine
  Framework-Lücke. P1 wird deshalb in diesem Plan **als ladbares
  Script + algorithmisch verifizierender Engine-Test** geliefert
  (Provider/Slot=Validator wird als geladen + slot-korrekt geprüft, und
  die Soll==Haben-Logik wird direkt via `RhaiEngine::run` für zwei
  Mini-Fixtures bewiesen). Das **runtime-Aufruf-Wiring** an die
  Spalte/Settings ist ein eigenständiges Framework-Folge-Item (siehe §7
  „Stufe-2-Handoff", neuer Punkt „Framework: validator_id-Slot
  einführen").
- **Konsequenz für P3:** keine Lücke — `filter_id` existiert und ist die
  natürliche Verdrahtung (Resolution Phase 1.5, gleicher Mechanismus
  wie `formatter_id`).

Diese Lücke ist **kein Plan-Blocker** (Q0013 ist die DoD/Teilmenge,
nicht der Framework-Ausbau, vgl. Spec D2/D3), aber sie ändert die Form
des P1-Tasks: keine Wiring-Edit-Schritte in `columns.json`/`settings.json`
für P1, dafür ein Engine-Driven-Test, der den Provider-Slot
(`ProviderSlot::Validator`) und die Balance-Auswertung gegen
`ScriptInputs.fields` direkt am Engine verifiziert.

Die Entscheidung zum konditionalen P2-Piloten (IBAN) ist **(c) defer**:
- Spec §1a hat verifiziert kein IBAN-Feld im Port (`star_money_account`:
  `bankCode`, `code`, `defaultName`, `companyId`, `datevAccountNr` — kein
  IBAN-String);
- Spec D4/D5 nennt P2 explizit „bedingt/Komfort" und **nicht** im
  Mindest-Pilot-Satz;
- Spec A2 markiert es als Annahme, nicht als Pflicht.
- Eine BBAN-Synthese aus `bankCode`+`code` würde zusätzlich
  Annahmen über die Code-Form der echten `d2v.db` voraussetzen
  (nicht repo-verifizierbar — `d2v.db` ist nicht im Repo).
- Eine IBAN-Spalten-Ergänzung in `star_money_account/columns.json`
  ist konfigurationsseitig billig, fügt aber dem Port ein Feld hinzu,
  das im Quell-Schema (D2V-Legacy `StarMoneyAccounts`) ohne Mapping-
  Quelle ist (`binding.json::columnMap` müsste entweder eine echte
  Quellspalte synthesiert nennen oder die Spalte als read-only NULL
  führen — beides wirft mehr Designfragen auf, als Q0013 lösen soll).

**Konsequenz:** P2 ist nicht Teil dieses Plans. §7 hat einen Eintrag
„zukünftiges Item — P2 IBAN-Validator (Entscheidung A2-a oder A2-b
nötig)".

---

## 2. Dateiplan

**Neu anzulegen:**
- `examples/d2v/scripts/d2v_balance_validator.rhai` — Soll==Haben-
  Balance-Validator (P1).
- `examples/d2v/scripts/d2v_balance_validator.manifest.json` —
  Provider/Validator-Manifest für P1.
- `examples/d2v/scripts/d2v_stack_filter.rhai` — DatevEntryStack-Filter
  (P3).
- `examples/d2v/scripts/d2v_stack_filter.manifest.json` —
  Provider/Filter-Manifest für P3.

**Zu modifizieren:**
- `examples/d2v/entities/datev_entry/columns.json` — auf der
  `stackId`-Spalte `filter_id: "script:d2v_stack_filter"` ergänzen
  (P3-Verdrahtung).

**Zu modifizieren (Tests):**
- `server/tests/loader_d2v.rs` — drei neue Tests für die zwei neuen
  Skripte (Loader-Pin, Slot-Korrektheit, Filter-Verdrahtung).
- `server/tests/script_engine.rs` — zwei neue Engine-Driven-Tests, die
  die Balance- und Stack-Filter-Logik gegen `ScriptInputs.fields`
  ausführen.

**NICHT geändert (bewusst):**
- `examples/d2v/scripts/d2v_value_type_label.{rhai,manifest.json}` — Spec
  §1 verifiziert „schon live", §3 P0; keine Änderung.
- `examples/d2v/entities/datev_entry/settings.json` — keine
  `validator_id`-Verdrahtung möglich (siehe §1 Lücke).
- `examples/d2v/entities/datev_entry/editor.json` — Editor-Sicht ist
  in Stufe 1 read-only-relevant; keine Pilot-bezogene Änderung.
- Loader-Code (`server/src/example/loader.rs`) — bewusst keine
  Framework-Änderung; der Loader unterstützt das Datei-Layout bereits.

---

## 3. Task 1: P1 Balance-Validator — RED-Test schreiben

**Files:**
- Modify: `server/tests/loader_d2v.rs` (neuer Test am Dateiende)
- Modify: `server/tests/script_engine.rs` (neuer Engine-Test am Dateiende)

- [ ] **Schritt 1: Loader-Pin-Test schreiben (RED)**

Am Ende von `server/tests/loader_d2v.rs` anhängen:

```rust
#[test]
fn d2v_balance_validator_script_loads_active() {
    let set = example::load(&d2v_dir()).expect("laden");
    let s = set
        .scripts
        .get("d2v_balance_validator")
        .expect("Script 'd2v_balance_validator' fehlt im Set");
    assert!(
        s.manifest_error.is_none(),
        "manifest_error: {:?}",
        s.manifest_error
    );
    assert!(
        matches!(
            s.kind,
            shared::script::ScriptKind::Provider {
                slot: shared::script::ProviderSlot::Validator
            }
        ),
        "P1 muss Provider mit slot=Validator sein, war {:?}",
        s.kind
    );
    let m = s.manifest.as_ref().expect("Manifest geparst");
    assert_eq!(
        m.tier,
        shared::script::ScriptTier::Reader,
        "P1 läuft auf Reader-Tier (read/compute-only)"
    );
    assert!(
        m.capabilities
            .iter()
            .any(|c| matches!(c, shared::script::CapabilityToken::ComputeOnly)),
        "P1 muss ComputeOnly deklarieren, hatte {:?}",
        m.capabilities
    );
}
```

- [ ] **Schritt 2: Engine-Driven-Balance-Test schreiben (RED)**

Am Ende von `server/tests/script_engine.rs` anhängen:

```rust
#[test]
fn d2v_balance_validator_returns_true_for_balanced_pair() {
    // Verifiziert die *Algorithmik* von P1 direkt am Engine — unabhängig
    // davon, ob `validator_id` als Spalten-Wiring-Slot existiert (heute
    // nicht; siehe Plan §1).
    use shared::script::engine::{ScriptCtx, ScriptInputs, ScriptValue};
    let source = include_str!(
        "../../examples/d2v/scripts/d2v_balance_validator.rhai"
    );
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile(source, &manifest).expect("compile P1");
    let host = std::sync::Arc::new(shared::script::testing::MockHostApi::new());

    // Balanciertes Paar: SOLL 100 + HABEN 100 → ok=true
    let mut fields = serde_json::Map::new();
    fields.insert("value".into(), serde_json::json!(100));
    fields.insert("valueType".into(), serde_json::json!("SOLL"));
    fields.insert("partnerValue".into(), serde_json::json!(100));
    fields.insert("partnerValueType".into(), serde_json::json!("HABEN"));
    let inputs = ScriptInputs {
        value: serde_json::Value::Null,
        fields,
    };
    let val = engine
        .run(&ast, inputs, host.clone(), ScriptCtx::default())
        .expect("balance-script muss laufen");
    assert_eq!(
        val,
        ScriptValue::Bool(true),
        "Balance SOLL 100 / HABEN 100 muss true liefern, war {val:?}"
    );
}

#[test]
fn d2v_balance_validator_returns_false_for_unbalanced_pair() {
    use shared::script::engine::{ScriptCtx, ScriptInputs, ScriptValue};
    let source = include_str!(
        "../../examples/d2v/scripts/d2v_balance_validator.rhai"
    );
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile(source, &manifest).expect("compile P1");
    let host = std::sync::Arc::new(shared::script::testing::MockHostApi::new());

    // Unbalanciert: SOLL 100 + HABEN 90 → ok=false
    let mut fields = serde_json::Map::new();
    fields.insert("value".into(), serde_json::json!(100));
    fields.insert("valueType".into(), serde_json::json!("SOLL"));
    fields.insert("partnerValue".into(), serde_json::json!(90));
    fields.insert("partnerValueType".into(), serde_json::json!("HABEN"));
    let inputs = ScriptInputs {
        value: serde_json::Value::Null,
        fields,
    };
    let val = engine
        .run(&ast, inputs, host.clone(), ScriptCtx::default())
        .expect("balance-script muss laufen");
    assert_eq!(
        val,
        ScriptValue::Bool(false),
        "Balance SOLL 100 / HABEN 90 muss false liefern, war {val:?}"
    );
}
```

- [ ] **Schritt 3: Tests laufen lassen — müssen fehlschlagen**

Ausführen:

```sh
cargo test -p server --test loader_d2v d2v_balance_validator_script_loads_active --target-dir target-test
cargo test -p server --test script_engine d2v_balance_validator --target-dir target-test
```

Erwartet:
- `loader_d2v` schlägt fehl mit „Script 'd2v_balance_validator' fehlt im Set"
  (die `.rhai`+`.manifest.json` existieren noch nicht).
- `script_engine` schlägt fehl mit Compile-Fehler aus `include_str!`
  (Datei nicht da → Compile-Time-Fehler bei `cargo test` selbst).

*Hinweis:* Wenn der Dev-Server (`server.exe`) lokal läuft, hält er die
`target/`-Lock — daher `--target-dir target-test` (Verzeichnis ist in
`.gitignore`).

- [ ] **Schritt 4: Commit (RED-Phase)**

```sh
git add server/tests/loader_d2v.rs server/tests/script_engine.rs
git commit -m "test(Q0013): P1 balance-validator RED-Tests (loader + engine)"
```

---

## 4. Task 2: P1 Balance-Validator — Skript + Manifest anlegen (GREEN)

**Files:**
- Create: `examples/d2v/scripts/d2v_balance_validator.rhai`
- Create: `examples/d2v/scripts/d2v_balance_validator.manifest.json`

- [ ] **Schritt 1: Manifest schreiben**

Inhalt `examples/d2v/scripts/d2v_balance_validator.manifest.json`:

```json
{
  "kind": { "kind": "provider", "slot": "validator" },
  "manifest": {
    "manifestVersion": 1,
    "tier": "reader",
    "capabilities": [ { "kind": "computeOnly" }, { "kind": "readI18n" } ]
  }
}
```

Hinweise zu den Feldern:
- `kind.kind = "provider"` + `slot = "validator"` (camelCase Wire-Form
  laut `ScriptKind`/`ProviderSlot` in `shared/src/script/model.rs`).
- `manifest.tier = "reader"` (kein Schreib-/Author-Bedarf — Validierung
  ist read/compute).
- `capabilities` = `computeOnly` (Arithmetik) + `readI18n` (für
  spätere Lokalisierung der Fehlermeldung; analog zum Referenz-
  Manifest `d2v_value_type_label.manifest.json`).
- Kein `timeoutMs` / `memoryKb` — Default-Manifest reicht (Pattern
  des Referenz-Manifests).

- [ ] **Schritt 2: Rhai-Quelle schreiben**

Inhalt `examples/d2v/scripts/d2v_balance_validator.rhai`:

```rhai
// P1 Soll/Haben-Balance-Validator.
//
// Erwartet in `fields` (siehe ScriptInputs.fields):
//   value             — Betrag der aktuellen Buchungszeile (Number)
//   valueType         — "SOLL" oder "HABEN" (String, post-Wire-Decode)
//   partnerValue      — Betrag der Gegen-Zeile (Number)
//   partnerValueType  — "SOLL" oder "HABEN" der Gegen-Zeile (String)
//
// Liefert `true` wenn (SOLL-Summe == HABEN-Summe), sonst `false`.
// Konvention: Spec §2.2#4 — read-time Plausibilität, keine Exception.
let v  = fields.value;
let vt = fields.valueType;
let pv = fields.partnerValue;
let pt = fields.partnerValueType;

let soll  = if vt == "SOLL"  { v } else { 0 };
let soll2 = if pt == "SOLL"  { pv } else { 0 };
let haben = if vt == "HABEN" { v } else { 0 };
let haben2= if pt == "HABEN" { pv } else { 0 };

(soll + soll2) == (haben + haben2)
```

Hinweise:
- Reine Arithmetik — bleibt sicher in `computeOnly`.
- Kein `db.entities(...)` / `db.fetch(...)` — keine `ReadOwnEntities`-
  Capability nötig (B3-Gating wird so gar nicht angesprochen).
- Kein `throw` — Stufe-1-Definition ist „liefert read-time einen
  Fehler" (Spec §2.2#4); für den Engine-Test reicht ein Bool. Die
  Übersetzung in eine `ScriptError::ValidationFailed`-Meldung gehört
  in die fehlende Framework-Wiring-Schicht (siehe §1 Lücke und §7
  Handoff). Mit `readI18n`-Capability ist die spätere
  `ctx.t("validation.datev_entry.unbalanced")`-Variante kompatibel,
  ohne dass das Manifest dann neu freigeschaltet werden muss.

- [ ] **Schritt 3: Tests aus Task 1 erneut laufen lassen — müssen jetzt grün sein**

```sh
cargo test -p server --test loader_d2v d2v_balance_validator_script_loads_active --target-dir target-test
cargo test -p server --test script_engine d2v_balance_validator --target-dir target-test
```

Erwartet: alle drei Tests PASS.

- [ ] **Schritt 4: Commit (GREEN-Phase)**

```sh
git add examples/d2v/scripts/d2v_balance_validator.rhai \
        examples/d2v/scripts/d2v_balance_validator.manifest.json
git commit -m "feat(Q0013): P1 Soll/Haben-Balance-Validator-Skript + Manifest"
```

---

## 5. Task 3: P3 Stack-Filter — RED-Tests schreiben

**Files:**
- Modify: `server/tests/loader_d2v.rs`
- Modify: `server/tests/script_engine.rs`

- [ ] **Schritt 1: Loader-Pin-Test schreiben (RED)**

Am Ende von `server/tests/loader_d2v.rs` anhängen:

```rust
#[test]
fn d2v_stack_filter_script_loads_active() {
    let set = example::load(&d2v_dir()).expect("laden");
    let s = set
        .scripts
        .get("d2v_stack_filter")
        .expect("Script 'd2v_stack_filter' fehlt im Set");
    assert!(s.manifest_error.is_none(), "manifest_error: {:?}", s.manifest_error);
    assert!(
        matches!(
            s.kind,
            shared::script::ScriptKind::Provider {
                slot: shared::script::ProviderSlot::Filter
            }
        ),
        "P3 muss Provider mit slot=Filter sein, war {:?}",
        s.kind
    );
    let m = s.manifest.as_ref().expect("Manifest geparst");
    assert_eq!(m.tier, shared::script::ScriptTier::Reader);
    assert!(
        m.capabilities
            .iter()
            .any(|c| matches!(c, shared::script::CapabilityToken::ComputeOnly)),
        "P3 muss ComputeOnly deklarieren"
    );
}

#[test]
fn d2v_datev_entry_stack_id_column_has_script_filter_id() {
    // Wiring-Pin: das stackId-Column-Meta muss filter_id =
    // "script:d2v_stack_filter" tragen (Spec §2.2#6, Resolution Phase 1.5
    // analog zu formatter_id).
    let set = example::load(&d2v_dir()).expect("laden");
    let col = set.entities["datev_entry"]
        .columns
        .iter()
        .find(|c| c.key == "stackId")
        .expect("datev_entry: stackId-Spalte fehlt");
    assert_eq!(
        col.filter_id.as_deref(),
        Some("script:d2v_stack_filter"),
        "stackId muss filter_id 'script:d2v_stack_filter' tragen, war {:?}",
        col.filter_id
    );
}
```

- [ ] **Schritt 2: Engine-Driven-Filter-Test schreiben (RED)**

Am Ende von `server/tests/script_engine.rs` anhängen:

```rust
#[test]
fn d2v_stack_filter_matches_when_stack_id_equals_selected() {
    use shared::script::engine::{ScriptCtx, ScriptInputs, ScriptValue};
    let source = include_str!(
        "../../examples/d2v/scripts/d2v_stack_filter.rhai"
    );
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile(source, &manifest).expect("compile P3");
    let host = std::sync::Arc::new(shared::script::testing::MockHostApi::new());

    // Match: stackId = 42, selectedStackId = 42 → true.
    let mut fields = serde_json::Map::new();
    fields.insert("stackId".into(), serde_json::json!(42));
    fields.insert("selectedStackId".into(), serde_json::json!(42));
    let inputs = ScriptInputs {
        value: serde_json::Value::Null,
        fields,
    };
    let val = engine
        .run(&ast, inputs, host.clone(), ScriptCtx::default())
        .expect("stack-filter muss laufen");
    assert_eq!(val, ScriptValue::Bool(true), "Match-Fall muss true sein");
}

#[test]
fn d2v_stack_filter_excludes_when_stack_id_differs() {
    use shared::script::engine::{ScriptCtx, ScriptInputs, ScriptValue};
    let source = include_str!(
        "../../examples/d2v/scripts/d2v_stack_filter.rhai"
    );
    let engine = server::script::engine::RhaiEngine::new();
    let manifest = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ..Default::default()
    };
    let ast = engine.compile(source, &manifest).expect("compile P3");
    let host = std::sync::Arc::new(shared::script::testing::MockHostApi::new());

    // Kein Match: stackId = 42, selectedStackId = 7 → false.
    let mut fields = serde_json::Map::new();
    fields.insert("stackId".into(), serde_json::json!(42));
    fields.insert("selectedStackId".into(), serde_json::json!(7));
    let inputs = ScriptInputs {
        value: serde_json::Value::Null,
        fields,
    };
    let val = engine
        .run(&ast, inputs, host.clone(), ScriptCtx::default())
        .expect("stack-filter muss laufen");
    assert_eq!(val, ScriptValue::Bool(false), "Non-Match-Fall muss false sein");
}
```

- [ ] **Schritt 3: Tests laufen lassen — müssen fehlschlagen**

```sh
cargo test -p server --test loader_d2v d2v_stack_filter --target-dir target-test
cargo test -p server --test loader_d2v d2v_datev_entry_stack_id_column_has_script_filter_id --target-dir target-test
cargo test -p server --test script_engine d2v_stack_filter --target-dir target-test
```

Erwartet:
- `d2v_stack_filter_script_loads_active`: FAIL („Script
  'd2v_stack_filter' fehlt im Set").
- `d2v_datev_entry_stack_id_column_has_script_filter_id`: FAIL
  („filter_id war None").
- Engine-Tests: FAIL beim `include_str!` (Datei fehlt).

- [ ] **Schritt 4: Commit (RED-Phase)**

```sh
git add server/tests/loader_d2v.rs server/tests/script_engine.rs
git commit -m "test(Q0013): P3 stack-filter RED-Tests (loader + wiring + engine)"
```

---

## 6. Task 4: P3 Stack-Filter — Skript + Manifest anlegen

**Files:**
- Create: `examples/d2v/scripts/d2v_stack_filter.rhai`
- Create: `examples/d2v/scripts/d2v_stack_filter.manifest.json`

- [ ] **Schritt 1: Manifest schreiben**

Inhalt `examples/d2v/scripts/d2v_stack_filter.manifest.json`:

```json
{
  "kind": { "kind": "provider", "slot": "filter" },
  "manifest": {
    "manifestVersion": 1,
    "tier": "reader",
    "capabilities": [ { "kind": "computeOnly" } ]
  }
}
```

Hinweise:
- `slot = "filter"` (camelCase, wie `ProviderSlot::Filter`
  serialisiert).
- Keine `readI18n`-Capability nötig — der Filter liefert einen Bool,
  keine Anzeige.

- [ ] **Schritt 2: Rhai-Quelle schreiben**

Inhalt `examples/d2v/scripts/d2v_stack_filter.rhai`:

```rhai
// P3 DatevEntryStack-Filter.
//
// Erwartet in `fields`:
//   stackId          — der stackId-Wert der Zeile (Integer)
//   selectedStackId  — der vom Filter-UI gewählte Stack (Integer; -1
//                      bzw. fehlend = "alle Stacks", → true)
//
// Liefert `true`, wenn die Zeile zum gewählten Stack gehört (oder kein
// Stack ausgewählt ist).
let sel = fields.selectedStackId;
let row = fields.stackId;
if sel == () || sel == -1 { true } else { row == sel }
```

Hinweise:
- `()` (Unit) ist Rhai's „missing" — Default-Verhalten wenn das
  Filter-UI keinen Wert gewählt hat.
- Reine Gleichheits-Prüfung; bewusst minimal (Spec §3 P3 „einfacher
  Gleichheits-/Mengen-Filter"). Mengen-Filter (mehrere ausgewählte
  Stacks) ist eine triviale Erweiterung in Stufe 2 — nicht hier.

- [ ] **Schritt 3: Engine-Tests laufen lassen — müssen jetzt grün sein**

```sh
cargo test -p server --test loader_d2v d2v_stack_filter_script_loads_active --target-dir target-test
cargo test -p server --test script_engine d2v_stack_filter --target-dir target-test
```

Erwartet:
- `d2v_stack_filter_script_loads_active`: PASS.
- Beide Engine-Tests: PASS.
- `d2v_datev_entry_stack_id_column_has_script_filter_id`: bleibt
  weiterhin FAIL — die Verdrahtung folgt in Task 5.

- [ ] **Schritt 4: Commit (GREEN-Phase, ohne Wiring)**

```sh
git add examples/d2v/scripts/d2v_stack_filter.rhai \
        examples/d2v/scripts/d2v_stack_filter.manifest.json
git commit -m "feat(Q0013): P3 DatevEntryStack-Filter-Skript + Manifest"
```

---

## 7. Task 5: P3 Stack-Filter — Verdrahtung in `datev_entry/columns.json`

**Files:**
- Modify: `examples/d2v/entities/datev_entry/columns.json`

- [ ] **Schritt 1: `filter_id` auf `stackId` ergänzen**

In `examples/d2v/entities/datev_entry/columns.json` die Zeile für
`stackId` ändern. Aktueller Inhalt (verifiziert Z. 16):

```json
{ "key": "stackId", "labelKey": "field.datev_entry.stack_id", "fieldType": { "kind": "integer" }, "sortable": true, "filterable": true }
```

Neuer Inhalt:

```json
{ "key": "stackId", "labelKey": "field.datev_entry.stack_id", "fieldType": { "kind": "integer" }, "sortable": true, "filterable": true, "filterId": "script:d2v_stack_filter" }
```

(Stil-Hinweis: JSON-Wire-Form ist camelCase →
`filterId` auf der Wire, nicht `filter_id`. Das `ColumnMeta::filter_id`
Rust-Feld serialisiert via `#[serde(rename_all = "camelCase")]` zu
`filterId`. Verifiziert in `shared/src/lib.rs` Z. 268+283.)

- [ ] **Schritt 2: Wiring-Test aus Task 3 laufen lassen — muss jetzt grün sein**

```sh
cargo test -p server --test loader_d2v d2v_datev_entry_stack_id_column_has_script_filter_id --target-dir target-test
```

Erwartet: PASS.

- [ ] **Schritt 3: Vollständiger Loader-Test-Sweep — Regression**

```sh
cargo test -p server --test loader_d2v --target-dir target-test
```

Erwartet: alle Tests PASS, inklusive der zwei
schon-existierenden Smoke-Pins
(`d2v_loads_all_17_entities`, `d2v_value_type_formatter_script_loads_active`).

- [ ] **Schritt 4: Commit (Wiring)**

```sh
git add examples/d2v/entities/datev_entry/columns.json
git commit -m "feat(Q0013): wire script:d2v_stack_filter on datev_entry.stackId"
```

---

## 8. Task 6: Verifikation (Workspace-Tests + Lint + manueller Smoke)

**Files:** (keine — reine Verifikation)

- [ ] **Schritt 1: Volle Workspace-Tests**

```sh
cargo test --workspace --target-dir target-test
```

Erwartet: PASS. Wenn ein dev-server lokal läuft, wird die normale
`target/` blockiert — `--target-dir target-test` (CLAUDE.md ist
explizit dazu).

- [ ] **Schritt 2: Format + Clippy (CLAUDE.md-Verifikations-Standard)**

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

Erwartet: beide PASS. Falls Clippy meckert: fix + neuer Commit
`chore(Q0013): clippy cleanup`.

- [ ] **Schritt 3: Manueller Smoke-Test (optional, fixture-abhängig)**

Voraussetzung: lokal liegt eine **anonymisierte Kopie** von `d2v.db`
(NIE die Original-Produktions-Datei — `examples/d2v/README.md` ist
explizit dazu, und MEMORY.md mahnt zu keinen-im-Repo-DBs). Wenn keine
Fixture verfügbar ist, diesen Schritt überspringen und im PR-/
Commit-Text vermerken.

```sh
export D2V_LEGACY_URL="sqlite:///absoluter/pfad/zu/d2v-kopie.db"
export DBLICIOUS_DATABASE_URL="sqlite::memory:"
cargo run -p server -- --data-dir ./examples/d2v
```

Verifikationen am laufenden Server (port 8000 / GraphiQL auf `/`):
- Server-Boot zeigt im Log das Laden von 17 Entities + 3 Skripten
  (`d2v_value_type_label`, `d2v_balance_validator`, `d2v_stack_filter`)
  — Loader stoppt nicht bei einem unbekannten Slot.
- `query { entities(entityType:"datev_entry", page:1, pageSize:5) { items { id fields } } }`
  liefert Zeilen, in denen `valueType` formatiert lesbar ist
  (bestehender Formatter funktioniert weiter — keine Regression).
- Die `stackId`-Spalte trägt im Server-Metadaten-Pfad
  `filterId="script:d2v_stack_filter"` (Smoke-Query gegen
  `columnMeta`, falls vorhanden — alternativ: das ist bereits durch
  Task 5's Loader-Test gepinnt, kein zweiter Beweis nötig).

*Hinweis:* Den Dev-Server **vor** Übergabe an den User wieder beenden
(MEMORY.md `feedback_background_servers.md`).

- [ ] **Schritt 4: Falls Verifikation rot — fix + Commit**

Wenn ein Schritt fehlschlägt: Ursache lokalisieren, Fix in einen
neuen Commit (`fix(Q0013): ...`), Verifikation neu durchlaufen. NIE
amend auf einem vorhergehenden GREEN-Commit.

---

## 9. Task 7: Konditionaler P2 IBAN-Pilot — Entscheidung dokumentieren

**Files:** (keine — Dokumentations-Entscheid)

- [ ] **Schritt 1: Entscheid bestätigen**

Per Plan-§1 oben: **(c) defer**. Begründung dort vollständig
(IBAN-Spalte fehlt im Port, P2 ist laut Spec D5 nicht im
Mindest-Satz, A2-a/b sind beide nicht ohne Folge-Designfragen
machbar).

- [ ] **Schritt 2: Folge-Item-Notiz erzeugen (NICHT erstellen — nur listen)**

Im Plan-Abschluss §7 unten ist der Eintrag enthalten:
„Q-Future: P2 IBAN-Validator — entscheidet zwischen A2-a (IBAN-Spalte
zu `star_money_account` ergänzen) und A2-b (BBAN-Synthese aus
`bankCode`+`code`)". KEIN `docs/queue/Q*.md` schreiben — Items legt
die CCM-Pipeline an, nicht dieser Plan.

- [ ] **Schritt 3: Kein Commit nötig**

Dieser Task ist rein dokumentarisch und Bestandteil dieses Plan-
Dokuments selbst.

---

## 10. Stufe-2-Handoff (Backlog-Mapping, KEINE Subtasks)

Jeder Punkt = eigenes zukünftiges CCM-Item. Hier nur One-Liner mit
Verweis auf die maßgebliche Quelle. Diese Items werden **nicht** in
diesem Plan ausgeführt.

| Stufe-2-Feature | Folge-Item (one-line handoff) |
|---|---|
| **U1 FK-Picker** (Konto/Bank-Name statt ID) | Spec §2.3, Analyse §5.1 / §7.1. Plan existiert bereits unter `docs/superpowers/plans/2026-05-20-u1-fk-referenz-picker-plan.md` (Commit `cbda328`). Q0013 baut darauf nicht auf, vermerkt aber: P3-Filter und P1-Validator profitieren später vom Picker, brauchen ihn aber nicht. |
| **Aggregation 1.7.12** (echte SuSa-/Saldo-Auswertung) | Spec §4 + §7.2. Folge-Item nötig: „1.7.12 — Aggregations-Queries Server-seitig". |
| **U3 Report-View** (DatevAccountView/SuSaView/Calculation-Baum-Layout) | Spec §4 + §7.3. Folge-Item: „U3 — Report-View (Cross-Table/Tree/Pivot)". |
| **U5 App-Context / YearSelector** | Spec §4 + §7.4. Folge-Item: „U5 — Globaler Jahresfilter". |
| **`FieldType::Tree` (1.7.20)** | Spec §4 + §7.5. Folge-Item: „1.7.20 — Tree-FieldType + Renderer". |
| **GenerateAccountEntries / Sub-Buchungen schreiben** | Spec §4 + §7.6, plus Analyse §5.1/§5.3. Voraussetzungen: U2 Save-Report, U6/F1 Long-Running-Action, Server-WriteEntity-Pfad. |
| **Invert / Generalumkehr** | Spec §4 + §7.6. Folge-Item nach U6/F1. |
| **DATEV-/Bank-CSV-Import** | Spec §4 + §7.7. Folge-Item: „1.7.15 — Bulk-Import". |
| **Q0012 — Packaging/Distribution** | Spec §6 D7 — eigenes Item, hier explizit abgegrenzt. |
| **Framework: `validator_id`-Slot einführen** *(neu, durch Plan-§1 entdeckt)* | `ColumnMeta` + `EntitySettings::field_type_defaults` um `validator_id` (Wire `validatorId`) ergänzen, Resolution Phase 1.5, GraphQL-Mapping anpassen. Erst dann kann P1 von „ladbar + algorithmisch beweisbar" zu „bei Listing live-geprüft" wandern. |
| **Q-Future: P2 IBAN-Validator** *(durch Task 7 (c)-defer)* | Entscheidet zwischen A2-a (IBAN-Spalte zu `star_money_account` ergänzen, `binding.json::columnMap` aktualisieren) und A2-b (BBAN-Synthese aus `bankCode`+`code`). Algorithmus (Modulo-97) bleibt eine `computeOnly`-Übung. |
| **P4 / P5 (optionale Komfort-Piloten)** | Spec §3 — Konto-Nummer-Formatter und Description-Aufbereitung. Niedrigschwellig (Pattern identisch zu P3/Formatter), aber nicht im Mindest-Satz und daher hier nicht gebaut. |

---

## 11. Out of Scope (für diesen Plan)

- **Alle Stufe-2-Features** aus Spec §4 (schreiben, aggregieren) und die
  in §10 oben aufgeführten Folge-Items.
- **U1 FK-Referenz-Picker** (Spec D2 / §2.3) — Konto/Bank bleiben in
  Stufe 1 als ID/Code sichtbar; eigener Plan existiert.
- **Framework-Änderungen** an `shared` (z.B. das in §1 entdeckte
  fehlende `validator_id`). Q0013 ist „minimale livable Teilmenge"
  ohne Framework-Erweiterung — wenn Framework-Wiring später kommt,
  rückt P1 von „ladbar+engine-belegt" zu „spaltengetrieben live"
  ohne Skript-Änderung.
- **P2 IBAN-Validator** — siehe Plan-§1 und Task 7, Entscheid
  **(c) defer**.
- **P4/P5 Komfort-Formatter** — Spec §3 explizit optional und nicht
  im Mindest-Satz.
- **Q0012 Packaging/Distribution** — Spec D7 explizit eigener Track.
- **Bookkeeping-Stdlib** — Spec D8 explizit out of scope.
- **Manueller Smoke-Test gegen die echte `d2v.db`** — nur mit
  anonymisierter Kopie; falls keine Fixture verfügbar, im
  Commit-/PR-Text vermerken (vgl. Task 6 Schritt 3 + MEMORY-Hinweis
  zu Datenschutz aus `examples/d2v/README.md`).

---

## 12. Selbstreview-Checkliste (vor PR)

- [ ] Jedes Spec-§2.2-Akzeptanz-Bullet hat eine Plan-Zuordnung
  (siehe §0 Scope-Anchor). Bullet #5 (Read-time-Vorschau) ist
  bewusst durch das bestehende `valueType`-Formatter teilweise
  abgedeckt; volle Saldo-Vorschau ist Stufe-2 (D6/A4) und kein
  Mindest-Pilot — explizit kein neuer Task.
- [ ] Keine TBD/TODO/„fülle ein"-Platzhalter.
- [ ] Alle Code-Blöcke enthalten den vollen Inhalt (Manifest komplett,
  Rhai-Source komplett, Test-Funktion komplett).
- [ ] Typkonsistenz: `ScriptKind::Provider { slot: ProviderSlot::Validator }`
  in Task 1 ↔ Task 2; `ProviderSlot::Filter` in Task 3 ↔ Task 4;
  `filterId` (camelCase Wire) in Task 5 ↔ Test in Task 3 (das Rust-
  Feld heißt `filter_id` — verifiziert in `shared/src/lib.rs` Z. 283).
- [ ] Verifikation (Task 6) deckt CLAUDE.md-Standard (cargo test,
  fmt, clippy) und enthält den `--target-dir target-test` Hinweis.
- [ ] §10/§11 listen alle nicht-gebauten Spec-Features mit One-Liner-
  Handoff, ohne Subtasks.
