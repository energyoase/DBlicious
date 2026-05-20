---
id: Q0001
created: 2026-05-20T00:00:00Z
status: done
priority: high
title: "CCM-Plugin lokale Discovery in Claude Code verifizieren"
spec: docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md
plan: docs/superpowers/plans/Q0001-ccm-plugin-lokale-discovery-in-claude.md
pending_question_id: null
resume_step: null
parent: null
artifacts:
  - docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md
  - docs/superpowers/plans/Q0001-ccm-plugin-lokale-discovery-in-claude.md
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
---

## Description

Vorarbeit fuer Milestone M1 der CCM-Approvals-Ready-Spec
([2026-05-20-ccm-approvals-ready-design.md](../superpowers/specs/2026-05-20-ccm-approvals-ready-design.md), Abschnitt D2).

**Frage:** Welche Mechanik in Claude-Code's `.claude/settings.json` laesst es
zu, ein Plugin **lokal** (kein Marketplace) zu installieren? Drei Hypothesen
laut Spec:

1. Eine Source-Definition unter `settings.json.plugins.sources.<name>.path`
2. Symlink `~/.claude/plugins/ccm/` -> CCM-Repo, dann Eintrag `"ccm@local": true`
3. Keine lokale Discovery moeglich — Marketplace-Pflicht oder Skills-only-Pfad

**Akzeptanzkriterien:**
- Eines der drei Szenarien ist praktisch verifiziert (durch Setup + Restart der Claude-Code-Session + Beobachtung).
- Bei Erfolg: Dokumentation der genauen Setup-Schritte in der Spec.
- Bei Misserfolg (Hypothese 3): Spec-Update mit Plan-B-Bestaetigung — Skills-Symlinks bleiben dauerhaft, kein Plugin.

**Bezug zur CCM-Pipeline:** Dieses Item ist der Lakmus-Test, ob der Q-Lifecycle in DBlicious ueberhaupt funktioniert. Wenn `/ccm-brainstorm Q0001` aus einer frischen Claude-Code-Session heraus diesen Body lesen, eine Spec erzeugen und den Status auf `brainstormed` setzen kann, ist M0 erfolgreich abgeschlossen.

## Log
- 2026-05-20T00:00:00Z — manual: created (M0-Bootstrap)
- 2026-05-20T22:22:00Z — ccm-brainstorm: status new → brainstormed, spec=docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md (commit 3200372)
- 2026-05-20T22:31:00Z — ccm-plan: status brainstormed → planned, plan=docs/superpowers/plans/Q0001-ccm-plugin-lokale-discovery-in-claude.md (commit 2588f4d)
- 2026-05-20T22:35:00Z — ccm-execute: status planned → executing, branch=dev (note: plan refers to feat/phase-0.6-source-architecture — überschrieben durch ccm-execute Branch-Policy)
- 2026-05-20T22:47:00Z — ccm-execute sub-agent: BLOCKED at Task 1 Step 1.4 (user-restart required). Phase-0 done + H2 junction created + settings.json edited (uncommitted). Backup at .claude/settings.json.bak-q0001. Phase-0-Befund committed f28f4bb. Phase-0-Finding: docs nennen `extraKnownMarketplaces`, nicht `plugins.sources` — H1 muss entsprechend angepasst werden.
- 2026-05-21T21:30:00Z — Resume nach Claude-Code-Restart. Beobachtung H2: V2.1 PASS (Schema akzeptiert ccm@local), V2.2 INKONKLUSIV (Skills aus M0-Junctions, nicht eindeutig), V2.3 FAIL (`.ccm/audit.log` mtime unverändert → SessionStart-Hook hat NICHT gefeuert; Hook manuell ausgeführt funktioniert). Root-Cause: Plugin-Discovery braucht Marketplace-Registry in `~/.claude/plugins/known_marketplaces.json` + Plugin-Entry in `installed_plugins.json` + Cache-Layout `cache/<marketplace>/<plugin>/<version>/`. H4-Hypothese formuliert (M1-Tooling-Track). H3 (Plan-B = Skills-only) bestätigt. Cleanup: ccm@local-Entry aus settings.json entfernt, Junction `~/.claude/plugins/ccm/` entfernt, Backup-File gelöscht.
- 2026-05-21T21:35:00Z — ccm-execute: status executing → done. Parent-Spec D2 + M1-AC + Risiken-Tabelle aktualisiert. CLAUDE.md mit Skills-only-Hinweis ergänzt. Spec §8 (Setup-Rezept/Negativ-Befund) ausgefüllt.
