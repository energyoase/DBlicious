---
id: Q0013
created: 2026-05-29T00:00:00Z
status: security-cleared
priority: medium
title: "Minimale livable d2v2019-Teilmenge im Example (gestaffelte DoD)"
spec: docs/superpowers/specs/Q0013-minimale-livable-d2v2019-teilmenge-design.md
plan: docs/superpowers/plans/Q0013-minimale-livable-d2v2019-teilmenge.md
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
  status: approved
  reviewer: claude
  notes_path: docs/reviews/Q0013-review.md
  requested_at: 2026-05-29T23:04:22Z
  decided_at: 2026-05-29T23:09:16Z
security_review:
  required: true
  status: cleared
  notes_path: docs/reviews/Q0013-security-review.md
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
- 2026-05-29T19:49:34Z — ccm-brainstorm: status new → brainstormed, spec=docs/superpowers/specs/Q0013-minimale-livable-d2v2019-teilmenge-design.md; security_review.required=true (Trigger: script, auth)
- 2026-05-29T22:42:46Z — ccm-plan: status brainstormed → planned, plan=docs/superpowers/plans/Q0013-minimale-livable-d2v2019-teilmenge.md; IBAN-Pilot deferred (c); validator_id-Wiring -> Stage-2-Framework-Folgeitem
- 2026-05-29T22:53:16Z — ccm-execute: status planned → executing (pre-approved via 'execute beide')
- 2026-05-29T23:03:42Z — ccm-execute: status executing → done, final_sha=7b0daad (P1 balance-validator + P3 stack-filter mit filterId-Wiring; IBAN deferred, validator_id-Wiring Stage-2; verification green) — awaiting review
- 2026-05-29T23:09:16Z — ccm-review: status done → reviewed (approved by claude), notes=docs/reviews/Q0013-review.md (3 non-blocking: F1 readI18n-Drift, F2 Manifest-Test-Kopplung, F3 P1 null-handling)
- 2026-05-29T23:17:11Z — ccm-security-review: status reviewed → security-cleared, notes=docs/reviews/Q0013-security-review.md (4 advisory; P3 filterId-Wiring ist heute dormant — ops_for_named/lookup_provider im Client-Filter-Pfad noch ungenutzt — Folgeitem-Kandidat)
