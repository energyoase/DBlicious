---
id: Q0002
created: 2026-05-20T22:00:00Z
status: new
priority: medium
title: "EntityTable: horizontale Scrollbar bei langen Tabellen nicht sichtbar"
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
  - usability
---

## Description

Der horizontale Scrollbalken der `EntityTable` sitzt am **unteren** Rand des
Tabellen-Wrappers (`overflow-x: auto;` auf `<div>`). Bei langen Tabellen
(viele Zeilen) liegt dieser Rand weit unterhalb des Viewports — der User sieht
nicht, dass die Tabelle überhaupt horizontal scrollbar ist, und müsste erst
vertikal bis zum Ende der Tabelle scrollen, um den Scrollbalken nutzen zu
können.

Beobachtet im D2V-Beispiel (`./examples/d2v`) bei Entities mit vielen Spalten
(z. B. `datev_entry`, `star_money_entry`) — das Scrollen nach rechts ist
faktisch nur per Shift+Mausrad oder Touchpad möglich, was nicht entdeckbar ist.

## Repro

1. Server mit D2V-Daten starten:
   `D2V_LEGACY_URL=sqlite:///pfad/zu/d2v.db cargo run -p server -- --data-dir ./examples/d2v`
2. Client starten: `cd client && trunk serve`
3. Browser auf http://127.0.0.1:8080 → eine spaltenreiche Entity (z. B.
   `datev_entry`) öffnen.
4. Beobachten: rechts liegen weitere Spalten außerhalb des Viewports, aber
   kein sichtbarer Hinweis (Scrollbalken / Schatten / Gradient).

## Expected

Der User sieht jederzeit, dass die Tabelle horizontal scrollbar ist, und kann
sie ohne vertikales Vorscrollen scrollen. Mögliche Ansätze (Entscheidung im
Brainstorm):

- Tabelle in einen Container mit `max-height` packen, sodass Vertikal-Scroll
  **innerhalb** der Tabelle bleibt und der horizontale Scrollbalken am unteren
  Tabellen-Rand jederzeit im Viewport sichtbar ist.
- Sticky / Floating-Scrollbar (z. B. zweite gekoppelte Scrollbar oben oder
  am Viewport-Boden).
- Visuelle Hinweise (Schatten/Gradient an den Tabellen-Rändern), wenn Inhalt
  außerhalb sichtbar liegt.

## Affected files

- `client/src/components/table/view.rs:96` — Haupt-Tabelle, `overflow-x: auto;`
- `client/src/components/table/table_view.rs:50` — Builder-Preview-Variante
- Styling-Anpassung muss durch das `DesignSystem`-Trait
  (`client/src/styling/`), keine Hard-coded-Styles in den Komponenten.

## Notes

Verwandt mit der `DesignSystem`-Architektur (CLAUDE.md: keine direkten
CSS-Klassen / Style-Strings in Komponenten). Lösung sollte in
`InlineDesign` (`styling/tokens.rs` / Style-Tokens) landen.
