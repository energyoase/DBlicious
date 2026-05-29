# Design: D2V-Example → eigenständiges Projekt mit dblicious-Binary-Abhängigkeit (Q0012)

Date: 2026-05-29
Status: Draft — awaiting user review
Typ: **Packaging-/Distributions-Spec.** Definiert, wie aus dem In-Repo-Example
`examples/d2v/` ein eigenständiges Projekt wird, das ein installiertes
dblicious-Server-**Binary** als Abhängigkeit konsumiert.

> Diese Spec wurde **nicht-interaktiv** als Sub-Agent erstellt. Wo das
> Brainstorming normalerweise rückgefragt hätte, steht eine begründete
> Best-Guess; alle solchen Stellen sind in §9 „Offene Fragen / Annahmen"
> markiert.

## 0. Frage

> Wie wird der D2V-2019-Daten-Port (heute `examples/d2v/`, ein `--data-dir`
> **innerhalb** des dblicious-Repos) zu einem **eigenständigen Projekt in
> eigenem Ordner/Repo**, das dblicious als Abhängigkeit konsumiert — und zwar
> nach dem **bereits entschiedenen** Modell „data-dir + installiertes
> dblicious-Server-Binary" (`dblicious-server --data-dir ./mein-d2v-projekt`),
> **nicht** als Rust-Crate-Dependency (verworfen)?

Die Antwort gliedert sich in fünf Achsen (entsprechend den Design-Fragen aus
Q0012): (1) Binary-Distribution & Versionierung, (2) Ordnerlayout des Standalone-
Projekts, (3) Naht zur geteilten Bookkeeping-Stdlib, (4) Konfig/Secrets, (5)
Migrationspfad ohne Test-Verlust.

## 1. Verifizierter Ist-Stand (gegen Repo, nicht gegen Marker)

| Fakt | Quelle (verifiziert) |
|---|---|
| Server-Binary heißt `dblicious-server`, CLI-Flag `--data-dir <DIR>` ist Pflicht; ohne data-dir Exit-Code 2. | `server/src/main.rs` (`#[command(name = "dblicious-server", … version)]`, `Args.data_dir: PathBuf`) |
| Admin-CLI heißt `dblicious` und ist **DB-only** (User/Gruppen/Designs/Security-Migration), braucht **kein** data-dir. | `cli/src/main.rs` (`#[command(name = "dblicious", … version)]`) |
| Der data-dir-Vertrag ist exakt das, was `example::load(dir)` parst: `config.{toml,json}`, `navigation.*`, `security/*`, `translatables/*`, `entities/<type>/{columns,editor,settings,binding,seed}.*`, `scripts/<id>.rhai` + `<id>.manifest.*`, `sources.toml`. | `server/src/example/loader.rs::load`, `mod.rs` (Layout-Doc) |
| Format-Dispatch (`.json`/`.toml`) ist bewusst erweiterbar — neue Formate via Match-Arm in `read_typed` + Eintrag in `SUPPORTED_EXTS`. | `server/src/example/format.rs` |
| Scripts werden **nur** aus `<data-dir>/scripts/` geladen — es gibt heute **keinen** geteilten/länder-parametrisierten Pfad. | `loader.rs::load_scripts` |
| `sources.toml` kennt `${VAR:-default}`-Env-Expansion; D2V nutzt das für `D2V_LEGACY_URL` und `DBLICIOUS_DATABASE_URL`. | `examples/d2v/sources.toml`, Source-Spec §3.6 |
| **Es existiert KEINE Top-Level-Version für das data-dir-/Loader-Format.** Nur Script-Manifeste tragen `manifestVersion: 1`. | `loader.rs` (kein Versions-Feld in `ConfigFile`); `examples/d2v/scripts/*.manifest.json` |
| Die drei In-Repo-Tests adressieren `examples/d2v` über einen **fest verdrahteten Relativpfad** `CARGO_MANIFEST_DIR/../examples/d2v`. | `server/tests/{loader_d2v,d2v_e2e,d2v_all_17_listable}.rs` |
| Workspace-Version ist `0.1.0` (gemeinsam für alle Crates). | `Cargo.toml` (`[workspace.package] version`) |

Diese Spec **baut keine** neuen Features in dblicious; sie definiert ein
Distributions-/Versions-Protokoll und einen Repo-Schnitt. Die einzige
empfohlene Code-Änderung ist eine **optionale Loader-Format-Version** (§2.2) —
klein, additiv, abwärtskompatibel.

## 2. Achse 1 — Binary-Distribution & Versionierung

### 2.1 Empfehlung: versionierte Release-Binary-Artefakte (mit `cargo install --git` als Fallback)

**Entscheidung:** dblicious wird als **versioniertes Release-Artefakt** pro
Ziel-Plattform ausgeliefert (GitHub-Release: `dblicious-server-vX.Y.Z-<target>`
+ `dblicious-vX.Y.Z-<target>`), erzeugt aus einem **Git-Tag** `vX.Y.Z`. Das
Standalone-Projekt **pinnt** eine dieser Versionen in seiner eigenen
`dblicious.lock`-/README-Angabe.

Begründung:
- Reproduzierbar: ein Tag → genau ein Binary-Satz; das Standalone-Projekt kann
  exakt benennen, gegen welche Version es validiert wurde.
- Kein Rust-Toolchain-Zwang beim Konsumenten (der Buchhalter/Betreiber des
  D2V-Projekts braucht kein `cargo`). Das passt zum verworfenen Crate-Modell:
  die Abhängigkeit ist das **Binary**, nicht die Crate.
- Die WASM-getunten Release-Profile (`opt-level="z"`, `lto`, …) sind ohnehin
  schon im Workspace-`Cargo.toml` — ein Release-Build ist der Normalfall.

**Fallback / Dev-Pfad:** `cargo install --git <repo> --tag vX.Y.Z dblicious-server`
für Entwickler ohne fertiges Artefakt (z.B. ungetaggter Zwischenstand). Liefert
dasselbe Binary aus Quelltext, nur ohne vorgebautes Artefakt.

**Container** (z.B. `ghcr.io/.../dblicious:X.Y.Z`) bleibt eine **spätere,
additive** Option (eigenes Item), kein Teil dieser Spec — das Standalone-D2V-
Projekt mountet seinen data-dir als Volume und ist daher container-tauglich „for
free", sobald jemand ein Image baut.

Kurz-Alternativen (verworfen / verschoben):
- *Nur `cargo install --git`*: zwingt jeden Konsumenten zur Rust-Toolchain →
  widerspricht dem „Binary als Abhängigkeit"-Geist. Daher nur Fallback.
- *Container-first*: zu schwer für den heutigen Single-Repo-Single-Betreiber-
  Stand; verschoben.
- *Crate-Dependency*: vom User verworfen — nicht designt.

### 2.2 data-dir ↔ Binary-Kompatibilität: Loader-Format-Version `dataDirFormat`

**Problem:** Heute gibt es keine Version für das data-dir-Format. Wächst der
Loader (neue Pflichtdateien, geänderte Wire-Form), kann ein neueres Binary ein
älteres data-dir still falsch interpretieren — oder umgekehrt.

**Entscheidung (kleine, additive Code-Änderung in dblicious):** Eine
**optionale** Loader-Format-Version im `config.{toml,json}`:

```toml
[meta]
dataDirFormat = 1          # SemVer-Major des data-dir-Vertrags
minServerVersion = "0.1.0" # optionale Untergrenze, rein informativ/Warnung
```

Loader-Verhalten (Vorschlag, abwärtskompatibel):
- **Fehlt `[meta]` ganz** → wie heute, keine Prüfung (alle bestehenden
  `examples/*` fahren unverändert hoch). Genau das Muster, das `loader.rs`
  schon für `[server]` nutzt (alles optional).
- **`dataDirFormat` gesetzt und > der vom Binary unterstützten Major** → harter
  Boot-Abbruch mit klarer Meldung („data-dir verlangt Format N, dieses Binary
  unterstützt bis M").
- **`dataDirFormat` < unterstützt** → Boot mit Warnung (Forward-Compat im
  selben Major; der Loader ist additiv, daher liest ein neueres Binary ein
  älteres Format weiter).
- `minServerVersion` ist eine reine **Warn-Schwelle** (das Binary kennt seine
  eigene `CARGO_PKG_VERSION`), kein harter Stopp — Binary-Version und data-dir-
  Format sind bewusst entkoppelt (das Format kann über mehrere Binary-Releases
  stabil bleiben).

Die Format-Version trägt **denselben Geist** wie `manifestVersion: 1` an den
Script-Manifesten und wie der bewusst erweiterbare `SUPPORTED_EXTS`/`read_typed`-
Dispatch: additiv, versioniert, kein stiller Bruch.

**Konsequenz für das Standalone-Projekt:** Es schreibt `[meta] dataDirFormat`
in seine `config.toml` und pinnt im README eine getestete dblicious-Version.
Damit ist „dieses Projekt läuft mit dblicious-server ≥ vX.Y.Z, data-dir-Format 1"
maschinenlesbar dokumentiert.

## 3. Achse 2 — Ordnerlayout des Standalone-Projekts

### 3.1 Was wandert aus (Schicht 3+4), was bleibt (Schicht 1+2)

Anwendung der 4-Schichten-Klassifikation aus `2026-05-24-…-gap-analysis.md` §4b
auf die **physischen** Dateien von `examples/d2v/` (nicht auf die
Feature-Konzepte — die sind Q0013):

| Datei/Verzeichnis heute | Schicht | Bleibt / wandert |
|---|---|---|
| `config.toml` | 3 (Installations-Config) | **wandert** ins Projekt (+ neue `[meta]`-Sektion, §2.2) |
| `navigation.json` | 3 | **wandert** |
| `entities/<type>/{columns,editor,settings,binding}.json` (17×) | 3 | **wandert** |
| `security/{users,groups}.json` | 3 | **wandert** |
| `translatables/{languages,entries,values}.json` | 3 (Installations-spezifisch); EN-Politur = 4 | **wandert** |
| `sources.toml` | 3 (welche DB-URLs/Sources) | **wandert** |
| `scripts/d2v_value_type_label.{rhai,manifest.json}` | 4 (lokales Script) **bzw. 2-Kandidat** | **wandert** heute mit; siehe §4 (Stdlib-Naht) |
| `README.md`, `.gitignore` | Projekt-Doku/Hygiene | **wandert** (angepasst) |
| `server/src/example/{loader,format}.rs`, Source-Architektur, Script-Engine | 1 (Framework) | **bleibt in dblicious**, wird Binary |
| ValueType-/Konto-Klassifikation-/Aggregation-Primitive (sofern gebaut) | 1 (Framework) | **bleibt in dblicious** (Q0013-Scope, nicht hier) |
| DE-Buchhaltungs-Stdlib (IBAN, SKR-Ranges, HGB-Gliederung, MwSt-Split) | 2 (Stdlib) | **weder noch** — eigene geteilte Bibliothek, §4 |

**Kernaussage:** Das Standalone-Projekt **ist** im Wesentlichen der heutige
Inhalt von `examples/d2v/` — 1:1 — plus eine `[meta]`-Sektion und eine
angepasste README. Es enthält **keinen Rust-Code**. Die „Framework"-Schicht
(Loader, Sources, Script-Engine) ist vollständig im Binary und wandert nicht.

### 3.2 Vorgeschlagener Baum des Standalone-Projekts

```
mein-d2v-projekt/                 # eigener Ordner / eigenes Git-Repo
├── README.md                     # Setup, gepinnte dblicious-Version, Datenschutz
├── .gitignore                    # *.db, .env  (aus examples/d2v/.gitignore)
├── .env.example                  # Vorlage für D2V_LEGACY_URL / DBLICIOUS_DATABASE_URL
├── dblicious-version             # eine Zeile: "vX.Y.Z" (gepinnte Binary-Version)
├── config.toml                   # [server] + [ui] + NEU [meta] dataDirFormat
├── navigation.json
├── sources.toml
├── security/
│   ├── users.json
│   └── groups.json
├── translatables/
│   ├── languages.json
│   ├── entries.json
│   └── values.json
├── entities/
│   ├── datev_account/{columns,editor,settings,binding}.json
│   ├── … (alle 17 Entity-Typen, unverändert)
│   └── susa_entry/{columns,editor,settings,binding}.json
└── scripts/
    ├── d2v_value_type_label.rhai
    └── d2v_value_type_label.manifest.json
```

Betrieb:
```sh
# Binary einmalig installieren (Release-Artefakt herunterladen oder cargo install --git)
dblicious-server --data-dir ./mein-d2v-projekt
```

`dblicious-version` (Plaintext) ist die **menschen- und CI-lesbare Pin**; die
maschinell durchgesetzte Schwelle ist `config.toml [meta] minServerVersion`/
`dataDirFormat` (§2.2). Bewusst zwei Artefakte: die Datei dokumentiert „womit
getestet", die `[meta]`-Werte erzwingen Mindest-Kompatibilität.

### 3.3 Wo lebt das Standalone-Projekt-Repo

Best-Guess (§9-A1): **eigenes Git-Repo** `d2v-dblicious-projekt`, **nicht** ein
Unterordner im dblicious-Repo. Begründung: der Sinn von Q0012 ist gerade die
Trennung; ein Unterordner würde die Kopplung beibehalten. Der genaue Repo-Name/
-Host ist eine Betreiber-Entscheidung (§9-A1).

## 4. Achse 3 — Naht zur geteilten Bookkeeping-Stdlib (Schicht 2)

Heute lädt der Loader **ausschließlich** `<data-dir>/scripts/` (verifiziert,
`load_scripts`). Eine geteilte, **länder-parametrisierte, zeit-versionierte**
Bookkeeping-Stdlib (gap-analysis §4b „Schicht 2": IBAN-Prüfung, SKR-Ranges,
HGB-Gliederung, MwSt-Split) existiert **nicht** und wird **hier nicht designt**.

**Diese Spec definiert nur die Naht (Seam), nicht den Mechanismus:**

- Das Standalone-Projekt konsumiert **heute** seine Scripts ausschließlich aus
  dem eigenen `scripts/`-Ordner (genau wie `examples/d2v/` heute). Das einzige
  vorhandene Script (`d2v_value_type_label`) wandert als lokales Script (Schicht
  4) mit.
- **Sobald** eine geteilte Stdlib gebaut wird (eigene Folge-Spec, eigenes
  Brainstorm — siehe gap-analysis §8.7 „neue Folge-Spec"), wird der Loader einen
  **zusätzlichen, geteilten Such-Pfad** kennen (z.B. eine mit dem Binary
  ausgelieferte Stdlib + eine Länder-Einstellung in `config.toml [meta] country = "DE"`).
  Das Standalone-Projekt würde diese Stdlib dann **referenzieren statt
  kopieren** — der lokale `scripts/`-Ordner enthielte nur noch die echten
  Schicht-4-Sonderfälle.

**Dependency-Hinweis (kein Bauauftrag):** Q0012 ist daher **kompatibel** mit
einer späteren Stdlib, aber **nicht blockiert** davon. Der Schnitt in §3 hält
die lokale `scripts/`-Schicht bewusst klein, damit ein späterer Stdlib-Loader
nur den geteilten Pfad ergänzt, ohne das Standalone-Projekt umzubauen. Diese
Naht ist die **einzige** bewusst offen gelassene Erweiterungsstelle dieser Spec.

## 5. Achse 4 — Konfig & Secrets im Standalone-Setup

Übernimmt 1:1 das heutige, im README dokumentierte und in `.gitignore`
abgesicherte Modell — der Repo-Schnitt ändert daran **nichts**, nur den Ort:

| Element | Heute (`examples/d2v/`) | Standalone-Projekt |
|---|---|---|
| `D2V_LEGACY_URL` | Env / `.env`, expandiert in `sources.toml` via `${D2V_LEGACY_URL:-…}` | identisch, `.env` im Projekt-Root |
| `DBLICIOUS_DATABASE_URL` | Env / `.env`, Default `sqlite://./dblicious-d2v.db` | identisch |
| Echte Prod-`d2v.db` | **nie eingecheckt**, nur Kopie, nie Original-Pfad | identisch — `.gitignore`: `*.db`, `*.db-shm`, `*.db-wal`, `.env` |
| `.env` | gitignored | identisch |

Ergänzung: eine **`.env.example`** (eingecheckt, ohne echte Werte) als Vorlage —
heute fehlt sie in `examples/d2v/`; im Standalone-Projekt ist sie sinnvoll, weil
ein neuer Betreiber sonst keine Referenz hat, welche Variablen zu setzen sind.

**Harte Datenschutz-Regel (unverändert, prominent ins Standalone-README):** Die
`d2v.db` enthält echte Buchungsdaten. Es wird **ausschließlich mit einer Kopie**
gearbeitet; das Original wird **niemals** eingecheckt, weitergegeben oder als
`D2V_LEGACY_URL`-Ziel gesetzt. Diese Regel ist projekt-tragend, nicht optional.

## 6. Achse 5 — Migrationspfad ohne Test-Verlust

### 6.1 Constraint

Drei In-Repo-Tests hängen an `examples/d2v/` über den Relativpfad
`CARGO_MANIFEST_DIR/../examples/d2v`:

| Test | Was er beweist | Braucht echte d2v.db? |
|---|---|---|
| `server/tests/loader_d2v.rs` | Reiner Datei-Loader: 17 Entities parsen, Bindings/PK-Arity/Read-Only/columnMap-Lückenlosigkeit, Nav-Gruppen, Script lädt, DirectionalEnum. **Kein DB-IO.** | nein |
| `server/tests/d2v_e2e.rs` | 3 Entities gegen **In-Memory-Fixture-SQLite** (DDL im Test selbst), Composite-PK-Get, Read-Only-Reject. | nein (baut Fixture inline) |
| `server/tests/d2v_all_17_listable.rs` | Alle 17 Table-Bindings `list_page` gegen **generierte** In-Memory-Fixture (DDL aus columnMap abgeleitet), `EXPLAIN QUERY PLAN`. | nein (Fixture aus Bindings generiert) |

**Wichtiger Befund:** Keiner der drei Tests braucht die echte `d2v.db`. Sie
prüfen den **Loader-Vertrag** und die **Binding-Korrektheit** — also genau das,
was das Framework (dblicious) garantieren muss, damit *irgendein* D2V-data-dir
funktioniert. Diese Tests sind **Framework-Regressionstests**, kein
Projekt-Inhalt.

### 6.2 Empfehlung: Mini-Fixture-Example im Repo behalten, Tests darauf umlenken

**Entscheidung:** `examples/d2v/` wird **nicht** ersatzlos entfernt. Statt
dessen:

1. **Standalone-Projekt** bekommt die **vollständige** 17-Entity-Konfiguration
   (§3.2) — das ist das echte D2V-Projekt.
2. Im dblicious-Repo bleibt ein **Mini-Fixture-Example** `examples/d2v-fixture/`
   (oder Umbenennung des heutigen `examples/d2v/`), das **genug** Entity-Typen
   enthält, um die drei Test-Invarianten weiter zu beweisen:
   - mindestens je ein Single-PK-, Composite-PK- und Read-Only-Binding,
   - die DirectionalEnum-Spalte (`datev_entry.valueType`),
   - das Provider-Script (`d2v_value_type_label`),
   - die 6 Nav-Gruppen-Annahme **entweder** beibehalten **oder** die Assertion
     auf die reduzierte Fixture anpassen.

   **Best-Guess (§9-A2):** Da `loader_d2v.rs` heute die **vollständige** 17er-
   Menge und exakte Zahlen (`EXPECTED_ENTITY_TYPES`, `len == 6` Nav-Gruppen,
   `table_bound.len() == 17`) asserted, ist die **risikoärmste** Variante:
   **das heutige `examples/d2v/` bleibt als komplette Fixture im Repo** (nur
   ggf. umbenannt zu `examples/d2v-fixture/`), und das Standalone-Projekt ist
   eine **Kopie/Extraktion** davon. So bleiben alle drei Tests **wortgleich
   grün** (nur der `d2v_dir()`-Pfad-Helper ändert sich, falls umbenannt). Das
   „Mini"-Schrumpfen der Fixture ist eine **optionale Folge-Aufgabe**, kein
   Q0012-Pflichtteil — es würde Test-Assertions anfassen und damit Risiko ohne
   Q0012-Mehrwert einführen.

3. Der einzige Code-Touch im dblicious-Repo ist dann der `d2v_dir()`-Helper in
   den drei Tests (ein Pfad-Literal, falls umbenannt) — **keine** Logik-Änderung.

### 6.3 Konsequenz für die Test-Strategie

- **dblicious-Repo** behält Framework-Regressionstests gegen eine
  **eingecheckte, synthetische** Fixture (keine echten Daten — die Tests bauen
  ihre SQLite-Fixtures bereits inline aus DDL/columnMap).
- **Standalone-Projekt** kann (optional, §9-A3) einen eigenen leichten
  Smoke-Test mitbringen — z.B. „`dblicious-server --data-dir . ` bootet gegen
  eine anonymisierte Mini-`d2v.db` und listet alle 17 Typen". Das ist
  Projekt-CI, nicht dblicious-CI, und nutzt das **Binary** (kein Cargo-Test) —
  konsistent zum Distributionsmodell.

## 7. Datenfluss (Standalone-Betrieb)

```
Betreiber                 Standalone-Projekt-Ordner            dblicious-server (Binary, gepinnt)
   |                              |                                      |
   | dblicious-server --data-dir ./mein-d2v-projekt                      |
   |---------------------------------------------------------->          |
   |                              |   example::load(dir)                 |
   |                              |<-------------------------------------|  liest config.toml [meta]
   |                              |   (dataDirFormat-Check §2.2)          |  → Boot-Abbruch bei Major-Mismatch
   |                              |   navigation/entities/security/...    |
   |                              |   sources.toml + ${D2V_LEGACY_URL}    |  Env-Expansion
   |                              |                                      |  ForeignSqliteSource.init() gegen d2v.db-Kopie
   |                              |                                      |  ManagedSqlite gegen DBLICIOUS_DATABASE_URL
   |  GraphQL :8000  <-------------------------------------------------- |  Server läuft
```

Der einzige neue Schritt gegenüber heute ist der `[meta]`-Versions-Check; alles
andere ist der bereits funktionierende `example::load` → `boot_registry`-Pfad
aus `main.rs`.

## 8. Decisions

1. **Distributionsmodell: versionierte Release-Binary-Artefakte pro Plattform**
   (aus Git-Tag `vX.Y.Z`), `cargo install --git --tag` als Dev-Fallback,
   Container später/additiv. Crate-Dependency bleibt verworfen (User-
   Entscheidung). (§2.1)
2. **Kompatibilität data-dir ↔ Binary: neue, optionale `[meta] dataDirFormat`-
   Version** in `config.{toml,json}` — SemVer-Major-Gate beim Boot, abwärts-
   kompatibel (fehlt = keine Prüfung). Plus informatives `minServerVersion`.
   Diese kleine, additive Loader-Erweiterung ist die **einzige** in dblicious
   nötige Code-Änderung. (§2.2)
3. **Repo-Schnitt:** Das Standalone-Projekt ist 1:1 der heutige `examples/d2v/`-
   Inhalt (Schicht 3+4) + `[meta]` + angepasste README/`.env.example`; **kein
   Rust-Code**. Framework (Loader, Sources, Script-Engine = Schicht 1) bleibt im
   Binary. (§3)
4. **Stdlib-Naht nur dokumentiert, nicht gebaut:** Heute lädt der Loader nur den
   projekt-lokalen `scripts/`-Ordner; ein geteilter Schicht-2-Stdlib-Pfad ist
   eine **separate Folge-Spec** (gap-analysis §8.7). Q0012 ist damit kompatibel,
   aber nicht davon blockiert. (§4)
5. **Secrets/Datenschutz unverändert:** `D2V_LEGACY_URL`/`DBLICIOUS_DATABASE_URL`
   via Env/`.env`, `*.db`+`.env` gitignored, echte Prod-DB nur als Kopie, nie
   eingecheckt — als projekt-tragende Regel ins Standalone-README. Neu:
   eingecheckte `.env.example`-Vorlage. (§5)
6. **Migration ohne Test-Verlust:** Das heutige `examples/d2v/` bleibt als
   eingecheckte **Framework-Regressions-Fixture** im dblicious-Repo (ggf.
   umbenannt zu `examples/d2v-fixture/`); das Standalone-Projekt ist eine
   Extraktion/Kopie davon. Die drei Tests bleiben grün; der einzige Touch ist
   der `d2v_dir()`-Pfad-Helper (nur bei Umbenennung). Fixture-Schrumpfen ist
   optionale Folge-Aufgabe, kein Pflichtteil. (§6)
7. **Out of scope (verwiesen, nicht dupliziert):** die Feature-Gap-Liste „welche
   d2v2019-Features livable sind" → **Q0013**; das Rust-Crate-Modell → verworfen.

## 9. Offene Fragen / Annahmen

> Non-interaktiv getroffene Best-Guesses (das Brainstorming hätte hier
> rückgefragt). Jede ist als Annahme markiert und im Implementierungsplan
> bestätigungs-bedürftig.

- **A1 — Repo-Ort/-Name des Standalone-Projekts.** Annahme: **eigenes Git-Repo**
  (Name z.B. `d2v-dblicious-projekt`), nicht Unterordner im dblicious-Repo
  (würde dem Trennungsziel widersprechen). Host/Name = Betreiber-Entscheidung.
- **A2 — Fixture bleibt vollständig (17 Entities) vs. „Mini".** Annahme:
  **vollständig behalten** (ggf. nur umbenannt), weil die drei Tests harte
  Zählungen (17 Entities, 6 Nav-Gruppen) asserten und ein Schrumpfen
  Test-Assertions ändern müsste → Risiko ohne Q0012-Mehrwert. „Mini" ist eine
  optionale spätere Aufräum-Aufgabe.
- **A3 — Standalone-Projekt-eigene CI/Smoke-Tests.** Annahme: **optional**; das
  Binary-Distributionsmodell impliziert binary-getriebene Smoke-Tests (nicht
  Cargo-Tests). Nicht Pflichtteil von Q0012.
- **A4 — `dataDirFormat`-Startwert.** Annahme: **`1`** für das heutige Layout;
  künftige Major-Brüche im Loader-Vertrag erhöhen ihn. Genaue Politik (was zählt
  als Major-Bruch) sollte beim Loader-Owner bestätigt werden.
- **A5 — Wer baut/hostet die Release-Artefakte.** Annahme: GitHub-Releases aus
  Tag-getriggertem CI im dblicious-Repo. Konkrete CI-Pipeline = eigenes,
  separates Item (nicht hier designt).
- **A6 — Zwei Pin-Artefakte (`dblicious-version`-Datei + `[meta]`-Werte).**
  Annahme: bewusst beide — Plaintext-Datei = „womit getestet" (Doku/CI), `[meta]`
  = erzwungene Mindest-Kompatibilität. Falls als redundant empfunden, kann das
  Implementierungs-Brainstorm auf eines reduzieren.

## 10. Referenzen (nicht restated)

- `docs/superpowers/specs/2026-05-24-d2v-script-first-gap-analysis.md` §4b —
  4-Schichten-Klassifikation (Framework / Bookkeeping-Stdlib / Installations-
  Config / lokales Template); §8.7 — Stdlib als künftige Folge-Spec.
- `docs/superpowers/specs/2026-05-19-dblicious-source-architecture-design.md` —
  Source/Binding-Architektur (`sources.toml`, `binding.json`, foreign-sqlite,
  Composite-PK, `${VAR:-default}`-Expansion).
- `examples/d2v/README.md` — heutiger Stand + Datenschutz-Regel.
- `server/src/example/{loader,format,mod}.rs` — data-dir-Vertrag (Code-Wahrheit).
- `server/src/main.rs` — Binary-Invocation (`dblicious-server --data-dir`).
- `server/tests/{loader_d2v,d2v_e2e,d2v_all_17_listable}.rs` — Migrations-
  Constraint (Framework-Regressionstests).
- **Q0013** — Schwester-Item: Feature-Gap „welche d2v2019-Features livable sind"
  (OUT OF SCOPE hier).
- `CLAUDE.md` — data-dir-Modell, „no demo content lives in the server crate".

## 11. Spec-Selbst-Review

- [x] Keine offenen `TBD`/`TODO`-Platzhalter. Offene Entscheidungen sind explizit
  in §9 als Annahmen markiert (vom non-interaktiven Modus erzwungen).
- [x] Interne Konsistenz: §2 (Versionierung) ↔ §3.2 (Pin-Artefakte) ↔ §7
  (Boot-Check) ↔ §8.2 stimmen überein; §6 (Migration) ↔ §9-A2 stimmen überein.
- [x] Scope: eine Spec für den Repo-Schnitt + Distributions-/Versions-Protokoll;
  Feature-Gap (Q0013) und Stdlib-Mechanismus (Folge-Spec) explizit verwiesen,
  nicht dupliziert.
- [x] Ambiguität: einzige nicht-triviale Entscheidung ist die
  `dataDirFormat`-Gate-Semantik (§2.2) — explizit ausbuchstabiert (fehlt = keine
  Prüfung / > = Stopp / < = Warnung).
