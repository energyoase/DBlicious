# Q0012 — D2V-Example zu eigenstaendigem Projekt — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Liefere die zwei *Framework*-Bausteine, die der Spec-Schnitt aus Q0012 fuer ein eigenstaendiges, auf das dblicious-Binary aufsetzendes D2V-Projekt verlangt: (a) eine additive, abwaertskompatible Loader-Format-Version `[meta] dataDirFormat` in `config.{toml,json}` mit Boot-Check im Server, und (b) eine versionierte, auf `vX.Y.Z`-Tags aufsetzende Release-Pipeline plus dokumentiertes Skeleton/Migrations-Rezept fuer das Standalone-Projekt. Die *physische* Erstellung des Standalone-Repos ist explizit nicht Teil dieses Plans (Stage-1-Schnitt der Spec).

**Architecture:** Zwei strikt entkoppelte Aenderungsachsen.

1. **Format-Version (Code, additiv).** Eine neue Konstante `DATA_DIR_FORMAT: u32 = 1` in `shared` ist die einzige Quelle der Wahrheit fuer Server/Client/CLI. Der Loader (`server/src/example/loader.rs`) liest eine neue, vollstaendig optionale `[meta]`-Sektion (Felder `dataDirFormat: u32`, `minServerVersion: String`) aus `config.{toml,json}`, vergleicht gegen `DATA_DIR_FORMAT` und reagiert nach Spec §2.2: fehlt = akzeptieren (v0-Backcompat), gleich/kleiner = laden (kleiner = warnen), groesser = harter Abbruch mit klarer Meldung. `minServerVersion` ist reine Warn-Schwelle gegen `env!("CARGO_PKG_VERSION")`. Keine Verhaltensaenderung fuer bestehende `examples/shop/` oder `examples/d2v/` (beide haben keine `[meta]`-Sektion).
2. **Release-Pipeline & Skeleton (Doku/CI).** Ein einziger GitHub-Actions-Workflow `.github/workflows/release.yml` triggert auf Git-Tags `v*.*.*`, baut auf einer Matrix (`linux-x86_64-unknown-gnu`, `windows-x86_64-msvc`; macOS optional) `server` + `cli` im Release-Profil, archiviert die zwei Binaries pro Target und haengt sie an einen GitHub-Release an. Der Workflow wird in diesem Plan *spezifiziert*, **nicht** als YAML eingecheckt — siehe Stage-1-Schnitt. Das Standalone-Projekt-Skeleton wird in `docs/standalone-projekt-skeleton.md` als kompletter, kopierbarer Ordnerbaum dokumentiert, mit One-liner-Setup-Rezept und expliziter `dblicious-server`-Versions-Pin-Stelle.

**Tech Stack:** Rust (workspace `shared`/`server`/`cli`), `serde`/`serde_json`/`toml`, `cargo test`, `cargo clippy`, `cargo fmt`, GitHub Actions (Workflow nur spezifiziert), Markdown-Doku.

---

## Scope-Anchor (Tasks ↔ Spec-Decisions)

| Task | Spec-Decision (§) | Was es liefert |
|---|---|---|
| 1 | §2.2 (Format-Version), §8.2 | Single-source-of-truth-Konstante `DATA_DIR_FORMAT` in `shared`. |
| 2 | §2.2, §8.2 | RED-Tests fuer Loader-Format-Check (4 Faelle: missing, match, mismatch-newer, mismatch-older). |
| 3 | §2.2, §8.2 | GREEN-Implementierung: `[meta]`-Sektion + Boot-Check im Loader. |
| 4 | §6 (Migration ohne Test-Verlust) | Regressions-Verifikation: `examples/d2v/` + `examples/shop/` laden weiter, alle drei D2V-Tests gruen. |
| 5 | §2.2 (Doku), CLAUDE.md "Conventions worth knowing" | CLAUDE.md + `examples/`-README dokumentieren `[meta] dataDirFormat`. |
| 6 | §2.1 (Release-Artefakte), §3.3 (Repo-Ort), §9-A5 | CI-Workflow-Spezifikation + Skeleton-Doku (eigenes Dokument, kein YAML). |
| 7 | §4 (Stdlib-Naht), §10 (Q0013-Verweis) | Naht-Hinweis am Code (Loader-Doc-Comment), keine Mechanik. |
| 8 | CLAUDE.md "Tests" | Verifikations-Pass (fmt + clippy + workspace-tests). |

---

## File Structure

**Geaenderte Produktions-Dateien:**

- `shared/src/lib.rs` — neue Konstante `pub const DATA_DIR_FORMAT: u32 = 1;` (eine Zeile + Doc-Comment).
- `server/src/example/loader.rs` — `ConfigFile` um `meta`-Feld erweitern, neue Struct `ConfigMeta`, Boot-Check-Block direkt nach dem `[server]`-Parsing in `load`.
- `server/src/example/mod.rs` — Layout-Doc-Block in `mod.rs:15-30` um `[meta]`-Hinweis ergaenzen (eine Zeile).

**Geaenderte/neue Test-Dateien:**

- `server/tests/loader_data_dir_format.rs` — **neu**, vier Test-Cases (missing-`[meta]`, matching, newer-major, older-major).

**Geaenderte Doku-Dateien:**

- `CLAUDE.md` — Abschnitt "Conventions worth knowing" um einen Bullet zu `[meta] dataDirFormat` ergaenzen.
- `docs/standalone-projekt-skeleton.md` — **neu**, vollstaendiger Ordnerbaum + One-liner-Setup + CI-Workflow-Spezifikation (kein YAML).

**Nicht beruehrt** (bewusst, siehe Stage-1-Schnitt): `examples/d2v/`, `examples/shop/`, `.github/workflows/`, `Cargo.toml`, `cli/`, `client/`. Das neue Standalone-Repo wird in diesem Plan **nicht** angelegt; die Skeleton-Doku ist der Handoff dafuer.

---

## Task 1: Format-Version-Konstante in `shared`

> Single source of truth fuer die data-dir-Format-Major. Server + zukuenftige CLI/Client-Konsumenten beziehen die Zahl ueber `shared::DATA_DIR_FORMAT`, nicht ueber Magic-Literale. Vorab vor Task 2, weil die Tests die Konstante referenzieren.

**Files:**
- Modify: `shared/src/lib.rs` (oben, nach `use serde::...`, vor `pub mod auth;`)

- [ ] **Step 1: Konstante mit Doc-Comment hinzufuegen**

In `shared/src/lib.rs`, direkt nach Zeile 7 (`use serde::{Deserialize, Serialize};`) und vor `pub mod auth;` einfuegen:

```rust
/// SemVer-**Major** des data-dir-Loader-Vertrags (`examples/<name>/`-Layout).
///
/// Wird vom Server beim Booten gegen `config.toml [meta] dataDirFormat`
/// verglichen (siehe `server/src/example/loader.rs`). Aenderungs-Politik:
/// - **Additiv** (neues optionales Feld, neue optionale Datei): **kein** Bump.
/// - **Breaking** (Pflichtfeld, geaenderte Semantik, umbenannte Wire-Form):
///   Bump um +1, alte data-dirs muessen migriert werden.
///
/// Default-Annahme bei Abwesenheit von `[meta] dataDirFormat`: "v0", wird
/// vom aktuellen Binary akzeptiert (Spec Q0012 §2.2 — Backward-Compat fuer
/// alle heute existierenden data-dirs).
pub const DATA_DIR_FORMAT: u32 = 1;
```

- [ ] **Step 2: Build verifizieren**

Run: `cargo build -p shared`
Expected: PASS, kein Warning, kein Fehler.

- [ ] **Step 3: Commit**

```bash
git add shared/src/lib.rs
git commit -m "feat(shared): add DATA_DIR_FORMAT constant (Q0012)"
```

---

## Task 2: RED — Loader-Tests fuer `[meta] dataDirFormat`

> Vier Test-Cases nach Spec §2.2: (a) fehlende `[meta]`-Sektion laedt (v0-Backcompat), (b) gleicher Wert laedt, (c) groesserer Wert bricht ab mit klarer Meldung, (d) kleinerer Wert laedt mit Warnung (Warnung wird hier nicht direkt asserted — Warnung = tracing-event, ist im Test nicht stabil pruefbar; getestet wird, dass es **nicht** abbricht). Pattern uebernommen von `server/tests/loader_sources.rs` (kein DB-IO, `tempfile::tempdir`).

**Files:**
- Create: `server/tests/loader_data_dir_format.rs`

- [ ] **Step 1: Test-Datei mit vier Cases anlegen**

```rust
//! Loader-Tests fuer `[meta] dataDirFormat` (Q0012 §2.2).
//!
//! Vier Faelle: missing (v0-Backcompat), match, mismatch-newer (Abbruch),
//! mismatch-older (laedt, kein Abbruch). Kein DB-IO — reines Loader-Schema.

use std::fs;

use shared::DATA_DIR_FORMAT;

fn write_config_toml(dir: &std::path::Path, content: &str) {
    fs::write(dir.join("config.toml"), content).expect("config.toml schreiben");
}

#[test]
fn loader_accepts_data_dir_without_meta_section() {
    // v0-Backcompat: alle heutigen examples/* (shop, d2v) haben keine
    // [meta]-Sektion und muessen unveraendert laden.
    let tmp = tempfile::tempdir().unwrap();
    write_config_toml(
        tmp.path(),
        r#"
[server]
name = "no-meta"
        "#,
    );
    let set = server::example::loader::load(tmp.path())
        .expect("data-dir ohne [meta] muss laden (Backcompat)");
    assert_eq!(set.config.name, "no-meta");
}

#[test]
fn loader_accepts_data_dir_with_matching_format_version() {
    let tmp = tempfile::tempdir().unwrap();
    write_config_toml(
        tmp.path(),
        &format!(
            r#"
[server]
name = "match"

[meta]
dataDirFormat = {DATA_DIR_FORMAT}
        "#
        ),
    );
    let set = server::example::loader::load(tmp.path())
        .expect("matching dataDirFormat muss laden");
    assert_eq!(set.config.name, "match");
}

#[test]
fn loader_rejects_data_dir_with_newer_format_version() {
    let tmp = tempfile::tempdir().unwrap();
    let newer = DATA_DIR_FORMAT + 1;
    write_config_toml(
        tmp.path(),
        &format!(
            r#"
[server]
name = "too-new"

[meta]
dataDirFormat = {newer}
        "#
        ),
    );
    let err = server::example::loader::load(tmp.path())
        .expect_err("neuere dataDirFormat-Major muss harten Abbruch ausloesen");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("dataDirFormat"),
        "Fehlermeldung muss 'dataDirFormat' enthalten, war: {msg}"
    );
    assert!(
        msg.contains(&newer.to_string()),
        "Fehlermeldung muss die data-dir-Version ({newer}) nennen, war: {msg}"
    );
    assert!(
        msg.contains(&DATA_DIR_FORMAT.to_string()),
        "Fehlermeldung muss die Binary-Version ({DATA_DIR_FORMAT}) nennen, war: {msg}"
    );
}

#[test]
fn loader_accepts_data_dir_with_older_format_version() {
    // Forward-Compat innerhalb der Major-Familie ist Definition: ein neueres
    // Binary kann ein aelteres Format weiter lesen (Loader ist additiv).
    // Wir testen explizit dataDirFormat=0, weil das die "vor-Q0012"-Aera
    // markiert; jede zukuenftige Erhoehung der Konstante muss diesen Pfad
    // gruen halten.
    let tmp = tempfile::tempdir().unwrap();
    write_config_toml(
        tmp.path(),
        r#"
[server]
name = "older-version"

[meta]
dataDirFormat = 0
        "#,
    );
    let set = server::example::loader::load(tmp.path())
        .expect("aeltere dataDirFormat-Major muss laden (Forward-Compat)");
    assert_eq!(set.config.name, "older-version");
}
```

- [ ] **Step 2: Tests laufen lassen — alle muessen FAILEN (RED)**

Run: `cargo test -p server --test loader_data_dir_format`
(Falls `server.exe`-Lock unter Windows: `cargo test --target-dir target-test -p server --test loader_data_dir_format`.)
Expected: 

- `loader_accepts_data_dir_without_meta_section` → **PASS** (Loader ignoriert unbekannte Sektion bereits heute — `serde(default)`).
- `loader_accepts_data_dir_with_matching_format_version` → **FAIL** (laedt zwar, aber wir asserten noch nichts Format-Spezifisches — gruen wird ueberraschend; ist OK, denn der Failure-Pin ist Test 3).
- `loader_rejects_data_dir_with_newer_format_version` → **FAIL** mit `expect_err` → Loader liefert heute `Ok(...)`, also bricht `expect_err` ab.
- `loader_accepts_data_dir_with_older_format_version` → **PASS** (selbe Begruendung wie Test 1).

Der entscheidende RED-Pin ist **Test 3**. Wenn er passt = Implementierung ist verifiziert.

- [ ] **Step 3: Kein Commit**

RED-Tests werden in Task 3 zusammen mit der Implementierung committet (atomarer "RED+GREEN")-Schritt — sonst bleibt der Branch in einem unverbindlichen RED-Zustand.

---

## Task 3: GREEN — `[meta]`-Parsing + Boot-Check im Loader

> Der eigentliche Code-Touch. Additiv, abwaertskompatibel. Der Check sitzt direkt nach dem `[server]`-Parsing in `load`, vor jeder weiteren Datei-Aktion — damit ein Mismatch *vor* irgendwelchen IO-Aufwand abbricht.

**Files:**
- Modify: `server/src/example/loader.rs` (`ConfigFile`-Struct erweitern, neuer Struct `ConfigMeta`, neuer Check-Block in `load`)

- [ ] **Step 1: `ConfigMeta`-Struct und `ConfigFile.meta`-Feld hinzufuegen**

In `server/src/example/loader.rs` direkt nach der bestehenden `ConfigServer`-Struct (Zeile 24-30) einfuegen:

```rust
/// Sektion `[meta]` aus `config.{toml,json}` (Q0012 §2.2).
///
/// Vollstaendig optional. Fehlt sie, gilt `dataDirFormat = 0` (vor-Q0012).
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ConfigMeta {
    /// SemVer-Major des data-dir-Vertrags, gegen den dieses Verzeichnis
    /// geschrieben wurde. Wird gegen `shared::DATA_DIR_FORMAT` verglichen.
    #[serde(default)]
    data_dir_format: Option<u32>,
    /// Optionale Mindest-Server-Version — reine Warn-Schwelle, kein Stopp.
    /// Format: `major.minor.patch` (lex-vergleichbar reicht uns hier nicht;
    /// wir tracen nur, wir parsen es nicht weiter — entkoppelt von SemVer).
    #[serde(default)]
    min_server_version: Option<String>,
}
```

Anschliessend in `ConfigFile` (Zeile 18-21) das `meta`-Feld ergaenzen:

```rust
/// Sektion `[server]` und `[meta]` aus `config.{toml,json}`.
#[derive(serde::Deserialize)]
struct ConfigFile {
    #[serde(default)]
    server: Option<ConfigServer>,
    #[serde(default)]
    meta: Option<ConfigMeta>,
}
```

- [ ] **Step 2: Boot-Check-Block in `load` einbauen**

In `server/src/example/loader.rs::load`, **direkt nach** dem `if let Some(cfg_file) = read_typed_opt::<ConfigFile>(config_path)? { ... }`-Block (also nach Zeile 55, vor dem `if config.name == "unnamed"`-Block in Zeile 56), einfuegen:

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
    let declared = meta.data_dir_format.unwrap_or(0);
    let supported = shared::DATA_DIR_FORMAT;
    if declared > supported {
        return Err(anyhow!(
            "data-dir '{}' verlangt dataDirFormat = {declared}, dieses Binary unterstuetzt bis {supported}. \
             Aktualisiere das dblicious-Binary (Spec Q0012 §2.2).",
            dir.display()
        ));
    }
    if declared > 0 && declared < supported {
        tracing::warn!(
            "data-dir '{}' verwendet dataDirFormat = {declared}; dieses Binary unterstuetzt bis {supported} (Forward-Compat — laeuft, sollte aber aktualisiert werden).",
            dir.display()
        );
    }
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

> Hinweis fuer den Executor: der doppelte `find_file`/`read_typed_opt`-Aufruf ist absichtlich — `serde`'s `default` macht `meta` bereits transparent, aber wir trennen die Sektion bewusst als eigene Phase, um die Diagnostik (Error-Message) lokal zu halten und das `[server]`-Parsing nicht umstrukturieren zu muessen. Der zweite Read ist billig (kleines TOML, einmal pro Boot).

- [ ] **Step 3: Tests laufen lassen — alle muessen passen (GREEN)**

Run: `cargo test -p server --test loader_data_dir_format`
(Bei Bedarf: `--target-dir target-test`.)
Expected: alle 4 Tests **PASS**.

Falls Test 3 (`loader_rejects_data_dir_with_newer_format_version`) failed: Fehlermeldung enthielt nicht beide Versions-Nummern oder das Wort `dataDirFormat`. Pruefe den Error-String im Code gegen die Assertions in der Test-Datei.

- [ ] **Step 4: Format + Clippy**

Run: `cargo fmt --check && cargo clippy -p server --all-targets -- -D warnings`
Expected: kein Output, Exit-Code 0.

- [ ] **Step 5: Commit**

```bash
git add server/src/example/loader.rs server/tests/loader_data_dir_format.rs
git commit -m "feat(loader): [meta] dataDirFormat boot check (Q0012 §2.2)"
```

---

## Task 4: Regressions-Verifikation — `examples/d2v/` + `examples/shop/` + 3 D2V-Tests bleiben gruen

> Spec §6: "die drei In-Repo-Tests adressieren `examples/d2v/` ueber `CARGO_MANIFEST_DIR/../examples/d2v` und beweisen den **Loader-Vertrag**". Der `[meta]`-Check darf sie **nicht** brechen, weil weder `examples/d2v/config.toml` noch `examples/shop/config.toml` heute eine `[meta]`-Sektion hat (verifiziert). Wir machen das hier explizit zur Akzeptanzbedingung.

**Files:** keine Aenderung — reiner Verifikations-Task.

- [ ] **Step 1: Die drei D2V-Framework-Regressionstests laufen lassen**

Run:
```
cargo test -p server --test loader_d2v
cargo test -p server --test d2v_e2e
cargo test -p server --test d2v_all_17_listable
```
(Bei Bedarf jeweils `--target-dir target-test` ergaenzen.)
Expected: alle drei Test-Files passen. Insbesondere muss `loader_d2v` weiterhin `EXPECTED_ENTITY_TYPES.len()` (17) treffen und `len == 6` Nav-Gruppen sehen.

- [ ] **Step 2: Manueller Smoke-Test gegen `examples/shop/`**

Run: `cargo run -p server -- --data-dir ./examples/shop`
Expected: Server faehrt hoch, tracing-Output zeigt **kein** `dataDirFormat`-Warning, kein Abbruch. Mit Strg+C beenden.

- [ ] **Step 3: Manueller Smoke-Test gegen `examples/d2v/`**

Run: `cargo run -p server -- --data-dir ./examples/d2v`
Expected: Server faehrt hoch, kein Warning, kein Abbruch. Mit Strg+C beenden.

- [ ] **Step 4: Cross-Check — neuer `[meta]`-Pin via tatsaechlichem data-dir testen**

Optional, aber empfehlenswert: temporaer `dataDirFormat = 1` in `examples/shop/config.toml` ergaenzen, Server starten (muss laden), Wert wieder entfernen, **nicht** committen. Das ist eine reine Eigenkontrolle; falls der Reviewer sie nicht wuenscht, ueberspringen.

- [ ] **Step 5: Kein Commit**

Task ist ein Verifikations-Pass; keine Code-Aenderung.

---

## Task 5: Doku — CLAUDE.md + Loader-Layout-Kommentar

> Spec §2.2: die neue Sektion muss in der "Conventions worth knowing"-Liste auftauchen, damit ein zukuenftiger Agent sie nicht stillschweigend wegrefaktoriert. Plus eine Zeile im Loader-Layout-Doc-Comment (`server/src/example/mod.rs`).

**Files:**
- Modify: `CLAUDE.md` (Abschnitt "Conventions worth knowing", neuer Bullet)
- Modify: `server/src/example/mod.rs` (Layout-Doc-Block, Zeile 15-30)

- [ ] **Step 1: Bullet zu CLAUDE.md hinzufuegen**

In `CLAUDE.md`, im Abschnitt "## Conventions worth knowing", **direkt nach** dem Bullet `- The `SortDirection` enum on the wire is `lowercase` (`"asc"`/`"desc"`); other shared types use `camelCase`.`, ergaenzen:

```markdown
- `config.toml` kennt eine optionale `[meta]`-Sektion mit `dataDirFormat: u32` (Major-Version des data-dir-Vertrags) und `minServerVersion: String` (Warn-Schwelle). Fehlt `[meta]` ganz, gilt "v0" und der aktuelle Binary akzeptiert; ein groesserer `dataDirFormat` als `shared::DATA_DIR_FORMAT` bricht den Boot ab (Spec Q0012 §2.2). Single source of truth ist `shared::DATA_DIR_FORMAT`. Additive Loader-Aenderungen erhoehen die Konstante **nicht**, breaking Aenderungen +1.
```

- [ ] **Step 2: Loader-Layout-Doc-Comment in `mod.rs` ergaenzen**

In `server/src/example/mod.rs`, im Doc-Block "Verzeichnislayout" (Zeile 15ff), **direkt nach** der Zeile `//!   config.{json,toml}                  optional, sonst Defaults`, anhaengen:

```rust
//!     [server]                            name, bind (optional)
//!     [meta]                              dataDirFormat: u32, minServerVersion: String
//!                                         (optional; Q0012 §2.2 — Boot-Check)
```

- [ ] **Step 3: Build + Doku-Sanitycheck**

Run: `cargo build -p server`
Expected: kein Warning (Doc-Comments brechen den Build nicht; aber falls `rustdoc` ein Format-Issue findet, korrigieren).

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md server/src/example/mod.rs
git commit -m "docs: [meta] dataDirFormat in CLAUDE.md + loader layout (Q0012)"
```

---

## Task 6: Standalone-Projekt-Skeleton + Release-Workflow-Spezifikation

> Spec §3.2 + §3.3 + §2.1 + §9-A1/A5. Der physische Standalone-Repo wird **nicht** in diesem Plan angelegt — das ist Stage-2-Arbeit und Betreiber-Entscheidung. Wir liefern hier nur das **maschinen-/menschlich kopierbare** Skeleton + Setup-Rezept + die genaue CI-Workflow-Spezifikation als Doku, damit ein zukuenftiger Executor (oder der Betreiber selbst) das Repo aus dem Stand bauen kann.

**Files:**
- Create: `docs/standalone-projekt-skeleton.md`

- [ ] **Step 1: Skeleton-Doku schreiben**

Schreibe `docs/standalone-projekt-skeleton.md` mit *exakt* folgendem Inhalt:

````markdown
# Standalone-D2V-Projekt — Skeleton & Release-Rezept (Q0012)

Status: Handoff-Doku. Wird durch Q0012 Stage 1 erzeugt; der eigentliche
Standalone-Repo wird in **Stage 2** durch den Betreiber angelegt
(siehe §"Wo lebt das Repo" unten).

## 0. Zweck

Q0012 §3 hat entschieden: der heutige Inhalt von `examples/d2v/` (Schicht 3+4)
wird zu einem **eigenstaendigen Git-Repo**, das ein installiertes
`dblicious-server`-Binary als Abhaengigkeit konsumiert. Dieses Dokument ist die
**kopierbare Vorlage** dafuer — Ordnerbaum, Pflichtdateien, Setup-Rezept,
Release-Pipeline-Spezifikation. Es enthaelt bewusst **keinen Rust-Code** und
**kein** `Cargo.toml`: das Standalone-Projekt ist eine reine Daten-/Konfig-
Sammlung.

## 1. Ordnerbaum des Standalone-Projekts

```
mein-d2v-projekt/                     # eigenes Git-Repo, eigener Ordner
├── README.md                         # Setup-Rezept (siehe §3 unten)
├── .gitignore                        # *.db *.db-shm *.db-wal .env
├── .env.example                      # Vorlage fuer D2V_LEGACY_URL etc.
├── dblicious-version                 # 1-Zeile-Plaintext: "vX.Y.Z"
├── config.toml                       # [server] + [ui] + [meta] (NEU)
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
│   ├── company/{columns,editor,settings,binding}.json
│   ├── datev_account/{columns,editor,settings,binding}.json
│   ├── datev_account_entry/{columns,editor,settings,binding}.json
│   ├── datev_calculation/{columns,editor,settings,binding}.json
│   ├── datev_calculation_entry/{columns,editor,settings,binding}.json
│   ├── datev_calculation_value/{columns,editor,settings,binding}.json
│   ├── datev_entry/{columns,editor,settings,binding}.json
│   ├── datev_entry_change_tracking/{columns,editor,settings,binding}.json
│   ├── datev_entry_group/{columns,editor,settings,binding}.json
│   ├── datev_entry_stack/{columns,editor,settings,binding}.json
│   ├── star_money_account/{columns,editor,settings,binding}.json
│   ├── star_money_bank/{columns,editor,settings,binding}.json
│   ├── star_money_booking_text/{columns,editor,settings,binding}.json
│   ├── star_money_credit_card/{columns,editor,settings,binding}.json
│   ├── star_money_credit_card_entry/{columns,editor,settings,binding}.json
│   ├── star_money_entry/{columns,editor,settings,binding}.json
│   └── susa_entry/{columns,editor,settings,binding}.json
└── scripts/
    ├── d2v_value_type_label.rhai
    └── d2v_value_type_label.manifest.json
```

Quelle: 1:1 Kopie der Schicht-3+4-Dateien aus `dblicious/examples/d2v/`
(verifiziert: 17 Entity-Typen, 1 Script). Die einzige strukturelle Neuerung
ist `[meta]` in `config.toml` und die `dblicious-version`-Plaintext-Pin.

## 2. Pflicht-Inhalt von `config.toml`

```toml
[server]
name = "d2v-2019"
bind = "127.0.0.1:8000"

[ui]
title = "D2V 2019 — Daten-Port"

[meta]
# SemVer-Major des data-dir-Loader-Vertrags. Muss <= dem entsprechen,
# was die installierte dblicious-Binary unterstuetzt (siehe
# `shared::DATA_DIR_FORMAT`). Heute (Q0012 §2.2): 1.
dataDirFormat = 1

# Warn-Schwelle. Das Binary loggt eine Warnung, wenn seine eigene
# Version != minServerVersion ist. Kein harter Stopp.
minServerVersion = "0.1.0"
```

## 3. Setup-Rezept (Standalone-README)

Das `README.md` im Standalone-Projekt sollte **mindestens** diesen Inhalt haben
(in einer Zeile als Cheat-Sheet, in Prosa darunter ausfuehrlicher):

```text
Install dblicious-server vX.Y.Z (siehe dblicious-version-Datei), kopiere d2v.db
an einen sicheren Ort, setze D2V_LEGACY_URL + DBLICIOUS_DATABASE_URL in .env,
starte: dblicious-server --data-dir .
```

Konkret:

1. **Binary installieren** — entweder vom GitHub-Release-Asset des in
   `dblicious-version` gepinnten Tags herunterladen, oder Dev-Fallback:
   ```sh
   cargo install --git https://github.com/<org>/dblicious --tag vX.Y.Z dblicious-server
   ```
2. **`d2v.db` bereitstellen** — Kopie der Produktions-DB an einen Pfad
   ausserhalb des Repos legen. **Niemals** das Original einbinden.
3. **`.env` aus `.env.example` erstellen** und mindestens setzen:
   ```sh
   D2V_LEGACY_URL=sqlite:///absoluter/pfad/zu/d2v-kopie.db
   DBLICIOUS_DATABASE_URL=sqlite://./dblicious-d2v.db
   ```
4. **Starten:**
   ```sh
   dblicious-server --data-dir .
   ```

## 4. Datenschutz-Regel (projekt-tragend, ins README)

Die `d2v.db` enthaelt echte Buchungsdaten. Es wird **ausschliesslich mit einer
Kopie** gearbeitet; das Original wird **niemals** eingecheckt, weitergegeben
oder als `D2V_LEGACY_URL`-Ziel gesetzt. `.gitignore` deckt `*.db`, `*.db-shm`,
`*.db-wal`, `.env` ab. Diese Regel ist projekt-tragend, nicht optional.

## 5. Wo lebt das Standalone-Repo

**Empfehlung (Spec §3.3, §9-A1):** eigenes Git-Repo `d2v-dblicious-projekt`
(Arbeitsname), **nicht** ein Unterordner im dblicious-Repo. Begruendung: der
Sinn von Q0012 ist die Trennung — ein Unterordner wuerde die Kopplung
beibehalten.

**Sibling-Folder-Option (verworfen):** ein paralleles Verzeichnis `../d2v/`
neben `dblicious/` waere technisch moeglich, aber operativ schwaecher
(geteilter Workspace, geteilte CI), und widerspricht dem Distributionsmodell
"installiertes Binary".

Konkreter Host (GitHub-Org, GitLab-Gruppe, etc.) = Betreiber-Entscheidung.

## 6. Release-Pipeline-Spezifikation (Q0012 §2.1 / §9-A5)

> Ein zukuenftiger Executor erzeugt daraus `.github/workflows/release.yml` im
> **dblicious**-Repo (nicht im Standalone-Projekt!). Hier nur die
> Spezifikation, damit der Plan keinen YAML-Drift einfuehrt.

### 6.1 Trigger

Tag-getrieben:
```yaml
on:
  push:
    tags:
      - 'v*.*.*'
```

### 6.2 Matrix

Mindest-Abdeckung (Spec §2.1, Empfehlung im Prompt):

| Runner | Target | Binary-Suffix |
|---|---|---|
| `ubuntu-latest` | `x86_64-unknown-linux-gnu` | (kein Suffix) |
| `windows-latest` | `x86_64-pc-windows-msvc` | `.exe` |
| `macos-latest` *(optional)* | `x86_64-apple-darwin` | (kein Suffix) |

### 6.3 Schritte pro Job

1. `actions/checkout@v4`
2. `dtolnay/rust-toolchain@stable` mit `targets: <matrix.target>`
3. `cargo build --release --target ${{ matrix.target }} -p server -p cli`
4. Binaries umbenennen / archivieren:
   - `dblicious-server-${{ github.ref_name }}-${{ matrix.target }}(.exe)`
   - `dblicious-${{ github.ref_name }}-${{ matrix.target }}(.exe)`
5. Komprimieren (`tar.gz` fuer Linux/macOS, `zip` fuer Windows).
6. Upload als Release-Asset via `softprops/action-gh-release@v2`.

### 6.4 Versions-Quelle

Die Version kommt aus `Cargo.toml`'s `[workspace.package] version`. Clap zieht
sie via `#[command(version)]` (heute schon in `server/src/main.rs:31-35` und
`cli/src/main.rs` so verdrahtet). Das **Tag** ist die kanonische
Veroeffentlichungs-ID; die Toml-Version muss vor dem Tag-Push entsprechend
gebumpt werden (sonst zeigt `--version` einen alten Wert).

### 6.5 Dev-Fallback (kein Workflow noetig)

Ohne Release-Artefakt:
```sh
cargo install --git https://github.com/<org>/dblicious --tag vX.Y.Z dblicious-server
cargo install --git https://github.com/<org>/dblicious --tag vX.Y.Z dblicious
```

## 7. Stdlib-Naht (Q0012 §4, Q0013, gap-analysis §4b)

Heute laedt das Standalone-Projekt seine Scripts ausschliesslich aus dem
eigenen `scripts/`-Ordner — genau wie heute `examples/d2v/`. Eine
geteilte, laender-parametrisierte Bookkeeping-Stdlib (Schicht 2) **existiert
nicht** und ist **nicht** Teil von Q0012. Sobald sie gebaut wird (eigene
Folge-Spec, Cross-Ref: Q0013 und `docs/superpowers/specs/2026-05-24-...-gap-analysis.md`
§4b), wird das Standalone-Projekt die Stdlib via **noch nicht festgelegten
Mechanismus** referenzieren — der lokale `scripts/`-Ordner enthielte dann nur
noch echte Schicht-4-Sonderfaelle. Q0012 ist mit dieser Erweiterung kompatibel,
aber **nicht** davon blockiert.

## 8. Was NICHT in den Standalone-Repo wandert

- Kein Rust-Code.
- Keine echte `d2v.db` (Datenschutz, §4).
- Kein `.env` mit echten Werten — nur `.env.example`.
- Keine Cargo-Tests. Optionale Smoke-Tests waeren **binary-getriebene**
  Shell-Skripte ("starte das Binary, GET /graphql, pruefe 200"), nicht
  `cargo test` (Spec §6.3, §9-A3 — optional, kein Q0012-Pflichtteil).
````

- [ ] **Step 2: Sanity-Read**

Lies `docs/standalone-projekt-skeleton.md` zurueck. Pruefe:
- Alle 17 Entity-Typen aus `server/tests/loader_d2v.rs::EXPECTED_ENTITY_TYPES` sind im Tree §1 aufgefuehrt.
- `[meta] dataDirFormat = 1` matcht `shared::DATA_DIR_FORMAT` aus Task 1.
- Kein YAML-Block ist als auszufuehrender Workflow markiert (alles ist Spezifikation, nicht Implementierung).

- [ ] **Step 3: Commit**

```bash
git add docs/standalone-projekt-skeleton.md
git commit -m "docs(plan): standalone-projekt skeleton + release spec (Q0012)"
```

---

## Task 7: Stdlib-Naht-Hinweis am Code (eine Zeile)

> Spec §4 verlangt nur die **Naht-Markierung**, nicht den Mechanismus. Wir kleben einen Cross-Reference-Kommentar an die Stelle im Loader, an der ein zukuenftiger Stdlib-Loader-Pfad einsetzen wuerde — damit ein spaeterer Agent die Stelle findet, ohne `scripts/` ueberraschend umstrukturieren zu muessen.

**Files:**
- Modify: `server/src/example/loader.rs` (vor `fn load_scripts`, Zeile ~190)

- [ ] **Step 1: Doc-Comment-Hinweis ergaenzen**

In `server/src/example/loader.rs`, **direkt vor** der Funktion `fn load_scripts(dir: &Path) -> Result<BTreeMap<String, ScriptSeed>>` (etwa Zeile 190), die folgende Doc-Note **vor** den existierenden Doc-Comment der Funktion setzen (falls keiner existiert: als alleinstehenden `//`-Block):

```rust
// Q0012 §4 / Q0013 / gap-analysis 2026-05-24 §4b: heute laedt diese
// Funktion ausschliesslich `<data-dir>/scripts/`. Eine spaetere,
// geteilte Bookkeeping-Stdlib (Schicht 2) wuerde hier einen zweiten,
// geteilten Such-Pfad ergaenzen (z.B. binary-mitgelieferter Standard-Ordner +
// `[meta] country = "DE"` als Selector). Die Erweiterung ist **additiv**
// und **nicht** Bestandteil von Q0012 — bitte nicht praeventiv vorbauen.
```

- [ ] **Step 2: Build + Clippy**

Run: `cargo build -p server && cargo clippy -p server --all-targets -- -D warnings`
Expected: PASS, kein Warning.

- [ ] **Step 3: Commit**

```bash
git add server/src/example/loader.rs
git commit -m "docs(loader): mark stdlib seam point (Q0012 §4 / Q0013)"
```

---

## Task 8: Verifikations-Pass (Pflicht laut CLAUDE.md)

> CLAUDE.md "Commands" verlangt fmt + clippy + workspace-tests. Letzter Pin vor "fertig".

**Files:** keine.

- [ ] **Step 1: Format-Check**

Run: `cargo fmt --check`
Expected: kein Output, Exit-Code 0. Bei Fail: `cargo fmt`, dann committen unter "chore: fmt (Q0012)".

- [ ] **Step 2: Clippy auf gesamtem Workspace**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: kein Warning, Exit-Code 0.

- [ ] **Step 3: Workspace-Tests**

Run: `cargo test --workspace`

Falls Windows-File-Lock auf `server.exe` (CLAUDE.md-Hinweis):
`cargo test --workspace --target-dir target-test`

Erwartete neue Test-Files in der Ausgabe: `loader_data_dir_format` (4 Tests, alle PASS). Bestehende Tests inklusive `loader_d2v`, `d2v_e2e`, `d2v_all_17_listable` bleiben gruen.

> Memory-Caveat (aus CLAUDE.md / Q0011-Erfahrung): falls der Workspace-Build OOM-aehnliche Symptome zeigt, einzelne Crates testen — `cargo test -p shared && cargo test -p server` — statt `--workspace`.

- [ ] **Step 4: Manueller Boot-Smoke `examples/shop/`**

Run: `cargo run -p server -- --data-dir ./examples/shop`
Expected: Server bindet auf `127.0.0.1:8000`, kein `dataDirFormat`-Warning (weil `[meta]` fehlt = v0-Backcompat). Strg+C.

- [ ] **Step 5: Manueller Boot-Smoke `examples/d2v/`**

Run: `cargo run -p server -- --data-dir ./examples/d2v`
Expected: Server bindet, kein Warning, kein Abbruch. Strg+C.

- [ ] **Step 6: Kein Commit**

Verifikations-Pass; alle Aenderungen sind in vorherigen Tasks committet.

---

## Out of Scope (explizit)

Die folgenden Punkte sind aus dem Prompt und aus Spec §8.7 / §10 als **out of scope** markiert. Sie sind **kein Teil dieses Plans** und werden in Folge-Items adressiert:

- **Erstellung des Standalone-Repos** (Stage 2 des Spec-Schnitts). Stage 1 dieser Plan liefert nur (a) den Framework-Hook im Server-Binary und (b) das Skeleton-Doku. Der Repo-Push, der initiale Commit, der Repo-Host und die Initialisierung der `dblicious-version`-Pin sind Stage 2.
- **YAML-Implementierung des Release-Workflows.** Task 6 spezifiziert ihn vollstaendig (Trigger, Matrix, Schritte, Versionsquelle). Die `.github/workflows/release.yml`-Datei selbst wird in einem eigenen Folge-Item erzeugt — dieser Plan beruehrt `.github/` nicht.
- **Bookkeeping-Stdlib-Mechanismus** (Schicht 2). Spec §4 + Task 7 markieren nur die Naht; der Mechanismus ist eine eigenstaendige Folge-Spec (siehe `docs/superpowers/specs/2026-05-24-...-gap-analysis.md` §8.7).
- **Feature-Gap "welche d2v2019-Features livable sind"** = **Q0013** (Schwester-Item, eigenes Plan-Dokument `docs/superpowers/plans/Q0013-...md`).
- **Fixture-Schrumpfen** (Spec §6.2 / §9-A2). Die Empfehlung der Spec ist, `examples/d2v/` als komplette Fixture **zu behalten**; ein "Mini-Fixture"-Refactor waere eine optionale spaetere Aufraeum-Aufgabe und wuerde Test-Assertions anfassen — bewusst nicht in Q0012.
- **Crate-Dependency-Modell** — vom User verworfen (Spec §0, §8.1), nicht designt, nicht implementiert.

---

## Self-Review (post-write, vom Plan-Author durchgefuehrt)

- **Spec-Coverage:** §2.1 → Task 6.6 (Release-Pipeline-Spec). §2.2 → Tasks 1+2+3 (Konstante + Tests + Loader-Check). §3.2 → Task 6.1 (Skeleton-Tree). §3.3 → Task 6.5 (Repo-Ort + Justification). §4 → Task 6.7 + Task 7 (Naht-Markierung). §5 → Task 6.3+6.4 (Setup-Rezept + Datenschutz). §6 → Task 4 (Regressions-Pin) + Task 6.8 (NICHT-im-Repo-Liste). §7 → Task 3 (Boot-Check-Block). §8 (alle Decisions) → durch obige Punkte abgedeckt. §9 (Annahmen) → A4 = `1` in Task 1; A1/A5 in Task 6; A2 in "Out of Scope"; A3 in Task 6.8; A6 in Task 6 (beide Pin-Artefakte vorhanden).
- **Placeholder-Scan:** kein `TBD`, kein "implement later", keine "handle edge cases"-Stub-Anweisungen — alle Code-Bloecke sind vollstaendig.
- **Type-Konsistenz:** `DATA_DIR_FORMAT: u32` in Task 1 → `shared::DATA_DIR_FORMAT` in Task 3 → `dataDirFormat = 1` in Task 6.2 → 4 Test-Cases in Task 2 referenzieren ebenfalls `DATA_DIR_FORMAT`. Konsistent.
- **TDD-Reihenfolge:** Task 1 (Konstante) → Task 2 (RED-Test) → Task 3 (GREEN-Implementierung + selbe-Commit mit Tests) → Task 4 (Regressions-Pin). Korrekt.
- **Spec-Code-Konflikt:** keiner gefunden. Spec §1 sagt "es existiert KEINE Top-Level-Version" — verifiziert in `loader.rs::ConfigFile` (nur `server`-Feld, kein `meta`). Spec §2.2 sagt "fehlt = keine Pruefung" — `serde(default)` auf `ConfigFile.meta` in Task 3 setzt das genau so um.
