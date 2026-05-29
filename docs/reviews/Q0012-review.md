# Q0012 d2v-example zu eigenstaendigem Projekt — Code-Review

Date: 2026-05-30
Reviewer: claude (code-review:code-review Sub-Agent, Adapter-Modus: lokales Diff statt GitHub-PR)
Scope: `git diff 250a875..5f085da` (5 Commits, +390/-1, 6 Dateien): `shared::DATA_DIR_FORMAT`-Konstante, optionale `[meta]`-Sektion im Loader, Boot-Check, 4 RED/GREEN-Tests, Standalone-Skeleton-Doku + Loader-Stdlib-Seam-Marker, ein Eintrag im CLAUDE.md.
Spec: `docs/superpowers/specs/Q0012-d2v-example-zu-eigenstaendigem-projekt-design.md`
Plan: `docs/superpowers/plans/Q0012-d2v-example-zu-eigenstaendigem-projekt.md`
Verdict: **APPROVE (mit zwei nicht-blockierenden Should-Fix)**

Bereich abgegrenzt: das Diff enthaelt nur die Framework-Grundlage fuer das Standalone-Projekt — kein Standalone-Repo, kein release-Workflow-YAML, keine Aenderung an `examples/d2v/config.toml` oder `examples/shop/config.toml`. Diff-Stat (6 Dateien) bestaetigt: keine `examples/**`-Mutation. Die Verifikation aus `ccm-execute` (alle Test-Suiten gruen, fmt/clippy clean) wurde nicht erneut ausgefuehrt — Vertrauen aufs Verifikations-Log im Plan + statische Code-Inspektion.

## Zusammenfassung

Das Diff legt sauber die additive Vertrags-Klausel fuer den data-dir-Loader: eine SemVer-Major-Pin als `pub const DATA_DIR_FORMAT: u32 = 1` im `shared`-Crate, ein optional `[meta]`-Block im `config.toml`, ein expliziter Boot-Check der nur die `declared > supported`-Richtung hart abbricht und alle anderen Faelle (missing, equal, older) waermsten- oder lautlos durchlaesst. Backward-Compat ist real: heutiges `examples/d2v/config.toml` und `examples/shop/config.toml` haben kein `[meta]` und werden nicht modifiziert — Test 1 pinnt diesen Pfad strukturell. Die 4 Tests sind echte RED-vor/GREEN-nach-Tests, decken alle vier Quadranten (missing/match/newer/older) ab, und pruefen im "rejects"-Pfad explizit auf Inhalt der Fehlermeldung (Token `dataDirFormat`, beide Versionsnummern). Der absichtliche Doppel-Read der Config-Datei ist gut kommentiert und seine Robustheits-Begruendung steht direkt am Code. `server/src/example/mod.rs` aendert sich um genau 3 Doku-Zeilen — keine Surface-Erweiterung.

**Zwei nicht-blockierende Should-Fix:**

1. `docs/standalone-projekt-skeleton.md` behauptet "verifiziert: 17 Entity-Typen, 1 Script" und listet nur `d2v_value_type_label.rhai/.manifest.json` im Tree — `examples/d2v/scripts/` enthaelt nach Q0013 aber **drei** Skripte (P1 Balance-Validator, P3 Stack-Filter, plus das Original). Die "17 Entity-Typen"-Zahl ist korrekt verifiziert; die Script-Zahl ist stale.
2. `min_server_version`-Vergleich ist exakt-String (`min_ver != our_ver`) statt SemVer-vergleichend. Ein Binary `0.2.0` und ein data-dir mit `minServerVersion = "0.1.0"` warnt, obwohl das semantisch okay ist (0.2.0 >= 0.1.0). Code-Kommentar erklaert die Intention ("lex-vergleichbar reicht uns hier nicht; wir tracen nur, wir parsen es nicht weiter"), aber Operator-Erwartung (Mindest-Schwelle = `>=`) wird verletzt. Da es **nur** eine Warnung ist, kein Stopp, ist das nicht-blockierend; sollte aber dokumentiert oder gefixt werden, bevor die erste echte Major-Lift dieses Feld scharf macht.

## Befunde

### F1 — Standalone-Skeleton-Doku zaehlt veraltete Script-Anzahl (Should-Fix, Confidence 95)

`docs/standalone-projekt-skeleton.md:53` und Tree-Block ab Zeile 36-44.

Doku-Auszug:

```
└── scripts/
    ├── d2v_value_type_label.rhai
    └── d2v_value_type_label.manifest.json
```

Und weiter:

> Quelle: 1:1 Kopie der Schicht-3+4-Dateien aus `dblicious/examples/d2v/` (verifiziert: 17 Entity-Typen, 1 Script).

Realer Bestand `examples/d2v/scripts/` (verifiziert via `git show 5f085da:examples/d2v/scripts/` und Glob):

- `d2v_balance_validator.rhai` + `.manifest.json` (Q0013 P1)
- `d2v_stack_filter.rhai` + `.manifest.json` (Q0013 P3)
- `d2v_value_type_label.rhai` + `.manifest.json` (Q0009)

= **drei** Skript-Paare, nicht eins. Die 17-Entity-Zaehlung ist korrekt (Glob auf `examples/d2v/entities/*` liefert genau 17 Eintraege).

**Ursache (Hypothese):** Skeleton-Doku wurde gegen den Pre-Q0013-Stand verfasst (P1+P3 sind in Q0013 ergaenzt worden — das ist seit dem Q0013-Review approved/merged). Q0012 wurde danach zwischen Plan und Execute laufengelassen, ohne die Doku auf den neuen Bestand zu refreshen. Symptom auch im Stdlib-Seam-Kapitel (`§7`): "Heute laedt das Standalone-Projekt seine Scripts ausschliesslich aus dem eigenen `scripts/`-Ordner — genau wie heute `examples/d2v/`" — Aussage stimmt strukturell, ist aber durch die Single-Script-Annahme im Tree ueberlagert.

**Auswirkung:** ein Betreiber, der die Skeleton-Doku als "kopierbare Vorlage" benutzt (Doku-Zitat §0: "Dieses Dokument ist die kopierbare Vorlage dafuer"), kopiert nur ein Drittel des heutigen Schicht-3+4-Bestands und verliert P1/P3 stillschweigend. Das ist Datenverlust auf dem Weg ins Standalone-Repo, auch wenn die Migration noch nicht stattgefunden hat.

**Fix:** Tree-Block + Quellen-Zeile aktualisieren, etwa:

```
└── scripts/
    ├── d2v_balance_validator.{rhai,manifest.json}
    ├── d2v_stack_filter.{rhai,manifest.json}
    └── d2v_value_type_label.{rhai,manifest.json}
```

und Quellen-Zeile auf "verifiziert: 17 Entity-Typen, 3 Scripts (Stand 2026-05-30)" mit kurzem Hinweis: "wird mit jedem Q0013-Folgepilot waermer — Doku synchron halten".

### F2 — `min_server_version`-Vergleich ist String-Equality, nicht SemVer-Schwelle (Should-Fix, Confidence 80)

`server/src/example/loader.rs:106-114`

```rust
if let Some(min_ver) = meta.min_server_version.as_deref() {
    let our_ver = env!("CARGO_PKG_VERSION");
    if min_ver != our_ver {
        tracing::warn!(
            "data-dir '{}' deklariert minServerVersion = '{min_ver}', dieses Binary ist {our_ver}. \
             Es findet keine harte Pruefung statt — bitte selbst verifizieren.",
            dir.display()
        );
    }
}
```

Feld-Name impliziert *Minimum*-Schwelle (Operator-Erwartung: "warne, wenn `our_ver < min_ver`"). Der Code vergleicht aber Strikt-Ungleichheit. Beispiel: Binary `0.2.0`, data-dir `minServerVersion = "0.1.0"` ⇒ Warnung obwohl 0.2.0 die Mindestanforderung uebertrifft.

Der Code-Kommentar erklaert die Wahl:

```rust
/// Format: `major.minor.patch` (lex-vergleichbar reicht uns hier nicht;
/// wir tracen nur, wir parsen es nicht weiter — entkoppelt von SemVer).
```

Die Begruendung ("wir parsen es nicht weiter") ist verteidigbar, kollidiert aber mit der Feld-Bezeichnung `minServerVersion`. Ein operator-freundlicheres Wording waere `requiredServerVersion` (= "muss exakt dieser sein") oder die Implementierung sollte ein bestehendes SemVer-Crate ziehen (`semver` ist schmal, ~50KB). Plan §3.1 hat nicht spezifiziert, welches Semantik-Modell hier gilt — das ist eine implizite Design-Entscheidung im Code.

**Auswirkung:** **nicht blockierend, weil reine Warn-Schwelle ohne Stopp.** Aber: sobald ein Standalone-Projekt das Feld setzt, wird der Operator entweder die Doku komplett lesen muessen (unwahrscheinlich) oder verwirrt sein, dass jede Minor-Bump des Binarys einen Warn-Trace produziert.

**Fix-Optionen (in absteigender Praeferenz):**

1. Feld in `requiredServerVersion` (oder `binaryVersionPin`) umbenennen — kommuniziert "Exact-Match-Erwartung" sauber. Loader-Code bleibt wie er ist.
2. Alternativ: `semver`-Crate ziehen, `min_ver.parse::<Version>().ok()` + `our.parse::<Version>().ok()`, dann `if our_v < min_v { warn }`. Bei Parse-Fehler: silently skip (keep "wir parsen nicht weiter"-Fallback).
3. Status quo lassen, aber den Doc-Kommentar von "Mindest-Schwelle" auf "Versions-Pin (Exact-Match-Warnung)" umformulieren — bewahrt API-Stabilitaet, dokumentiert die Semantik klarer.

CLAUDE.md beschreibt das Feld als "Warn-Schwelle" — der Term "Schwelle" impliziert eine Schwellenwert-Semantik (>=), nicht Equality. Doc-Drift zwischen CLAUDE.md und Code ist die eigentliche Ursache; entweder beide auf Schwelle (= Variante 2) oder beide auf Pin (= Variante 1 oder 3) ausrichten.

## Nicht-Befunde — gezielt geprueft, alles in Ordnung

### NB1 — Backward-Compat-Pfad (vollstaendig gepinnt)

Loader-Schritt fuer "missing `[meta]`": `find_file(dir, "config")` → `read_typed_opt::<ConfigFile>(...)` → `cf.meta` → `.unwrap_or_default()` → `meta.data_dir_format.unwrap_or(0)`. Drei Stufen `Option`-Unwrap landen alle bei 0. Anschliessend `declared > supported (1)` = false, `declared > 0 && declared < supported` = false → silently accepted. Verifiziert durch:

- Test 1 (`loader_accepts_data_dir_without_meta_section`) — kein `[meta]`, name="no-meta", erwartet Erfolg + name korrekt durchgereicht.
- Test 4 (`loader_accepts_data_dir_with_older_format_version`) — `dataDirFormat = 0` explizit, Forward-Compat-Pfad.

Beide `examples/*/config.toml` (d2v + shop) sind im Diff nicht angefasst (Diff-Stat: 6 Dateien, beide nicht dabei) — Backward-Compat ist real, nicht nur behauptet.

### NB2 — Error-Meldung-Qualitaet

`server/src/example/loader.rs:88-93`

```
"data-dir '{}' verlangt dataDirFormat = {declared}, dieses Binary unterstuetzt bis {supported}. \
 Aktualisiere das dblicious-Binary (Spec Q0012 §2.2)."
```

- Pfad: ✓ (`dir.display()`)
- Erwartete Version: ✓ (`supported`)
- Deklarierte Version: ✓ (`declared`)
- Action-Hinweis: ✓ ("Aktualisiere das dblicious-Binary")
- Spec-Pointer: ✓ ("Q0012 §2.2")

Anmerkung: die "downgrade your data-dir"-Variante fehlt — aber das ist semantisch korrekt, weil der einzige Pfad in diesen Branch `declared > supported` ist, und ein Operator ein zu neues data-dir nicht "downgraden" kann (Information geht verloren). "Upgrade binary" ist die einzige Action. Test 3 pinnt explizit, dass Token `dataDirFormat`, `declared`-Version und `supported`-Version in der Meldung stehen — gute Anti-Regression.

### NB3 — Absichtliches Doppel-Lesen der Config

`server/src/example/loader.rs:75-83`

```rust
// ---- Loader-Format-Version (Q0012 §2.2) ----
// Re-parse minimal, um `[meta]` getrennt vom `[server]`-Pfad zu lesen.
// (Wir koennten das im ersten Read mitnehmen; das hier ist robust gegen
// spaetere Refactorings der ConfigFile-Struct und entkoppelt den Check
// explizit als eigene Phase.)
let meta = {
    let config_path = find_file(dir, "config");
    read_typed_opt::<ConfigFile>(config_path)?
        .and_then(|cf| cf.meta)
        .unwrap_or_default()
};
```

- Begruendung im Code: ✓ (3-Zeilen-Block, explizit als "absichtliches Re-Read" markiert).
- Spec-Anker: ✓ ("Q0012 §2.2").
- Konsistenz zwischen den beiden Reads: ✓ (gleicher `find_file(dir, "config")`, gleicher `ConfigFile`-Typ — kein Race moeglich, weil reiner File-Read aus dem gleichen Verzeichnis innerhalb einer Funktion).
- Maintenance-Risiko: gering — sollte ein zukuenftiger Refactor `ConfigFile` aufteilen, kompiliert dieser Block weiterhin gegen den `meta`-Pfad. Genau die Robustheit, die der Kommentar verspricht.

Performance: 2x ein File-Read + 2x TOML/JSON-Parse beim Boot — unkritisch, weil einmalig.

### NB4 — `shared::DATA_DIR_FORMAT`-Hygiene

`shared/src/lib.rs:9-21`

- `pub const` im Crate-Root: ✓ (selbe Ebene wie restliche `pub use` / `pub mod`).
- Type-Wahl `u32`: ✓ (SemVer-Major bleibt im rationalen Bereich; kein Overflow-Risiko).
- Naming `SCREAMING_SNAKE_CASE`: ✓ (Rust-Idiom).
- Doc-Comment mit Aenderungs-Policy: ✓ (additiv = kein Bump, breaking = +1, mit konkreten Beispielen).
- Cross-Reference zum Loader: ✓ (Kommentar verweist auf `server/src/example/loader.rs`).
- Default-Annahme-Doku: ✓ (Abwesenheit = "v0" = aktueller Binary akzeptiert).
- Wert-Wahl `1`: ✓ — die Erst-Vergabe nach Backward-Compat-Pfad (`0` = vor-Q0012, `1` = aktueller Vertrag). Konsistent zum Skeleton-Doc-Beispiel (`dataDirFormat = 1`).

Keine Duplikation gefunden: `Grep` auf `DATA_DIR_FORMAT` zeigt nur `shared/src/lib.rs:21` (Definition), `server/src/example/loader.rs:85` (Use), `server/tests/loader_data_dir_format.rs` (3 Test-Verwendungen) — Single Source of Truth ist eingehalten.

### NB5 — `server/src/example/mod.rs`-Aenderung (3 Zeilen)

Diff zeigt 3 hinzugefuegte Zeilen im Modul-Doc-Comment-Block:

```
+//!     [server]                            name, bind (optional)
+//!     [meta]                              dataDirFormat: u32, minServerVersion: String
+//!                                         (optional; Q0012 §2.2 — Boot-Check)
```

Reine Doku-Erweiterung im Modul-Header. Keine Surface-Aenderung, kein neuer `pub`-Item. Genau das was der Prompt-Sender erwartet hat.

### NB6 — CLAUDE.md-Sentence-Faktizitaet

`CLAUDE.md:75`

```
- `config.toml` kennt eine optionale `[meta]`-Sektion mit `dataDirFormat: u32` (Major-Version des data-dir-Vertrags) und `minServerVersion: String` (Warn-Schwelle). Fehlt `[meta]` ganz, gilt "v0" und der aktuelle Binary akzeptiert; ein groesserer `dataDirFormat` als `shared::DATA_DIR_FORMAT` bricht den Boot ab (Spec Q0012 §2.2). Single source of truth ist `shared::DATA_DIR_FORMAT`. Additive Loader-Aenderungen erhoehen die Konstante **nicht**, breaking Aenderungen +1.
```

- Beide Felder korrekt benannt: ✓
- Typen korrekt: ✓ (`u32`, `String`)
- Backward-Compat-Aussage korrekt: ✓ (passt zum Loader-Code).
- Single-Source-of-Truth-Aussage korrekt: ✓.
- Aenderungs-Policy konsistent mit `shared`-Doc-Comment: ✓.
- Aber: Feld `minServerVersion` als "Warn-Schwelle" beschrieben — passt zu F2-Befund (Code macht Equality, nicht Schwelle). Hier ist die CLAUDE.md die "richtigere" Beschreibung; Code-Implementierung divergiert.

### NB7 — Test-Suite-Vollstaendigkeit

`server/tests/loader_data_dir_format.rs` (104 Zeilen, 4 Tests):

1. `loader_accepts_data_dir_without_meta_section` — Backward-Compat (missing `[meta]`).
2. `loader_accepts_data_dir_with_matching_format_version` — Happy-Path.
3. `loader_rejects_data_dir_with_newer_format_version` — RED-Pin (entscheidender Boot-Stopp).
4. `loader_accepts_data_dir_with_older_format_version` — Forward-Compat (`dataDirFormat = 0` explizit).

Vier Quadranten = {missing, equal, newer, older}, alle abgedeckt. Test 3 pinnt die Fehlermeldung an drei konkrete Tokens (`dataDirFormat`, `newer`-Wert, `DATA_DIR_FORMAT`-Wert) — gute Anti-Drift gegen waessrige Refactorings. Test 4 hat einen anti-regressions-orientierten Doc-Comment ("jede zukuenftige Erhoehung der Konstante muss diesen Pfad gruen halten"), was der korrekte Hinweis fuer kuenftige Bump-Operatoren ist.

Mit dem aktuellen `DATA_DIR_FORMAT = 1` werden die Branch-Warn-Pfade (`declared > 0 && declared < supported` ⇒ Forward-Compat-Warn) durch die Tests **nicht** beruehrt — Test 4 trifft den `declared == 0`-Pfad, der wegen `declared > 0`-Guard die Warn-Pruefung ueberspringt. Beim naechsten Bump auf `DATA_DIR_FORMAT = 2` wuerde Test 4 (mit `dataDirFormat = 0`) immernoch greedy laufen (kein Warn, weil `0 > 0`-Guard false). Ein zusaetzlicher Test fuer den "deklariert v1, Binary auf v2"-Pfad fehlt heute — aber das ist akademisch, weil dieser Pfad heute nicht existiert (`DATA_DIR_FORMAT = 1`, kein `0 < declared < 1`-Bereich moeglich). Bei Bump auf `=2` waere ein Test `loader_warns_on_outdated_but_compatible_format_version` sinnvoll. **Nicht heute, weil heute nicht moeglich.**

### NB8 — Q0013-Interaktion (Skripte werden weiterhin geladen)

`server/src/example/loader.rs:243-249` (neu): Doc-Kommentar markiert den `load_scripts`-Einstieg als Stdlib-Naht-Punkt fuer eine zukuenftige Schicht-2-Stdlib (gepaart mit dem `§7 Stdlib-Naht`-Kapitel im Skeleton-Doc). Code-Pfad zu `load_scripts(dir)` ist unveraendert — alle drei Skripte aus `examples/d2v/scripts/` werden weiterhin geladen, der Boot-Check fuer `[meta]` kommt zeitlich VOR `load_scripts` und blockiert nur bei mismatch-newer. Bei mismatch-older (= Forward-Compat) gibt es einen Warn, aber kein Stopp ⇒ `load_scripts` laeuft normal.

Strukturell verifiziert: der `[meta]`-Check ist bei Zeile ~75-114 platziert, das `load_scripts(dir)`-Call kommt erst bei ~200 — der Check ist ein early-out fuer `> supported`, danach gleiche Code-Pfade wie vorher.

Supplementaerer `script_run`-Test (5 ok) und `loader_d2v_schema_check`-Test (1 ok) aus dem ccm-execute-Log bestaetigen empirisch, dass keine Skript-Regression auftritt.

### NB9 — Skeleton-Doc-Strukturqualitaet

`docs/standalone-projekt-skeleton.md` (206 Zeilen, 9 nummerierte Sektionen):

- §0 Zweck (Spec-Cross-Ref Q0012 §3)
- §1 Ordnerbaum (factual issue siehe F1)
- §2 Pflicht-Inhalt config.toml (mit `[meta] dataDirFormat = 1` Beispiel — konsistent zu `shared::DATA_DIR_FORMAT = 1`)
- §3 Setup-Rezept
- §4 Datenschutz-Regel (`*.db` nie in Git, `.env` nie checken)
- §5 Wo lebt das Repo (eigenes Repo vs. Sibling-Folder)
- §6 Release-Pipeline-Spezifikation (Trigger, Matrix, Schritte, Versions-Quelle, Dev-Fallback) — als Spec ohne YAML, wie vom Prompt verlangt
- §7 Stdlib-Naht (cross-ref zu Q0013, gap-analysis)
- §8 Was NICHT in den Standalone-Repo wandert (kein Rust-Code, keine echte `d2v.db`, kein `.env`)

Struktur ist sauber, jede Sektion hat klaren Scope. Keine fehlerhaften GitHub-Actions-Behauptungen (verifiziert: `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `softprops/action-gh-release@v2` sind valide Action-Referenzen aus dem Marketplace; `targets`-Parameter ist gueltig). `cargo install --git ... --tag vX.Y.Z dblicious-server` ist syntaktisch korrekt fuer Cargo. Plaintext-Pin-Datei `dblicious-version` ist ein Skeleton-Design-Choice, nicht loader-enforced — Doku-Vertrag.

Cross-Doc-Konsistenz: §2-Beispiel `dataDirFormat = 1` matcht `shared::DATA_DIR_FORMAT = 1` und `examples`-Konfiguration des CLAUDE.md-Eintrags. ✓

**Kein Strukturproblem.** Einziger faktischer Drift ist die Script-Zahl (F1).

### NB10 — Keine sicherheitsrelevanten Aenderungen

Der Check ist reines Lesen einer Versions-Zahl + numerischer Vergleich. Keine neuen Netz-Calls, keine neuen Datei-Schreib-Pfade, keine neuen Sub-Process-Calls, keine neuen FFI-Calls. `tracing::warn!` ist eine in-Memory-Operation. Threat-Surface unveraendert.

## Empfehlungen (priorisiert)

1. **F1 fixen vor dem Standalone-Repo-Bootstrap** (Stage 2 per Spec): Skeleton-Doc-Tree auf 3 Skripte aktualisieren. Sonst kopiert der Betreiber stillschweigend nur ein Drittel des Bestands.
2. **F2 entscheiden**: entweder Feld umbenennen (`requiredServerVersion`), `semver`-Crate ziehen, oder Doc-Comment + CLAUDE.md auf "Pin"-Semantik harmonisieren. Status quo ist trag-faehig, aber das Drift-Risiko zwischen Feld-Name und Semantik wird groesser je mehr Operatoren das Feld setzen.
3. **Test fuer Forward-Compat-Warn-Pfad** bei naechstem `DATA_DIR_FORMAT`-Bump nachziehen (heute akademisch).

## Verdict-Begruendung

APPROVE: Das Diff macht genau das was die Spec verlangt — additive, ruekwaerts-vertraegliche Vertrags-Klausel, mit Tests, Doc-Cross-Refs, expliziter Aenderungs-Policy und Single-Source-of-Truth. Beide Should-Fix sind nicht-blockierend: F1 betrifft eine Doku-Datei die Stage 2 noch nicht aktiv konsumiert (das Standalone-Repo existiert noch nicht), F2 betrifft einen reinen Warn-Pfad ohne Boot-Stopp. Die framework-Substanz (Konstante, Check, Tests, Backward-Compat) ist solide gebaut.
