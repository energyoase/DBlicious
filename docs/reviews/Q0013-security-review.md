# Q0013 — Security-Review (D2V-2019-Minimal-Pilot-Skripte)

- Datum: 2026-05-30
- Reviewer: claude (`security-review`-Skill, Security-Verdikt-Pass — separat vom
  Korrektheits-Review in `docs/reviews/Q0013-review.md`)
- Scope: Implementations-Diff `git diff 5c8dec1..7b0daad` (7 Dateien, +277/-1):
  - `examples/d2v/scripts/d2v_balance_validator.{rhai,manifest.json}` (P1 Validator)
  - `examples/d2v/scripts/d2v_stack_filter.{rhai,manifest.json}` (P3 Filter)
  - `examples/d2v/entities/datev_entry/columns.json` (wiring `filterId="script:d2v_stack_filter"` auf `stackId`)
  - `server/tests/loader_d2v.rs`, `server/tests/script_engine.rs` (4 Loader-Tests + 4 Engine-Tests)
- Threat-Model: Q0013 fuegt zwei neue capability-gated Rhai-Skripte in den
  Provider-Slot-Mechanismus, der Q0009 (Sandbox-Haertung) + Q0011
  (Audit-Fidelity, `HostErrorPayload`-Sentinel) bereits abgesichert haben.
  Akzeptierter Scope hier: **kein Sandbox-Escape, keine Capability-Escalation,
  kein Audit-Fidelity-Regress, kein Information-Disclosure-Regress, kein
  unbeobachtetes Re-Wiring der Provider-Resolution.**

> **VERDIKT: CLEARED** — die zwei neuen Pilot-Skripte bewegen sich vollstaendig
> innerhalb des Q0009/Q0011-Sandbox-Modells, ueben keine ueber-deklarierte
> Capability tatsaechlich aus und beruehren weder den Q0011-Sentinel-Transport
> noch den `provider_lookup`-Trust-Pfad in einer neuen Weise. Der von der
> Code-Review als F1 markierte Least-Privilege-Drift (`readI18n` ueber-deklariert
> auf P1) ist sicherheitstechnisch **harmlos** und bleibt advisory — siehe A1.

---

## 1. Capability-Deklarationen sind ehrlich und minimal (Least-Privilege)

### 1.1 d2v_balance_validator (P1) — `[ComputeOnly, ReadI18n]`

Statische AST-Pruefung des Skript-Source (`d2v_balance_validator.rhai`):
liest ausschliesslich `fields.value`, `fields.valueType`, `fields.partnerValue`,
`fields.partnerValueType` aus dem Scope, vergleicht mit `"SOLL"`/`"HABEN"`,
arithmetik + Vergleich, gibt `bool` zurueck. **Kein Aufruf** von `ctx.t(...)`,
`db.entities(...)`, `db.entity(...)` oder einer anderen Host-Funktion (verifiziert
per Grep ueber den Skript-Source).

→ `ReadI18n` ist **ueber-deklariert**. Sicherheitsfolge: **keine**.

Begruendung: `Sandbox::gate` (`server/src/script/sandbox.rs:57-91`) wird nur
beruehrt, wenn das Skript eine Host-Funktion **tatsaechlich aufruft**. Die
Manifest-Capability-Liste ist die *Obergrenze* dessen, was Gates akzeptieren
wuerden, nicht eine vorab-genutzte Berechtigung. Da P1 keinen `ctx.t(...)`-Pfad
ausfuehrt, taucht `ReadI18n` weder im `token_uses`-Ledger noch im
`script_audit_log.tokens_used`-Reporting auf — die Capability wird nicht
*ausgeuebt*, nur ungenutzt deklariert.

Pruefung gegen den Save-Validator: P1 deklariert `tier=reader`. `validate_manifest`
(`server/src/script/save.rs:53-98`) deckelt Reader-Caps via
`default_tokens_for_tier(Reader)` (`shared/src/script/capability.rs:50-78`) auf
`[ReadOwnEntities, ReadI18n, ComputeOnly, EmitUiNode{Leaf}]`. `ReadI18n` ist im
Reader-Set zulaessig — der Save-Path wuerde diese Kombination nicht ablehnen.

**Achtung Trust-Pfad:** der Example-Loader (`server/src/example/loader.rs::load_scripts`
+ `server/src/data.rs::seed_scripts_from_example`) ruft `validate_manifest`
**NICHT** auf — `seed_scripts_from_example` schreibt das Manifest direkt in die
DB-Zeile, sofern es parseable ist. Das ist konsistent mit der Datenverzeichnis-
Trust-Annahme aus CLAUDE.md ("Server refuses to start without `--data-dir`; the
data-dir maintainer is trusted"). Es bedeutet aber konkret, dass ein
Datenverzeichnis-Maintainer ein Manifest mit Caps deklarieren kann, die
`validate_manifest` ablehnen wuerde (z.B. `WriteEntity` auf einem
Reader-Tier-Skript). Q0013 stellt diese Praktik nicht aus — beide Skripte bleiben
strikt im Reader-Set — aber der Trust-Pfad ist erwaehnenswert (siehe A3).

→ **OK.** Das ueber-deklarierte `ReadI18n` ist eine Hygiene-/Audit-Klarheit-Frage,
keine Eskalation. Selbst wenn ein spaeterer Edit das Skript dazu bringt, `ctx.t`
zu nutzen, traegt der `Sandbox::gate` den `ReadI18n`-Use korrekt in den Ledger
— die Capability ist legitim deklariert, nur eben aktuell ungenutzt.

### 1.2 d2v_stack_filter (P3) — `[ComputeOnly]`

Statische AST-Pruefung: liest `fields.selectedStackId` + `fields.stackId`,
vergleicht. **Kein** Host-Call. `ComputeOnly` ist exakt das passende Token.

→ **OK.**

### 1.3 Tier `reader` fuer beide Slots

- **Validator-Slot:** der Code-Review-Reviewer hat plausibel argumentiert, dass
  das `validator_id`-Wiring im ColumnMeta heute nicht existiert (Stage-2-deferred);
  P1 ist daher Engine-only erreichbar (Tests). Selbst wenn das Wiring kaeme:
  Validator-Slot liest nur Daten der Zeile, validiert, gibt `bool`. Kein DB-Write,
  kein Audit-Write — Reader-Tier ist die richtige Wahl.
- **Filter-Slot:** Stack-Filter liest Filter-Input + Row-Cell, vergleicht, gibt
  `bool`. Identische Erwaegung. Reader-Tier passt.

→ **OK.**

## 2. Sandbox/Capability-Fit: kein Skript greift ueber sein Manifest hinaus

Geprueft per direkter AST-Lese am Skript-Source:

- P1 `d2v_balance_validator.rhai`: keine `db.entities`/`db.entity`/`ctx.t`-Calls,
  kein `audit.log`/`workflow.*`/`ui.*`-Token-Vokabular, kein Import von
  anderen Scripts. Reines Compute.
- P3 `d2v_stack_filter.rhai`: identische Auspraegung.

Pinned durch die existierenden Engine-Invarianten:
- Rhai-`Engine::new_raw()` plus die fuenf erlaubten Packages
  (Arithmetic/Logic/BasicString/BasicArray/BasicMap) — kein `StandardPackage`
  (`server/src/script/engine/rhai.rs:89-108`).
- `eval`/`import`/`print`/`debug` werden per `disable_symbol` verboten.
- Selbst wenn das Skript eine Host-Methode aufrufen wuerde, deren Capability
  nicht im Manifest steht, wuerde `Sandbox::gate` mit
  `ScriptError::CapabilityDenied` unmaskable (per `try`/`catch` nicht fangbar)
  abbrechen.

Damit ist nicht nur die *aktuelle* Capability-Nutzung sauber — die Sandbox haelt
sie auch in Zukunft eingegrenzt, falls jemand das Skript erweitert.

→ **OK.**

## 3. Input-Handling — Filter-Inputs als attacker-controllable Surface

Der P3 Stack-Filter liest `fields.stackId` und `fields.selectedStackId`. Trust-
Klassifikation:

- `fields.stackId`: kommt aus der Row → server-seitig vertrauenswuerdig.
- `fields.selectedStackId`: kommt aus dem Filter-UI → **client/request-controlled**.
  Sobald das `script:`-Filter-Wiring spaeter (Stage-2) an einen Request-Pfad
  angeschlossen wird, ist dieser Wert von einer nicht-authentifizierten Quelle
  bestimmt.

Pruefung des Pfades, den der Wert nimmt:
1. Wire-decode: JSON-Number → `serde_json::Value::Number` → `json_to_dynamic`
   (`server/src/script/engine/rhai.rs:366-390`) → `Dynamic::from(i64)` oder
   `Dynamic::from(f64)`. JSON-String/Object kommt als String/Map an — kein
   String-zu-Code-Pfad existiert (es gibt keinen `eval` mehr).
2. Verwendung im Skript: `sel == ()`, `sel == -1`, `row == sel` — drei
   primitive Vergleiche. Keine String-Konkat, keine Format-Strings, keine
   String-gebaute DB-Query (waere ueberhaupt nicht moeglich: `db.entities` ist
   die einzige Datenfetch-Schiene, und das Skript ruft sie nicht).
3. Engine-Garantie: `eval` ist disabled (Pinned per Q0009-Test
   `engine_rejects_eval` in `script_engine.rs`).

→ **Keine Injection-Surface.** Selbst wenn ein Angreifer `selectedStackId` auf
einen exotischen Wert (z.B. `{"foo": "bar"}` oder einen String mit Quotes) setzt,
landet das als `Dynamic::Map`/`Dynamic::String`, der `==`-Vergleich mit `()`/`-1`
schlaegt fehl, der `row == sel`-Vergleich liefert `false`. Keine Code-Ausfuehrung.

Das gleiche gilt fuer P1: alle vier `fields.*`-Reads gehen durch
`json_to_dynamic`, alle Vergleiche sind primitiv (`==`), die Arithmetik nutzt das
`ArithmeticPackage` (registriertes Built-in, keine String-Interpolation).

→ **OK.**

## 4. Interaktion mit dem Q0011-Sentinel-Transport — KEIN REGRESS

Frage: bringt Q0013 einen neuen Pfad, der `HostErrorPayload` faelschen oder umgehen
koennte?

Geprueft:

- **Kein `throw`-Statement** in beiden Skripten (Grep ueber die `.rhai`-Files).
  Damit existiert keine Rhai-`throw`-Pfad, der den `EvalAltResult::ErrorRuntime`-
  Payload mit einem skript-konstruierten `Dynamic` befuellen koennte.
- **Kein Host-Call** in beiden Skripten. `script_err_to_rhai`
  (`engine/rhai.rs:303-311`) wird gar nicht erreicht. Der `HostErrorPayload`-Pfad
  ist tot fuer Q0013.
- **Null/Missing-Field-Pfad (Code-Review F3):** das P1-Skript arithmetiert auf
  `fields.value` ohne Default. Wenn die Wire-Form `null` liefert, kommt der Wert
  als `Dynamic::UNIT` an, `soll + soll2` ist ein Rhai-Type-Mismatch, das
  generiert intern `EvalAltResult::ErrorRuntime` (Rhai-eigener Fehler, **NICHT**
  ueber `script_err_to_rhai`). Der Payload dieses Fehlers ist ein Rhai-eigener
  Dynamic — **kein** `HostErrorPayload`-Custom-Typ. In `map_rhai_err`
  (`engine/rhai.rs:419-424`) faellt der `try_cast::<HostErrorPayload>()`
  entsprechend in den `None`-Zweig → generischer
  `ScriptError::HostError { source: descr }`. Das ist die **gewollte**
  Q0011-Semantik: alles, was kein authentifizierter Host-Sentinel ist, wird
  generisch reportet. Der Skript-Author kann hierueber **keinen** typisierten
  `kind` (z.B. `timeout`, `capabilityDenied`) erzwingen.

Damit verifiziert: P1's Null-Pfad-Robustheits-Luecke (F3) ist eine
*Funktionalitaets*-Frage (Validator wirft statt `false` zurueckzugeben), keine
*Sicherheits*-Frage. Q0011 schliesst den Forge-Vektor an der Wurzel, und Q0013
fuegt keine neue forge-faehige Konstruktion hinzu.

→ **OK. Kein Q0011-Regress.**

## 5. Information-Disclosure ueber Error-Messages — KEIN NEUER LEAK

Wenn P1 auf `null` arithmetiert und mit `HostError { source: descr }` failed,
welche Information traegt `descr`?

`descr = EvalAltResult::to_string()` — Rhai-Standardformatierung. Fuer einen
Arithmetik-Type-Mismatch z.B. `"Runtime error: Arithmetic error: + operator not
supported for (), 0 (line N, position M)"`. Inhalt:

- Skript-eigene Token (Operator, Werte): vom Skript-Author bekannt, kein Leak.
- Zeile/Position: zeigt auf die Skript-Datei, deren Source eh dem Author
  bekannt ist (er hat sie geschrieben).
- **Keine Host-Internals:** kein DB-Pfad, kein File-Path, kein Secret, kein
  Entity-ID-Format, keine Schema-Struktur.

Vergleich mit dem `formatter`-Pfad (gleicher `HostError`-Reporting): in
`d2v_value_type_label` schon live, gleiche Leak-Klasse → kein Q0013-spezifischer
Leak.

Frage: kann ein Skript-Author die Felder der Row probieren ("schema-Discovery"),
indem er Failures verursacht und `descr` liest? Antwort: der Skript-Author hat
ohnehin Zugriff auf alle `fields.*` via direktem Read aus dem Scope — das
Schema ist fuer ihn nicht geheim. Die `descr` traegt nichts, das er nicht
anderweitig direkt lesen koennte.

→ **OK. Keine neue Disclosure-Surface.**

## 6. Wiring-Trust-Boundary — `filterId="script:d2v_stack_filter"`

Die Aenderung an `examples/d2v/entities/datev_entry/columns.json` setzt
`stackId.filterId="script:d2v_stack_filter"`. Geprueft, ob das einen neuen
Trust-Pfad oeffnet:

### 6.1 Heutiger Server-Pfad (Ausfuehrung)

`ops_for_named` in `shared/src/ops.rs:440-454` ist der einzige Server-Pfad, der
heute auf `filter_id` reagiert: er behandelt nur `TEXT_STARTS_WITH` und
`TEXT_CASE_SENSITIVE` namentlich; jeder andere Wert (inkl. `"script:d2v_stack_filter"`)
faellt in `_ => ops_for(field)` zurueck — **kein Script-Aufruf**. Der Server
ignoriert den Filter-Slot fuer Skripte heute komplett (konsistent mit der
CLAUDE.md-Aussage: `schema.rs::QueryRoot::entities` ignoriert `filter`-Args).

### 6.2 Heutiger Client-Pfad (Ausfuehrung)

`client/src/script/provider_lookup.rs::lookup_provider` ist die einzige
Resolution-Schiene fuer `script:`-IDs auf der Client-Seite. Wird heute
**ausschliesslich** vom Formatter-Pfad gerufen (`client/src/components/table/formatters.rs:66`).
Die Filter-Pipeline (Body-Row-Filterung) konsumiert `filter_id` nicht durch
diesen Pfad — der Skript-Filter-Wiring ist dormant, bis ein zukuenftiger Patch
ihn anschliesst.

→ **Wiring ist heute reine Metadaten-Deklaration.** Das Skript existiert in der
DB, der Filter-ID-Eintrag existiert auf der Spalte, aber kein Pfad fuehrt
Daten von der Filter-UI durch das Skript. Stage-2-Material.

### 6.3 Trust-Modell der Provider-Resolution (vs. Q0009-Remediation)

Q0009 hat die Resolution-Kette zentralisiert (`provider_lookup`): einmal Slot-Match,
einmal State-Check, einmal Engine-Run mit Sandbox. Dieselbe Funktion wird beim
Filter-Anschluss in Stage-2 verwendet werden — neue Trust-Surface entsteht dort,
nicht hier. **Q0013 selbst:** kein Resolution-Pfad-Aufruf, kein Audit-Eintrag,
kein Sandbox-Run fuer den Filter-Slot.

### 6.4 Malicious-Data-Dir-Maintainer

Frage: kann ein boesartiger `--data-dir`-Maintainer den `filterId` auf ein
attacker-controlled Skript zeigen? Antwort: **ja, aber das ist die etablierte
Trust-Grenze.** Der Data-Dir-Maintainer kann auch heute schon eigene
Manifeste mit eigenen Caps deklarieren (siehe §1.1 — `seed_scripts_from_example`
ruft `validate_manifest` nicht). Das ist konsistent mit dem
Trust-Modell, das CLAUDE.md ("server refuses to start without `--data-dir`")
und der `examples/`-Konvention etablieren. Q0013 verschiebt diese Grenze
**nicht** — es nutzt sie nur in der vorgesehenen Weise.

→ **OK. Gleiche Trust-Grenze wie der bereits live laufende formatter-Slot.**

## 7. Manifest-Trust — kein Self-Promotion-Pfad

Frage: kann ein Skript sein eigenes Manifest umgehen?

Das Manifest ist `Vec<CapabilityToken>` aus dem JSON, in `Sandbox::capabilities`
geclont (`sandbox.rs:43-52`). Es lebt im `Arc<Mutex<RunState>>` und ist vom
Skript-Scope **nicht** erreichbar (nicht als Rhai-Variable exponiert, keine
Methode auf `HostBridge` registriert, die es zurueckgibt). Der Skript-Author
kann den Vec nicht mutieren.

Slot↔Capability-Constraints: die Slot-Pruefung (`provider_lookup.rs:104-114`)
erzwingt, dass `script.kind == Provider{slot=expected_slot}` ist. Ein Skript
mit Slot=Validator kann nicht im Slot=Formatter-Aufruf landen — die Resolution
liefert `SlotMismatch`-Fallback. Damit ist es nicht moeglich, ein Skript in
einem Slot zu missbrauchen, fuer den seine Caps unangemessen waeren.

P1 + P3: beide Slot/Cap-Kombos sind koherent — Filter braucht keinen DB-Read,
Validator braucht keinen DB-Read (jeweils Reader-Tier mit ComputeOnly +/-
ReadI18n).

→ **OK.**

## 8. Test-Surface — keine Production-Bypass

Code-Review F2 hat angemerkt, dass die neuen P1/P3-Engine-Tests ein eigenes
`ScriptManifest::default()`-aehnliches Manifest konstruieren statt das
on-disk-Manifest zu laden. Sicherheitstechnische Frage: oeffnet das einen Pfad,
auf dem ein Test-only-API in Produktion bypassable wird?

Pruefung:
- `RhaiEngine::with_manifest` ist die *gleiche* API, die `save.rs:118`,
  `run.rs:118/187` und `provider_lookup.rs:116` aufrufen. Die Tests nutzen
  also die **Production-API**, nicht eine spezielle Test-Schiene. Beweis dafuer,
  dass sie nur den Test-Tree als Quelle nutzen: `cfg(test)` ist nicht
  erforderlich, weil die API auch in Produktion existieren muss.
- Was Tests neu konstruieren, ist die Caller-Seite (`ScriptManifest { ... }`),
  nicht eine Hintertuer in der Engine. In Produktion stammt das Manifest aus
  dem DB-Row, das vom Save-Pfad ueber `validate_manifest` gegangen ist (Save-
  Pfad) oder vom Loader-Pfad (Example-Trust, §1.1). Der Test-Caller umgeht
  *seinen* Validator nicht — er hat ja gar keinen.
- Konkretes Risiko: kann ein Production-Caller versehentlich ein fabriziertes
  `ScriptManifest` mit ueber-Caps an die Engine reichen? Theoretisch ja, aber
  jeder Production-Pfad (Save/Run/ProviderLookup) liest das Manifest aus dem
  DB-Row, der wiederum von `save_script` (validiert) oder
  `seed_scripts_from_example` (trusted Data-Dir) befuellt wurde. Es gibt keinen
  Production-Code-Pfad, der `RhaiEngine::with_manifest(&fabrizierte_manifest)`
  ruft.

→ **OK. Keine Test-only-Hintertuer.**

---

## Cross-Cuts

### Tests sind echte Sicherheits-Pins

Die Loader-Tests pinnen `kind=Provider{slot=Validator|Filter}` + `tier=Reader` +
`ComputeOnly` in der Manifest-Form. Bricht ein zukuenftiger Manifest-Schema-Change
diese Erwartungen, fallen die Tests rot — bevor ein faelschlich erhoehter
Slot/Tier in Produktion landet.

Empfehlung (advisory): die Code-Review-F1-Empfehlung — eine **negative**
Assertion ("P1 deklariert KEIN ReadI18n") — wuerde zusaetzlich vor Drift in die
*andere* Richtung schuetzen. Heute deklariert P1 ReadI18n, also wuerde diese
Assertion gerade rot fallen — sie ist konsequent erst nach dem F1-Fix
sinnvoll.

### Audit-Granularitaet bleibt wie Q0011 advisory A2

Falls in Stage-2 das Filter-Wiring durchgeschaltet wird, laeuft das Skript ueber
denselben `run_and_persist`-Pfad (mit `script_audit_log`-Eintrag pro Run, Token-
Use-Ledger, outcome-Tag). Q0013 bringt keine neue Audit-Granularitaet — was Q0011
advisory A2 gesagt hat (forged `throw` ist von echtem `HostError` nicht
unterscheidbar) gilt unveraendert. Bleibt advisory.

---

## Verifikation

- AST-Walk per Hand ueber `examples/d2v/scripts/d2v_balance_validator.rhai` +
  `d2v_stack_filter.rhai`: kein `throw`, kein `ctx.t`, kein `db.entities`,
  kein `db.entity`, keine String-Interpolation.
- Grep ueber den Diff: keine Aenderung an `engine/rhai.rs`, `sandbox.rs`,
  `provider_lookup.rs`, `save.rs`, `run.rs`. Das Q0009-/Q0011-Sicherheits-
  Geruest ist unangetastet.
- Cross-Check: `ops_for_named` in `shared/src/ops.rs` reagiert nicht auf den
  `script:`-Prefix → Server-seitig kein neuer Ausfuehrungspfad fuer P3.
- Cross-Check: `client/src/components/table/formatters.rs` ist die einzige
  Resolution-Call-Site fuer `script:`-Provider — der Filter-Slot ist client-
  seitig dormant.
- Bestaetigt durch Q0011-Security-Review-Pin: `forged_throw_does_not_determine_reported_error_kind`
  + Q0011-Audit-Pins. Q0013 fuegt keinen neuen Wurf-Pfad hinzu.

## Advisory (nicht-blockierend)

- **A1 (Least-Privilege-Drift, P1):** `d2v_balance_validator.manifest.json`
  deklariert `ReadI18n`, das Skript nutzt es nicht. Sicherheitstechnisch
  harmlos (Capability wird nicht ausgeuebt), aber Audit-Klarheit + zukuenftige
  Lift-/Cap-Analysen leiden. Identisch zu Code-Review F1. Fix: 1-Zeilen-Edit
  im Manifest + optional eine negative Loader-Test-Assertion.
- **A2 (Null-Pfad-Robustness, P1, Funktionalitaet):** P1 schmeisst auf
  `fields.value == ()` einen `HostError`, statt `false` zurueckzugeben.
  Sicherheits-irrelevant (kein neuer Forge-Pfad — siehe §4 / §5), aber wenn
  P1 spaeter an `validator_id`-Wiring angeschlossen wird und der Validator-Slot
  in einer Listen-Sicht laueft, kann ein einzelner `null`-Wert die ganze
  Spalten-Validierung zum Abbruch bringen statt einzelne Zeilen als "nicht
  balanciert" zu markieren. Identisch zu Code-Review F3.
- **A3 (Loader-Trust-Pfad, dokumentarisch):** `seed_scripts_from_example` ruft
  `validate_manifest` nicht. Das ist konsistent mit dem Data-Dir-Trust-Modell,
  liesse sich aber in einer spaeteren Iteration zumindest als Warnung in Logs
  formulieren, damit ein versehentlich ueber-deklarierender Data-Dir-Maintainer
  einen Hinweis bekommt. Nicht Q0013-Aufgabe.

## Fazit

Alle acht Security-Fokusbereiche sind sauber. Beide neuen Skripte bewegen sich
strikt innerhalb des Q0009/Q0011-Sandbox-Modells, ueben ihre deklarierten
Capabilities nicht ueber, beruehren den `HostErrorPayload`-Sentinel nicht, und
das Filter-Wiring (`script:d2v_stack_filter`) ist heute reine Metadaten ohne
Ausfuehrungspfad. Der ueber-deklarierte `ReadI18n` auf P1 ist Hygiene, keine
Sicherheits-Defizienz. **CLEARED.**
