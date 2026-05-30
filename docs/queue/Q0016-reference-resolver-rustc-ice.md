---
id: Q0016
created: 2026-05-30T20:01:01Z
status: new
priority: medium
title: "rustc-ICE in server/tests/reference_resolver.rs (pre-existing, out-of-scope aus Q0011)"
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
  status: pending
  reviewer: null
  notes_path: null
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
  - test
  - toolchain
  - rustc-ice
  - follow-up
---

## Description

Der Q0011-Reviewer (`docs/reviews/Q0011-review.md`) hat einen **pre-existing,
out-of-scope rustc-ICE (Internal Compiler Error)** beim Kompilieren von
`server/tests/reference_resolver.rs` notiert. Der Fehler ist **nicht** durch
Q0011 verursacht (Q0011 fasste nur `*/script/engine/rhai.rs` an) und wurde
deshalb dort bewusst nicht behandelt — hiermit als eigenes Bug-Item erfasst.

`server/tests/reference_resolver.rs` ist vorhanden (9988 B, 5 `#[tokio::test]`:
`raw_no_display_field_yields_empty_labels`, `raw_with_display_field_resolves_label`,
`gql_entities_carries_reference_labels_field`, `shop_seed_order_customer_label_resolved`,
`gql_settings_carries_display_field_for_customer`).

### Zu diagnostizieren (ccm-debug)

- **Reproduktion:** ICE deterministisch reproduzieren (`cargo test -p server
  --test reference_resolver --target-dir target-test` — `target-test/` wegen
  Windows-`server.exe`-Lock & gitignored). Exakte ICE-Meldung + Backtrace
  (`RUST_BACKTRACE=1`), betroffene rustc-Version (`rust-toolchain.toml`),
  Trigger-Konstrukt (welcher Test/welche Zeile).
- **Hypothese:** rustc-Bug vs. Makro-/Trait-Konstrukt, das den ICE auslöst.
- **Lösungsraum:** Toolchain-Bump (vgl. `rust-toolchain.toml`), Umschreiben des
  Trigger-Konstrukts, oder Minimal-Repro + Upstream-Issue. Workaround vs. Fix
  abwägen.

### Hinweis

Da es ein **ICE** ist (Compile-Zeit, nicht Test-Logik), kann er den
`cargo test`-Lauf für das gesamte `server`-Crate blockieren — Priorität medium,
weil er die Test-Baseline (vgl. Q0010 fmt/clippy-Baseline) gefährdet.

### Referenzen

- `docs/reviews/Q0011-review.md` — Ursprungs-Notiz (out-of-scope-Befund).
- `server/tests/reference_resolver.rs` — betroffene Datei.
- `rust-toolchain.toml` — gepinnte Toolchain.
- CLAUDE.md — Test-Hinweis `--target-dir target-test` (Windows-Lock).

## Log
- 2026-05-30T20:01:01Z — manual: created (type=bug; pre-existing rustc-ICE, out-of-scope aus Q0011-Review; bestätigt: reference_resolver.rs vorhanden mit 5 Tests)
