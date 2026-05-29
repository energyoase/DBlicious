---
id: Q0013
created: 2026-05-29T00:00:00Z
status: new
priority: medium
title: "Minimale livable d2v2019-Teilmenge im Example (gestaffelte DoD)"
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
  - d2v
  - gap-analysis
  - script-first
  - definition-of-done
---

## Description

Frage: **was fehlt, damit die *funktionierenden* d2v2019-Features im
`examples/d2v/` tatsächlich nutzbar ("livable") sind** — explizit OHNE die
nie-funktionalen/Stub-Features. Die Script-First-Gap-Analyse
(`docs/superpowers/specs/2026-05-24-d2v-script-first-gap-analysis.md`) hat die
Gap-Liste gegen den heutigen Stand bereits erstellt; dieses Item soll daraus
eine **fokussierte, gestaffelte Definition-of-Done für eine minimale
livable-Teilmenge** machen (User-Entscheidung 2026-05-29: gestaffelt).

**Ausgeklammert** (broken/Stub/obsolet, laut Glossary & Analyse §0): Company,
DatevEntryGroup, DatevEntryChangeTracking, StarMoney↔DATEV-Reconciliation,
StarMoneyCreditCard, DescriptionSplitter, StarMoney-Import.

### Gestaffelte DoD (im Brainstorm zu schärfen)

- **Stufe 1 (jetzt — read/display/validate):** Konten/Buchungen/Stammdaten
  anzeigen + filtern; ValueType SOLL/HABEN-Format; IBAN-Validierung;
  read-time-Vorschau kleiner Mengen — größtenteils HEUTE machbar
  (Formatter-/Validator-/Filter-Scripts; der Loader-Pfad für
  `examples/d2v/scripts/<id>.rhai` + Manifest steht, verifiziert). Größte
  Sperre: **U1 FK-Referenz-Picker** (Konto-/Bank-Auswahl).
- **Stufe 2 (später — schreibend/aggregierend):** GenerateAccountEntries
  persistieren (U2/F1), Invert/Storno, DATEV-/Bank-Import (Bulk-Import
  1.7.15), effiziente Aggregation (1.7.12) für SuSa/Calculation; U3
  Report-View, U5 YearSelector, `FieldType::Tree`.

### Im Brainstorm zu klären

- Welche genaue Feature-/Entity-Teilmenge ist Stufe-1-"livable" (DoD)?
- Welche Scripts lassen sich HEUTE als `examples/d2v/scripts/`-Dateien anlegen
  (Pilot: ValueType-Formatter, IBAN-Validator, Stack-Filter)?
- Welche Controls/Fähigkeiten sind harte Voraussetzung für Stufe 1 (U1 jetzt)
  vs. Stufe 2 (Rest)?
- Abgrenzung zu **Q0012** (Packaging/Distribution) — dieses Item ist die
  **Feature-Teilmenge**, nicht die Distribution.

### Referenzen

- `docs/superpowers/specs/2026-05-24-d2v-script-first-gap-analysis.md` — primär (script-first-Brille, Feature-Matrix §4, Lücken-Cluster §5).
- `docs/superpowers/specs/2026-05-20-d2v-on-dblicious-gap-analysis.md` — ältere Gesamt-Analyse (U1–U7, F1/F2, T1–T23).
- `examples/d2v/README.md`, `examples/d2v/entities/*/`.

## Log
- 2026-05-29T14:35:00Z — manual: created (Scope-Split aus /ccm-brainstorm-Anfrage; Schwester-Item Q0012)
