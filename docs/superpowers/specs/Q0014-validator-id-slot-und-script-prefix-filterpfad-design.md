# Q0014 — `validator_id`-Slot + `script:`-Prefix im Filter-Pfad (Design)

- **Queue-Item:** `docs/queue/Q0014-validator-id-slot-und-script-prefix-filterpfad.md`
- **Quell-Item:** Q0013 (`docs/queue/done/Q0013-minimale-livable-d2v2019-teilmenge.md`)
- **Status:** brainstormed (Scope-Entscheidungen gelockt, 2026-05-30)
- **Typ:** feature, additive Wire-Erweiterung

## Problemstellung

Q0013 hat zwei d2v-Scripts ausgeliefert, die zur Laufzeit **nicht** greifen
(„dormant", siehe Q0013-Security-Review-Notiz):

- **Lücke A:** `d2v_balance_validator` (Validator-Slot, ComputeOnly + ReadI18n,
  `examples/d2v/scripts/d2v_balance_validator.{rhai,manifest.json}`) konnte nur
  *ladbar + engine-getestet* shippen, weil `shared::ColumnMeta` keinen
  `validator_id`-Slot trägt. Es gibt keine Metadaten-Stelle, die einen Validator
  pro Spalte erzwingt.
- **Lücke B:** `datev_entry.stackId` trägt `"filterId": "script:d2v_stack_filter"`
  (`examples/d2v/entities/datev_entry/columns.json:16`), aber kein Pfad
  konsumiert den `script:`-Prefix im Filter. Der Filter-Resolver
  (`client/src/components/table/filters/mod.rs::resolve_filter_id`) kennt nur
  benannte Built-in-Filter-Widgets und fällt bei unbekannter ID auf den
  FieldType-Default zurück — der Skript-Filter läuft nie.

Ziel von Q0014: **beide Metadaten live machen** (nicht erneut dormant
ausliefern), in minimalem Scope. Die Editor-UI-Verdrahtung für Validatoren und
serverseitiges Filter-Pushdown bleiben bewusst draußen (siehe „Out of scope").

### Bestandsaufnahme (verifiziert gegen aktuellen Code)

- `shared::ColumnMeta` (`shared/src/lib.rs:282–307`) hat
  `comparator_id`/`filter_id`/`editor_id`/`formatter_id`/`action_ids`, **kein
  `validator_id`**.
- `shared::FieldTypeDefaults` (`shared/src/settings.rs:25–46`) trägt
  `filter_id`/`editor_id`/`formatter_id` (+ `allowed_*`), **kein
  `validator_id`**.
- Der `script:`-Mechanismus existiert bereits und funktioniert für Formatter:
  `client/src/script/provider_lookup.rs` (`SCRIPT_PREFIX`, `parse_script_id`,
  `lookup_provider`, `script_value_to_json`). Live über `FieldCell`
  (`client/src/components/table/formatters.rs:44–78`). Q0014 **erweitert** die
  Reichweite dieses Mechanismus auf Validator- und Filter-Slot — etabliert ihn
  nicht neu.
- Es gibt ein echtes Validierungs-Runtime: `client/src/validation/mod.rs`
  (`ValidationSystem`, `TaskFn`-Closures, `ValidationSystem::run`), konsumiert
  synchron beim Save in `client/src/routes/editor.rs:134`. Heute kommen Tasks
  ausschließlich aus `EditorMeta::required` (`import_required_from`).
- **`LocalSource` existiert bereits vollständig** (`client/src/components/table/data_source.rs:190–316`)
  und wertet Prädikate via `ops_for_named` (Built-in-Ops) aus
  (`passes()`, Zeilen 217–251). `ScriptSource`
  (`client/src/script/data_source.rs`) wickelt `LocalSource` als DataSource-
  Adapter. Der Gap für Lücke B ist damit **enger** als „LocalSource bauen": es
  fehlt nur der Zweig in `passes()`, der einen `script:`-Filter pro Zeile als
  boolesches Skript-Prädikat auswertet statt über die Built-in-Ops.
- **Kein ColumnMeta-Wire-Pin existiert.** `shared/tests/field_type_wire_format.rs`
  pinnt nur `FieldType`. Q0014 muss einen ColumnMeta-Roundtrip-Pin **neu
  anlegen**.
- `shared::DATA_DIR_FORMAT` = `1` (`shared/src/lib.rs:20`).

---

## Lücke A — `validator_id`-Slot (Cut-Line C)

### Cut-Line (gelockt)

`validator_id` additiv in `ColumnMeta` + `FieldTypeDefaults` einführen,
GraphQL-Re-Wrap + Client-Deser mitziehen, ColumnMeta-Wire-Pin **neu anlegen**,
und einen Resolver-Helper, der `script:`-Validatoren erkennt. **Live bewiesen**
durch Konstruktion des Script-Validator-`TaskFn` und Lauf durch das **echte**
`ValidationSystem::run` (System-Level-Test) — die Metadaten sind damit echt
live, nicht dormant. **Nicht** in die `editor.rs`-Load-Closure verdrahten (das
Editor-UI-Hookup ist explizites Folge-Item, siehe „Out of scope").

### Felder spiegeln `formatter_id` 1:1

`validator_id: Option<String>` an jeder Stelle, an der `formatter_id` heute
steht, additiv mit `#[serde(default, skip_serializing_if = "Option::is_none")]`:

| Datei | Stelle | Änderung |
|---|---|---|
| `shared/src/lib.rs` | `ColumnMeta` (~:302) | `validator_id: Option<String>` neben `formatter_id`; Doc-Kommentar spiegeln |
| `shared/src/settings.rs` | `FieldTypeDefaults` (~:33) | `validator_id: Option<String>` neben `formatter_id`; `allowed_validator_ids: Vec<String>` analog zu `allowed_formatter_ids` (Konsistenz, additiv) |
| `server/src/schema.rs` | `SimpleObject`-`ColumnMeta` (~:39) | `validator_id: Option<String>` |
| `server/src/data.rs` | `convert_column` (~:84) | `validator_id: c.validator_id` |
| `client/src/graphql/queries.rs` | `COLUMNS_QUERY`-Selection (~:68), `RawColumnMeta` (~:88), `fetch_columns`-Mapping (~:122) | `validatorId` ins Selection-Set; `validator_id: Option<String>` in `RawColumnMeta`; im Mapping `validator_id: c.validator_id` |

**Hinweis Konstruktor-Call-Sites:** `ColumnMeta { … }`-Literale ohne
`..Default::default()` müssen das neue Feld setzen. Verifizierte Call-Sites:
`server/src/reference.rs:76` und Test-Helfer dort. (Mechanisch; `validator_id: None`.)

### Resolver-Helper

In `client/src/components/registries/resolve.rs::resolve_implementation_id`
(~:26) den `match registry { "filter" | "editor" | "formatter" … }` um den Arm
`"validator" => column.validator_id.clone()` erweitern und in Stufe 2
(`field_type_defaults`) `"validator" => defaults.validator_id.clone()`. Damit
folgt der Validator derselben Resolution-Kette (Spalte → FieldTypeDefaults →
[kein Client-Hardcoded-Fallback für Validatoren]).

### Script-Validator-`TaskFn` (das Live-Stück)

Neue Funktion in `client/src/validation/mod.rs` (oder einem Geschwister-Modul,
z.B. `validation/script_task.rs`), die eine `validator_id` mit `script:`-Prefix
in einen `ValidationTask` übersetzt. Aufbau spiegelt den Formatter-Pfad aus
`formatters.rs:66`:

- `TaskFn`-Closure erhält `(target, value, fields)`.
- Baut `ScriptInputs { value, fields }` (`shared/src/script/engine.rs:22`).
- Ruft `lookup_provider(&validator_id, ProviderSlot::Validator, &registry, host, ctx, inputs)`.
- `LookupResult::Ok { value }` → boolesch interpretieren
  (`ScriptValue::Bool(false)` = Validierungsfehler → `Some(ValidationMessage::error(target, key))`;
  `Bool(true)`/andere Slot-Konventionen → `None`).
  - **Konvention:** Der Validator-Slot gibt `true` = „valide" zurück (so liest
    `d2v_balance_validator.rhai`: `(soll+soll2) == (haben+haben2)`).
  - Bei JSON-Rückgabe optional `script_value_to_json` für strukturierte
    Validator-Errors (vorhanden in `provider_lookup.rs:179`); für Q0014 genügt
    der boolesche Pfad.
- `LookupResult::Fallback`/`NotAScriptId` → `None` (kein Block; Audit-Log hat
  `lookup_provider` bereits gefüllt). Fallback heißt „Validator nicht aktiv" →
  nicht blockieren.

Synchron, gleiches Muster wie Formatter — keine async-Brücke nötig, passt
direkt in `ValidationSystem::run`.

### Akzeptanz (Lücke A)

- ColumnMeta-Wire-Pin grün (siehe Teststrategie).
- System-Test: `validator_id = "script:d2v_balance_validator"` → Script-`TaskFn`
  konstruiert → in `ValidationSystem` registriert → `ValidationSystem::run` mit
  balancierten vs. unbalancierten `fields` liefert leeres vs. blockierendes
  `ValidationResult`. Das beweist: Metadaten sind live.

---

## Lücke B — `script:`-Prefix im Filter-Pfad (Resolver + minimales LocalSource-Prädikat)

### Cut-Line (gelockt)

Filter-Resolver erkennt den `script:`-Prefix, **und** ein minimaler
client-seitiger Prädikat-Eval-Pfad: `LocalSource` (existiert bereits) soweit
erweitern, dass ein `script:`-Filter-Prädikat (`d2v_stack_filter`) eine Zeile
filtert, mit End-to-End-Test, der beweist, dass `d2v_stack_filter` Zeilen per
`stackId` ein-/ausschließt. **Minimal halten:** nur was der boolesche Per-Row-
Prädikat-Pfad braucht; **kein** serverseitiges Filtern, **kein** allgemeines
Filter-Pushdown.

### Strukturelle Klärung

Ein `script:`-Filter ist **kein** Filter-Input-Widget (text/range), sondern ein
**Per-Row-Boolean-Prädikat**: `row.stackId == selectedStackId`. Er gehört
deshalb in die **Auswertung** (`LocalSource::passes`), nicht (nur) in die
Widget-Resolution (`resolve_filter_id` → `table_view.rs`). Der Widget-Resolver
darf einen `script:`-Filter **nicht** als Built-in-Widget rendern; der `_ =>
default_id_for(...)`-Fallback in `resolve_filter_id` (`filters/mod.rs:50`) tut
das heute fälschlich. Q0014 fügt dort einen frühen `script:`-Erkenner ein, der
**kein** Built-in-Widget liefert (kein sichtbares Filter-Input für die
Skript-Spalte; der ausgewählte Stack kommt aus dem bestehenden Filter-State,
nicht aus einem neuen Widget). Der `selectedStackId`-Wert wird dem Skript über
`ScriptInputs.fields` mitgegeben (das Skript liest `fields.selectedStackId`,
siehe `d2v_stack_filter.rhai:10`).

### Felder/Funktionen

| Datei | Stelle | Änderung |
|---|---|---|
| `client/src/components/table/filters/mod.rs` | `resolve_filter_id` (~:35) | Früh-Return: trägt `column.filter_id` den `SCRIPT_PREFIX`, liefere `None` (kein Built-in-Widget) statt FieldType-Default — verhindert das fälschliche Rendern eines Range-Widgets über `datev_entry.stackId` |
| `client/src/components/table/data_source.rs` | `ColumnLookup` (~:184), `LocalSource::new` (~:197), `LocalSource::passes` (~:217) | `ColumnLookup` um `filter_id: Option<String>` ergänzen (heute schon gehalten, aber nur für `ops_for_named` genutzt); in `passes()` pro Prädikat prüfen: hat die Spalte einen `script:`-Filter → Skript-Prädikat-Pfad statt `ops_for_named` |
| (Skript-Prädikat-Helper) | neu, z.B. in `data_source.rs` oder `client/src/script/` | Funktion, die `(filter_id, entity.fields, selected_value)` → `bool` via `lookup_provider(ProviderSlot::Filter)` auswertet; `ScriptValue::Bool` → Prädikat-Ergebnis; `Fallback`/`NotAScriptId`/Nicht-Bool → Zeile durchlassen (`true`, nicht ausschließen) |

### Wiring-Detail: woher kommt `selectedStackId`?

Der vom UI gewählte Stack lebt im bestehenden `FilterCriteria`-State
(`predicates` für die Spalte `stackId`). Beim Aufruf des Skript-Prädikats wird
der Predicate-Vergleichswert als `selectedStackId` in eine **angereicherte**
`fields`-Map injiziert (Kopie von `entity.fields` + Schlüssel
`selectedStackId`), bevor `ScriptInputs` gebaut wird. Fehlt/`-1` →
`d2v_stack_filter` liefert `true` (alle Stacks). So bleibt das Skript die einzige
Quelle der Filterlogik; `LocalSource` reicht nur Zeilenwerte + Auswahl durch.

### `LocalSource`-Verfügbarkeit im Live-Pfad

`EntityListPage` konstruiert heute eine `RemoteSource`. Q0014 baut den
Skript-Filter im `LocalSource`-Pfad (der via `ScriptSource` bereits existiert).
Die **Live-Verdrahtung** des Filters in `EntityListPage` (RemoteSource →
LocalSource-Umschaltung für gefilterte Skript-Spalten) ist für den minimalen
Scope **nicht** erforderlich: der End-to-End-Test treibt `LocalSource` direkt
(realer `lookup_provider` + reale Engine + `d2v_stack_filter`). Das beweist den
Prädikat-Pfad live, ohne das volle DataSource-Routing in `EntityListPage`
anzufassen. (Das Page-Level-Routing ist eine natürliche, aber separate
Erweiterung — nicht in dieser Cut-Line.)

### Akzeptanz (Lücke B)

- `resolve_filter_id` rendert für `script:`-Spalten **kein** Built-in-Widget.
- End-to-End-Test: `LocalSource` mit `datev_entry`-artigen Zeilen
  (`stackId` 1/2/3) + Spalten-`filter_id = "script:d2v_stack_filter"` + Filter
  `selectedStackId = 2` → `fetch()` liefert nur Zeilen mit `stackId == 2`;
  `selectedStackId = -1`/fehlend → alle Zeilen. Lauf durch realen
  `lookup_provider` + reale Rhai-Engine.

---

## Teststrategie

1. **ColumnMeta-Wire-Pin (neu):** `shared/tests/column_meta_wire_format.rs`.
   - Roundtrip `ColumnMeta { …, validator_id: Some("script:d2v_balance_validator"), … }`
     → JSON enthält `"validatorId"` → zurück deserialisiert gleich.
   - `validator_id: None` → Feld wird **nicht** serialisiert (`skip_serializing_if`).
   - Deserialisierung einer ColumnMeta-JSON **ohne** `validatorId` → `None`
     (Abwärtskompatibilität, Beweis für additiv). Muster spiegelt
     `shared/tests/reference_wire_format.rs`.
2. **ValidationSystem-System-Test (Lücke A, live):** in `client` (z.B.
   `client/src/validation/mod.rs`-`#[cfg(test)]` oder
   `client/tests/`). Registriert den Script-Validator-`TaskFn` für
   `script:d2v_balance_validator` in einer echten `ValidationSystem`-Instanz,
   ruft `run(entity_type, fields)` mit (a) balancierten und (b) unbalancierten
   Soll/Haben-`fields`. Erwartet leeres bzw. blockierendes `ValidationResult`.
   Skript-Quelle = die echte `examples/d2v/scripts/d2v_balance_validator.rhai`
   (oder inline gespiegelt), Lauf über reale Engine + `MockHostApi`
   (`shared::script::testing::MockHostApi`, vgl.
   `provider_lookup.rs`-Tests).
3. **End-to-End-LocalSource-Prädikat-Test (Lücke B):** treibt `LocalSource`
   mit `script:d2v_stack_filter` (siehe Akzeptanz Lücke B). Beweist
   Ein-/Ausschluss per `stackId`. Reale Engine, kein Mock-Prädikat.

### Windows-Test-Caveat

`server.exe`/`rlib`-Datei-Locks unter Windows: Tests mit
`cargo test --target-dir target-test ...` laufen lassen (`target-test/` ist in
`.gitignore`). **Falls** `E0786` / `STATUS_STACK_BUFFER_OVERRUN` / rlib-Link-
Fehler auftauchen: das ist die bekannte transiente target-test-Cache-Korruption
aus Q0016 — `cargo clean` bzw. frisches Target-Dir, **kein** echter Bug.

---

## `DATA_DIR_FORMAT` bleibt unverändert

Beide Felder (`validator_id`, `allowed_validator_ids`) sind **additiv** mit
`#[serde(default)]`: alte data-dirs ohne `validatorId` laden weiter (Feld =
`None`/leer). Nach CLAUDE.md-Konvention erhöhen additive Loader-Änderungen
`shared::DATA_DIR_FORMAT` **nicht**. `DATA_DIR_FORMAT` bleibt `1`. Der
ColumnMeta-Wire-Pin sichert genau diese Abwärtskompatibilität ab (Test 1c).

---

## Out of scope / Follow-up

- **Editor-UI-Hookup für Validatoren:** `validator_id` in die
  `editor.rs`-Load-Closure (~:110) verdrahten, sodass beim Editor-Load neben
  `import_required_from` auch die Script-Validatoren pro Spalte registriert und
  beim Save (`editor.rs:134`) ausgewertet werden. Eigenes Folge-Item — Q0014
  beweist nur den `ValidationSystem::run`-Pfad system-level.
- **Page-Level-DataSource-Routing für Skript-Filter:** Umschaltung
  `RemoteSource → LocalSource` in `EntityListPage`, damit der Skript-Filter im
  echten Tabellen-UI greift. Q0014 testet `LocalSource` direkt.
- **Serverseitiges Filtern / allgemeines Filter-Pushdown:** Der Server ignoriert
  `filter`-Args weiterhin (`schema.rs::QueryRoot::entities`). Kein
  Server-Side-Prädikat-Eval in Q0014.
- **Strukturierte Validator-Error-Payloads** (JSON-Rückgabe via
  `script_value_to_json` statt bool): optional, nicht in dieser Cut-Line.

---

## Dateien-Touch-Übersicht

**shared:**
- `shared/src/lib.rs` — `ColumnMeta.validator_id`
- `shared/src/settings.rs` — `FieldTypeDefaults.validator_id` + `allowed_validator_ids`
- `shared/tests/column_meta_wire_format.rs` — **neu**

**server:**
- `server/src/schema.rs` — `SimpleObject`-`ColumnMeta.validator_id`
- `server/src/data.rs` — `convert_column`
- `server/src/reference.rs` — Konstruktor-Call-Site (`validator_id: None`)

**client:**
- `client/src/graphql/queries.rs` — `COLUMNS_QUERY` + `RawColumnMeta` + `fetch_columns`
- `client/src/components/registries/resolve.rs` — `"validator"`-Arm
- `client/src/validation/mod.rs` (ggf. `validation/script_task.rs` neu) — Script-Validator-`TaskFn` + System-Test
- `client/src/components/table/filters/mod.rs` — `script:`-Früh-Return in `resolve_filter_id`
- `client/src/components/table/data_source.rs` — `ColumnLookup.filter_id`, `passes()`-Skript-Zweig, End-to-End-Test

**Fixtures (existieren bereits, nur als Testeingang genutzt):**
- `examples/d2v/scripts/d2v_balance_validator.{rhai,manifest.json}`
- `examples/d2v/scripts/d2v_stack_filter.{rhai,manifest.json}`
- `examples/d2v/entities/datev_entry/columns.json` (`stackId.filterId` ist bereits gesetzt)
