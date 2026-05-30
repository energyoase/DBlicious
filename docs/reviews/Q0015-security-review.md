# Q0015 — Security Review

**Diff scope:** `7122130..9afeff1`
**Reviewer:** automated security-review pass (CCM)
**Date:** 2026-05-30
**Verdict:** cleared (0 blocking)

## Scope summary

Three files changed (+114 / -14):

- `docs/standalone-projekt-skeleton.md` — pure-docs sync of the `scripts/` listing (1 → 3 d2v script names) plus an advisory blockquote. No code, no executable content.
- `server/src/example/loader.rs` — replaced an exact-string `min_ver != our_ver` compare with a pure helper `server_version_warning(min_ver, our_ver) -> Option<String>` using SemVer ordering. Warn-only via `tracing::warn!`, no boot abort, warn-and-skip on malformed input. Plus 6 unit tests.
- `server/Cargo.toml` / `Cargo.lock` — added `semver = "1"` as a direct dep of `server`; resolves to the already-locked `1.0.28`, single `+ "semver",` edge line in the lockfile.

This is a small, warn-only change to the data-dir/`[meta]` contract. No auth, crypto, IPC, network, DB, or secret-handling surface is touched.

## Focus-area findings

### 1. SemVer parsing — DoS / panic / unbounded-input (N/A, confirmed safe)

`min_ver` originates exclusively from the operator-controlled `config.toml` `[meta].minServerVersion` field (traced: `ConfigMeta.min_server_version` → `meta.min_server_version.as_deref()` at loader.rs:141 → `server_version_warning`). It is not attacker-reachable over the network; it is local filesystem config supplied by whoever runs the server with `--data-dir`. `our_ver` is the compile-time `env!("CARGO_PKG_VERSION")` constant.

- **No panic path.** Both `Version::parse` calls use `match`; the `Err` arm returns `Some(msg)` (warn-and-skip). The only `expect`/`unwrap` in the diff are inside `#[cfg(test)]` test bodies. The malformed-input cases (`"latest"`, `"nonsense"`) are explicitly covered by `malformed_min_ver_warns_and_skips` and `malformed_our_ver_warns_and_skips`. Warn-and-skip is total — every `Err` and both ordering branches return a defined value.
- **No DoS / hang.** `semver` 1.0.28's `Version::parse` is a single-pass, linear, recursive-descent-free parser (no backtracking, no catastrophic regex, recursion bounded by the dotted-segment count which is itself bounded by input length). Worst case is O(n) over the operator-provided string. A "huge" input would cost a linear allocation/scan once at startup, not a hang — and the input is local operator config, not a request payload, so there is no amplification vector. No unbounded recursion.
- **Boot is never aborted** by this path (the hard-abort `dataDirFormat` check at loader.rs:128 is unchanged and out of this diff's added logic).

### 2. Logging hygiene (N/A — no sensitive leak)

The `tracing::warn!` at loader.rs:143 emits `dir.display()` (the data-dir path, already logged identically by the pre-existing `dataDirFormat` warnings two lines up) plus the two version strings. The data-dir path and version strings are operator-known, non-secret operational metadata. The malformed-input branches echo the bad version string back verbatim — this is operator-supplied config being reflected into the operator's own logs, not user/PII/credential data. No secrets, tokens, connection strings, or DB content are logged. No format-string injection risk: the untrusted values are interpolated as data (`{min_ver}`, `{our_ver}`) into a literal format string, not used as the format string itself.

### 3. Supply-chain — `semver = "1"` (benign, confirmed)

`semver` 1.0.28 was already a transitive node in the tree (pulled by cargo/sea-orm/typst tooling). The Cargo.lock diff is a single `+ "semver",` line adding `server` as a dependent of the **existing** node — no new crate, no new version, no new transitive subtree, no duplicate-version fan-out. `semver` is a first-party `rust-lang/...` crate (dtolnay), low-risk, widely audited. Pinning `"1"` (caret) is appropriate and matches the rest of the manifest's style. No supply-chain concern.

### 4. Skeleton doc as future-pipeline spec (quick glance — no insecure guidance)

The doc edit only updates the `scripts/` file listing (now 3 `.rhai`/`.manifest.json` d2v scripts) and adds an advisory blockquote telling maintainers to keep the list synced with `examples/d2v/scripts/`. The `.rhai` scripts referenced are data-dir content executed by the existing d2v scripting layer — that execution surface is out of this diff's scope and unchanged by it. The doc introduces no new credential handling, no "disable verification" guidance, no insecure-default recommendation. The `dblicious-version`-Plaintext-Pin it mentions is the same `[meta]` contract this change implements. Nothing actionable.

### 5. Secrets / .env / real-DB / auth-crypto-IPC-network (N/A — none touched)

Confirmed: no `.env`, no real database content, no credentials, no auth/session/crypto code, no network or IPC surface appears anywhere in the diff. The change is confined to a startup config-version warning and a docs sync.

## Conclusion

No blocking security issues. The change is a strict improvement in robustness (the old `!=` compare would warn on every patch/minor mismatch in either direction; the new helper warns only when the binary is genuinely older, and degrades to warn-and-skip on malformed input instead of silently mis-comparing). All identified focus areas are either N/A or confirmed safe. Cleared.
