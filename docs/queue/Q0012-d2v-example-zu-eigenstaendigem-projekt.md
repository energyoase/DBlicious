---
id: Q0012
created: 2026-05-29T00:00:00Z
status: done
priority: medium
title: "d2v: Example zu eigenständigem Projekt mit dblicious-Binary-Abhängigkeit"
spec: docs/superpowers/specs/Q0012-d2v-example-zu-eigenstaendigem-projekt-design.md
plan: docs/superpowers/plans/Q0012-d2v-example-zu-eigenstaendigem-projekt.md
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
design_path: null
linked_issue: null
linked_pr: null
verification_ops: []
tags:
  - d2v
  - packaging
  - distribution
  - data-dir
  - standalone-project
---

## Description

Heute lebt der D2V-2019-Daten-Port als data-dir `examples/d2v/` **innerhalb**
des dblicious-Repos (Betrieb via `cargo run -p server -- --data-dir
./examples/d2v`; Track A/B fertig — 17 Entitäten als foreign-sqlite, siehe
`examples/d2v/README.md`). Ziel dieses Items: **das Example zu einem
eigenständigen Projekt in einem eigenen Ordner/Repo machen, das dblicious als
Abhängigkeit konsumiert** — statt als In-Repo-Example mitzuwandern.

**Gewähltes Abhängigkeitsmodell (User-Entscheidung 2026-05-29): Data-dir +
installiertes dblicious-Binary.** Das Projekt = eigener Ordner mit
`config.toml`, `navigation.json`, `entities/`, `scripts/`, `templates/`,
eigener `d2v.db`-Kopie; betrieben durch ein installiertes/released
dblicious-Server-Binary (`dblicious --data-dir ./mein-d2v-projekt`). Die
"Abhängigkeit" ist das **Binary/Release**, NICHT eine Cargo-Crate (Rust-Crate-
Modell wurde bewusst verworfen).

### Im Brainstorm zu klären (Design-Fragen)

- **Distribution/Versionierung des Binaries:** Wie wird dblicious so
  ausgeliefert, dass ein externes Projekt es als Abhängigkeit pinnen kann
  (`cargo install` vs. Release-Artefakt vs. Container)? Wie wird Kompatibilität
  **data-dir ↔ Binary-Version** sichergestellt (Schema-/Loader-Format-
  Versionierung; der Loader-Format-Dispatch ist in `server/src/example/format.rs`)?
- **Was wandert aus, was wird generalisiert:** vgl. die 4-Schichten-
  Klassifikation der 2026-05-24-Analyse (§4b: Framework / Bookkeeping-Stdlib /
  Installations-Config / lokales Template). Welche Teile von `examples/d2v/`
  gehören in den eigenen Ordner (Schicht 3+4), welche bleiben/werden Framework
  bzw. Stdlib?
- **Geteilte Bestandteile:** Wie referenziert/lädt das Standalone-Projekt eine
  evtl. geteilte Bookkeeping-Stdlib? Heute lädt der Loader nur
  `<data-dir>/scripts/` (kein geteilter/länder-parametrisierter Pfad).
- **Konfig/Secrets im Standalone-Setup:** `D2V_LEGACY_URL`,
  `DBLICIOUS_DATABASE_URL` — wie gehandhabt (.env, niemals echte Prod-DB
  einchecken; Datenschutz wie im README).
- **Migrationspfad ohne Test-Verlust:** `examples/d2v/` → eigener Ordner, ohne
  die In-Repo-Tests zu verlieren (`server/tests/loader_d2v.rs`, `d2v_e2e.rs`,
  `d2v_all_17_listable.rs`). Bleibt ein Mini-Fixture-Example im Repo?

### Out of scope

- Die Feature-Gap-Liste selbst (welche d2v2019-Features livable sind) → **Q0013**.
- Rust-Crate-Dependency-Modell (verworfen zugunsten data-dir + Binary).

### Referenzen

- `examples/d2v/README.md` — heutiger Stand + Datenschutz.
- `docs/superpowers/specs/2026-05-24-d2v-script-first-gap-analysis.md` §4b — 4-Schichten-Klassifikation.
- `docs/superpowers/specs/2026-05-19-dblicious-source-architecture-design.md`.
- `CLAUDE.md` — data-dir-Modell, "no demo content lives in the server crate".

## Log
- 2026-05-29T14:35:00Z — manual: created (Scope-Split aus /ccm-brainstorm-Anfrage; Schwester-Item Q0013)
- 2026-05-29T19:49:34Z — ccm-brainstorm: status new → brainstormed, spec=docs/superpowers/specs/Q0012-d2v-example-zu-eigenstaendigem-projekt-design.md; security_review.required=true (Trigger: secrets, script, wasm)
- 2026-05-29T22:50:09Z — ccm-plan: status brainstormed → planned, plan=docs/superpowers/plans/Q0012-d2v-example-zu-eigenstaendigem-projekt.md; Standalone-Standort = eigenes Git-Repo (d2v-dblicious-projekt)
- 2026-05-29T23:17:59Z — ccm-execute: status planned → executing (pre-approved via 'execute beide')
- 2026-05-29T23:36:19Z — ccm-execute: status executing → done, final_sha=5f085da (shared::DATA_DIR_FORMAT + [meta] dataDirFormat Loader-Boot-Check, additiv & backward-compatible; docs/standalone-projekt-skeleton.md + CLAUDE.md-Notiz; verification green) — awaiting review
