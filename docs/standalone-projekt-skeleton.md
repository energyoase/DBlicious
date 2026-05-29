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
