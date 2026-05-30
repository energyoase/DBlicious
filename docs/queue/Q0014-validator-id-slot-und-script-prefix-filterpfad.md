---
id: Q0014
created: 2026-05-30T20:01:01Z
status: reviewed
priority: medium
title: "Stage-2: validator_id-Slot in ColumnMeta/EntitySettings + script:-Prefix im Filter-Pfad konsumieren"
spec: docs/superpowers/specs/Q0014-validator-id-slot-und-script-prefix-filterpfad-design.md
plan: docs/superpowers/plans/Q0014-validator-id-slot-und-script-prefix-filterpfad.md
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
  notes_path: docs/reviews/Q0014-review.md
  requested_at: 2026-05-30T22:12:53Z
  decided_at: 2026-05-30T22:17:52Z
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
  - framework
  - stage-2
  - follow-up
---

## Description

Stage-2-Framework-Folgeitem aus **Q0013** (siehe dessen Plan-Log
2026-05-29T22:42:46Z „validator_id-Wiring -> Stage-2-Framework-Folgeitem" und
die Security-Review-Notiz 2026-05-29T23:17:11Z: P3-`filterId`-Wiring ist heute
**dormant**). Es geht um zwei zusammenhängende Lücken, die verhindern, dass die
in Q0013 ausgelieferten d2v-Scripts zur Laufzeit greifen:

### Lücke A — kein `validator_id`-Slot

`shared::ColumnMeta` trägt heute `filter_id`, `editor_id`, `formatter_id`
(`shared/src/lib.rs:296/299/302`), **aber kein `validator_id`**. Dadurch konnte
der in Q0013 gelieferte `d2v_balance_validator` (Validator-Slot, ComputeOnly +
ReadI18n; `examples/d2v/scripts/d2v_balance_validator.{rhai,manifest.json}`) nur
als *ladbar + engine-getestet* shippen — die Spalten-/Settings-Verdrahtung fehlt.
DoD: `validator_id: Option<String>` additiv in `ColumnMeta` (und passendem
`EntitySettings`-Pendant) ergänzen, Loader/Resolver/GraphQL-Re-Wrap mitziehen,
Wire-Format-Pin im `shared/tests/` aktualisieren, im Validierungs-Pfad
auflösen.

### Lücke B — `script:`-Prefix wird im Filter-Pfad nicht konsumiert

Q0013 hat `"filterId": "script:d2v_stack_filter"` auf `datev_entry`-`stackId`
verdrahtet, aber der Client-Provider-Lookup
(`client/src/components/registries/resolve.rs:27` → `column.filter_id.clone()`)
behandelt den `script:`-Prefix nicht — die Metadaten sind „dormant".
DoD: Filter-Provider-Resolver erkennt den `script:<id>`-Prefix und lädt das
Script statt eines benannten Built-in-Ops; Test, der den End-to-End-Filterpfad
mit `script:`-Prefix abdeckt.

### Akzeptanzkriterien (skizziert, im Brainstorm/Plan zu schärfen)

- `validator_id` additiv (kein Wire-Break; `DATA_DIR_FORMAT` bleibt unverändert,
  da additiv — vgl. CLAUDE.md-Konvention).
- d2v `balance_validator` greift über `validator_id` zur Laufzeit.
- d2v `stack_filter` greift über `filterId: script:…` zur Laufzeit.
- Beide Seiten (server-Re-Wrap + client-Deser) konsistent; Wire-Pin grün.

### Referenzen

- `docs/queue/done/Q0013-minimale-livable-d2v2019-teilmenge.md` — Quell-Item.
- `docs/reviews/Q0013-review.md`, `docs/reviews/Q0013-security-review.md`.
- `shared/src/lib.rs:273-302` (Implementations-ID-Slots).
- `client/src/components/registries/resolve.rs` (Provider-Resolver).
- `examples/d2v/scripts/d2v_balance_validator.*`, `examples/d2v/scripts/d2v_stack_filter.*`.

## Log
- 2026-05-30T20:01:01Z — manual: created (Stage-2-Folgeitem aus Q0013; verifizierte Lücken A validator_id-Slot fehlt in shared/src/lib.rs, B script:-Prefix in resolve.rs:27 unbehandelt)
- 2026-05-30T21:22:24Z — ccm-brainstorm: status new → brainstormed, spec=docs/superpowers/specs/Q0014-validator-id-slot-und-script-prefix-filterpfad-design.md; security_review.required=true (Trigger: script). Locked Scope (User 2026-05-30): A=cut-line-C (validator_id additiv + GraphQL/Client-Plumbing + neuer ColumnMeta-Wire-Pin + Script-Validator live via echtem ValidationSystem::run, editor.rs-Hookup=Folgeitem), B=Resolver+minimaler LocalSource-Prädikat (script:-Branch in LocalSource::passes + filters/mod.rs script:-Guard, E2E-Test mit d2v_stack_filter). DATA_DIR_FORMAT bleibt 1. Refinement: LocalSource existiert bereits (data_source.rs:190-316), Lücke B ist nur ein script:-Branch.
- 2026-05-30T21:30:53Z — ccm-plan: status brainstormed → planned, plan=docs/superpowers/plans/Q0014-validator-id-slot-und-script-prefix-filterpfad.md (9-Task-TDD). Anker bestätigt; 2 Implementierungs-Divergenzen eingebaut: (1) ScriptRegistry nicht Clone → Arc<ScriptRegistry> in LocalSource; (2) MockHostApi testing-gated → Arc<dyn HostApi>-Konstruktor-Injection (Prod: RenderHost). Client-Test-Harness = native #[test] in client/tests/ (wie script_provider_lookup.rs), +futures dev-dep für block_on; neuer Message-Key SCRIPT_VALIDATION_KEY.
- 2026-05-30T21:31:32Z — ccm-execute: status planned → executing (pre-approved via 'alle drei der Reihe nach')
- 2026-05-30T22:12:15Z — ccm-execute: status executing → done, final_sha=0774e1b (9 Tasks TDD, 9 Commits e8bd9f7..0774e1b). Lücke A live via echtem ValidationSystem::run, Lücke B live via LocalSource::passes script:-Prädikat — beide durch Integration-Tests mit echtem Rhai-Engine bewiesen. Neuer ColumnMeta-Wire-Pin (column_meta_wire_format.rs, 5 Tests), DATA_DIR_FORMAT bleibt 1. Verify grün: fmt/clippy(3 Crates)/shared+client+server-Tests. — awaiting review
- 2026-05-30T22:17:52Z — ccm-review: status done → reviewed (approved by claude), notes=docs/reviews/Q0014-review.md (0 blocking; 2 non-blocking out-of-scope: (1) selectedStackId via from_f64 erreicht Rhai als FLOAT, INT-Row coerced — heute korrekt, künftig härten; (2) global_search-Loop führt script:-Filter-Spalte noch durch ops_for_named, benign). Reviewer re-ran LocalSource-Test (2 passed).
