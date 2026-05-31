---
id: Q0018
created: 2026-05-31T08:54:12Z
status: new
priority: low
title: "Q0014-Härtungen: script-Filter INT-Coercion + global_search-Guard + Per-Row-Aggregate-Op-Budget"
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
  status: pending
  reviewer: null
  notes_path: null
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
  - script-first
  - hardening
  - follow-up
  - data-source
---

## Description

Drei kleine Härtungen aus den Q0014-Reviews — **alle in
`client/src/components/table/data_source.rs`**, deshalb gebündelt (ein Item =
ein Execution, kein Parallel-Konflikt auf derselben Datei). Alle drei sind
non-blocking-Advisories, keine Bugs.

### H1 — script-Filter INT-Coercion (Q0014 Review non-blocking #1)

`script_predicate` injiziert den Filterwert (`selectedStackId`) via
`serde_json::Number::from_f64`, d.h. er erreicht Rhai als **FLOAT**, während die
Row-`stackId` ein **INT** ist. Heute korrekt, weil Rhai 1.24 INT/FLOAT bei `==`
coerced — aber spröde gegen künftige Engine-Numerik-Strictness. DoD: ganze
Floats zu Integer normalisieren (oder die JSON-Number typtreu durchreichen),
Test, der einen INT-vs-FLOAT-Vergleich pinnt.

### H2 — global_search-`script:`-Guard (Q0014 Review non-blocking #2)

Die `global_search`-Schleife in `LocalSource::passes` führt eine
`script:`-Filter-Spalte noch durch `ops_for_named`/`matches_search` statt durch
den `script:`-Branch (der nur die Per-Predicate-Schleife guarded). Heute benign
(unbekannte `script:`-id fällt auf Default-Ops zurück), aber unsauber. DoD:
`script:`-Spalten in der global-search-Schleife überspringen oder explizit
behandeln; Kommentar/Guard; ggf. Test.

### H3 — Per-Row-Filter-Aggregate-Op-Budget (Q0014 Security-Review Advisory #1)

Per-Row-Filter-Scripts haben heute **kein aggregiertes Cross-Row-Op-Budget** —
jedes Script läuft bis zum `set_max_operations(50_000)`-Limit *pro Zeile*. Bei
großem `page_size` × Script nahe Limit kann das den (eigenen) Browser-Tab
spürbar bremsen (self-inflicted, client-side). DoD: ein aggregiertes
Op-/Zeit-Budget über die gefilterte Page (oder eine page-size-Obergrenze für
script-gefilterte Spalten); dokumentieren. Kein Trust-Boundary — reine
Resource-Hygiene.

### Out of scope

- Server-seitiges Filtern / generelles Filter-Pushdown (war schon in Q0014 OOS).
- Änderungen am Sandbox-/Capability-Modell selbst.

### Referenzen

- `docs/reviews/Q0014-review.md` (non-blocking #1, #2)
- `docs/reviews/Q0014-security-review.md` (advisory #1)
- `client/src/components/table/data_source.rs` (`LocalSource::passes`, `script_predicate`, global_search-Loop)

## Log
- 2026-05-31T08:54:12Z — manual: created (3 gebündelte Q0014-Review-Härtungen, alle in data_source.rs; H1 INT-Coercion, H2 global_search-Guard, H3 Per-Row-Aggregate-Budget)
