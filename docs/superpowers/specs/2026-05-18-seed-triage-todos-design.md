# Design — Seed initial triage TODOs from ROADMAP.md

**Date:** 2026-05-18
**Status:** approved (pending review)
**Owner:** solo (Joscha Zeier)

## Goal

Populate `docs/triage/` with one-shot `T*.md` files so that `/dev-executor`
has a real backlog drawn from the project's existing roadmap, rather
than running on empty.

## Source of truth

`ROADMAP.md` (1083 lines). Each "Arbeitspaket" table row in a phase
section is a candidate. An item is **open** when:

- Its status column shows `offen`, OR
- It has no status column at all (Phase 1.5, 1.7, 3, 4 use that shape), AND
- The git log shows no commit closing it.

Completed items (`✅` mark or matching git-log entry like
`6699f01 Phase 1.5.4 + 1.5.5`) are excluded.

Phases 5–11 are explicitly **out of scope** — they are strategic
sketches without concrete file references, not actionable for an
executor.

## Item inventory

Plan: 1 smoke-test + 54 roadmap-derived items = **55 triage files total**.

### Smoke test (T0001)

Trivial doc-only change: append a one-line cross-reference to
`docs/triage/README.md` pointing at `ROADMAP.md` as the seed source.
Purpose: exercise the full executor pipeline (start-commit, body edit,
test run, done-commit, file move into `done/`) on a change that
cannot break `cargo test`.

### Roadmap-derived (T0002–T0055)

| T-ID  | Roadmap | Size | Phase | Touches (primary)                                          |
|-------|---------|------|-------|------------------------------------------------------------|
| T0002 | 0.7.2   | M    | Auth  | `server/src/entity/{permission,role,role_assignment}.rs`   |
| T0003 | 0.7.3   | M    | Auth  | `server/src/auth/resolver.rs` (new)                        |
| T0004 | 0.7.4   | M    | Auth  | `server/src/schema.rs`                                     |
| T0005 | 0.7.6   | S    | Auth  | `server/src/audit.rs` (new)                                |
| T0006 | 0.7.7   | S    | Auth  | `server/src/schema.rs` (debug endpoint)                    |
| T0007 | 0.7.8   | S    | Auth  | `cli/src/cmd/migrate_security.rs` (new)                    |
| T0008 | 1.5.1   | M    | Impl  | `shared/src/settings.rs`, `server/src/example/loader.rs`   |
| T0009 | 1.5.2   | S    | Impl  | `shared/src/lib.rs`, `server/src/schema.rs`                |
| T0010 | 1.5.3   | M    | Impl  | `server/src/entity/`, `server/src/auth/`                   |
| T0011 | 1.5.6   | M    | Impl  | `shared/tests/`, `server/tests/`                           |
| T0012 | 1.6     | M    | Build | `server/src/schema.rs`, `cli/src/cmd/design.rs` (new)      |
| T0013 | 1.7.1   | M    | ERP-A | `server/src/sequences/` (new)                              |
| T0014 | 1.7.2   | M    | ERP-A | `server/src/fx/` (new)                                     |
| T0015 | 1.7.3   | M    | ERP-A | `server/src/schema.rs`, `period_locks` table               |
| T0016 | 1.7.4   | M    | ERP-A | `shared/src/settings.rs`, `server/src/schema.rs`           |
| T0017 | 1.7.5   | L    | ERP-B | `shared/src/state_machine.rs`, `server/src/sm/`            |
| T0018 | 1.7.6   | M    | ERP-B | `server/src/approval/` (new)                               |
| T0019 | 1.7.7   | L    | ERP-B | `server/src/jobs/` (new)                                   |
| T0020 | 1.7.8   | M    | ERP-C | `server/src/storage/` (new)                                |
| T0021 | 1.7.9   | M    | ERP-C | `server/src/pdf/` (new)                                    |
| T0022 | 1.7.10  | M    | ERP-C | `server/src/email/` (new)                                  |
| T0023 | 1.7.11  | M    | ERP-C | `server/src/signing/` (new)                                |
| T0024 | 1.7.12  | M    | ERP-D | `server/src/schema.rs` (aggregation resolver)              |
| T0025 | 1.7.13  | M    | ERP-D | `server/src/search/` (new)                                 |
| T0026 | 1.7.14  | S    | ERP-D | `server/src/export/` (new)                                 |
| T0027 | 1.7.15  | M    | ERP-D | `server/src/import/` (new)                                 |
| T0028 | 1.7.16  | M    | ERP-E | `server/src/gdpr/` (new)                                   |
| T0029 | 1.7.17  | M    | ERP-E | `server/Cargo.toml`, `shared/src/lib.rs`                   |
| T0030 | 1.7.18  | M    | ERP-E | archive path on top of 1.7.8/1.7.9                         |
| T0031 | 1.7.19  | M    | ERP-F | `server/src/webhooks/` (new)                               |
| T0032 | 1.7.20  | M    | ERP-F | `server/src/data.rs`, `shared/src/lib.rs` (Tree FieldType) |
| T0033 | 1.7.21  | M    | ERP-F | `server/src/rest/` (new)                                   |
| T0034 | 2.4     | M    | Plug  | `server/src/plugins/host_functions.rs` (new)               |
| T0035 | 2.5     | S    | Plug  | `server/src/entity/plugin.rs`                              |
| T0036 | 2.6     | M    | Plug  | `server/src/schema.rs`, `client/src/routes/plugins.rs`     |
| T0037 | 2.7     | S    | Plug  | `examples/plugins/` (new)                                  |
| T0038 | 2.8     | S    | Plug  | `server/src/entity/plugin_invocation.rs`                   |
| T0039 | 3.1     | S    | AI    | `server/src/ai/mod.rs` (new)                               |
| T0040 | 3.2     | M    | AI    | `shared/src/migration.rs` (new)                            |
| T0041 | 3.3     | M    | AI    | `server/src/ai/schema_proposer.rs` (new)                   |
| T0042 | 3.4     | M    | AI    | builds on 3.3                                              |
| T0043 | 3.5     | M    | AI    | `server/src/ai/validator.rs`, `Cargo.toml`                 |
| T0044 | 3.6     | M    | AI    | `client/src/routes/migrations.rs` (new)                    |
| T0045 | 3.7     | L    | AI    | `server/src/migrations/` (new)                             |
| T0046 | 3.8     | S    | AI    | `server/src/db.rs`                                         |
| T0047 | 3.9     | S    | AI    | `server/src/schema.rs` (mutation)                          |
| T0048 | 4.1     | L    | Gen   | `codegen/` (new)                                           |
| T0049 | 4.2     | XL   | Gen   | builds on 4.1                                              |
| T0050 | 4.3     | S    | Gen   | builds on 4.1                                              |
| T0051 | 4.4     | M    | Gen   | `cli/`                                                     |
| T0052 | 4.5     | M    | Gen   | `server/src/plugins/`                                      |
| T0053 | 4.6     | -    | Gen   | external audit; informational T-file only                  |
| T0054 | 4.7     | M    | Gen   | `benches/` (new)                                           |
| T0055 | 4.8     | M    | Gen   | `docs/`                                                    |

## Mapping rules

### Severity ← Size

| Roadmap size | Triage severity |
|--------------|-----------------|
| S            | low             |
| M            | medium          |
| L            | high            |
| XL           | high            |
| smoke test   | crash           |

Rationale: severity here is a proxy for the dev-executor's priority
sort (`crash > high > medium > low`), not user-facing impact. ROADMAP
size encodes risk/duration; the `crash` slot is hijacked for the
smoke test only, so it sorts to position 1 and validates the executor
pipeline before any real-work item runs. After T0001 completes, sort
naturally falls back to the size-derived ordering.

### Triage file template

```yaml
---
id: T<NNNN>
created: 2026-05-18
source: manual
severity: <mapped from size>
status: open
parallel: false
touches:
  - <comma-split paths from Bezug column>
artifacts:
  - ROADMAP.md#<roadmap-id>
issue: null
---

# <roadmap-id> — <title from Paket column, German preserved>

## Diagnose

Verbatim copy of the Roadmap `Paket` description, plus any
phase-level context (Ziel, Status quo) that bears on this item.
Cross-reference dependencies inline: `# needs T<NNNN>` (parsed by
dev-executor) per the dependency-mapping rule below.

## Repro

Not applicable — feature work, no bug repro path.

## Vorschlag

Verbatim copy of the Roadmap `Akzeptanz` column. This is the executor's
acceptance gate. The executor's `Vorschlag` step should treat the
Akzeptanz line as the contract.

## Log

- 2026-05-18 — manual: seeded from ROADMAP.md §<roadmap-id> (size=<S|M|L|XL>)
```

### Dependency encoding

Use `# needs T<NNNN>` lines in the **Diagnose** section (per the
dev-executor skill, which parses those). Mapped dependencies:

| T-ID  | Needs                   | Reason                                  |
|-------|-------------------------|-----------------------------------------|
| T0003 | T0002                   | 0.7.3 needs 0.7.2 (table schema)        |
| T0004 | T0003                   | 0.7.4 needs 0.7.3 (resolver)            |
| T0006 | T0003                   | 0.7.7 reads from resolver               |
| T0007 | T0002                   | migration writes into 0.7.2 schema      |
| T0010 | T0002                   | needs auth schema for permissions on    |
|       |                         | implementation choice                   |
| T0016 | T0004                   | append-only check rides on enforcement  |
| T0017 | T0004                   | transition permissions need 0.7.4       |
| T0018 | T0017                   | approvals build on state-machine        |
| T0023 | T0021, T0022            | signing extends PDF + email             |
| T0026 | T0004                   | export permission-gated                 |
| T0030 | T0020, T0021            | archive composes storage + pdf          |
| T0034 | T0002                   | host capability gate uses auth          |
| T0036 | T0035                   | upload mutation needs `plugins` table   |
| T0040 | (none — schema-only)    |                                         |
| T0041 | T0040                   | proposer emits the schema               |
| T0042 | T0041                   | function-call loop wraps proposer       |
| T0043 | T0040                   | validator type-checks proposals         |
| T0044 | T0043                   | UI shows diff from validator output     |
| T0045 | T0043, T0046            | executes validated proposals + snapshot |
| T0047 | T0041, T0042            | endpoint triggers proposer + loop       |
| T0049 | T0048                   | templates depend on the codegen crate   |
| T0050 | T0048                   | scaffold uses the codegen crate         |
| T0051 | T0048                   | CLI wraps codegen                       |
| T0054 | most of 1, 2, 3         | benchmarks need realistic features      |
| T0055 | most of 1, 2, 3, 4      | docs document the finished platform     |

Items not listed have no hard dependency.

### `touches:` mapping

Pull from the ROADMAP `Bezug` column. Multiple paths comma-separated
in the source → YAML list. Existing paths only (verified via `Glob`
during implementation); paths suffixed `(new)` are emitted as the
intended new path.

### Severity escalation for blockers

T0048 (4.1, codegen crate, L → high) and T0049 (4.2, codegen templates,
XL → high) both block multiple downstream items. **Not** escalated
beyond `high` — the dev-executor's priority sort already biases toward
items that unblock more work via dependency-count, so they will surface
naturally.

## Implementation outline

The actual write-out is a single batch:

1. Create T0001 (smoke test) verbatim.
2. For each of T0002–T0055: read the roadmap row, render the
   template, write to `docs/triage/T<NNNN>-<short-slug>.md`.
3. Update `.claude/triage-monitor-state.json`: set
   `next_triage_id = 56`.
4. Single commit on `dev`:
   `chore(triage): seed 55 initial TODOs from ROADMAP.md`.

The slug for each filename is the kebab-cased English summary of the
Roadmap title (e.g. `T0013-number-sequence-service.md`). Filenames
stay ASCII even though the bodies contain German — matches the dev-
executor's glob pattern (`T*.md`) and avoids Windows path issues.

## Out of scope

- **Phases 5–11**: strategic sketches only, no concrete file refs.
  Re-evaluate when one is promoted into the planned roadmap.
- **Refactoring or splitting roadmap items**: every T-file maps 1:1 to
  a roadmap row. If a row is too large for the executor to handle in
  one go, that's the executor's problem to surface — not the seeder's
  to second-guess.
- **Reordering / re-prioritising**: T-IDs are assigned in roadmap
  reading order. The dev-executor's priority sort handles runtime
  ordering.
- **GitHub issue mirror**: `triage.output_mode = "files"` in config —
  no `gh issue create` calls. Switch later if useful.
- **Pushing the seed commit**: leave it local on `dev`. Push remains a
  deliberate user action.

## Acceptance criteria

- 55 files exist under `docs/triage/`, named `T0001-…` through `T0055-…`.
- `git status` clean after the seed commit.
- `cargo test --workspace` still green (smoke test verifies executor
  pipeline doesn't break the build).
- `.claude/triage-monitor-state.json` has `next_triage_id: 56`.
- `dev-executor` invoked manually picks T0001 first (severity=`crash`
  outranks all other items in the priority sort).
