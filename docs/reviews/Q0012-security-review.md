# Q0012 — Security-Review (Standalone-Projekt-Skeleton + dataDirFormat-Boot-Check)

- Datum: 2026-05-30
- Reviewer: claude (`security-review`-Skill, Security-Verdikt-Pass — separat vom
  Korrektheits-Review in `docs/reviews/Q0012-review.md`)
- Scope: Q0012-Implementierungs-Diff `250a875..5f085da` (6 Dateien, +390/-1):
  `CLAUDE.md`, `docs/standalone-projekt-skeleton.md`,
  `server/src/example/loader.rs`, `server/src/example/mod.rs`,
  `server/tests/loader_data_dir_format.rs`, `shared/src/lib.rs`.
- Threat-Model: Trusted-Operator-data-dir (CLAUDE.md, §"No demo content")
  bleibt die Annahme. Q0012 fuegt einen Boot-Check und eine Dokumentations-
  Vorlage hinzu, oeffnet aber keinerlei neue Trust-Boundary.

> **VERDIKT: CLEARED** — der Boot-Check ist defensiv (Parser-bound auf `u32`,
> Pflicht-Pfad ist abbruchsicher, keine Panics auf attacker-controlled Input).
> Die Standalone-Skeleton-Doku ist hinsichtlich Secret-Handling konservativ:
> `.env` ist im `.gitignore`-Whitelist drin, die Datenschutz-Regel ist
> projekt-tragend formuliert, echte DB wird explizit ausgeschlossen. Drei
> advisory Punkte (A1-A3) — alle nicht-blockierend.

---

## Fokus 1 — Untrusted-TOML-Parsing-Surface

### 1a. Type-Bounds und Panic-Sicherheit (sauber)

- `data_dir_format: Option<u32>` (`loader.rs:43`). serde+toml deserialisieren
  Negativ-Zahlen (`dataDirFormat = -1`) als TOML-`Integer` und werfen einen
  Range-Error beim Cast in `u32` → `anyhow`-Bubble-Up via `read_typed_opt`,
  **kein Panic**. Werte `> u32::MAX` (z.B. `dataDirFormat = 99999999999`)
  desgleichen.
- `unwrap_or(0)` (`loader.rs:86`) ist `Option`-Unwrap, kein Result-Unwrap —
  keine Panic-Surface.
- Der `> supported`-Vergleich (`loader.rs:88`) ist arithmetisch trivial und
  ueberlaeuft nicht (zwei `u32`-Vergleich, kein Subtraktions-Pfad).
- `min_server_version: Option<String>` (`loader.rs:48`) wird per `!=`
  verglichen, nie geparst — kein Risiko durch malformed SemVer. Die
  String-Laenge ist nur durch TOML-Reader-Kapazitaet begrenzt; ein operator-
  controlled `config.toml` ist per Trust-Model nicht-feindlich, ein
  Multi-MB-String wuerde zwar viel Speicher allozieren, ist aber kein
  Q0012-Regress (`navigation.json` / Translatables sind heute schon
  unbegrenzt). Kein neues DoS-Vector.

### 1b. `deny_unknown_fields` — keine Aenderung des Status

`ConfigFile`, `ConfigServer`, `ConfigMeta` haben **kein**
`deny_unknown_fields`. Das ist konsistent mit dem Pre-Q0012-Stand
(`examples/d2v/config.toml` hat `[locale]` und `[ui]`, die der bestehende
Loader stillschweigend ignoriert hat — und das ist gewollt, sonst waere die
neue `[meta]`-Sektion ein Breaking Change). Kein neuer Vector.

### 1c. Doppel-Read / TOCTOU

Der Loader liest `config.{toml,json}` **zweimal** (`loader.rs:63` und
`loader.rs:81`) — die `[server]`-Phase und die `[meta]`-Phase. Ein
Atomic-Swap des Files zwischen den beiden Reads ist theoretisch moeglich:

- Phase 1 sieht `dataDirFormat = 1`, Phase 2 sieht `dataDirFormat = 999`
  (oder umgekehrt: Phase 1 sieht `0`, Phase 2 sieht eine boese `[server]`-
  Config). **Im Trust-Model (Operator-controlled data-dir) ist das ein
  Operator-Fehler, kein Security-Issue.** Ein Angreifer, der das File
  swappen kann, hat schon Schreibrechte im data-dir und damit jede andere
  Eskalation auch. Boot passiert einmal pro Prozessstart, die Race-Surface
  ist gleich Null in der Praxis.
- Die zweite Phase parsed nicht erneut den `[server]`-Block, sondern wirft
  ihn weg (`.and_then(|cf| cf.meta)`). Ein boes-getauschter `[server]`-Block
  beim zweiten Read hat **null** Wirkung — die `config.name` ist schon
  belegt. Saubere Trennung.
- Der zweite Read kostet einen redundanten File-IO + TOML-Parse. Performance-
  Hygiene, keine Security-Bedeutung. **A1 (advisory):** wenn der Loader spaeter
  hot-path-relevant wird, kann der Doppel-Read zusammengefuehrt werden — die
  Begruendung im Kommentar (Refactoring-Robustheit) ist akzeptabel.

## Fokus 2 — Error-Message-Disclosure auf Mismatch

Die Fehlermeldung ist (`loader.rs:89-93`):

```
data-dir '{dir.display()}' verlangt dataDirFormat = {declared},
dieses Binary unterstuetzt bis {supported}. Aktualisiere das dblicious-Binary
(Spec Q0012 §2.2).
```

- `{dir.display()}` ist der vom Operator via `--data-dir` uebergebene Pfad —
  Operator-controlled, nicht Angreifer-controlled. Selbst in shared
  Error-Reportern (Sentry o.ae.) ist das die geringst-sensitive
  Information; absoluter Pfad ist ueblich.
- `{declared}` ist die geparste `u32` aus `config.toml` — null
  Injection-Surface (`u32::Display` ist `0`-`9`-only).
- `{supported}` ist eine Compile-Time-Konstante.
- Die `tracing::warn!`-Pfade (`loader.rs:96-99`, `:104-108`) sind analog:
  `{declared}`, `{min_ver}`, `{our_ver}` sind alle entweder geparste Numerik,
  vom Operator gesetzte Strings oder Compile-Time-Werte. **`{min_ver}` ist
  der einzige operator-controlled String, der ungetrimmt in den Logs
  landet.** Ein Operator kann das selbst, ein Angreifer braucht
  Schreibzugriff auf `config.toml` — wieder im Trust-Model.

**Kein Log-Injection-Konzern**, weil `tracing` strukturierte Felder
nutzt (Format-String ist statisch, Werte kommen als Argumente — keine
Newline-/ANSI-Eskalation in `tracing`'s Default-Subscribers, die das auf
Aggregator-Seite (z.B. Loki) gerendert wuerde).

Kein Sekret-Leck: weder die echte DB-URL noch `D2V_LEGACY_URL` noch ein
Auth-Token findet seinen Weg in diese Fehler/Warnungen — sie werden vom
Loader gar nicht gelesen.

## Fokus 3 — Backward-Compat-Enforcement

### 3a. Missing-`[meta]` = v0 = akzeptiert

`loader.rs:80-85`: `read_typed_opt::<ConfigFile>(...).?and_then(|cf| cf.meta).unwrap_or_default()`.
`ConfigMeta::default()` (durch `#[derive(Default)]`) liefert
`data_dir_format: None`, `min_server_version: None`. Der nachfolgende
`meta.data_dir_format.unwrap_or(0)` macht aus dem `None` exakt `0`, was die
Pre-Q0012-Aera markiert. Damit faellt das gesamte `> supported`-Gate weg
(`0 > 1` ist false) und der Boot laeuft durch. Sauber.

**Strukturell verifiziert:** `examples/d2v/config.toml` (`[locale] [ui]`,
kein `[server]`, kein `[meta]`) und `examples/shop/config.toml` (`[server]`,
kein `[meta]`) erreichen beide den `Option<ConfigMeta>::None`-Pfad → `default`
→ `data_dir_format = None`. Test `loader_accepts_data_dir_without_meta_section`
deckt das empirisch ab.

### 3b. Partial-`[meta]` (nur `minServerVersion`, kein `dataDirFormat`)

Pfad: `cf.meta = Some(ConfigMeta { data_dir_format: None, min_server_version: Some("0.1.0") })`.
`unwrap_or(0)` → `declared = 0` → akzeptiert. Die `min_ver`-Warnung
greift, der Boot laeuft. Korrektes Verhalten. **Kein versehentlicher
"required-Pfad" durch partial-Sektion.**

### 3c. `dataDirFormat = 0` explizit

Test `loader_accepts_data_dir_with_older_format_version` deckt das ab.
Korrekt.

## Fokus 4 — `docs/standalone-projekt-skeleton.md` (Security-Korrektheit)

### 4a. Secret-Handling (sauber)

- `.gitignore` (`§1`) listet `*.db *.db-shm *.db-wal .env` — vollstaendige
  SQLite-Dateifamilie + Env-File. Korrekt.
- `.env.example` als Template (`§1`, `§3-3`, `§8`) — Standard-Pattern, kein
  Sekret im Tree.
- `§4 Datenschutz-Regel` explizit projekt-tragend, **nicht optional**:
  "ausschliesslich mit einer Kopie" der `d2v.db`, "Niemals" das Original
  einbinden. Die Regel ist im selben Dokument an mehreren Stellen wiederholt
  (`§3-2`, `§4`, `§8`), das ist robust gegen Skim-Reading.
- `D2V_LEGACY_URL` und `DBLICIOUS_DATABASE_URL` als Env-Variablen (`§3-3`) —
  keine Hartcodierung in Config. Korrekt.

### 4b. Release-Pipeline-Spezifikation (`§6`)

Das Doc beschreibt eine *Spezifikation*, kein YAML. Pruefung der genannten
Patterns:

- `on: push: tags: 'v*.*.*'` (`§6.1`) — Standard-Pattern, keine
  PR-Trigger-Eskalation (kein `pull_request_target` o.ae., das wuerde
  Secrets an forks leaken).
- `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`,
  `softprops/action-gh-release@v2` (`§6.3`) — alles populaere, gepflegte
  Actions. **A2 (advisory):** das Doc verwendet Major-Version-Pinning
  (`@v4`, `@v2`), nicht SHA-Pinning. Fuer einen Release-Workflow, der
  signierte Artefakte hochlaedt, ist das ein bekanntes Supply-Chain-Risiko
  (siehe `tj-actions/changed-files` 2025-03). **Nicht-blockierend** fuer
  Q0012, weil (a) das Doc nur eine Spec ist (kein ausgeliefertes YAML), (b)
  Major-Pinning Stand der Praxis bei den meisten OSS-Repos ist, (c) ein
  Folge-Executor das YAML schreibt und dort ein SHA-Pin nachgereicht werden
  kann. Wert: erwaehnen, nicht erzwingen.
- `cargo build --release ... -p server -p cli` (`§6.3.3`) — built nur die
  beiden veroeffentlichbaren Binaries, nicht den ganzen Workspace. Sauber.
- Upload als Release-Asset via `softprops/action-gh-release@v2` — kein
  Pfad, der Build-Time-Secrets in Artefakte leakt (das waere z.B. ein
  versehentlich gebackener `.env`-Dump). Solange der Operator das Doc
  buchstabengetreu befolgt, gibt es nichts, was sensitiven Inhalt
  durchreicht.
- Kein `GITHUB_TOKEN`-Echo, kein `secrets.X`-`echo`-Pattern, kein expliziter
  Secret-Dump in das Repo. Korrekt.

### 4c. `cargo install --git` Dev-Fallback (`§3-1`, `§6.5`)

```sh
cargo install --git https://github.com/<org>/dblicious --tag vX.Y.Z dblicious-server
```

`--tag` ist als Revision-Pin **akzeptabel** — Cargo verifiziert nicht
kryptographisch gegen Tag-Drift (ein Repo-Owner kann den Tag re-pointen),
aber gegenueber der Alternative `--git` ohne Tag/Branch ist es das
deutlich saerkere Pattern. **A3 (advisory):** fuer hoechste
Reproducibility-Anforderungen waere `--rev <sha>` besser, aber Tag-Pin ist
in der Rust-Praxis Standard und konsistent mit dem `dblicious-version`-
Plaintext-Pin im Standalone-Projekt. Kein Security-Defekt.

Keine `cargo install --git` ohne Pin (z.B. `--branch main` als Fallback) —
das waere ein klares Anti-Pattern und ist im Doc nicht enthalten.

### 4d. Was fehlt? — keine sicherheitskritische Luecke

Erwogene Pruefpunkte, die das Doc *nicht* abdeckt, aber auch nicht muss:

- **Backup-Politik fuer die Kopie der `d2v.db`** — operativ, kein
  Crypto-Issue. Out of scope.
- **`umask` / Filesystem-Permissions fuer `.env`** — Standard-OS-Verhalten,
  kein Q0012-Skelett-Mandat.
- **Verifikation der GitHub-Release-Asset-Signaturen** — heute baut der
  Workflow `softprops/action-gh-release` keine Sigstore-/cosign-Signatur in
  die Spec ein. Das ist konsistent damit, dass dblicious heute noch nicht
  signiert released wird. Sobald das im Haupt-Repo eingefuehrt wird, sollte
  das Skelett-Doc nachziehen — aber das ist Folge-Arbeit, kein
  Q0012-Blocker.

## Fokus 5 — `minServerVersion` semantischer Drift (F2 im Review)

Code: `min_ver != our_ver` (exact-string), nicht `>=`-SemVer. Folgen fuer
Security:

- **Worst Case heute:** das Doc empfiehlt `minServerVersion = "0.1.0"`
  (`docs/standalone-projekt-skeleton.md §2`). Wenn das Binary `0.2.0`
  released wird, feuert die Warnung **immer**, obwohl `0.2.0 >= 0.1.0`
  semantisch passt. Das ist Alert-Fatigue, kein Security-Gate-Bypass.
- **Kann ein boeses data-dir die Warnung *unterdruecken*?** Ja —
  `minServerVersion = "<exakt unsere Version>"`. Dann feuert die Warnung
  nie. **Aber:** die Warnung war von Anfang an nur ein operativer
  Vermerk ("bitte selbst verifizieren"), kein Boot-Stopp, kein
  Security-Gate. Im Trust-Model (Operator-controlled data-dir) ist das
  Suppression kein neues Risiko — der Operator haette die Sektion auch
  einfach weglassen koennen. Sauber.
- **Kann ein boeses data-dir eine legitime Warnung *vortaeuschen*?** Ja —
  ein zufaelliger String. Aber das ist eine Operator-Warnung, kein
  Auth-Pfad, kein Audit-Log-Eintrag, der downstream als Beweis benutzt
  wuerde. Kein Security-Konzern.

**Fazit: F2 (warn-only) hat null Security-Impact.** Der Code-Reviewer hat
das korrekt als nicht-blockierend klassifiziert; aus Security-Sicht
unterschreibe ich das.

## Fokus 6 — `shared::DATA_DIR_FORMAT`-Exposure

`shared/src/lib.rs:20`: `pub const DATA_DIR_FORMAT: u32 = 1;`

- `pub const` (nicht `pub static`) — keine `&'static`-Adresse, der Wert ist
  konstant-eingebettet bei jeder Verwendung. **Nicht mutierbar**, weder via
  `unsafe` noch via interner Mutability — der Compiler verbietet das.
- Downstream-Crates (`server`, `client`, `cli`, kuenftig das Standalone-
  Projekt-Tooling) koennen sich auf das Symbol beziehen. Hygiene-Punkt
  (kein Security-Issue): ein externer Konsument koennte den Literal `1`
  statt `shared::DATA_DIR_FORMAT` hartcodieren — siehe CLAUDE.md-Eintrag,
  der genau dagegen warnt ("Single source of truth ist
  `shared::DATA_DIR_FORMAT`"). Akzeptable Governance-Massnahme.
- Keine Information-Disclosure-Implikation. Die Konstante ist Teil des
  oeffentlichen Vertrags.

## Fokus 7 — Sandbox-Pfade

`git diff 250a875..5f085da --name-only -- server/src/script/ shared/src/script/ client/src/script/`
liefert **leere Ausgabe**. Der Diff fasst keinen Sandbox-Pfad an. Die
Q0011-Sentinel-Hardening (`HostErrorPayload`, `try_cast`) bleibt
unberuehrt. Keine Sandbox-Regression-Surface.

## Fokus 8 — Q0013-Interaktion (Stale-Doc-Konsequenz)

`docs/standalone-projekt-skeleton.md §1` listet nur **ein** Script
(`d2v_value_type_label.rhai`), Q0013 hat aber drei Pilot-Scripts
hinzugefuegt (`balance-validator`, `stack-filter`, plus
`d2v_value_type_label`). Das ist F1 im Code-Review als doc-only flagged.

Security-Frame:

- **`balance-validator`** ist ein Integritaets-Check (validiert
  Buchungs-Salden). Wenn ein Operator das Skelett-Doc 1:1 kopiert und das
  Validator-Script *nicht* mitnimmt, **fehlt eine Integritaets-Pruefung**
  in seinem Standalone-Projekt. Das ist eine Funktionalitaets-Regression
  mit Daten-Integritaets-Implikation, **kein klassisches
  Security-Defizit** (kein Auth-Bypass, kein Sandbox-Escape, kein
  Sekret-Leck). Im Trust-Model "Operator-controlled data" ist das ein
  Self-Inflicted-Loss, nicht ein Angriffsvektor.
- **`stack-filter`** ist eine Anzeige-Filter-Logik. Reine UX, kein
  Security-Belang.
- **`d2v_value_type_label`** ist ein Label-Mapper. Reine UX, kein
  Security-Belang.

**Klassifikation:** Funktionalitaets-Regression mit
Daten-Integritaets-Folge (balance-validator). Advisory, **nicht
blockierend fuer Q0012-Archival**, weil:
1. Q0013 ist nach Q0012 im selben dev-Branch gelandet;
2. das Skelett-Doc ist ausdruecklich eine "Handoff-Doku" (§Status), nicht
   eine ausgelieferte Vorlage;
3. der Folge-Update (Skelett-Doc §1 erweitert um die zwei Q0013-Scripts)
   ist trivial und im Code-Review (F1) bereits als Item benannt.

Aber **es ist sinnvoll, das Doc-Update nicht zu vergessen**, weil ein
Operator, der das Skelett buchstabengetreu kopiert, ohne Validator-Script
auskommt → potentiell unentdeckte Buchungs-Drift in seiner
Sicht-Anwendung. Kein Q0012-Security-Defekt, aber ein Q0013-Doc-Followup.

---

## Zusammenfassung

| Fokus | Befund | Schwere |
|---|---|---|
| 1. Untrusted-TOML-Parsing | Sauber — u32-Bounds, kein Panic, kein DoS-Regress | OK |
| 2. Error-Disclosure | Sauber — keine Sekrete, nur Operator-Pfade in Fehlern | OK |
| 3. Backward-Compat | Sauber — Missing/Partial-`[meta]` korrekt akzeptiert | OK |
| 4a. Skeleton Secret-Handling | Sauber — `.gitignore`, `.env.example`, Datenschutz-Regel | OK |
| 4b. Release-Pipeline-Spec | Sauber — keine PR-Trigger-Eskalation, Token-Echo, Asset-Leak | OK (A2) |
| 4c. `cargo install --git --tag` | Sauber — Tag-Pin akzeptabel, kein unbound `--branch main` | OK (A3) |
| 5. `minServerVersion` Drift | Null Security-Impact, nur Alert-Fatigue | OK |
| 6. `pub const DATA_DIR_FORMAT` | Sauber — immutable, kein Disclosure | OK |
| 7. Sandbox-Pfade | Diff fasst sie nicht an | OK |
| 8. Q0013-Interaktion | Daten-Integritaets-Followup, kein Security-Defekt | Advisory |

### Advisory (nicht-blockierend)

- **A1**: Der Doppel-Read von `config.toml` (`loader.rs:63`, `:81`) ist
  Performance-Hygiene. Wenn der Loader hot-path-relevant wird, in einem
  Pass parsen. Keine Security-Konsequenz.
- **A2**: `actions/checkout@v4`, `softprops/action-gh-release@v2` etc. in
  der Release-Pipeline-Spec verwenden Major-Version-Pinning. Fuer
  geringes Supply-Chain-Restrisiko (Stichwort `tj-actions/changed-files`)
  waere SHA-Pinning robuster. Folge-Executor (der das tatsaechliche YAML
  schreibt) kann das nachreichen — die Spec selbst ist nicht falsch.
- **A3**: `cargo install --git ... --tag vX.Y.Z` ist akzeptables Pinning;
  fuer maximale Reproducibility waere `--rev <sha>` strenger. Konsistent
  mit Cargo-Practice, kein Defekt.

### Blocking — keine.

### Empfehlung

**Cleared for archival.** Die zwei Code-Review-Items (F1: Stale-Doc
post-Q0013, F2: `minServerVersion` `!=` vs. `>=`) sind aus
Security-Sicht non-blocking. F1 hat eine **operative
Daten-Integritaets-Folge** (fehlender balance-validator-Pfad fuer
Operator-Copies), die als Q0013-Doc-Followup eingeplant werden sollte —
aber Q0012 selbst ist die korrekte Bezugseinheit fuer die heutige
Archivierung.
