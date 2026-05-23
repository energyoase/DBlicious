---
id: Q0007
created: 2026-05-23T00:00:00Z
status: new
priority: medium
title: "Named Views: Drag-Reorder am Header in der UI verdrahten (I2 aus Q0005-Review)"
spec: docs/superpowers/specs/2026-05-21-q0005-named-views-design.md
plan: null
pending_question_id: null
resume_step: null
parent: Q0005
artifacts: []
source: review
fingerprint: null
requirements: null
assigned_worker: null
type: feature
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
  - ui
  - table
  - q0005-followup
---

## Description

`compute_reorder`, `HeaderRect`, `DragState` sind in
`client/src/components/table/column_editor/mod.rs` als reine Funktionen
implementiert und über `client/src/components/table/mod.rs` re-exportiert.
Sie haben Unit-Tests (4 Stück, in `reorder_tests`-Modul) — aber **kein
einziger Call-Site existiert** in `client/src/`.

Heute kann der User die Spalten-Reihenfolge nur per `Position`-Input im
Popover anpassen. Die Spec (Q0005 §6.3) versprach Drag-Reorder per
Pointer-Events auf dem `<th>`. Das ist nicht ausgeliefert.

## Affected files

- `client/src/components/table/table_view.rs` (`HeaderCell`) — `pointerdown`/`pointermove`/`pointerup`-Handler
- `client/src/components/table/column_editor/mod.rs` — bestehende `compute_reorder` ist die Ziel-API
- `client/src/routes/mod.rs` — `pending_overrides` Signal ist das Ziel des `order`-Updates

## Expected

Im Edit-Mode: User klickt-und-zieht einen Spalten-Header. Während des Drags
ghosting-Visual; beim Drop wird `compute_reorder(headers, drag)` aufgerufen
und das Resultat als `order`-Werte in `pending_overrides` geschrieben (eine
Override pro betroffener Spalte).

## Notes

Aufgetaucht im Final-Review von Q0005. Reine Verdrahtungs-Aufgabe, kein
Logik-Bug — die Math und Tests sind schon da.
