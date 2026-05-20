---
id: Q0003
created: 2026-05-21T00:00:00Z
status: new
priority: high
title: "BuilderPage Auth-Gate prüft Wildcard \"*\" — sollte per-Entity prüfen (analog EditorPage)"
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
type: bug
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
  - auth
  - builder
  - inconsistency
---

## Description

`client/src/routes/mod.rs:226` (in `BuilderPage`) prüft den Zugriff via
`auth.is_allowed("*", PermissionOp::Update)` — also Wildcard. Damit kann nur
ein User mit explizitem `entityType: "*"`-Permission die Builder-Route öffnen,
selbst wenn er Update-Rechte auf *jede einzelne* konkrete Entity hat.

Inkonsistent mit `EditorPage` (`client/src/routes/editor.rs:50`), die die
Permission **pro Entity-Type** prüft: `auth.is_allowed(&entity_type, perm_op)`.

Konsequenz im D2V-Beispiel: Default-User `bookkeeper@local` (Update auf alle
14 D2V-Entitäten) wird vom Builder ausgeschlossen, obwohl er die jeweilige
Entity selbst editieren darf. Die Route ist faktisch unbenutzbar ohne
Wildcard-Admin-User.

## Repro

1. Server: `D2V_LEGACY_URL=… cargo run -p server -- --data-dir ./examples/d2v`
2. Client: `cd client && trunk serve`
3. Login als `bookkeeper@local` / `bookkeeper`.
4. Browser auf `http://127.0.0.1:8080/builder/datev_entry`.
5. Gate verweigert Zugriff, obwohl der User `datev_entry` updaten darf.

## Expected

`BuilderPage` prüft `auth.is_allowed(&entity_type(), PermissionOp::Update)` —
gleiches Muster wie `EditorPage`. Wer eine Entity editieren darf, darf auch
ihr Layout (Designer/Builder) bearbeiten.

Alternative bei expliziter Trennung: eine eigene Permission-Operation
`PermissionOp::Design` einführen (semantisch sauberer, aber größerer Cut).

## Affected files

- `client/src/routes/mod.rs:224-226` — Auth-Gate von Wildcard auf Entity-Type
- ggf. `shared/src/security.rs` — falls neue `Design`-Op eingeführt wird

## Notes

Wenn das Auth-Gate korrigiert ist, fehlt für End-User-Discoverability
zusätzlich noch ein Nav-Link in `client/src/components/navigation.rs` — siehe
[[Q0004-builder-keine-nav-link]].
