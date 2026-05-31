# Q0018 — Q0014-Härtungen: script-Filter INT-Coercion + global_search-Guard + Per-Row-Aggregate-Op-Budget

- **Queue item:** `docs/queue/Q0018-q0014-haertungen-scriptfilter-numerik-globalsearch-budget.md`
- **Status bei Spec-Erstellung:** `new`
- **Typ:** feature (hardening bundle), priority `low`
- **Scope:** **ausschließlich** `client/src/components/table/data_source.rs` plus zugehörige Tests in `client/tests/`.
- **Referenzen:** `docs/reviews/Q0014-review.md` (non-blocking #1, #2), `docs/reviews/Q0014-security-review.md` (advisory #1).

Drei kleine, **non-blocking** Härtungen aus den Q0014-Reviews — keine Bugs,
reine Robustheit/Resource-Hygiene. Bewusst **gebündelt**, weil alle drei
dieselbe Datei berühren (`data_source.rs`); ein Item = eine Execution =
kein Parallel-Konflikt auf der Datei.

## Ist-Stand (verifiziert gegen den Code)

### Relevante Stellen in `data_source.rs`
- `LocalSource::passes` (Z. 248–297): Per-Predicate-Schleife über
  `filter.predicates` mit `script:`-Branch (Z. 260–271), gefolgt von der
  `global_search`-Schleife (Z. 282–295).
- `script_predicate` (Z. 372–408): wertet ein `script:`-Filter-Predicate für
  **eine** Zeile aus; injiziert den Filterwert als `selectedStackId`.

### H1 — Wie der FLOAT entsteht (Numerik-Pfad)
- `FilterPredicate::NumberEquals { value: f64 }` (`shared/src/lib.rs` Z. 375–377)
  — der Filterwert ist ein `f64`.
- In `script_predicate` (Z. 384–388):
  ```rust
  let sel_value = selected
      .and_then(serde_json::Number::from_f64)   // f64 -> JSON-Number (float-typisiert)
      .map(Value::Number)
      .unwrap_or(Value::Null);
  fields.insert("selectedStackId".into(), sel_value);
  ```
- `serde_json::Number::from_f64(2.0)` erzeugt eine **float-typisierte**
  JSON-Number; deren `.as_i64()` liefert `None`.
- Beim Marshalling nach Rhai (`client/src/script/engine/rhai.rs::json_to_dynamic`,
  Z. 255–278):
  ```rust
  serde_json::Value::Number(n) => {
      if let Some(i) = n.as_i64() { Dynamic::from(i) }   // -> Rhai INT
      else { Dynamic::from(n.as_f64().unwrap_or(0.0)) }  // -> Rhai FLOAT
  }
  ```
  → `selectedStackId` erreicht das Skript als **FLOAT**, während `stackId`
  (aus `serde_json::json!(<i64>)`) als **INT** ankommt.
- Das Skript `examples/d2v/scripts/d2v_stack_filter.rhai` vergleicht
  `row == sel`. Rhai 1.24 coerced INT/FLOAT bei `==`, **heute korrekt** — aber
  spröde gegen künftige Engine-Numerik-Strictness.

### H2 — Wie der global_search-`script:`-Bypass aussieht
- Der `script:`-Guard existiert **nur** in der Per-Predicate-Schleife
  (Z. 260–271). Die `global_search`-Schleife (Z. 282–295) iteriert über
  **alle** Spalten via `columns.iter()` und ruft für jede Spalte unverändert
  ```rust
  let ops = ops_for_named(&col.field_type, col.comparator_id.as_deref(), col.filter_id.as_deref());
  ops.matches_search(&value, needle)
  ```
- Für eine `script:`-Filter-Spalte ist `filter_id = Some("script:…")`.
  `ops_for_named` kennt diese ID nicht → fällt auf die `field_type`-Default-Ops
  zurück (heute benign). Es läuft **kein** Skript in der global_search-Schleife
  (gut), aber die `script:`-Spalte wird **implizit** über ihren Roh-`field_type`
  durchsucht — unsauber und nicht intendiert.

### H3 — Op-Budget heute
- `apply_limits` (`client/src/script/engine/rhai.rs` Z. 77–82) setzt
  `engine.set_max_operations(50_000)` **pro Run**. Optionaler
  `manifest.timeout_ms` als Wall-Clock-Deadline pro Run (`sandbox.rs`).
- `LocalSource::passes` ruft `script_predicate` **einmal pro Zeile** auf
  (`fetch` filtert über `items.iter().filter(...)`, Z. 334–338).
- Es gibt **kein** aggregiertes Cross-Row-Budget: bei `page_size` Zeilen ×
  Skript nahe 50 000 Ops kann der Filter `page_size × 50 000` Ops im
  Browser-Tab des Nutzers verbrennen. **Self-inflicted, client-side, kein
  Trust-Boundary** — der Nutzer bremst nur seinen eigenen Tab.

## Soll-Verhalten

### H1 — script-Filter INT-Coercion (typtreuer Filterwert)
**Ziel:** Ganzzahlige Filterwerte erreichen das Skript als Rhai-**INT**, damit
der Vergleich `row == sel` nicht von Rhais INT/FLOAT-Coercion abhängt.

**Fix (in `script_predicate`):** Ersetze die `Number::from_f64`-Konstruktion
durch eine Normalisierung „ganze Floats → Integer-typisierte JSON-Number":

```rust
let sel_value = match selected {
    Some(v) if v.is_finite() && v.fract() == 0.0
        && v >= i64::MIN as f64 && v <= i64::MAX as f64 =>
    {
        // Ganzer Wert -> integer-typisierte JSON-Number -> json_to_dynamic
        // marshalt nach Rhai-INT (deckt sich mit der INT-typisierten Row-stackId).
        Value::Number((v as i64).into())
    }
    Some(v) => serde_json::Number::from_f64(v)
        .map(Value::Number)
        .unwrap_or(Value::Null), // echte Nachkommastelle bleibt FLOAT
    None => Value::Null,
};
```

- Begründung: `Value::Number(i64_into)` ist int-typisiert → `as_i64()` greift
  in `json_to_dynamic` → Rhai-INT. Identisch zur Behandlung der Row-Werte.
- Range-Guard (`>= i64::MIN .. <= i64::MAX`) verhindert UB beim `as i64`-Cast
  für sehr große Floats; out-of-range fällt sauber auf den FLOAT-Pfad zurück.
- `NaN`/`inf` → `is_finite()`-Guard schützt; `from_f64` liefert dort ohnehin
  `None` → `Value::Null` (= „kein Stack ausgewählt", Skript gibt `true`).
- Der `-1`-Sentinel (`filter_for_stack(-1.0)`) bleibt funktional gleich,
  erreicht das Skript jetzt aber als INT `-1` statt FLOAT `-1.0`.
- **Kommentar** an der Stelle erklärt die Symmetrie zur Row-Numerik und den
  Grund (Engine-Coercion-Unabhängigkeit).

### H2 — global_search-`script:`-Guard
**Ziel:** `script:`-Filter-Spalten werden in der global_search-Schleife
**explizit übersprungen** statt implizit über ihren Roh-`field_type` durchsucht.

**Fix (in der `columns.iter().any(...)`-Schleife in `passes`):** Spalten mit
`filter_id` mit `SCRIPT_PREFIX` aus der global-search auslassen:

```rust
let hit = columns.iter().any(|(key, col)| {
    // H2: `script:`-Filter-Spalten sind boolesche Per-Row-Praedikate und
    // haben keine sinnvolle Substring-Suche. Aus der global_search
    // ausklammern (statt implizit ueber den Roh-field_type zu suchen).
    if col
        .filter_id
        .as_deref()
        .is_some_and(|fid| fid.starts_with(SCRIPT_PREFIX))
    {
        return false;
    }
    let value = entity.fields.get(key).cloned().unwrap_or(Value::Null);
    let ops = ops_for_named(/* … unverändert … */);
    ops.matches_search(&value, needle)
});
```

- Semantik: Eine `script:`-Filter-Spalte trägt **nicht** zu einem
  global_search-Treffer bei. Das ist die korrekte Lesart — ein boolesches
  Filter-Skript hat keinen durchsuchbaren Textwert.
- Bewusst **keine** Skript-Ausführung in der global_search (kein Per-Zeichen-
  /Per-Needle-Run); reine Auslassung. Hält H3 (Op-Budget) klein.

### H3 — Per-Row-Filter-Aggregate-Op-Budget (Resource-Hygiene)
**Ziel:** Eine Obergrenze, wie viele `script:`-Filter-Runs ein einzelner
`fetch`-Aufruf über die Datenmenge auslösen darf, damit ein teures Skript ×
große Datenmenge den eigenen Tab nicht unbeobachtet blockiert.

**Gewählter Ansatz: Run-Count-Budget pro `fetch` (deterministisch, plattform-
neutral, WASM-tauglich — keine Wall-Clock nötig).**

- Eine modul-lokale Konstante:
  ```rust
  /// H3 (Q0018): Aggregat-Budget fuer `script:`-Filter-Runs pro fetch.
  /// Jeder Run ist bereits durch set_max_operations(50_000) gedeckelt;
  /// dieses Budget begrenzt die *Anzahl* der Runs ueber die Datenmenge,
  /// damit `n_rows x teures Skript` den eigenen Browser-Tab nicht
  /// unbeobachtet blockiert. Reine Resource-Hygiene, kein Trust-Boundary.
  const MAX_SCRIPT_FILTER_RUNS: usize = 5_000;
  ```
  (Wert: konservativ; 5 000 Runs × 50 000 Ops ist eine grobe, dokumentierte
  Obergrenze. Begründung im Kommentar; bewusst kein Tuning-Knopf.)
- **Verdrahtung:** Das Budget wird **im `fetch`-Filterpass** geführt, weil nur
  dort die Run-Anzahl über alle Zeilen sichtbar ist. `passes` bekommt einen
  `&mut`-Zähler (oder einen `&Cell<usize>`/`&mut usize`) hereingereicht;
  `script_predicate` wird nur aufgerufen, solange das Budget nicht erschöpft
  ist. Konkret in `fetch` (statt `items.iter().filter(...)`):
  ```rust
  let mut script_runs_left = MAX_SCRIPT_FILTER_RUNS;
  let mut budget_exhausted = false;
  let filtered: Vec<Entity> = items
      .iter()
      .filter(|e| Self::passes(
          e, &req.filter, &columns,
          scripts.as_ref(), host.as_ref(),
          &mut script_runs_left, &mut budget_exhausted,
      ))
      .cloned()
      .collect();
  ```
- **Verhalten bei Erschöpfung:** Sobald `script_runs_left == 0`, läuft **kein
  weiteres Filter-Skript** mehr. Die verbleibenden Zeilen werden für die
  betroffene `script:`-Spalte **durchgelassen** (fail-open, konsistent mit der
  bestehenden „kein Skript ⇒ Zeile durchlassen"-Semantik in `passes`, Z. 268).
  `budget_exhausted` wird gesetzt, sodass der Pfad **einmalig** ein
  Audit-Log-/Konsolen-Warning emittieren kann (siehe unten). Built-in-Ops-
  Predicates und global_search bleiben vom Budget unberührt.
- **Sichtbarkeit:** Beim ersten Überschreiten ein einmaliges
  `web_sys::console::warn`-Logging bzw. — falls bereits eine Audit-Queue-
  Hilfsfunktion erreichbar ist — ein Audit-Eintrag. Mindestanforderung: ein
  Kommentar + die `budget_exhausted`-Flag, die ein Warning auslöst. (Pures
  stilles Durchlassen ist erlaubt, aber ein Log ist erwünscht.)
- **Dokumentation:** Modul-Doc-Kommentar in `data_source.rs` ergänzen, der die
  drei Schranken nennt: (a) `set_max_operations` pro Run, (b) optionaler
  `timeout_ms` pro Run, (c) **neu:** `MAX_SCRIPT_FILTER_RUNS` aggregiert pro
  `fetch`.

**Verworfene Alternativen:**
- *Page-size-Cap nur für script-gefilterte Spalten:* würde die
  Pagination-Semantik (`total_count`) verfälschen — die Filterung läuft vor der
  Pagination über die ganze in-memory-Menge. Abgelehnt.
- *Wall-Clock-Aggregat-Deadline:* WASM-Zeitquelle ist optional/degradiert
  (`sandbox.rs`); ein Run-Count ist deterministisch und plattformneutral —
  passt zur bestehenden „operation-limit als Primärschranke"-Linie (Q0009).

## Signatur-Änderung `passes`

`LocalSource::passes` ist privat (nur in `fetch` aufgerufen) — die zusätzlichen
Budget-Parameter sind eine rein interne Erweiterung, keine API-Änderung. Falls
ein `&mut usize`-Paar die Closure-Borrows verkompliziert, ist ein
`Cell<usize>`/`struct ScriptBudget { left: usize, exhausted: bool }` als
übergebene `&mut`-Referenz die saubere Variante. Implementer wählt die
borrow-freundlichste Form; das Soll-Verhalten oben ist verbindlich.

## Tests (`client/tests/`)

Erweiterung der bestehenden `client/tests/local_source_script_filter.rs`
(E2E mit realer Rhai-Engine + realem `lookup_provider`) — derselbe Test-Stil,
keine neue Datei nötig (eine neue Datei ist optional, falls thematisch sauberer).

### H1-Test — INT-vs-FLOAT-Vergleich gepinnt
- Ein Filter-Skript, das **typstreng** vergleicht (kein `==`-Coercion-Verlass),
  z.B. ein Spalten-Filter-Skript:
  ```rhai
  let sel = fields.selectedStackId;
  let row = fields.stackId;
  if sel == () || sel == -1 { true } else { type_of(row) == type_of(sel) && row == sel }
  ```
  (`type_of`-Vergleich erzwingt, dass `sel` als INT ankommt; mit dem alten
  FLOAT-Pfad würde `type_of(sel) == "f64"` ≠ `type_of(row) == "i64"` und der
  Test schlägt fehl → pinnt H1.)
- Zeilen `row("a",1), row("b",2), row("c",3)`; `filter_for_stack(2.0)` ⇒
  Ergebnis `["b"]`. **Vor** dem Fix (FLOAT) wäre das Ergebnis leer.
- Zusätzlich: `filter_for_stack(-1.0)` ⇒ alle 3 Zeilen (Sentinel weiterhin
  funktional, jetzt als INT `-1`).
- Hinweis: Das Skript ist test-lokal als String einzubetten (analog zu
  `client/tests/script_renderer.rs::component_script`), damit das echte
  `examples/d2v/...`-Skript unverändert bleibt.

### H2-Test — global_search überspringt script:-Spalten
- Setup mit **zwei** Spalten: eine normale Text-Spalte (`name`, built-in ops)
  und eine `script:`-Filter-Spalte (`stackId`, `filter_id =
  "script:d2v_stack_filter"`).
- `FilterCriteria { global_search: Some("2"), predicates: vec![] }`.
- Assertion: Ein global_search-Needle, das **nur** auf den `stackId`-Rohwert
  einer Zeile „passen würde", liefert mit dem Fix **keinen** Treffer über die
  `script:`-Spalte (die Spalte ist ausgeklammert). Treffer nur, wenn der Needle
  auf die **Text-Spalte** passt.
- Konkrete Konstruktion: Zeile mit `name="apple", stackId=2` und Zeile mit
  `name="banana", stackId=9`. `global_search="2"` darf **nicht** Zeile „apple"
  allein wegen `stackId==2` matchen (script:-Spalte ausgeklammert) ⇒ Ergebnis
  leer (kein `name` enthält „2"). Vor dem Fix würde „apple" über den
  `stackId`-Rohwert matchen.

### H3-Test — Aggregat-Run-Budget greift
- Konstante test-sichtbar machen oder den Pfad indirekt prüfen. Pragmatisch:
  - Variante A (bevorzugt, ohne API-Aufweichung): Test mit einer Datenmenge
    `> MAX_SCRIPT_FILTER_RUNS` Zeilen und einem `script:`-Filter, der **alle**
    Zeilen ausschließen würde (`selectedStackId` matcht keine Zeile). Erwartung:
    Die Zeilen **jenseits** des Budgets werden **durchgelassen** (fail-open),
    d.h. `total_count > 0` obwohl das Skript alle ausschließen wollte —
    beweist, dass das Budget gegriffen und weitere Runs unterbunden hat.
  - Damit der Test schnell bleibt, darf `MAX_SCRIPT_FILTER_RUNS` für den Test
    über eine `#[cfg(test)]`-Konstante / einen kleinen Default herabgesetzt
    werden **oder** der Test nutzt `cfg`-gated einen kleineren Wert. Implementer
    wählt; bevorzugt eine `const` in der Größe, die den Test in < 1 s hält
    (z.B. Budget so wählen, dass nur einige hundert Runs nötig sind), ohne den
    Produktionswert zu verfälschen. Falls kein test-injizierbarer Weg ohne
    API-Änderung sauber ist: NEEDS-DECISION (siehe unten) — Default ist
    Variante A mit `cfg(test)`-Override der Konstante.
- Mindest-Assertion: Bei Datenmenge ≤ Budget ist das Verhalten **unverändert**
  zu den bestehenden Tests (kein Regress) — das decken die existierenden
  `selected_stack_includes_only_matching_rows` /
  `unselected_stack_passes_all_rows` bereits ab.

## Definition of Done
- [ ] H1: `script_predicate` normalisiert ganze Floats zu int-typisierten
      JSON-Numbers; Kommentar erklärt Symmetrie zur Row-Numerik.
- [ ] H1-Test pinnt einen typstrengen INT-vs-FLOAT-Vergleich (grün; wäre vor
      dem Fix rot).
- [ ] H2: global_search-Schleife klammert `script:`-Filter-Spalten explizit
      aus; Kommentar/Guard vorhanden.
- [ ] H2-Test belegt die Auslassung.
- [ ] H3: Aggregat-Run-Budget (`MAX_SCRIPT_FILTER_RUNS`) im `fetch`-Filterpass;
      fail-open bei Erschöpfung; einmaliges Warning; Modul-Doc dokumentiert alle
      drei Schranken.
- [ ] H3-Test belegt, dass das Budget greift (fail-open jenseits des Budgets).
- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test -p client` (bzw. `--target-dir target-test` falls Dev-Server-Lock)
      grün.
- [ ] Keine Änderung außerhalb `data_source.rs` + `client/tests/`. Insbesondere
      **keine** Änderung an Sandbox/Capability-Modell, `json_to_dynamic`,
      `lookup_provider` oder `set_max_operations`.

## Out of Scope (unverändert aus Q0014/Item)
- Server-seitiges Filtern / generelles Filter-Pushdown.
- Änderungen am Sandbox-/Capability-Modell oder an `set_max_operations` selbst.
- Globale Numerik-Normalisierung außerhalb des `selectedStackId`-Injektionspfads.

## Security-Trigger-Wörter
Die Beschreibung berührt Begriffe, die einen Security-Review **triggern
könnten**: *sandbox*, *capability*, *resource limit / DoS / op-budget*,
*script execution*, *untrusted input (Filterwert ins Skript)*. **Einordnung:**
Alle drei Punkte sind ausdrücklich **kein Trust-Boundary** (Security-Review
Advisory #1: self-inflicted, client-side, bereits durch `set_max_operations`
gedeckelt). `security_review.required` im Queue-Item steht auf `false` und
sollte so bleiben — diese Härtungen *verengen* nur Resource-/Robustheits-
Verhalten, sie öffnen keine neue Angriffsfläche.

## NEEDS-DECISION
Keine blockierende Ambiguität. Eine **nicht-blockierende** Implementer-Wahl,
hier mit gesetztem Default:
- **H3-Budgetwert & Test-Injektion:** Produktionswert `MAX_SCRIPT_FILTER_RUNS
  = 5_000`; für den Test eine `#[cfg(test)]`-gesetzte kleinere Konstante (oder
  Variante-A-Konstruktion). Falls der Implementer einen sauberen
  test-injizierbaren Weg ohne API-Aufweichung bevorzugt, ist das zulässig —
  Default bleibt `cfg(test)`-Override.
