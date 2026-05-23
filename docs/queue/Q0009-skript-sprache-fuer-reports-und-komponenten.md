---
id: Q0009
created: 2026-05-23T00:00:00Z
status: done
priority: medium
title: "Skript-Sprache für Reports, Custom-Komponenten und Capability-Provider (Rhai-Engine, später WASM)"
spec: docs/superpowers/specs/2026-05-23-q0009-skript-sprache-design.md
plan: docs/superpowers/plans/Q0009-skript-sprache-fuer-reports-und-komponenten.md
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
  status: revise
  reviewer: claude
  notes_path: docs/reviews/Q0009-review.md
  requested_at: 2026-05-23T00:00:00Z
  decided_at: 2026-05-23T00:00:00Z
security_review:
  required: true
  status: flagged
  notes_path: docs/reviews/Q0009-review.md
diagnosis_path: null
design_path: docs/superpowers/specs/2026-05-23-q0009-skript-sprache-design.md
linked_issue: null
linked_pr: null
verification_ops: []
tags:
  - scripting
  - reports
  - builder
  - extensibility
  - sandbox
  - rhai
  - wasm-forward
  - codegen-forward
---

## Description

DBlicious bekommt eine eingebettete Skript-Sprache, mit der **User selbst
Reports schreiben, eigene Komponenten bauen, Custom-Behavior (Validatoren,
Computed Columns, Formatter) ergänzen und Workflow-Aktionen definieren**
können. Skripte greifen über eine engine-agnostische Host-API auf die
gleichen Daten zu, die die UI sieht — Rechte und ACL werden serverseitig
durchgesetzt, der Skript-Sandbox erzwingt Capability-Tokens beidseitig.

Sprachstrategie:

- **Primär: Rhai** (Rust-native, sandbox-by-default, klein, codegen-fähig).
- **Später: WASM-Plugin-Engine** (Extism, Phase 2) für TypeScript/Python/Go.
- **Lua: nicht als native Engine** — via WASM-Plugin abgedeckt.

Architektur-Eckpunkte (Details im Spec):

1. **Hybrid-Integration**: Skripte sind entweder *Capability-Provider*
   (kleine Pure-Functions in Registries — Formatter, Filter, Computed,
   Validator, RowAction) oder *Runtime-Komponenten* (eigener
   `UiNode::Script`-Variant neben Tabelle/Report). Gleiche Engine, gleiche
   Sandbox, austauschbarer Rückgabe-Slot.
2. **Server/Client-Symmetrie**: Identische Host-API auf beiden Seiten,
   Skripte laufen beidseitig vollwertig — Server für Schreiben/SSR/Export,
   Client für Live-Anzeige und Builder-Preview. Server bleibt die Autorität
   für Rechte und Persistenz.
3. **Vier Capability-Tiers** (Reader / Author / Developer / Admin) mit
   verschiedenen Token-Sets und Limits (Timeout, Memory). Manifest deklariert
   exakt die genutzten Tokens — nicht mehr.
4. **Draft-State**: Skripte mit Parse-/Manifest-/Tier-Fehlern werden mit
   `state=Draft` und `last_error` gespeichert (keine Arbeit verloren), sind
   aber nicht ausführbar.
5. **Lift-and-Lock**: Provider- und Component-Skripte sind statisch
   analysierbar und können zur Build-Zeit zu Rust transpiliert werden
   (Phase 4) — kompiliert in eine erstklassige Registry-Funktion bzw. einen
   neuen `UiNode`-Variant.
6. **Forward-Compat zu Codegen-Profilen**: spezialisierte Clients (Beispiel:
   Score-Display, der nur eine einzelne Entity sieht) können per
   Capability-Manifest aller Skripte tree-shaked werden.

## Affected files (zu erwarten)

- `shared/src/script/*.rs` — Script, ScriptManifest, CapabilityToken, ScriptError, UiNode::Script-Variante
- `shared/src/script/engine.rs` — engine-agnostischer Trait
- `shared/src/script/testing.rs` — MockHostApi für Symmetrie-Tests
- `server/src/script/{engine,sandbox,host}/*.rs` — Rhai-Engine + Sandbox + Host-Functions (`db`, `ui`, `i18n`, `ctx`, `audit`)
- `server/src/entity/{script,script_version,script_audit_log}.rs` — SeaORM-Modelle
- `server/src/example/loader.rs` — Skript-Sidecar-Format `scripts/<id>.rhai` + `<id>.manifest.{json,toml}`
- `client/src/script/{engine,sandbox,host}/*.rs` — Spiegel-Implementation für WASM-Client
- `client/src/components/script_renderer.rs` — neue UiNode::Script-Renderer
- Tabellen-Registries (Formatter/Filter/Computed/Validator) — Skript-Lookup-Pfad

## Notes

- Spec: [Skript-Sprache Design (2026-05-23)](../superpowers/specs/2026-05-23-q0009-skript-sprache-design.md)
- Vorbedingung (soft): Phase 1.5 Resolution-Kette, Phase 1.6 Designer-Persistenz, Phase 1.7.12 Aggregations-Layer
- Komplementär: [[u3-report-view-design]] — Report-View nutzt Skripte als optionale Datenquelle
- Forward-Refs (eigene Specs, hier nur Hooks): WASM-Plugin-Engine (Phase 2), Codegen-Profile (Phase 4)
- Security-Review erforderlich: Skripte sind ein Code-Execution-Pfad. Sandbox-Garantien, Capability-Durchsetzung und Symbol-Disable-Liste müssen vor Merge geprüft werden.

## Log

- 2026-05-23T00:00:00Z — manual: created (status=brainstormed, spec=docs/superpowers/specs/2026-05-23-q0009-skript-sprache-design.md)
- 2026-05-23T00:00:00Z — ccm-plan: status brainstormed → planned, plan=docs/superpowers/plans/Q0009-skript-sprache-fuer-reports-und-komponenten.md (sub-agent commit=7d5e2973)
- 2026-05-23T00:00:00Z — ccm-execute: status planned → executing (approval via ccm-plan §9, WIP gestasht)
- 2026-05-23T00:00:00Z — ccm-execute: Phase 1 done (8 commits, 21 tests) — df047a1..d13f52a
- 2026-05-23T00:00:00Z — ccm-execute: Phase 2 done (8 commits, 20 tests) — e60e49b..167b570
- 2026-05-23T00:00:00Z — ccm-execute: Phase 3 done (5 commits, 20 tests) — 5569f75..fde269c
- 2026-05-23T00:00:00Z — ccm-execute: Phase 4 done (4 commits, 21 tests) — 070743a..4eae75d
- 2026-05-23T00:00:00Z — ccm-execute: Phase 5 done (4 commits) — 3e185fa..b6df6ac
- 2026-05-23T00:00:00Z — ccm-execute: Phase 6 done (6 commits, 5 tests) — f673e3a..ed1af62
- 2026-05-23T00:00:00Z — ccm-execute: status executing → done, final_sha=4cba6b2. Verification-Gate: alle 87 Q0009-Tests isoliert grün (shared 21, server_engine 19, persistence 4, loader 3, save 10, run 3, symmetry 2, graphql 5, client 20). Workspace-Run zeigte 3 flaky-Fails in cli/migrate_security (Concurrency-Lock mit Parallel-Session, isoliert grün bestätigt — pre-existing, Commit 816a485, nicht Q0009). clippy/fmt-Baseline → Q0010.
- 2026-05-23T00:00:00Z — HINWEIS: security_review.required=true — Item NICHT nach done/ verschoben. Ausstehend: /ccm-review Q0009 + /ccm-security-review Q0009 vor Archivierung.
- 2026-05-23T00:00:00Z — ccm-review: Verdict NEEDS-WORK (review.status=revise, security_review.status=flagged), notes=docs/reviews/Q0009-review.md. 3 Blocker (B1 unmaskable nicht enforced, B2 new_raw ohne packages, B3 Sandbox-Gate nicht im run-Pfad verdrahtet) + 5 should-fix (S4 token_eq Tier-Bypass u.a.). Architektur solide, aber Skript-Ausführung funktional unvollständig + Enforcement-Bugs vor produktiver Nutzung zu fixen.
