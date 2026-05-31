# Q0017 — `validator_id` on-save Editor-Hookup (Stage-3 von Q0014 Lücke A)

- **Queue-Item:** `docs/queue/Q0017-validator-id-onsave-editor-hookup.md`
- **Vorgänger:** Q0014 (`docs/queue/done/Q0014-validator-id-slot-und-script-prefix-filterpfad.md`)
- **Status:** design
- **Typ:** feature
- **Tags:** d2v, script-first, validation, stage-3, follow-up
- **Security-Trigger:** `script`, `sandbox`, `rhai`, `validator`, `i18n-read` (siehe §9)

---

## 1. Problem & Kontext

Q0014 hat den additiven `validator_id`-Slot auf `ColumnMeta` eingeführt und in einem
System-Level-Test (`client/tests/validation_script_task.rs`) bewiesen, dass ein
`script:`-Validator **live** durch die echte `ValidationSystem::run`-Engine läuft —
reale Rhai-Engine, kein Mock-Prädikat. Die letzte Meile wurde dort **bewusst
deferred** (cut-line-C): die Script-Validatoren werden heute **nicht** in die
`ValidationSystem` der Live-Editor-UI registriert. Heute speist sich die Editor-
Validierung ausschließlich aus `EditorMeta.required` (`editor.rs:113`,
`ValidationSystem::import_required_from`). Ein gesetzter `validator_id` hat in der
echten Editor-UI **keine Wirkung** — der `d2v_balance_validator` greift nie.

Dieses Item schließt die Lücke: beim Editor-Load werden Spalten mit gesetztem
`validator_id` (Prefix `script:`) als `TaskFn` in dieselbe `ValidationSystem`-
Instanz registriert, die der on-save-Pfad (`editor.rs:134`) abfragt. Eine
fehlschlagende Validierung blockiert den Save in der Live-UI und zeigt die
`ValidationMessage`.

### 1.1 Schlüssel-Befund aus der Code-Recherche (verifiziert)

**Der Editor sieht `validator_id` heute nicht.** Der Editor konsumiert `EditorMeta`
(via `fetch_editor`), und `shared::EditorPropertyMeta` (`shared/src/editor.rs:42`)
trägt **kein** `validator_id`-Feld — nur statische Constraints (`min_length`,
`min`, `max`, `pattern`, `required`). Der `validator_id`-Slot lebt auf
`shared::ColumnMeta` (`shared/src/lib.rs:307`) und wird über `fetch_columns`
(`client/src/graphql/queries.rs:108`, Feld bereits durchgereicht) ausgeliefert.

→ **Konsequenz für das Design:** Der Editor muss die Validator-IDs aus den
**Columns** beziehen, nicht aus `EditorMeta`. Das vermeidet jede Änderung am
`shared`-Wire-Vertrag und am Server (`fetch_columns` liefert `validatorId` bereits).

**Kein d2v-Column setzt heute `validatorId`.** `examples/d2v/.../datev_entry/columns.json`
referenziert `formatterId`/`filterId`-Skripte, aber **keinen** `validatorId`. Damit
der `d2v_balance_validator` end-to-end greift (DoD-Punkt 3), muss die Spalte `value`
in dieser data-dir-Datei den Validator referenzieren.

**Der i18n-Key fehlt.** Q0014 definiert `SCRIPT_VALIDATION_KEY = "validation.script"`
(`client/src/validation/script_task.rs:22`). `t("validation.script")` normalisiert zu
Fluent-ID `validation-script` (`i18n/mod.rs:270`). Dieser Eintrag existiert in **keiner**
`main.ftl` — die Message würde heute als roher Key gerendert. Muss ergänzt werden.

---

## 2. Ziel & Definition of Done

1. Beim Editor-Load werden Columns mit `validator_id` (Prefix `script:`) als
   `ValidationTask` in dieselbe `ValidationSystemHandle`-Instanz registriert, die
   der on-save-Pfad (`editor.rs:134`) abfragt — additiv **nach**
   `import_required_from`.
2. Eine fehlschlagende Validierung blockiert den Save in der Live-UI
   (`result.has_blocking()` → `return` vor `saving.set(true)`) und füllt
   `validation_messages`, sodass `FieldEditor` die `ValidationMessage` unter dem
   Feld zeigt (Message-Key `validation.script` aus Q0014).
3. `d2v_balance_validator` greift end-to-end im Editor (SOLL/HABEN-Bilanz): die
   d2v-data-dir-Column `value` referenziert den Validator.
4. Der i18n-Key `validation-script` existiert in `de`/`en`/`fr` `main.ftl`.
5. Test: ein Editor-Integration-Test (native `#[test]` in `client/tests/`)
   beweist, dass ein verletzender Datensatz **nicht** speicherbar ist (blocking)
   und ein valider Datensatz durchläuft — über dieselbe Registrierungs-Funktion,
   die der Editor verwendet.

### 2.1 Out of Scope / Invarianten (NICHT aufweichen)

- **Server-seitige Validierung bleibt autoritativ.** Der client-seitige Validator
  ist UX-Frühwarnung. Kein Server-Code wird angefasst (Q0014-Security-Advisory).
- **Validator-fail-open bleibt akzeptabel:** Script-Fehler / Missing / NotActive /
  SlotMismatch / Nicht-Bool ⇒ Save erlaubt. Der `script_validator_task`-Pfad
  (Q0014) implementiert das bereits (`_ => None`). Der Server fängt es ohnehin ab.
- **Kein `shared`-Wire-Change.** Insb. **kein** `validator_id` auf
  `EditorPropertyMeta` — der Editor liest Columns.
- **Kein neuer GraphQL-Query.** `fetch_columns` existiert und liefert `validatorId`.

---

## 3. Lösungsansätze (3, mit Trade-offs)

### Ansatz A — Editor zieht Columns zusätzlich und registriert daraus *(empfohlen)*

Die Editor-Load-Closure (`editor.rs:104` `LocalResource`) ruft zusätzlich zu
`fetch_editor` ein `fetch_columns(&entity_type)`. Eine neue reine Funktion
`register_script_validators(sys, &columns, env)` läuft über die Columns, baut für
jede mit `validator_id`-`script:`-Prefix via `script_validator_task` (Q0014) einen
`TaskFn` und registriert ihn unter dem Entity-Typ. Host/Registry/Ctx kommen aus dem
bereits providedten `ScriptRenderEnv`-Context (`use_script_render_env`).

- **+** Kein `shared`-Change, kein Server-Change, keine neue Query.
- **+** Wiederverwendet exakt den Q0014-`script_validator_task` und `ScriptRenderEnv`
  (denselben `RenderHost` + dieselbe `Arc<ScriptRegistry>` wie der Tabellen-/
  Formatter-Pfad).
- **+** Registrierungs-Logik ist eine **reine Funktion** → ohne Leptos-Mount testbar
  (wie Q0014).
- **−** Zweiter GraphQL-Roundtrip beim Editor-Load (`fetch_editor` + `fetch_columns`).
  Akzeptabel: beide sind klein, laufen einmal pro Editor-Öffnung, und die Tabelle
  fetcht Columns ohnehin schon — der Server-Pfad ist warm.

### Ansatz B — `validator_id` additiv auf `EditorPropertyMeta` ziehen

`shared::EditorPropertyMeta` bekommt ein additives `validator_id`-Feld; Server-
`editor_meta`-Builder (`data.rs`) füllt es aus den Column-Defaults; Editor liest es
aus `EditorMeta`.

- **+** Editor braucht nur eine Query (`fetch_editor`).
- **−** Wire-Change in `shared` + Server-Builder + `EDITOR_QUERY` + Client-Mapping +
  neuer Wire-Pin-Test. Deutlich größerer Blast-Radius, berührt den Server.
- **−** Verletzt „kein `shared`-Change / kein Server-Change" aus dem Item-Scope.
- **Verworfen:** zu invasiv für ein „last-mile-wiring"-Item.

### Ansatz C — Validatoren komplett aus `Settings`/`FieldTypeDefaults` ableiten

Resolution-Kette `ColumnMeta.validator_id` → `PropertySettings.validator_id` →
`FieldTypeDefaults.validator_id` im Editor nachbauen (analog Formatter-Resolution).

- **+** Vollständige Symmetrie zum Formatter-/Filter-Resolver.
- **−** Über-Engineering für dieses Item: heute referenziert nur **eine** Column
  einen Validator. Die Resolution-Kette ist ein eigenes Folge-Item wert.
- **Verworfen (YAGNI):** Q0017 wired den `ColumnMeta.validator_id`-Pfad; die
  mehrstufige Resolution ist Scope eines späteren Items.

**Entscheidung: Ansatz A.**

---

## 4. Architektur & Datenfluss

```
Editor-Load (LocalResource async closure, editor.rs:104)
  ├─ fetch_editor(type)  ──► EditorMeta ──► import_required_from(sys)   [bestehend]
  └─ fetch_columns(type) ──► Vec<ColumnMeta>
                                │
                                ▼
        register_script_validators(sys, &columns, &env)   [NEU, reine Fn]
                                │  pro Column mit validator_id="script:…":
                                │    script_validator_task(key, vid, registry, host, ctx)  [Q0014]
                                ▼
                       sys.register(type, ValidationTask{target:key, task})

on-save (editor.rs:127 Callback, synchron)
  └─ validation_for_save.run(type, &fields)  ──► ValidationResult
        ├─ result.has_blocking() ⇒ messages setzen + return (Save geblockt)   [bestehend]
        └─ sonst ⇒ RemoteSource.save(...)  ──► Server validiert autoritativ
```

**Zentrale Beobachtung zur Reaktivität (der Deferral-Grund):** Die
`ValidationSystemHandle` ist **nicht** reaktiv (`Arc<Mutex<ValidationSystem>>`,
`validation/mod.rs:206`). Sie wird beim Load gefüllt und beim Save-Event synchron
abgefragt. `ValidationSystem::run` ist synchron (Q0014 bewiesen). Der `script_task`-
`TaskFn` macht den `lookup_provider`-Run **zur Save-Zeit synchron** — kein `async`,
kein Signal im Pfad. Damit ist die Registrierung strukturell identisch zu den
`required`-Tasks: ein `Arc<Fn(...)>` in derselben Registry. **Keine reaktive
Umstrukturierung der `ValidationSystem` nötig.**

Das einzige reaktive Detail: die `Arc<ScriptRegistry>` wird in `app.rs` **async**
vom Server befüllt (`refresh_from_server`, bump `scripts_version`). Registrierung
und Save sind aber zeitlich entkoppelt: der Validator-`TaskFn` führt den
Registry-Lookup **lazy zur Save-Zeit** aus (nicht zur Registrierungs-Zeit). Beim
Speichern ist die Registry längst befüllt. Ist sie es ausnahmsweise nicht
(Missing) ⇒ Fallback ⇒ Save erlaubt (fail-open, Server autoritativ). **Kein
Timing-Race blockiert je fälschlich.**

---

## 5. Komponenten & Verträge

### 5.1 NEU: `register_script_validators` (reine Funktion)

Platzierung: `client/src/validation/script_task.rs` (neben `script_validator_task`),
damit der Test sie ohne Leptos-Mount aufrufen kann und sie nah am Q0014-Helper liegt.

```rust
/// Registriert für jede Column mit `validator_id` (Prefix `script:`) einen
/// `ValidationTask` im `sys`, gebunden an den Entity-Typ. Columns ohne
/// (Script-)Validator werden ignoriert. Reine Funktion — kein Leptos.
pub fn register_script_validators(
    sys: &mut ValidationSystem,
    entity_type: &str,
    columns: &[shared::ColumnMeta],
    registry: std::sync::Arc<ScriptRegistry>,
    host: std::sync::Arc<dyn HostApi>,
    ctx: ScriptCtx,
) {
    for col in columns {
        let Some(vid) = col.validator_id.as_deref() else { continue };
        if let Some(task) = script_validator_task(
            &col.key, vid, registry.clone(), host.clone(), ctx.clone(),
        ) {
            sys.register(entity_type, ValidationTask { target: col.key.clone(), task });
        }
    }
}
```

- **Eingaben:** `&mut ValidationSystem`, Entity-Typ, Columns, Registry, Host, Ctx.
- **Effekt:** registriert 0..n Tasks. Non-`script:`-`validator_id` ⇒ `script_validator_task`
  liefert `None` ⇒ übersprungen (Q0014 kennt keine statischen Validator-IDs).
- **Abhängigkeiten:** `script_validator_task` (Q0014), `ScriptRegistry`, `HostApi`,
  `ScriptCtx` — alle bestehend.

### 5.2 Editor-Load-Closure (`editor.rs:104` erweitern)

Innerhalb der bestehenden `LocalResource`-Closure, nach dem
`import_required_from`-Block:

```rust
// bestehend:
if let Some(m) = meta.clone() {
    editor_meta.set(Some(m.clone()));
    validation.update(|sys| sys.import_required_from(&m));
}
// NEU: Script-Validatoren aus Columns registrieren.
if let Ok(columns) = fetch_columns(&entity_type).await {
    let env = use_script_render_env();          // Registry + Host + ctx-Felder
    let ctx = env.script_ctx();                 // siehe §5.3
    validation.update(|sys| {
        register_script_validators(
            sys, &entity_type, &columns,
            env.registry.clone(), env.host.clone(), ctx,
        );
    });
}
```

`fetch_columns` ist bereits importiert-fähig (`graphql::queries`). `use_script_render_env`
liefert den in `app.rs` providedten `ScriptRenderEnv` (Registry = dieselbe `Arc`,
die async befüllt wird; Host = `RenderHost`).

> **Hinweis Leptos-Context:** `use_script_render_env()` muss im reaktiven/Component-
> Scope laufen. Die `LocalResource`-Closure läuft im Component-Scope von `EditorBody`,
> hat also Context-Zugriff — wie schon `use_validation_system`/`use_header_registry`
> dort konsumiert werden. Der Env-Handle (`Arc` + `RwSignal`) ist `Clone`+`'static`
> und darf in die `async`-Closure gemoved werden; alternativ vor dem
> `LocalResource::new` einmal `use_script_render_env()` greifen und klonen
> (analog `validation_for_load`/`header_for_load` Pattern, `editor.rs:102-103`).
> **Plan-Vorgabe:** Env **vor** `LocalResource::new` greifen und in die Closure
> moven — vermeidet Context-Zugriff im async-Body.

### 5.3 `ScriptCtx` für den Editor-Validierungs-Run

`d2v_balance_validator` ist `ComputeOnly + ReadI18n`. `RenderHost.i18n_t`
(`render_host.rs:24`) liefert `crate::i18n::t` → ReadI18n ist erfüllt. Die
`ScriptCtx` trägt locale/user/tenant. Der Tabellen-Filter-Pfad nutzt heute
`ScriptCtx::default()` (`data_source.rs:399`). **Best-guess-Entscheidung:** Wir
spiegeln das und reichen die `ScriptRenderEnv`-Felder durch, falls ein Konstruktor
existiert; sonst `ScriptCtx::default()` (der Balance-Validator liest keine
ctx-Felder, nur `fields`). Ein kleiner Helfer `ScriptRenderEnv::script_ctx()` (baut
`ScriptCtx` aus `locale`/`user_id`/`tenant_id`) ist wünschenswert, aber **optional**;
ist er nicht trivial, ist `ScriptCtx::default()` für dieses Item ausreichend und
konsistent mit dem Filter-Pfad.

### 5.4 data-dir: `validatorId` auf d2v `value`-Column

`examples/d2v/entities/datev_entry/columns.json`, Column `value`:

```json
{ "key": "value", …, "validatorId": "script:d2v_balance_validator" }
```

Damit greift der Validator end-to-end im Editor (DoD-3). Reine data-dir-Änderung,
additiv, kein Code.

### 5.5 i18n: `validation-script`-Key

In `client/locales/{de,en,fr}/main.ftl` ergänzen, z.B.:

- `de`: `validation-script = Validierung fehlgeschlagen.`
- `en`: `validation-script = Validation failed.`
- `fr`: `validation-script = Échec de la validation.`

(Wording im Plan finalisieren; Hauptsache der Key existiert, sonst rendert die
Message roh.)

---

## 6. Test-Strategie

### 6.1 Primärtest — native `#[test]` (Pflicht, DoD-5)

Neue Datei `client/tests/editor_validator_hookup.rs` (oder Erweiterung von
`validation_script_task.rs`). Mustert sich am bestehenden Q0014-Test:

1. `ScriptRegistry` mit `d2v_balance_validator` (Active, ComputeOnly+ReadI18n) füllen.
2. `Vec<ColumnMeta>` bauen, deren `value`-Column `validator_id =
   "script:d2v_balance_validator"` trägt (plus eine Column ohne Validator als
   Negativ-Kontrolle).
3. `register_script_validators(&mut sys, "datev_entry", &columns, reg, MockHostApi, ctx)`.
4. **Assert blocking:** `sys.run("datev_entry", unbalanced_fields)` →
   `res.has_blocking() == true`, Message-Key `validation.script`, `target == "value"`.
5. **Assert pass:** `sys.run("datev_entry", balanced_fields)` → `res.is_empty()`.
6. **Assert non-script ignoriert:** eine Column mit `validator_id = Some("static-x")`
   erzeugt keinen Task (oder ohne `validator_id` ⇒ kein Task).

Damit ist **exakt der Pfad** getestet, den der Editor benutzt
(`register_script_validators` ist die einzige neue Logik; der on-save-Block ist
unverändertes Bestandsverhalten, das Q0014 bereits an `run`/`has_blocking` koppelt).

### 6.2 Editor-Mount-Test — bewusst NICHT

Ein voller Leptos-Mount-/wasm-bindgen-Test des `EditorBody`-DOM würde eine
Browser-Test-Harness (`wasm-bindgen-test` + headless) verlangen, die das Projekt
heute nicht fährt (alle bestehenden Client-Tests sind native `#[test]`). Der
on-save-Save-Block ist trivialer, unveränderter Glue-Code (`has_blocking()` →
`return`); ihn über die reine `register_script_validators`+`run`-Kette zu testen
ist die **leichteste Variante, die den Save-Block tatsächlich ausübt**. Manuelle
Verifikation im laufenden Client (SOLL≠HABEN ⇒ Save blockt + Message) gehört in den
Plan-Verifikations-Schritt, nicht in CI.

### 6.3 Regressions-Guards

- `cargo test -p shared` (Wire-Pins unverändert grün — kein `shared`-Change).
- `cargo test -p client` (neuer Test + Q0014-Test grün).
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`.

---

## 7. Betroffene Dateien

| Datei | Änderung |
|---|---|
| `client/src/validation/script_task.rs` | NEU `register_script_validators` (reine Fn) |
| `client/src/routes/editor.rs` | Load-Closure: `fetch_columns` + Registrierung nach `import_required_from`; Env vor `LocalResource` greifen |
| `client/locales/de/main.ftl` | `validation-script`-Key |
| `client/locales/en/main.ftl` | `validation-script`-Key |
| `client/locales/fr/main.ftl` | `validation-script`-Key |
| `examples/d2v/entities/datev_entry/columns.json` | `value`-Column: `validatorId` |
| `client/tests/editor_validator_hookup.rs` | NEU Integration-Test |
| (optional) `client/src/components/script_renderer.rs` | `ScriptRenderEnv::script_ctx()`-Helfer, falls §5.3-Variante mit echtem ctx gewählt |

**Kein** Server-Code, **kein** `shared`-Wire-Typ.

---

## 8. Risiken & Annahmen

- **Annahme:** `use_script_render_env()` ist im `EditorBody`-Scope verfügbar. Falls
  ein Editor je außerhalb von `AppLayout`/`provide_script_render_env` gemountet wird,
  müsste die Registrierung defensiv `use_context::<ScriptRenderEnv>()`-`Option`
  behandeln (kein Env ⇒ keine Script-Validatoren ⇒ Save erlaubt, fail-open).
  **Plan-Vorgabe:** `use_context::<ScriptRenderEnv>()` (Option) statt
  `expect`-`use_script_render_env`, damit der Editor ohne Env nicht panict.
- **Annahme:** `fetch_columns` und `fetch_editor` liefern konsistente Keys
  (`value`-Column ↔ `value`-Property). Verifiziert: beide lesen denselben
  data-dir-Eintrag server-seitig.
- **Race (entkräftet):** Registry async befüllt — Lookup ist lazy zur Save-Zeit ⇒
  fail-open, nie ein falscher Block.
- **Doppelregistrierung:** `import_required_from` und `register_script_validators`
  können beide für dasselbe Feld Tasks anlegen (required + balance). Das ist
  gewollt — beide laufen, beide Messages erscheinen. `ValidationSystem::register`
  pusht additiv; kein Konflikt.
- **Re-Load:** Öffnet der User denselben Editor erneut im selben App-Leben, könnte
  derselbe Task ein zweites Mal registriert werden (die `ValidationSystem`-Instanz
  lebt App-weit via Context). **Plan-Vorgabe:** entweder (a) vor der Registrierung
  die Tasks dieses Entity-Typs zurücksetzen (neue `ValidationSystem`-Methode
  `clear(entity_type)`), oder (b) Idempotenz akzeptieren, weil doppelte identische
  Messages dedupliziert dargestellt werden können. Empfehlung: (a) `clear(entity_type)`
  am Anfang der Load-Closure — sauber und billig; betrifft auch das bestehende
  `import_required_from` (das heute dasselbe Re-Load-Verhalten hätte). Im Plan
  entscheiden; (a) bevorzugt.

---

## 9. Security-Betrachtung

Security-Trigger-Wörter: **script, sandbox, rhai, validator, i18n-read**.

- Der Validierungs-Run läuft durch denselben `lookup_provider` → `RhaiEngine` +
  `Sandbox` (Deadline-Check) wie Q0014. Keine neue Engine, keine neue
  Capability-Oberfläche. Host = `RenderHost`: `db_fetch`/`db_patch` sind
  `ServerOnlyFunction`-Errors; nur `i18n_t`/`audit_log` sind erreichbar
  (ComputeOnly+ReadI18n). Kein Datenfluss-Eskalationspfad.
- **Client-Validierung bleibt nicht-autoritativ.** Der Server validiert weiterhin
  autoritativ beim eigentlichen Save (`RemoteSource.savable().create/update`). Der
  Client-Validator ist reine UX-Frühwarnung. Diese Invariante wird **nicht**
  aufgeweicht — kein Code-Pfad macht den Client-Check zur einzigen Schranke.
- **Fail-open ist bewusst sicher:** Script-Fehler ⇒ Save erlaubt ⇒ Server lehnt ab,
  falls die Daten wirklich invalide sind. Ein kompromittiertes/fehlerhaftes
  Client-Script kann maximal eine *fehlende* Client-Warnung verursachen, nie einen
  unzulässigen Schreibvorgang erzwingen.
- Kein neuer Netzwerk-Endpunkt, kein neuer Persistenz-Pfad, keine
  Capability-Erweiterung. `security_review.required` bleibt aus Sicht des Items
  `false` (Item-Frontmatter); die Q0014-Security-Advisory-Invariante ist im Design
  explizit verankert (§2.1, §9).

---

## 10. Offene Punkte für den Plan

- §5.3: `ScriptCtx::default()` vs. echter ctx aus Env — Plan finalisiert (Default
  genügt für den Balance-Validator; echter ctx ist nice-to-have).
- §8 Re-Load: `clear(entity_type)` (bevorzugt) vs. Idempotenz akzeptieren.
- §5.5: finales FTL-Wording.
