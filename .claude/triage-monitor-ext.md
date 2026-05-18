# Project-specific triage-monitor extensions

This file is read by `/triage-monitor` after its generic scan. Append
project-specific scan sources, supervision steps, or post-scan hooks
here. The generic monitor doesn't know about anything project-specific;
this is where you teach it.

## Project context

Rust workspace: `shared`, `server`, `client` (Leptos CSR/WASM), `cli`.
Code comments and FTL strings are in German; identifiers are English.
See `CLAUDE.md` for the full architecture brief.

## Inline-marker scan

The generic monitor walks `git log` on `main` and the working tree
for `FIXME` / `TODO` / `HACK` markers. No project-specific tweaks
needed today.
