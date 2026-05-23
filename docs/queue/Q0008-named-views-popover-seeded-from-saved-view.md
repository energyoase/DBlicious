---
id: Q0008
created: 2026-05-23T00:00:00Z
status: new
priority: low
title: "Named Views: Popover-Inputs aus gespeicherter Server-View seeden (I3 aus Q0005-Review)"
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
type: bug
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

`ColumnEditorPopover.current_override` ist derzeit aus `pending_overrides`
abgeleitet (`client/src/routes/mod.rs::EntityListPage`). Wenn der User die
Popover für eine Spalte öffnet, deren Override schon **server-gespeichert**
ist (kein pending), zeigen die Popover-Inputs „kein Override" — obwohl die
Tabelle die Werte schon spiegelt (durch den C1-Fix in
[[Q0005-in-place-column-editor-auf-entity-table]]).

Effekt: User sieht im Popover „leere" Felder, obwohl die Spalte de facto
schon einen Server-Override hat. Verwirrend, aber kein Daten-Verlust:
wenn der User das Feld unverändert lässt, bleibt der Server-Override
erhalten; wenn er es ändert, schreibt das einen neuen pending Override
über den Server-Stand.

## Affected files

- `client/src/routes/mod.rs::EntityListPage` (~`ov_sig`-Signal-Derivation)

## Expected

`ov_sig` derived from: pending HashMap → falls vorhanden, das nehmen;
falls nicht, dann aus `current_view.properties` den passenden
`ViewPropertyOverride` ziehen; falls auch dort nicht vorhanden, `None`.

Skizze:

```rust
let ov_sig: Signal<Option<ViewPropertyOverride>> = Signal::derive(move || {
    pending_overrides.with(|p| p.get(&key_for_sig).cloned())
        .or_else(|| current_view.get().and_then(|w| w.take()).and_then(|v| {
            v.properties.into_iter().find(|ov| ov.key == key_for_sig)
        }))
});
```

## Notes

Aufgetaucht im Final-Review von Q0005. Kosmetik / UX-Konsistenz — nicht
blockierend.
