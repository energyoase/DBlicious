# Q0015 — Code Review

Status: **approve**
Datum: 2026-05-30
Reviewer: code-review (Claude)
Diff-Scope: `7122130..9afeff1` (4 commits)
Spec: `docs/superpowers/specs/Q0015-skeleton-doku-stale-und-minserverversion-semantik-design.md`
Plan: `docs/superpowers/plans/Q0015-skeleton-doku-stale-und-minserverversion-semantik.md`

## Scope

Zwei warn-only Fixes am data-dir-/`[meta]`-Vertrag, gebündelt:

1. Doku: `docs/standalone-projekt-skeleton.md` scripts/-Block 1 → 3 d2v-Script-Paare (reine Doku).
2. Code: `server/src/example/loader.rs` minServerVersion von Exact-String-`!=` auf SemVer-`>=`-Schwelle via pure Helper `server_version_warning`. `semver = "1"` zu `server/Cargo.toml`.

Geänderte Dateien: `Cargo.lock` (+1 Zeile, Dep-Edge), `docs/standalone-projekt-skeleton.md`, `server/Cargo.toml`, `server/src/example/loader.rs`.

## Befunde

Keine blockierenden Befunde. Die Implementierung folgt der Spec praktisch verbatim (§1.5 Helper, §1.6 Doc-Comment, §1.7 Test-Tabelle).

### Korrektheit — SemVer-`>=`-Logik

- `if our < min { warn }` implementiert die Soll-Semantik (Spec §1.2) korrekt: warnt nur, wenn Binary älter als Minimum.
- `semver::Version`'s `Ord` ist reine SemVer-Precedence. Die in der Spec erwähnten `0.x`-Sonderregeln betreffen nur *Requirement-Matching* (Caret), **nicht** das `Version`-Ordering — plain `<` ist hier also korrekt. Keine `VersionReq`-Verwechslung.
- Tabelle verifiziert: `0.2.0 ⩾ 0.1.0` → None; `0.1.0 ⩾ 0.1.0` → None; `0.1.0 < 0.2.0` → Some; `0.1.0 < 0.1.5` → Some.

### Warn-and-skip bei malformed Input (Spec §1.4)

- Beide Parse-Fehler-Arme (`min_ver`, `our_ver`) liefern `Some(msg)` ohne `panic`, ohne Boot-Abbruch. Korrekt warn-only.
- Anmerkung (non-blocking): Der `our_ver`-Parse-Fehler-Pfad ist in Produktion praktisch tot, da `CARGO_PKG_VERSION` aus der Workspace-Version `0.1.0` (gültiges SemVer) abgeleitet wird. Die Spec verlangt diesen Pfad jedoch explizit als defensives warn-and-skip; er ist billig und durch einen Test gepinnt. Kein Handlungsbedarf.

### Purität / Testbarkeit (Spec §1.5)

- `server_version_warning` ist `fn`-privat (kein `pub`), rein (kein I/O, kein `tracing`), pfad-agnostisch. Die `dir.display()`-Präfixierung bleibt an der Call-Site.
- Test-Modul importiert via `use super::server_version_warning` — bestätigt modul-private Erreichbarkeit ohne Export der Surface.
- 6 Tests bilden die §1.7-Tabelle 1:1 ab; `binary_older_than_min_warns` assertet beide Versionen + `"zu alt"`, `malformed_min_ver_warns_and_skips` assertet `"kein gueltiges SemVer"`.

### Deutsche Warn-Messages

- Wortlaut deckt sich verbatim mit Spec §1.5. Inhaltlich korrekt ("Reine Warnung, kein Stopp", "bitte ein neueres dblicious-Binary installieren").
- Doc-Comment auf `ConfigMeta::min_server_version` auf SemVer-`>=`-Semantik umgeschrieben (§1.6); alte "lex-vergleichbar / parsen es nicht weiter"-Begründung entfernt (per Grep verifiziert: keine Reste).

### Doku-Akkuratheit (Befund 1)

- scripts/-Block listet jetzt exakt die 3 Paare aus `examples/d2v/scripts/` (per `ls` verifiziert: `d2v_balance_validator`, `d2v_stack_filter`, `d2v_value_type_label`, je `.rhai` + `.manifest.json`). Alphabetisch, matcht den realen Bestand.
- Quellen-Zeile auf "17 Entity-Typen, 3 Scripts (Stand 2026-05-30)" aktualisiert.
- Die 17-Entity-Zahl ist **unverändert** und weiterhin korrekt: `examples/d2v/entities/` enthält 17 Verzeichnisse; `server/tests/d2v_all_17_listable.rs` assertet die Zahl 17 unabhängig.
- Ein-Zeilen-Anti-Drift-Hinweis (§2.3) als Blockquote ergänzt — keine CI/Test-Maschinerie, korrekt minimal.

### Projekt-Konventionen

- Kommentare deutsch, Identifier englisch — konform.
- `shared::DATA_DIR_FORMAT` **unverändert** (= 1); `shared/` im Diff komplett unangetastet. Additive Änderung → korrekt **kein** Konstanten-Bump (CLAUDE.md: "Additive Loader-Aenderungen erhoehen die Konstante **nicht**").
- Kein `examples/**`-Mutation.
- `semver = "1"` mit erklärendem Kommentar; löst auf bereits gelockte `1.0.28` auf (Cargo.lock `+1` = neue Direkt-Dep-Edge, kein neuer Knoten/Bump). Konform mit Spec §1.3.

## Verifikations-Log (aus ccm-execute, plausibel)

- `cargo fmt --check` exit 0
- `cargo clippy -p server --all-targets -D warnings` exit 0 (scoped auf `-p server` per Plan-OOM-Caveat, nicht `--workspace`)
- `cargo test -p server --lib` exit 0 (87 passed, inkl. 6 neue `server_version_warning`-Tests)
- `cargo test -p server --test loader_data_dir_format` exit 0 (4 passed)

## Verdict

**approve** — saubere, low-risk Umsetzung exakt nach Spec. Keine blockierenden Issues. Eine non-blocking Beobachtung (toter `our_ver`-Defensiv-Pfad) ist spec-konform gewollt und getestet.
