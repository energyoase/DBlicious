---
id: Q0004
created: 2026-05-21T00:00:00Z
status: done
priority: medium
title: "Builder/Designer ist nicht aus der Navigation erreichbar"
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
  - navigation
  - discoverability
  - builder
---

## Description

Die Route `/builder/:entity_type` ist im Router registriert
(`client/src/app.rs:108`) und voll funktional (Drag&Drop, Live-Preview,
Save mit Optimistic Locking), aber **nirgends aus der UI verlinkt**.

`client/src/components/navigation.rs` rendert nur die GraphQL-`navigation`-
Bäume aus dem Server. Es existiert kein Eintrag „Designer" / „Builder" /
„Ansicht anpassen" — die Route ist nur per Direkt-URL erreichbar.

Effekt: Selbst Admins / Designer-User können die Builder-UI nicht entdecken;
End-User wissen nicht, dass die Layout-Anpassung überhaupt existiert.

## Optionen (Entscheidung im Brainstorm)

- **A) Globaler Designer-Tab** im Haupt-Nav (eigene Route `/designer/:type`
  oder Sub-Route der Entity-Liste). Anschluss-Punkt: bestehende
  `DesignerPage` (`/designer`).
- **B) Toggle „Layout bearbeiten" pro Entity-Listenansicht**:
  Button/Mode-Switch in `EntityListPage`, der dieselbe Entity in den Builder
  schaltet — kontextuell, der User weiß welche Entity er gerade editiert.
- **C) Per-Entity-Link im Server-Nav-Tree**: jeder Nav-Knoten bekommt
  optional einen „⚙ Designer"-Sub-Link (server-driven über `NavigationNode`).

## Expected

End-User / Designer kann ohne URL-Memorisieren in die Builder-Ansicht für
die aktuell betrachtete Entity wechseln. Permission-Gating (analog
[[Q0003-builder-auth-gate-wildcard-zu-streng]]) blendet die Option für
Nicht-Berechtigte aus.

## Affected files

- `client/src/components/navigation.rs` (Option A/C)
- `client/src/routes/mod.rs` — `EntityListPage` (Option B)
- evtl. `shared/src/lib.rs::NavigationNode` (Option C — Wire-Vertrag erweitern)

## Notes

Verwandt: [[Q0003-builder-auth-gate-wildcard-zu-streng]] muss zuerst gefixt
sein, sonst zeigt der Nav-Link auf eine 403-Seite für legitime Editor-User.
