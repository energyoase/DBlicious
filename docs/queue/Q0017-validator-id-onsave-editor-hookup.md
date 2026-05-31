---
id: Q0017
created: 2026-05-31T08:54:12Z
status: brainstormed
priority: medium
title: "validator_id on-save Editor-Hookup (Stage-3 von Q0014 Lücke A)"
spec: docs/superpowers/specs/Q0017-validator-id-onsave-editor-hookup-design.md
plan: null
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
  - d2v
  - script-first
  - validation
  - stage-3
  - follow-up
---

## Description

Stage-3-Folgeitem aus **Q0014** (Lücke A, cut-line-C). Q0014 hat den
`validator_id`-Slot additiv eingeführt und bewiesen, dass ein `script:`-Validator
**live** durch das echte `ValidationSystem::run` läuft (System-Level-Test
`client/tests/validation_script_task.rs`). **Bewusst deferred** wurde dort die
letzte Meile: das Registrieren der Script-Validatoren in die `ValidationSystem`
der **Live-Editor-UI** beim Laden, sodass eine Verletzung den Save im echten
Editor blockiert + eine `ValidationMessage` zeigt.

### Ziel (DoD)

- Beim Editor-Load (`client/src/routes/editor.rs`, on-save-Pfad ~:134, wo heute
  Tasks nur aus `EditorMeta.required` kommen) werden Spalten mit gesetztem
  `validator_id` (bzw. `script:<id>`) in die `ValidationSystem` registriert —
  via `lookup_provider(ProviderSlot::Validator)` + dem in Q0014 gebauten
  `client/src/validation/script_task.rs`-`TaskFn`.
- Eine fehlschlagende Validierung blockiert den Save in der Live-UI und zeigt die
  `ValidationMessage` (Message-Key `SCRIPT_VALIDATION_KEY`, aus Q0014).
- d2v `d2v_balance_validator` greift end-to-end im Editor (SOLL/HABEN-Bilanz).
- Test: Editor-Komponenten-/Integration-Test, dass ein verletzender Datensatz
  nicht speicherbar ist und die Message erscheint.

### Out of scope / Hinweise

- Server-seitige Validierung bleibt die **autoritative** Prüfung — der
  client-seitige Validator ist UX/Frühwarnung. Diese Invariante NICHT aufweichen
  (Q0014-Security-Review Advisory). Validator-fail-open (Script-Fehler ⇒ Save
  erlaubt) bleibt akzeptabel, weil der Server autoritativ ist.
- Reaktive/async-Concerns des Editor-Pfads sind hier der Hauptaufwand (waren der
  Grund für das Deferral in Q0014).

### Referenzen

- `docs/queue/done/Q0014-validator-id-slot-und-script-prefix-filterpfad.md`
- `docs/reviews/Q0014-review.md`, `docs/reviews/Q0014-security-review.md`
- `client/src/routes/editor.rs` (on-save), `client/src/validation/{mod,script_task}.rs`
- `client/src/script/provider_lookup.rs`
- `examples/d2v/scripts/d2v_balance_validator.*`

## Log
- 2026-05-31T08:54:12Z — manual: created (Stage-3-Folgeitem aus Q0014 Lücke A; Editor-UI on-save-Hookup, in Q0014 cut-line-C bewusst deferred)
- 2026-05-31T09:18:58Z — ccm-brainstorm (via ccm-loop, parallel): status new → brainstormed, spec=docs/superpowers/specs/Q0017-validator-id-onsave-editor-hookup-design.md; security_review.required=true (Trigger: script, sandbox, wasm). Ansatz A: Editor holt zusätzlich fetch_columns + register_script_validators (Q0014 script_validator_task), 0 shared/server-Changes; ValidationSystem non-reaktiv+synchron ⇒ Deferral-Hindernis auflösbar. Impl-Gaps: d2v value-Spalte braucht validatorId=script:d2v_balance_validator + FTL-Key validation-script fehlt. Keine blockierende Decision.
