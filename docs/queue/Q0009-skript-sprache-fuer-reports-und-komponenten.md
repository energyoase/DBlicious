---
id: Q0009
created: 2026-05-23T00:00:00Z
status: planned
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
  status: null
  reviewer: null
  notes_path: null
  requested_at: null
  decided_at: null
security_review:
  required: true
  status: null
  notes_path: null
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
