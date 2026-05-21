---
id: Q0005
created: 2026-05-21T00:00:00Z
status: new
priority: high
title: "In-Place Column-Editor auf EntityTable (Header-Klick → Position/Visibility/Filter/Format)"
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
  - table
  - customization
  - builder-alternative
---

## Description

Die bestehende Builder-UI (`/builder/:entity_type`, `BuilderCanvas` mit
`UiTree`/`UiNode`-Drag&Drop) ist ein Figma-artiger Layout-Editor — sie macht
für den Anwendungsfall „Spalten in einer Listenansicht anpassen" *keinen
Sinn*. User-Feedback wörtlich:

> Was ich wollte, ist dass die Tabelle bestehen bleibt, vlt mit reduzierter
> Datenanzahl, und bei null Daten Beispieldaten, und man die Table-Heads
> verändern kann: Position, visible:t/f, Filtermöglichkeiten, Formatierung
> der Daten, etc.

Gewünschtes Modell: **In-Place-Editor auf der echten `EntityTable`** —
Excel-/Airtable-Style. Die Tabelle bleibt sichtbar und live, der User klickt
auf einen Spaltenheader (oder ein Edit-Mode-Toggle) und bearbeitet die
Eigenschaften der konkreten Spalte direkt.

## Gewünschte Edit-Operationen pro Spalte

1. **Position** (Reorder per Drag oder Up/Down-Buttons im Popover)
2. **Visibility** — sichtbar / versteckt (entspricht heute
   `EntitySettings.property[].visibility = Hidden`)
3. **Filter-Operator** (welche Filter-Implementations aus der
   `FilterRegistry` für diese Spalte verfügbar sein sollen)
4. **Datenformatierung** (Locale, Decimal-Stellen, Currency-Symbol,
   Date-Format — sitzt heute in `client/src/components/table/formatters.rs`
   und im jeweiligen `FieldType`)
5. **Min-Width / Spaltenbreite** (entspricht
   `EntitySettings.property[].minWidth`)
6. **Label-Override** (i18n-Key oder freier Text)
7. Optional: Sortable on/off, Default-Sort-Richtung

## Datenfallback bei leerer Tabelle

Wenn die Tabelle 0 Rows hat, sollen **synthetische Beispieldaten** angezeigt
werden, damit Formatierung/Spaltenbreite/etc. sichtbar editierbar sind.
`synthesize_preview_rows` in `client/src/components/table/builder_preview.rs`
existiert bereits und erzeugt deterministische Platzhalterwerte pro
`FieldType` — direkt wiederverwendbar.

## Mögliche UI-Ansätze (Entscheidung im Brainstorm)

- **A) Edit-Mode-Toggle in der TopMenu** der `EntityListPage`: aktiviert,
  werden Header klickbar und öffnen ein Popover mit den Eigenschaften der
  Spalte. Inline-Reorder per Drag der Header. Save-Button im TopMenu.
- **B) Sidebar-Editor**: parallel zur Tabelle eine ausklappbare Sidebar mit
  der vollständigen Spaltenliste; Auswahl in der Sidebar markiert die
  Spalte in der Tabelle.
- **C) Modal mit Spaltenliste**: ein zentrales Modal mit allen Spalten, in
  dem die ganzen Properties bearbeitet werden (klassischer
  „Tabellenansicht anpassen"-Dialog).

## Verhältnis zum bestehenden `/builder/:entity_type`

Der heutige `BuilderCanvas` ist konzeptionell weiter (komponiert ganze
UI-Trees mit `UiNode`, `EventTrigger`, Bindings → Phase 4 Codegen-Ziel) und
soll **nicht** ersetzt werden — die zwei Werkzeuge bedienen verschiedene
Anwendungsfälle:

| | Heutiger BuilderCanvas | Q0005 In-Place-Editor |
|---|---|---|
| Zielgruppe | Designer/Power-User | End-User & Admin |
| Granularität | Beliebige UI-Komposition | Spaltenebene |
| Persistenz | `entity_designs.tree` (versioniert) | `EntitySettings` per User oder global |
| Discovery | Eigene Route | Direkt in jeder Listenansicht |

Es ist explizit zu klären, ob der In-Place-Editor in **`EntitySettings`** der
Wahrheit ist (global pro Entity-Type — heutiger Stand), oder ob er
**per-User-Overrides** schreibt (analog [[Q0002-tabelle-horizontaler-scroll-nicht-sichtbar]]
+ U7-Filter-Builder „Saved Filters"). Letzteres entspricht der „Meine
Ansicht"-Erwartung aus klassischen ERP/BI-Tools.

## Affected files (zu erwarten)

- `client/src/routes/mod.rs` — `EntityListPage` (Edit-Mode-Toggle in
  TopMenu, oder Sidebar-Slot, oder Modal-Trigger)
- `client/src/components/table/view.rs` — Header-Klick-Handler im
  Edit-Mode; Drag-Reorder der Spaltenheader
- `client/src/components/table/data_source.rs` — Fallback auf
  `synthesize_preview_rows`, wenn Page 0 Rows liefert (Edit-Mode-only)
- Neue Komponente `client/src/components/table/column_editor.rs` (Popover
  oder Sidebar — je nach Variante A/B/C)
- `shared/src/lib.rs` — falls per-User-Overrides: neue Tabelle/Mutation
  (z. B. `userEntityViews`)
- Server-Mutation zum Persistieren der `EntitySettings`-Änderungen

## Notes

- Verwandt: [[Q0002-tabelle-horizontaler-scroll-nicht-sichtbar]] —
  vermutlich im gleichen Zug fixierbar.
- Q0004 (Nav-Link Builder) ist davon weiterhin unabhängig: der heutige
  Builder bleibt erreichbar, das hier ist ein zweiter, kontextueller
  Customization-Pfad.
- Verwandt: ROADMAP §1.8 U7 (Filter-Builder + Saved-Filters,
  per-User-Persistenz) — sollte mit diesem Item zusammengeplant werden,
  damit „Meine Ansicht" und „Mein Filter" konsistent persistieren.
