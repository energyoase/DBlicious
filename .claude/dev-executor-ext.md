# Project-specific dev-executor extensions

This file is read by `/dev-executor` to learn project conventions.

## Commands

- **Test**: `cargo test --workspace`.
  On Windows, if a local dev `server.exe` might be running (file lock),
  use `cargo test --target-dir target-test --workspace` instead.
  `target-test/` is in `.gitignore`.
- **Lint**: `cargo clippy --workspace -- -D warnings`.
- **Format**: `cargo fmt`.
- **Build (WASM client)**: `cd client && trunk build`.
  Needs `wasm32-unknown-unknown` target (pinned in `rust-toolchain.toml`).

## Conventions

- Code comments and FTL strings are in German; identifiers are English.
- Don't loosen the workspace release profile (`opt-level = "z"`, `lto`,
  `codegen-units = 1`, `strip`) without reason — tuned for WASM size.
- `field_type` round-trips as JSON, not as a typed GraphQL union.
- Don't bypass `DesignSystem` with hard-coded styles in client components.
- See `CLAUDE.md` for the full architecture brief and additional
  conventions worth knowing.

## Post-iteration hook

After each closed item, run `cargo fmt` and `cargo clippy --workspace -- -D warnings`.
