---
id: Q0015
created: 2026-05-30T20:01:01Z
status: executing
priority: low
title: "Skeleton-Doku auf 3 Scripts aktualisieren + minServerVersion-Semantik (>= statt ==)"
spec: docs/superpowers/specs/Q0015-skeleton-doku-stale-und-minserverversion-semantik-design.md
plan: docs/superpowers/plans/Q0015-skeleton-doku-stale-und-minserverversion-semantik.md
pending_question_id: null
resume_step: null
parent: null
artifacts: []
source: manual
fingerprint: null
requirements: null
assigned_worker: null
type: feature
review:
  status: pending
  reviewer: null
  notes_path: null
security_review:
  required: true
  status: null
  notes_path: null
diagnosis_path: null
design_path: null
linked_issue: null
linked_pr: null
verification_ops: []
tags:
  - docs
  - data-dir
  - d2v
  - follow-up
  - low-risk
---

## Description

Zwei kleine, niedrig-riskante Befunde aus dem Review von **Q0012**
(`docs/reviews/Q0012-review.md`, F1 + F2). Bündeln, weil beide den
data-dir-/`[meta]`-Vertrag betreffen.

### Befund 1 — `docs/standalone-projekt-skeleton.md` ist stale (Q0012 F1 / Sec-A4)

Das Skeleton-Dokument (entstanden in Q0012, 206 Zeilen) beschreibt den
`scripts/`-Stand mit **1 Script**, obwohl Q0013 inzwischen **3** Artefakte
ausgeliefert hat: `d2v_balance_validator.{rhai,manifest.json}` und
`d2v_stack_filter.{rhai,manifest.json}`. Doku auf den realen Stand bringen
(Script-Liste + ggf. der 17-Entity-Baum, falls dort ebenfalls gedriftet).

### Befund 2 — `minServerVersion`-Semantik-Drift (Q0012 F2)

`server/src/example/loader.rs:103` vergleicht heute mit
`if min_ver != our_ver` — ein **Exakt-Match**, der bei *jeder* abweichenden
(auch neueren, eigentlich kompatiblen) Binary-Version warnt. Der Feldname
`minServerVersion` impliziert eine **`>=`-Semantik** (warne nur, wenn das Binary
*älter* als das geforderte Minimum ist). DoD: semver-`>=`-Vergleich (oder
zumindest dokumentierte, korrekte Untergrenzen-Logik); die Warn-Message
entsprechend präzisieren; Test, der „Binary neuer als min → keine Warnung" und
„Binary älter als min → Warnung" abdeckt.

Hinweis: Beides ist **Warn-only** (kein harter Boot-Abbruch) — der harte Gate
ist `dataDirFormat` (Q0012, `shared::DATA_DIR_FORMAT`). Risiko gering.

### Referenzen

- `docs/reviews/Q0012-review.md` (F1, F2), `docs/reviews/Q0012-security-review.md` (A4).
- `docs/standalone-projekt-skeleton.md`.
- `server/src/example/loader.rs:95-111`.
- `docs/queue/done/Q0012-...`, `docs/queue/done/Q0013-...`.

## Log
- 2026-05-30T20:01:01Z — manual: created (Q0012-Review-Follow-ups F1+F2 gebündelt; verifiziert: loader.rs:103 `min_ver != our_ver` Exakt-Match)
- 2026-05-30T20:08:11Z — ccm-brainstorm: status new → brainstormed, spec=docs/superpowers/specs/Q0015-skeleton-doku-stale-und-minserverversion-semantik-design.md; security_review.required=true (Trigger: script). Spec-Kern: semver-`>=` via gelockte `semver`-Crate hinter pure `server_version_warning`-Helper (warn-and-skip bei malformed), Skeleton-Doku scripts/-Block 1→3 (17-Entity-Baum bleibt korrekt)
- 2026-05-30T20:11:35Z — ccm-plan: status brainstormed → planned, plan=docs/superpowers/plans/Q0015-skeleton-doku-stale-und-minserverversion-semantik.md (5-Task-TDD: semver=\"1\" zu server/Cargo.toml ohne Lockfile-Bump, test-first `server_version_warning`-Helper mit 6 Fällen, Loader-Rewire, Skeleton scripts/ 1→3; Verify via target-test/)
- 2026-05-30T20:12:30Z — ccm-execute: status planned → executing (pre-approved via 'alle drei der Reihe nach')
