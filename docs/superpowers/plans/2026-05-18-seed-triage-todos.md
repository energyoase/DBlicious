# Seed Initial Triage TODOs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate 55 triage TODO files in `docs/triage/` (1 forced-first smoke test + 54 ROADMAP-derived items) so `/dev-executor` has a real backlog.

**Architecture:** Single batch operation. Render a fixed YAML-frontmatter + Markdown template once per item, with field values extracted from `ROADMAP.md` tables per a documented mapping. No code changes; only new files in `docs/triage/` plus a state-file bump. One commit on the `dev` branch.

**Tech Stack:** Markdown + YAML frontmatter. No compilation, no test fixtures. Doc-only — `cargo test --workspace` must stay green as a sanity check that nothing was accidentally written outside `docs/`.

**Reference docs (read before starting):**
- Spec: `docs/superpowers/specs/2026-05-18-seed-triage-todos-design.md` (item inventory, severity/dependency mappings)
- Source: `ROADMAP.md` (the work-package tables)
- Format owner: `~/.claude/skills/triage-monitor/SKILL.md` (triage file template)
- Consumer: `~/.claude/skills/dev-executor/SKILL.md` (priority sort, dependency parser)

**Pre-flight assumptions:**
- Current branch is `dev` (executor's commit branch).
- Working tree clean except for the still-uncommitted `.claude/settings.local.json` from the previous session — leave it alone, this plan does not touch settings.
- `docs/triage/` exists and contains only `README.md` + `done/.gitkeep`.
- `.claude/triage-monitor-state.json` has `next_triage_id: 1`.

---

## File Structure

**Created (55 files):**

```
docs/triage/T0001-smoke-test-readme-crossref.md
docs/triage/T0002-permissions-roles-tables-loader.md
docs/triage/T0003-server-effective-resolver.md
docs/triage/T0004-crud-permission-enforcement.md
docs/triage/T0005-audit-log.md
docs/triage/T0006-why-allowed-debug-endpoint.md
docs/triage/T0007-migrate-security-cli.md
docs/triage/T0008-field-type-defaults-settings.md
docs/triage/T0009-columnmeta-impl-id-fields.md
docs/triage/T0010-per-user-impl-resolver.md
docs/triage/T0011-impl-resolution-tests.md
docs/triage/T0012-builder-design-persistence.md
docs/triage/T0013-number-sequence-service.md
docs/triage/T0014-fx-rate-store.md
docs/triage/T0015-period-locks.md
docs/triage/T0016-gobd-append-only.md
docs/triage/T0017-state-machine-engine.md
docs/triage/T0018-approval-workflow.md
docs/triage/T0019-background-job-scheduler.md
docs/triage/T0020-file-storage-abstraction.md
docs/triage/T0021-pdf-generation.md
docs/triage/T0022-email-send.md
docs/triage/T0023-digital-signature.md
docs/triage/T0024-aggregation-queries.md
docs/triage/T0025-fulltext-search.md
docs/triage/T0026-excel-csv-export.md
docs/triage/T0027-bulk-import.md
docs/triage/T0028-dsgvo-tooling.md
docs/triage/T0029-encryption-at-rest.md
docs/triage/T0030-long-term-archive.md
docs/triage/T0031-webhooks.md
docs/triage/T0032-hierarchy-tree-helper.md
docs/triage/T0033-openapi-rest-adapter.md
docs/triage/T0034-plugin-host-functions.md
docs/triage/T0035-plugin-storage-table.md
docs/triage/T0036-plugin-upload-mutation.md
docs/triage/T0037-example-plugins.md
docs/triage/T0038-plugin-invocation-audit.md
docs/triage/T0039-ai-client-abstraction.md
docs/triage/T0040-migration-proposal-schema.md
docs/triage/T0041-rag-schema-proposer.md
docs/triage/T0042-function-calling-loop.md
docs/triage/T0043-rusty-schema-diff.md
docs/triage/T0044-migration-approval-ui.md
docs/triage/T0045-two-phase-migration.md
docs/triage/T0046-sqlite-snapshot.md
docs/triage/T0047-json-import-endpoint.md
docs/triage/T0048-ast-codegen-crate.md
docs/triage/T0049-codegen-templates.md
docs/triage/T0050-cargo-scaffold-integration.md
docs/triage/T0051-cli-export-command.md
docs/triage/T0052-wasi-nn-evaluation.md
docs/triage/T0053-external-security-audit.md
docs/triage/T0054-performance-benchmarks.md
docs/triage/T0055-documentation.md
```

**Modified:**
- `.claude/triage-monitor-state.json` (bump `next_triage_id: 1` → `next_triage_id: 56`)

**Untouched:** all crate code, all existing docs, `.claude/settings.{json,local.json}`, `.gitignore`, `docs/triage/README.md`, `docs/triage/done/.gitkeep`.

---

## Common rendering rules

### YAML frontmatter

For every file:

```yaml
---
id: T<NNNN>
created: 2026-05-18
source: manual
severity: <crash | high | medium | low>     # see mapping below
status: open
parallel: false
touches:
  - <path>                                   # see touches mapping
artifacts:
  - ROADMAP.md#<roadmap-id>                  # e.g. ROADMAP.md#0.7.2
issue: null
---
```

### Severity from ROADMAP size

| Roadmap size | Triage severity |
|--------------|-----------------|
| S            | `low`           |
| M            | `medium`        |
| L            | `high`          |
| XL           | `high`          |
| smoke (T0001 only) | `crash`   |

### Body sections

```markdown
# <roadmap-id> — <Paket-column title, German preserved>

## Diagnose

<Verbatim text from the "Paket" column. If the phase has additional
relevant context — e.g. Phase 0.7's "Target-Modell" or "Granularitaet"
bullet that this item depends on — paraphrase it in one sentence and
cite via `siehe ROADMAP.md §<phase-id>`.>

<Dependency lines, if any, on their own line:>
# needs T<NNNN>
# needs T<MMMM>

## Repro

Not applicable — feature work, no bug repro path.

## Vorschlag

<Verbatim text from the "Akzeptanz" column. This is the executor's
acceptance contract.>

## Log

- 2026-05-18 — manual: seeded from ROADMAP.md §<roadmap-id> (size=<S|M|L|XL>)
```

### Per-item data

The authoritative per-item mapping (roadmap-id, slug, size, touches, deps) is the table in `docs/superpowers/specs/2026-05-18-seed-triage-todos-design.md` § "Item inventory" and § "Dependency encoding". The spec's table replaces a duplicated table in this plan — if anything contradicts, the spec wins.

For each T-file:
1. Find the matching ROADMAP row by `<roadmap-id>` (e.g. `0.7.2`).
2. Copy the `Paket` cell verbatim into Diagnose, and `Akzeptanz` cell verbatim into Vorschlag.
3. Take `touches:` from the spec table (the spec already pre-extracted from `Bezug`).
4. Take `severity:` from the size→severity mapping above.
5. Take dependencies from the spec's dependency-mapping table; render as `# needs T<NNNN>` lines at the bottom of Diagnose.

**Special case — T0053 (4.6 external audit):** The ROADMAP "Größe" column has `(extern)` and the work isn't code. Render with `severity: high`, `touches: []`, Diagnose copies the line verbatim, Vorschlag says: `Externer Sicherheits-Audit der WASM-Sandbox und der AI-Pfade. Audit-Bericht liegt vor.` No code change expected; this T-file is informational and `/dev-executor` will likely either skip it (no `touches` for sub-agent disjointness) or close it as a no-op once the audit happens externally.

---

### Task 1: Pre-flight verification

**Files:**
- Read: `docs/superpowers/specs/2026-05-18-seed-triage-todos-design.md`
- Read: `ROADMAP.md`
- Read: `.claude/triage-monitor-state.json`

- [ ] **Step 1: Confirm working branch is `dev`**

Run: `git branch --show-current`
Expected: `dev`

If output is anything else: STOP. The seed must commit on `dev`. Switch with `git switch dev` and re-run.

- [ ] **Step 2: Confirm `docs/triage/` is empty of T-files**

Run: `ls docs/triage/`
Expected: `README.md  done`

If any `T*.md` exists: STOP. Either this plan already ran (check `git log --oneline -- docs/triage/`) or a manual file is present that needs human review.

- [ ] **Step 3: Confirm state file starts at 1**

Run: `cat .claude/triage-monitor-state.json`
Expected: `"next_triage_id": 1` somewhere in the JSON.

If it's already > 1: STOP. The state file is out of sync with the filesystem; investigate before proceeding.

- [ ] **Step 4: Read the spec once and note the inventory table**

The spec contains the canonical T-id → roadmap-id → slug → size → touches mapping. Have it open during Tasks 2 and 3.

---

### Task 2: Write the smoke test (T0001)

**Files:**
- Create: `docs/triage/T0001-smoke-test-readme-crossref.md`

- [ ] **Step 1: Write the smoke test triage file with this exact content:**

````markdown
---
id: T0001
created: 2026-05-18
source: manual
severity: crash
status: open
parallel: false
touches:
  - docs/triage/README.md
artifacts:
  - docs/superpowers/specs/2026-05-18-seed-triage-todos-design.md
issue: null
---

# Smoke test — README cross-reference

## Diagnose

This is the first triage item processed by `/dev-executor` after the
initial seeding. Its purpose is to validate the executor pipeline end
to end on a trivial doc-only change:

- start-commit ("chore(triage): start T0001")
- file edit
- test run (`cargo test --workspace` — no code changed, must pass)
- close-commit ("chore(triage): close T0001")
- `git mv` to `docs/triage/done/`

Severity `crash` is a priority-sort hack — there is no actual crash;
it forces this item to position 1 in the executor's queue. After it
closes, sort naturally falls back to the size-derived ordering of the
real backlog.

## Repro

Not applicable — meta-task, no bug.

## Vorschlag

Append one line to `docs/triage/README.md` directly under the existing
"Open items live here…" paragraph:

```
Initial backlog seeded 2026-05-18 from ROADMAP.md (see
`docs/superpowers/specs/2026-05-18-seed-triage-todos-design.md`).
```

The text is English (the README is English already). German would
require also touching surrounding tone — out of scope for a smoke test.

Acceptance:
- `docs/triage/README.md` has the new line.
- `cargo test --workspace` exits 0.
- This T-file is moved to `docs/triage/done/T0001-smoke-test-readme-crossref.md`.

## Log

- 2026-05-18 — manual: seeded as smoke test (size=trivial, severity hijacked for priority)
````

- [ ] **Step 2: Verify the file exists and is valid**

Run: `ls docs/triage/T0001-smoke-test-readme-crossref.md`
Expected: path prints.

Run: `head -5 docs/triage/T0001-smoke-test-readme-crossref.md`
Expected: starts with `---` and `id: T0001`.

---

### Task 3: Write the roadmap-derived files (T0002–T0055)

**Files:**
- Create: 54 files matching the names in the File Structure section above.

This task generates 54 files using the common rendering rules. Two worked examples appear in full below to anchor the format. The remaining 52 files follow the same shape with data pulled from the spec's inventory table and the corresponding ROADMAP.md row.

- [ ] **Step 1: Read the worked examples below**

**Worked example A — T0002 (size M, has no dependencies):**

ROADMAP row reference: `ROADMAP.md` § 0.7 table, row `0.7.2`:
- Paket: `permissions-Tabelle (mit tenant_id NULL) + roles + role_assignments (SeaORM) + Loader-Format security/{permissions,roles,role_assignments}.{toml,json}`
- Größe: `M`
- Bezug: `server/src/entity/{permission,role,role_assignment}.rs, server/src/example/loader.rs`
- Akzeptanz: `Loader-Roundtrip-Test gruen; Role-Zuweisungen an User UND Group werden gelesen`

Rendered file `docs/triage/T0002-permissions-roles-tables-loader.md`:

````markdown
---
id: T0002
created: 2026-05-18
source: manual
severity: medium
status: open
parallel: false
touches:
  - server/src/entity/permission.rs
  - server/src/entity/role.rs
  - server/src/entity/role_assignment.rs
  - server/src/example/loader.rs
artifacts:
  - ROADMAP.md#0.7.2
issue: null
---

# 0.7.2 — permissions-Tabelle + roles + role_assignments (SeaORM) + Loader

## Diagnose

permissions-Tabelle (mit tenant_id NULL) + roles + role_assignments
(SeaORM) + Loader-Format security/{permissions,roles,role_assignments}.{toml,json}.

Target-Modell und Persistenz-Schema sind in ROADMAP.md §0.7 oben definiert
(Subject = User|Group|Role, flache `permissions`-Tabelle mit Priority,
`tenant_id NULL` für Single-Tenant-Default). Phase 0.7 enforced
zunächst nur `EntityType` und `EntityProperty` — `EntityInstance`
(Row-Level) wirft `not_implemented`.

## Repro

Not applicable — feature work, no bug repro path.

## Vorschlag

Loader-Roundtrip-Test gruen; Role-Zuweisungen an User UND Group werden gelesen.

## Log

- 2026-05-18 — manual: seeded from ROADMAP.md §0.7.2 (size=M)
````

**Worked example B — T0004 (size M, has dependency on T0003):**

ROADMAP row: `0.7.4` — `Enforcement in CRUD-Resolvern + GraphQL-Mutation-Schicht`, size M, Bezug `server/src/schema.rs`, Akzeptanz `Negative Tests: nicht autorisierter Aufruf → Error-Code forbidden`.

Per the spec's dependency table, T0004 needs T0003.

Rendered file `docs/triage/T0004-crud-permission-enforcement.md`:

````markdown
---
id: T0004
created: 2026-05-18
source: manual
severity: medium
status: open
parallel: false
touches:
  - server/src/schema.rs
artifacts:
  - ROADMAP.md#0.7.4
issue: null
---

# 0.7.4 — Enforcement in CRUD-Resolvern + GraphQL-Mutation-Schicht

## Diagnose

Enforcement in CRUD-Resolvern + GraphQL-Mutation-Schicht.

Setzt auf den Server-Resolver `effective(user, resource, op)` aus
0.7.3 auf; nutzt das `Op`-Enum aus 0.7.1. Erlaubt erst, wenn die
Resolver-Antwort `Allow` ist — sonst GraphQL-Error mit
`extensions.code = "forbidden"`.

# needs T0003

## Repro

Not applicable — feature work, no bug repro path.

## Vorschlag

Negative Tests: nicht autorisierter Aufruf → Error-Code forbidden.

## Log

- 2026-05-18 — manual: seeded from ROADMAP.md §0.7.4 (size=M)
````

Notice:
- Touches contains exactly the paths from `Bezug`, one per YAML list item, comma-split.
- The `# needs T<NNNN>` line is on its own line at the bottom of Diagnose.
- Diagnose's first paragraph is verbatim from `Paket`; the second is one short context sentence (cross-reference). For items with no useful cross-reference, omit the second paragraph entirely.
- Vorschlag is verbatim from `Akzeptanz`.

- [ ] **Step 2: Render T0003 and T0005 through T0055 following the same procedure**

For each row, repeat:
1. Open the matching ROADMAP row.
2. Copy `Paket` text verbatim → first paragraph of Diagnose.
3. (Optional) add one sentence of phase-level context if the Paket text references concepts defined elsewhere in the phase (e.g. "Target-Modell", "Resolution-Prioritaet", "MigrationProposal"). Skip if the Paket is self-contained.
4. Add `# needs T<NNNN>` line(s) from the spec's dependency table.
5. Copy `Akzeptanz` text verbatim → Vorschlag.
6. Render the YAML frontmatter using the size→severity map and the spec's `touches` column.

**For ROADMAP rows that lack an "Akzeptanz" column** (e.g. Phase 1.5 table, Phase 2 table, Phase 3 table, Phase 4 table — these tables don't have that column), substitute with: the phase's "Deliverable" bullet that most closely matches the item, paraphrased into a single sentence. If no clear deliverable maps, write: `Akzeptanz aus ROADMAP nicht explizit — Executor entscheidet vor Beginn, was als "fertig" gilt, und schreibt es in eine Log-Zeile.`

- [ ] **Step 3: Special case T0053 (4.6 — external audit)**

Render with size mapped from `(extern)` → severity `high`, `touches: []`, Vorschlag:

```
Externer Sicherheits-Audit der WASM-Sandbox und der AI-Pfade.
Audit-Bericht liegt vor.
```

Note in Diagnose: `Dieses Item ist informationell — kein Code-Change. /dev-executor wird es vermutlich überspringen oder als no-op schliessen, sobald der externe Audit stattgefunden hat.`

- [ ] **Step 4: Verify file count**

Run: `ls docs/triage/T*.md | wc -l`
Expected: `55`

Run: `ls docs/triage/T*.md | head -3; echo ---; ls docs/triage/T*.md | tail -3`
Expected: first three are T0001, T0002, T0003; last three are T0053, T0054, T0055.

- [ ] **Step 5: Spot-check frontmatter validity**

Run a frontmatter sanity check via python:
```
python -c "
import os, re
files = sorted(f for f in os.listdir('docs/triage') if f.startswith('T') and f.endswith('.md'))
assert len(files) == 55, f'expected 55, got {len(files)}'
for f in files:
    with open(f'docs/triage/{f}', encoding='utf-8') as fh:
        body = fh.read()
    assert body.startswith('---\n'), f'{f}: missing leading ---'
    fm_end = body.find('\n---\n', 4)
    assert fm_end > 0, f'{f}: missing trailing ---'
    fm = body[4:fm_end]
    for key in ['id:', 'created:', 'source:', 'severity:', 'status: open', 'parallel: false', 'touches:', 'artifacts:', 'issue: null']:
        assert key in fm, f'{f}: missing key {key!r}'
print('OK', len(files), 'files')
"
```
Expected last line: `OK 55 files`.

---

### Task 4: Bump the monitor state file

**Files:**
- Modify: `.claude/triage-monitor-state.json`

- [ ] **Step 1: Read current state**

Run: `cat .claude/triage-monitor-state.json`
Expected output includes `"next_triage_id": 1`.

- [ ] **Step 2: Update next_triage_id to 56**

Edit the file so its content is exactly:

```json
{
  "last_bug_zip_mtime": 0,
  "last_godot_log_offset": 0,
  "last_main_sha": "72082d186c4f12dbd08774eecc582a70eecd0ffd",
  "next_triage_id": 56
}
```

If `last_main_sha` is now different (someone pushed to main between sessions), keep the newer SHA. Only `next_triage_id` changes.

- [ ] **Step 3: Validate JSON**

Run: `python -c "import json; print(json.load(open('.claude/triage-monitor-state.json'))['next_triage_id'])"`
Expected: `56`.

---

### Task 5: Verify acceptance criteria

- [ ] **Step 1: File count matches the plan**

Run: `ls docs/triage/T*.md | wc -l`
Expected: `55`.

- [ ] **Step 2: No new files outside the planned set**

Run: `git status`
Expected (under "Untracked files"): only `docs/triage/T0001-…` through `docs/triage/T0055-…` (55 lines) plus the modified `.claude/triage-monitor-state.json` (and the pre-existing modified `.claude/settings.local.json` from before this plan started). No other changes.

- [ ] **Step 3: cargo test still green**

Run: `cargo test --workspace`

Note: Windows file-lock caveat per `CLAUDE.md` — if a local `server.exe` is running and the build fails on lock, use:

```
cargo test --target-dir target-test --workspace
```

Expected: tests pass. (Plan only added markdown + JSON, so a green pre-state must still be green; if this fails, the failure is unrelated to the seed and pre-existed.)

- [ ] **Step 4: Confirm no T-files in `done/`**

Run: `ls docs/triage/done/`
Expected: only `.gitkeep`. (The dev-executor moves files to `done/` — the seeder doesn't.)

---

### Task 6: Commit the seed

**Files:**
- All 55 new T-files plus `.claude/triage-monitor-state.json`.

Note: `.claude/triage-monitor-state.json` is **gitignored** (see `.gitignore` line `triage state files`), so it will NOT be staged. Only the 55 T-files plus any plan/spec docs not yet committed will be in the commit. This is intentional — the state file is per-machine, not version-controlled.

- [ ] **Step 1: Stage exactly the T-files**

Run:
```
git add docs/triage/T0001-smoke-test-readme-crossref.md \
        docs/triage/T0002-permissions-roles-tables-loader.md \
        docs/triage/T0003-server-effective-resolver.md \
        docs/triage/T0004-crud-permission-enforcement.md \
        docs/triage/T0005-audit-log.md \
        docs/triage/T0006-why-allowed-debug-endpoint.md \
        docs/triage/T0007-migrate-security-cli.md \
        docs/triage/T0008-field-type-defaults-settings.md \
        docs/triage/T0009-columnmeta-impl-id-fields.md \
        docs/triage/T0010-per-user-impl-resolver.md \
        docs/triage/T0011-impl-resolution-tests.md \
        docs/triage/T0012-builder-design-persistence.md \
        docs/triage/T0013-number-sequence-service.md \
        docs/triage/T0014-fx-rate-store.md \
        docs/triage/T0015-period-locks.md \
        docs/triage/T0016-gobd-append-only.md \
        docs/triage/T0017-state-machine-engine.md \
        docs/triage/T0018-approval-workflow.md \
        docs/triage/T0019-background-job-scheduler.md \
        docs/triage/T0020-file-storage-abstraction.md \
        docs/triage/T0021-pdf-generation.md \
        docs/triage/T0022-email-send.md \
        docs/triage/T0023-digital-signature.md \
        docs/triage/T0024-aggregation-queries.md \
        docs/triage/T0025-fulltext-search.md \
        docs/triage/T0026-excel-csv-export.md \
        docs/triage/T0027-bulk-import.md \
        docs/triage/T0028-dsgvo-tooling.md \
        docs/triage/T0029-encryption-at-rest.md \
        docs/triage/T0030-long-term-archive.md \
        docs/triage/T0031-webhooks.md \
        docs/triage/T0032-hierarchy-tree-helper.md \
        docs/triage/T0033-openapi-rest-adapter.md \
        docs/triage/T0034-plugin-host-functions.md \
        docs/triage/T0035-plugin-storage-table.md \
        docs/triage/T0036-plugin-upload-mutation.md \
        docs/triage/T0037-example-plugins.md \
        docs/triage/T0038-plugin-invocation-audit.md \
        docs/triage/T0039-ai-client-abstraction.md \
        docs/triage/T0040-migration-proposal-schema.md \
        docs/triage/T0041-rag-schema-proposer.md \
        docs/triage/T0042-function-calling-loop.md \
        docs/triage/T0043-rusty-schema-diff.md \
        docs/triage/T0044-migration-approval-ui.md \
        docs/triage/T0045-two-phase-migration.md \
        docs/triage/T0046-sqlite-snapshot.md \
        docs/triage/T0047-json-import-endpoint.md \
        docs/triage/T0048-ast-codegen-crate.md \
        docs/triage/T0049-codegen-templates.md \
        docs/triage/T0050-cargo-scaffold-integration.md \
        docs/triage/T0051-cli-export-command.md \
        docs/triage/T0052-wasi-nn-evaluation.md \
        docs/triage/T0053-external-security-audit.md \
        docs/triage/T0054-performance-benchmarks.md \
        docs/triage/T0055-documentation.md
```

- [ ] **Step 2: Verify staged set**

Run: `git diff --cached --stat | tail -2`
Expected: `55 files changed, <N> insertions(+)` (N is the total line count of all bodies).

Run: `git diff --cached --name-only | wc -l`
Expected: `55`.

- [ ] **Step 3: Commit**

Run:
```
git commit -m "$(cat <<'EOF'
chore(triage): seed 55 initial TODOs from ROADMAP.md

T0001 is a forced-first smoke test (severity=crash) that validates
the dev-executor pipeline on a doc-only change. T0002–T0055 derive
from ROADMAP.md open work packages spanning phases 0.7, 1.5, 1.6,
1.7, 2, 3, 4. Severity mapped from roadmap size; dependencies
encoded inline as `# needs T<NNNN>` per the dev-executor parser.

See docs/superpowers/specs/2026-05-18-seed-triage-todos-design.md.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 4: Verify commit landed cleanly**

Run: `git log -1 --stat | tail -3`
Expected: shows `55 files changed`.

Run: `git status`
Expected: working tree clean except for pre-existing `.claude/settings.local.json` modification. `.claude/triage-monitor-state.json` is gitignored — it won't appear.

- [ ] **Step 5: Do NOT push**

The seed lives on local `dev` only. Push is a deliberate user action and is explicitly out of scope for this plan.

---

## Self-Review

**Spec coverage:**
- 1 smoke test + 54 ROADMAP items = 55 files. ✓ (Task 2 + Task 3)
- Severity mapping S→low, M→medium, L/XL→high, smoke→crash. ✓ (Common rendering rules)
- Dependencies via `# needs T<NNNN>`. ✓ (Worked example B in Task 3)
- `touches:` from ROADMAP `Bezug`. ✓ (Common rendering rules + Task 3 Step 2)
- State file bump to 56. ✓ (Task 4)
- Single commit on dev. ✓ (Task 6)
- No push. ✓ (Task 6 Step 5)
- Out-of-scope items (phases 5–11, GH mirror, push, refactoring) are not in any task. ✓

**Placeholder scan:**
- Tasks 2 and 3 show full content. Task 3 Step 2 references the spec rather than re-duplicating the inventory table — acceptable because the spec lives in the same repo and is the canonical source. The Akzeptanz-substitution rule covers the "missing column" case explicitly rather than punting.
- No "TBD", "implement later", "add error handling".

**Type consistency / cross-task:**
- T-id format `T<NNNN>` consistent everywhere.
- Slug format kebab-case English consistent in File Structure and Task 6 Step 1.
- Severity values match between spec and plan.
- `# needs` line uses uppercase T per the dev-executor parser (skill says `# needs T0002`). ✓
- Smoke-test severity is `crash` in spec, plan, and template — consistent. ✓

**Spec gaps:** The spec acceptance criteria mentions `.claude/triage-monitor-state.json` has `next_triage_id: 56` — covered in Task 4. The spec mentions `cargo test --workspace` stays green — covered in Task 5. The spec mentions clean `git status` after commit — covered in Task 6 Step 4 (modulo the pre-existing settings.local.json carve-out, which the spec doesn't address but the plan does).
