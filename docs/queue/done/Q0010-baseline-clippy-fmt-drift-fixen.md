---
id: Q0010
created: 2026-05-23T00:00:00Z
status: done
priority: low
title: "Baseline-Drift: cargo clippy -- -D warnings + cargo fmt --check schlagen auf dev fehl"
spec: null
plan: null
pending_question_id: null
resume_step: null
parent: null
artifacts: []
source: manual
fingerprint: null
requirements: null
assigned_worker: null
type: maintenance
review:
  status: null
  reviewer: null
  notes_path: null
  requested_at: null
  decided_at: null
security_review:
  required: false
  status: null
  notes_path: null
diagnosis_path: null
design_path: null
linked_issue: null
linked_pr: null
verification_ops: []
tags:
  - lint
  - maintenance
  - ci-readiness
---

## Description

Beim Q0009-Sub-Agent-Verification-Gate ist aufgefallen: zwei Standard-CI-
Gates sind auf dem aktuellen `dev`-Baseline rot, völlig unabhängig von
Q0009.

### Befunde (vom Q0009-Sub-Agent, 2026-05-23, dev@~69f44d0)

**1. `cargo clippy --workspace --target-dir target-test -- -D warnings`**
- 6 Errors in `shared/src/{validation,auth,menu}.rs` (Last touched in
  Commit `fcdd068`, pre-Q0009).

**2. `cargo fmt --check`**
- „Dozens of Diff in …"-Einträge quer durch `cli/`, `client/`, `server/`,
  `shared/` — über Files, die der Sub-Agent für Q0009 Phase 1 *nicht*
  angefasst hat. Also nicht von Q0009-Phase-1 produzierte Drift,
  sondern länger bestehender Stand.

### Warum als separates Item

Q0009 hat als Bedingung formuliert: „explicit file paths, no unrelated
cleanup" und „Don't modify unrelated files". Den Drift im Rahmen von
Q0009 mitzufixen, würde diese Regel direkt verletzen und Files anfassen,
die nichts mit Skripting zu tun haben. Daher: eigenes Item.

### Akzeptanzkriterien

1. `cargo clippy --workspace --target-dir target-test -- -D warnings`
   exit 0 auf `dev`.
2. `cargo fmt --check` exit 0 auf `dev`.
3. Keine semantischen Code-Änderungen — nur Lint-Fixes und `cargo fmt`-
   Anwendung.
4. Falls clippy-Lints semantische Änderungen nahelegen (z.B.
   `needless_collect` umzustellen): pro Fall einzeln entscheiden, im
   Zweifel `#[allow]` mit Begründungs-Kommentar.

## Affected files (zu erwarten)

- `shared/src/validation.rs`
- `shared/src/auth.rs`
- `shared/src/menu.rs`
- weitere clippy-Treffer nach `cargo clippy` Re-Run
- `cli/src/main.rs`, `client/**/*.rs`, `server/**/*.rs`, `shared/**/*.rs`
  via `cargo fmt`

## Notes

- Q0009 verifiziert Phase 2+ NICHT gegen diese zwei Gates, solange
  Q0010 offen ist (Baseline ist rot, das ist akzeptiert).
- Sobald Q0010 abgeschlossen ist, gelten diese Gates für Q0009 wieder
  als Pflicht — entsprechend dem Spec-§12-Testing-Modell.

## Log

- 2026-05-24T00:00:00Z — manual: created (status=new) aus Q0009-Verification-Gate-Befund.
- 2026-05-24T00:00:00Z — Baseline-Fixes durch Parallel-Session: `41a6032`
  (clippy -D warnings, u.a. shared/src/{validation,auth,menu}.rs) + `34db512`
  (cargo fmt workspace-weit).
- 2026-05-24T00:00:00Z — Re-Drift gefixt: Q0009-Revise (`ce03304`, `50a3476`)
  und Typst-PDF-Backend (`84b3d00`) wurden ohne fmt/clippy committet und
  brachen die Baseline erneut. Fix in `40761b8`: 7 Script-Files (fmt) +
  rhai.rs `match_result_ok` + typst.rs `doc-overindented-list-items` 3x.
- 2026-05-24T00:00:00Z — closed (status=done), final_sha=40761b8.
  Verifikation-Gate: `cargo clippy --workspace --target-dir target-test --
  -D warnings` = exit 0; `cargo fmt --check` = clean fuer alle committeten
  Files. Rest-WT-Drift (`server/tests/email_template.rs`) gehoert zur
  laufenden Email-Template-WIP und ist HEAD-clean — nicht Q0010-Scope.
- HINWEIS: Baseline-Hygiene ist ein wiederkehrendes Thema, solange
  Parallel-Sessions Features ohne fmt/clippy committen. Empfehlung fuer
  spaeter: pre-commit-Hook (fmt + clippy) statt periodischer Aufraeum-Items.
