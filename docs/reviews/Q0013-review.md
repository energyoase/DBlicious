# Q0013 Minimal-livable D2V-2019-Teilmenge — Code-Review

Date: 2026-05-30
Reviewer: claude (code-review:code-review Sub-Agent, Adapter-Modus: lokales Diff statt GitHub-PR)
Scope: `git diff 5c8dec1..7b0daad` (6 Commits, +277/-1, 7 Dateien). Pilot-Skripte P1 (Balance-Validator) + P3 (Stack-Filter) + Engine-/Loader-Tests + Wiring auf `datev_entry.stackId`.
Spec: `docs/superpowers/specs/Q0013-minimale-livable-d2v2019-teilmenge-design.md`
Plan: `docs/superpowers/plans/Q0013-minimale-livable-d2v2019-teilmenge.md`
Verdict: **APPROVE (mit einem Should-Fix non-blocking)**

Bereich abgegrenzt: das Diff enthaelt **keine** Lifecycle-Doc-Commits, kein `validator_id`-Framework, kein IBAN-Skript — alle drei sind plan-konform deferred. Die Verifikation aus `ccm-execute` (loader_d2v 12 ok inkl. 4 neuer, script_engine 26 ok inkl. 4 neuer, d2v_e2e 3 ok, d2v_all_17_listable 2 ok, `cargo fmt --check`, `cargo clippy --all-targets -D warnings`) wurde nicht erneut ausgefuehrt — siehe `verification-before-completion`-Logfile im Plan, das Diff selbst ist gruen.

## Zusammenfassung

Stage-1-livable-Pilot-Skripte sind technisch sauber: Provider-Slot-Manifeste folgen exakt der `d2v_value_type_label`-Vorlage (gleicher Manifest-Wire-Form, gleiche `kind`+`tier`+`manifestVersion`+`capabilities`-Toplevel-Struktur), der Loader-Pfad (`server/src/example/loader.rs::load_scripts`) wird ohne Aenderung erweitert, das `filterId`-Wiring auf `datev_entry.stackId` nutzt die per Q0009 stabilisierte `provider_lookup`-Aufloesung analog zum bereits live laufenden `formatterId` auf `valueType`. Die 4 Loader-Tests + 4 Engine-Tests sind echte RED-vor/GREEN-nach-Tests, decken positive **und** negative Pfade ab, und pinnen die wesentlichen Manifest-Felder (`kind/slot`, `tier`, `ComputeOnly`). Die Verifikation aus `ccm-execute` (alle Test-Suiten gruen, fmt/clippy clean) ist plausibel angesichts des Diff-Umfangs.

**Einziger nicht-blockierender Befund:** `d2v_balance_validator.manifest.json` deklariert `readI18n`, obwohl das Skript `ctx.t(...)` nirgendwo aufruft — Least-Privilege-Verletzung, vermutlich ein Copy-Paste aus dem Vorbild `d2v_value_type_label.manifest.json` (das ReadI18n echt nutzt). Sicherheitstechnisch harmlos (Capability ist nur deklariert, nicht ausgeuebt), Audit-Klarheit + Spec-Konformitaet leiden aber.

## Befunde

### F1 — Over-deklarierte Capability `readI18n` auf P1 Balance-Validator (Should-Fix, Confidence 95)

`examples/d2v/scripts/d2v_balance_validator.manifest.json:6`

```
"capabilities": [ { "kind": "computeOnly" }, { "kind": "readI18n" } ]
```

Das Skript `d2v_balance_validator.rhai` ruft **keinen** `ctx.t(...)` und auch sonst keine Host-fn auf. Es liest nur `fields.value`/`valueType`/`partnerValue`/`partnerValueType` aus dem Skript-Scope, macht arithmetik + Vergleich, gibt einen `bool` zurueck. Damit ist `ReadI18n` ueber-deklariert.

- Vorbild `d2v_value_type_label.manifest.json` deklariert `readI18n` zu Recht (Skript ruft `ctx.t(...)`).
- Spec §4 (Least-Privilege im Manifest) verlangt: Manifest deklariert genau die Tokens, die das Skript braucht. Tier-Default-Set ist eine Obergrenze, kein Minimum.
- Auswirkung: keine — `Sandbox::gate` traegt jeden Token-Use in den Audit-Ledger ein; ein nie aufgerufenes `ctx.t(...)` taucht im Audit nicht auf, der Sandbox-Pfad wird nicht beruehrt. Aber: Audit-Reviews und kuenftige Lift-/Permission-Analysen lesen die Manifest-Deklaration zuerst — eine ungenutzte Capability verwaessert die Aussagekraft.

**Fix:** in `examples/d2v/scripts/d2v_balance_validator.manifest.json` die `readI18n`-Entry entfernen:

```json
"capabilities": [ { "kind": "computeOnly" } ]
```

Der zugehoerige Loader-Test `d2v_balance_validator_script_loads_active` (server/tests/loader_d2v.rs:262-296) pruefe ich zusaetzlich um eine negative Assertion ("P1 deklariert KEIN ReadI18n") — sonst kann die Drift erneut auftreten ohne dass ein Test rot wird.

**Optional verschaerfen** (Stage-2-Hinweis, nicht jetzt): `Spec §4` koennte eine Validierungsregel im Save-Pfad bekommen, die ungenutzte Tokens als Warning meldet (statisch via AST-Walk pruefbar — `ctx.t`/`db.entities`/... sind alle Methoden-Calls auf `HostBridge`, analog zur bereits existierenden `analyze_lift_capability` in `server/src/script/engine/rhai.rs:335`).

### F2 — P1-Engine-Tests testen das on-disk-Manifest nicht, sondern einen Override (Minor, Confidence 70)

`server/tests/script_engine.rs:471-485, 535-550`

Beide P1-Engine-Tests konstruieren ein `ScriptManifest` mit `capabilities: vec![CapabilityToken::ComputeOnly]` und kompilieren damit den Skript-Source. Sie testen also die **Algorithmik** des Skripts gegen ein **synthetisches** Manifest, nicht gegen die echte `d2v_balance_validator.manifest.json`.

Solange F1 ungefixt ist (`readI18n` im echten Manifest), divergiert das Test-Manifest vom Production-Manifest. Beobachtbar fuer die Korrektheits-Aussage ist das aktuell nicht (die Compute-Path nimmt nur `ComputeOnly`-Pfade), aber: wenn jemand in Stage-2 das P1-Skript um `ctx.t(...)` erweitert und die Tests gruen sind, faellt die Drift erst auf, wenn das Production-Manifest geprueft wird.

**Empfehlung (nicht-blockierend):** mindestens **ein** Engine-Test pro Skript sollte das echte Manifest aus der `.manifest.json`-Datei laden und damit die Engine fuettern. Pattern existiert bereits indirekt im Loader-Test (`set.scripts.get(...)` liefert `manifest`), liesse sich kombinieren. Das ist Stage-2-Material, kein Blocker fuer Q0013.

### F3 — `null`/missing-Fields-Robustheit des Balance-Validators (Minor, Confidence 60)

`examples/d2v/scripts/d2v_balance_validator.rhai:10-20`

Das Skript liest `fields.value`/`valueType`/`partnerValue`/`partnerValueType` ohne Default-Handling. Wenn ein Feld auf der Wire-Form `null` ist, kommt es per `json_to_dynamic` (`server/src/script/engine/rhai.rs:366`) als `Dynamic::UNIT` an. Die anschliessende arithmetik `(soll + soll2) == (haben + haben2)` mit `soll = ()` schlaegt im Rhai-`ArithmeticPackage` als Type-Mismatch-Runtime-Error fehl (`ErrorRuntime`-Variant) — landet via `map_rhai_err` (`engine/rhai.rs:438`) als `ScriptError::HostError { source: "..." }`. Das Skript wirft also doch eine Exception, obwohl der Header-Kommentar "Liefert `true` wenn ... sonst `false`. Konvention: ... read-time Plausibilitaet, keine Exception." verspricht.

Fuer den d2v-Shop-Beispieldatensatz (gut definiert, alle Felder gesetzt) tritt das nicht auf — daher Confidence 60 statt 90. Aber: sobald das Skript an echte d2v-Daten kommt (Stage-2), kann ein einzelner `null`-Wert das Skript abbrechen lassen statt `false` zurueckzugeben, was den Validator-Slot in einem Listen-Sichten-Kontext unbenutzbar machen koennte (jede Zeile mit unvollstaendigen Daten wirft, statt visuell als "nicht balanciert" markiert zu werden).

**Optional fix:** defensive Lese-Funktion:

```rhai
let v  = if fields.value == ()        { 0 } else { fields.value };
let vt = if fields.valueType == ()    { ""  } else { fields.valueType };
// analog fuer pv, pt
```

oder am Anfang ein Early-Exit:

```rhai
if fields.value == () || fields.valueType == () { return false; }
```

Da Stage-1-Pilot — "livable", nicht "produktionsreif" — und der Stack-Filter dieselbe `()`-Konvention bereits sauber haendelt (`d2v_stack_filter.rhai:11`), faellt das hier als Inkonsistenz auf. Nicht-blockierend.

## Was sauber war (positiv hervorheben)

- **Wiring-Pin-Test** (`server/tests/loader_d2v.rs:248-263`) ist genau die richtige Art Regressions-Test: er pinnt, dass `datev_entry.stackId.filter_id == "script:d2v_stack_filter"` aus den Loader-Daten kommt. Bricht das, weiss der naechste Reviewer sofort warum.
- **Engine-Tests negativ + positiv** (Match/Non-Match fuer P3, balanciert/nicht-balanciert fuer P1) — exakt die Test-Form, die der Plan §3 verlangt.
- **Manifest-Form** (`{"kind":{"kind":"provider","slot":...}, "manifest":{...}}`) ist 1:1 identisch zur Vorlage `d2v_value_type_label.manifest.json` — kein Drift, kein neues Format.
- **Kommentare im Skript-Header** (was wird aus `fields` erwartet, was wird zurueckgegeben, welche Spec-Stelle die Konvention rechtfertigt) sind die richtige Doku-Form fuer Pilot-Skripte.
- **Q0011 Host-Error-Transport** ist hier korrekt nicht beruehrt: beide Skripte liefern Werte zurueck, werfen nicht und rufen keine Host-fn, die `script_err_to_rhai` triggern koennte. `map_rhai_err`'s Sentinel-Pfad ist irrelevant.
- **ComputeOnly auf P3** ist die richtige Capability-Wahl — Filter braucht keinen DB-Zugriff, kein i18n, nichts. Reader-Tier passt.

## Was deferred ist und NICHT bemaengelt wird

- `validator_id` als ColumnMeta-Slot — Plan §1 deferred auf Stage-2-Framework. Das ist der Grund, warum P1 nur per Engine-Test, nicht per Wiring getestet werden kann. **Kein Befund.**
- P2 IBAN-Validator — Plan Entscheidung (c): kein IBAN-Feld in den 17 Entities, daher entfaellt. **Kein Befund.**
- Server-seitiges Filter-Apply (`schema.rs::QueryRoot::entities` ignoriert `filter` weiterhin, CLAUDE.md dokumentiert das) — der Filter-Slot ist client-seitig zu erwarten und liegt ausserhalb Q0013-Scope. **Kein Befund.**

## Verdikt-Begruendung

F1 ist der einzige Befund mit "fix-vor-merge"-Charakter, aber:
- harmlos (over-deklarierte Capability traegt keine Sicherheitsfolge, weil das Skript die Capability nicht ausuebt);
- behebbar durch eine 1-Zeilen-Aenderung im Manifest + 1 zusaetzliche Loader-Test-Assertion;
- der Plan ist Stage-1-livable, nicht produktionsreif — die Erwartung an Least-Privilege-Strenge ist proportional.

Daher: **APPROVE**, mit der Bitte den F1-Fix entweder direkt nachzuziehen (1 Commit, ~5 Zeilen) oder als kleinen Follow-up zu queuen. F2 + F3 sind Stage-2-Material.

## Quellen + Verifikation

- Diff: `git diff 5c8dec1..7b0daad`, 7 Files, +277/-1
- Vorbild: `examples/d2v/scripts/d2v_value_type_label.{rhai,manifest.json}` (in 5c8dec1 bereits live)
- Loader: `server/src/example/loader.rs::load_scripts` (Zeile 190-268)
- Capability-Modell: `shared/src/script/capability.rs::CapabilityToken`, `shared/src/script/model.rs::ProviderSlot`
- ColumnMeta `filter_id`: `shared/src/lib.rs:283`
- Engine + Q0011-Sentinel: `server/src/script/engine/rhai.rs:295-310 (HostErrorPayload), :408-442 (map_rhai_err)`
- Verifikation (ccm-execute-Log): loader_d2v 12 ok / script_engine 26 ok / d2v_e2e 3 ok / d2v_all_17_listable 2 ok / fmt + clippy gruen.
