---
id: Q0018
created: 2026-05-31T08:54:12Z
status: reviewed
priority: low
title: "Q0014-Härtungen: script-Filter INT-Coercion + global_search-Guard + Per-Row-Aggregate-Op-Budget"
spec: docs/superpowers/specs/Q0018-q0014-haertungen-scriptfilter-numerik-globalsearch-budget-design.md
plan: docs/superpowers/plans/Q0018-q0014-haertungen-scriptfilter-numerik-globalsearch-budget.md
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
  notes_path: docs/reviews/Q0018-review.md
  requested_at: 2026-05-31T09:59:40Z
  decided_at: 2026-05-31T10:03:49Z
security_review:
  required: true
  status: requested
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
- 2026-05-31T09:18:58Z — ccm-brainstorm (via ccm-loop, parallel): status new → brainstormed, spec=docs/superpowers/specs/Q0018-q0014-haertungen-scriptfilter-numerik-globalsearch-budget-design.md; security_review.required=true (Trigger: script, sandbox, wasm). Alle 3 Cuts gegen Code verifiziert: H1 from_f64→FLOAT (json_to_dynamic rhai.rs 255-278), H2 global_search-Loop ohne script:-Guard (data_source.rs 282-295), H3 set_max_operations(50_000) per-run (rhai.rs 77-82) → deterministisches Run-Count-Budget MAX_SCRIPT_FILTER_RUNS. Test-Harness local_source_script_filter.rs. Keine blockierende Decision.
- 2026-05-31T09:25:06Z — ccm-plan (via ccm-loop, parallel): status brainstormed → planned, plan=docs/superpowers/plans/Q0018-q0014-haertungen-scriptfilter-numerik-globalsearch-budget.md (7-Task-TDD: H1 int-typed selectedStackId, H2 script:-skip in global_search, H3 ScriptBudget run-count). Anker alle bestätigt (passes 248-297, script-branch 260-271, global_search 282-295, script_predicate 372-408, apply_limits rhai.rs:78). Mechanik-Note: cfg(debug_assertions) statt cfg(test) fürs Budget-Override (Integration-Tests kompilieren client als Dep ohne crate-cfg(test)). Keine blockierende Decision.
- 2026-05-31T09:59:15Z — ccm-execute (via ccm-loop, PARALLEL in isoliertem Worktree): status planned → executing → done, final_sha=7d8d8f9 (Merge-Commit; 1 Implement-Commit 88a6e97). H1 int-typed selectedStackId (Rhai INT statt FLOAT), H2 global_search überspringt script:-Spalten, H3 ScriptBudget(MAX_SCRIPT_FILTER_RUNS=5000, debug_assertions=200) fail-open per fetch. 8 Filter-Tests (je H rot-vor-fix). Nur data_source.rs + local_source_script_filter.rs. Worktree --no-ff gemerged; kombinierte client-Verification grün. — awaiting review
- 2026-05-31T10:03:49Z — ccm-review (via ccm-loop, parallel): status done → reviewed (approved by claude), notes=docs/reviews/Q0018-review.md (0 blocking; 2 non-blocking: (1) debug_assertions koppelt Budget → 'trunk serve'-Dev-Build nutzt 200-Run-Threshold statt 5000, dev-only/fail-open/informational; (2) guarded i64-Cast korrekt). H1/H2/H3 alle korrekt verifiziert, scope exakt data_source.rs+Test, kein Sandbox/Server-Change.
