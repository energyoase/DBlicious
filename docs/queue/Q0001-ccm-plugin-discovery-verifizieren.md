---
id: Q0001
created: 2026-05-20T00:00:00Z
status: new
priority: high
title: "CCM-Plugin lokale Discovery in Claude Code verifizieren"
spec: docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md
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
