# Q0015 — Skeleton-Doku-Stale + minServerVersion-Semantik Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix two low-risk warn-only findings on the data-dir/`[meta]` contract: sync the stale `scripts/` block in the standalone skeleton doc (1 → 3 scripts), and replace the exact-string `minServerVersion` comparison with a SemVer `>=` threshold that only warns when the binary is older than the declared minimum.

**Architecture:** The substance is a single pure, module-private helper `server_version_warning(min_ver, our_ver) -> Option<String>` extracted from the inline loader logic so the warn decision is unit-testable without a tracing-capture harness. The loader call-site fires `tracing::warn!` only on `Some`. SemVer comparison via the `semver` crate (already locked at 1.0.28 transitively — no lockfile bump). The skeleton-doc fix is pure Markdown. Both parts are warn-only; the hard `dataDirFormat` gate stays untouched.

**Tech Stack:** Rust, `semver = "1"`, `tracing`, existing `server/src/example/loader.rs`. Tests are inline `#[cfg(test)]` unit tests in `loader.rs` (the helper stays `fn`-private and must not be exported across the `loader` surface, so an external `server/tests/*.rs` file cannot reach it).

**Spec:** `docs/superpowers/specs/Q0015-skeleton-doku-stale-und-minserverversion-semantik-design.md`

**Windows caveat:** A local dev `server.exe` may hold a file-lock. All `cargo test`/`cargo build` commands for the `server` crate use `--target-dir target-test` (gitignored) to avoid the lock.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `server/Cargo.toml` | Add `semver = "1"` as a direct dependency | Modify |
| `server/src/example/loader.rs` | Add pure `server_version_warning` helper, rewire the call-site, update the `min_server_version` doc-comment, add inline `#[cfg(test)]` tests | Modify |
| `docs/standalone-projekt-skeleton.md` | Sync `scripts/` block (1 → 3 scripts) + source line + one-line anti-drift note | Modify (pure docs) |

---

## Task 1: Add the `semver` dependency

**Files:**
- Modify: `server/Cargo.toml` (under `[dependencies]`, currently ends at the `typst-as-lib` block around line 95)

- [ ] **Step 1: Add the dependency**

In `server/Cargo.toml`, add the following line at the end of the `[dependencies]` section (after the `typst-as-lib = { ... }` block on line 95, before the `[dev-dependencies]` section on line 97):

```toml
# Q0015 §1.3: SemVer-Vergleich fuer die minServerVersion-Warn-Schwelle.
# `semver` 1.0.28 ist bereits transitiv im Lockfile (cargo-/sea-orm-/typst-
# Tooling) — `semver = "1"` loest auf denselben Knoten, kein Lockfile-Bump.
semver = "1"
```

- [ ] **Step 2: Verify it resolves without a lockfile bump**

Run: `cargo build -p server --target-dir target-test`
Expected: builds cleanly; `git diff Cargo.lock` shows **no** new `[[package]]` entry for `semver` (it was already locked at 1.0.28). If `Cargo.lock` changed, stop and investigate — the spec asserts no bump.

- [ ] **Step 3: Commit**

```bash
git add server/Cargo.toml Cargo.lock
git commit -m "build(server): add semver dependency for Q0015 minServerVersion check

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

(If `Cargo.lock` is unchanged, just `git add server/Cargo.toml`.)

---

## Task 2: Write the failing tests for `server_version_warning`

**Files:**
- Modify: `server/src/example/loader.rs` — append an inline `#[cfg(test)] mod tests` block at the end of the file (after `load_entity_type`, after line 349)

The helper does not exist yet, so the module won't compile — that is the expected "failing test" state for this TDD step.

- [ ] **Step 1: Write the failing test module**

Append to the end of `server/src/example/loader.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::server_version_warning;

    #[test]
    fn binary_newer_than_min_no_warning() {
        // Binary 0.2.0 erfuellt minServerVersion 0.1.0 → keine Warnung.
        assert_eq!(server_version_warning("0.1.0", "0.2.0"), None);
    }

    #[test]
    fn binary_equal_to_min_no_warning() {
        // Genau am Minimum → keine Warnung.
        assert_eq!(server_version_warning("0.1.0", "0.1.0"), None);
    }

    #[test]
    fn binary_older_than_min_warns() {
        // Binary 0.1.0 zu alt fuer minServerVersion 0.2.0 → Warnung,
        // die beide Versionen und "zu alt" nennt.
        let msg = server_version_warning("0.2.0", "0.1.0")
            .expect("aelteres Binary muss warnen");
        assert!(msg.contains("0.2.0"), "Message muss min_ver nennen: {msg}");
        assert!(msg.contains("0.1.0"), "Message muss our_ver nennen: {msg}");
        assert!(msg.contains("zu alt"), "Message muss 'zu alt' enthalten: {msg}");
    }

    #[test]
    fn binary_older_minor_warns() {
        // Patch-/Minor-Unterschied innerhalb derselben Major → Warnung.
        assert!(server_version_warning("0.1.5", "0.1.0").is_some());
    }

    #[test]
    fn malformed_min_ver_warns_and_skips() {
        // Nicht-SemVer min_ver → warn-and-skip, Message nennt den Grund.
        let msg = server_version_warning("latest", "0.1.0")
            .expect("unparsbares min_ver muss warnen (skip)");
        assert!(
            msg.contains("kein gueltiges SemVer"),
            "Message muss den Parse-Grund nennen: {msg}"
        );
    }

    #[test]
    fn malformed_our_ver_warns_and_skips() {
        // Nicht-SemVer our_ver (interner Build-Defekt) → warn-and-skip,
        // kein panic, kein Boot-Abbruch.
        assert!(server_version_warning("0.1.0", "nonsense").is_some());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail to compile**

Run: `cargo test -p server --target-dir target-test server_version_warning`
Expected: FAIL — compile error `cannot find function server_version_warning in this scope` (the helper does not exist yet).

- [ ] **Step 3: Commit the failing tests**

```bash
git add server/src/example/loader.rs
git commit -m "test(loader): add failing tests for server_version_warning (Q0015)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Implement `server_version_warning` and rewire the loader

**Files:**
- Modify: `server/src/example/loader.rs` — add the helper function, rewrite the call-site at lines 101-110, update the `min_server_version` doc-comment at lines 44-48

- [ ] **Step 1: Add the pure helper function**

In `server/src/example/loader.rs`, insert the following function immediately **before** `pub fn load(dir: &Path) -> Result<ExampleSet> {` (i.e. after the `ConfigMeta` struct that ends at line 49, before the `/// Laed das Beispiel ...` doc-comment on line 51):

```rust
/// Prueft die optionale `minServerVersion`-Untergrenze gegen die laufende
/// Binary-Version. Reine Warn-Schwelle (kein Stopp): liefert `Some(msg)`, wenn
/// eine Warnung geloggt werden soll, sonst `None`.
///
/// Warn-Faelle:
/// - Binary aelter als das deklarierte Minimum (`our < min`),
/// - `min_ver` ist kein gueltiges SemVer (Check uebersprungen),
/// - `our_ver` ist kein gueltiges SemVer (interner Build-Defekt; Check uebersprungen).
///
/// Kein Warn-Fall: `our >= min`.
fn server_version_warning(min_ver: &str, our_ver: &str) -> Option<String> {
    let min = match semver::Version::parse(min_ver) {
        Ok(v) => v,
        Err(_) => {
            return Some(format!(
                "minServerVersion = '{min_ver}' ist kein gueltiges SemVer — \
                 Versions-Check uebersprungen."
            ));
        }
    };
    let our = match semver::Version::parse(our_ver) {
        Ok(v) => v,
        Err(_) => {
            return Some(format!(
                "interne Binary-Version '{our_ver}' ist kein gueltiges SemVer — \
                 minServerVersion-Check uebersprungen."
            ));
        }
    };
    if our < min {
        return Some(format!(
            "data-dir verlangt minServerVersion = '{min_ver}', dieses Binary ist \
             {our_ver} (zu alt). Reine Warnung, kein Stopp — bitte ein neueres \
             dblicious-Binary installieren."
        ));
    }
    None
}
```

- [ ] **Step 2: Rewire the loader call-site**

Replace the existing block at lines 101-110:

```rust
    if let Some(min_ver) = meta.min_server_version.as_deref() {
        let our_ver = env!("CARGO_PKG_VERSION");
        if min_ver != our_ver {
            tracing::warn!(
                "data-dir '{}' deklariert minServerVersion = '{min_ver}', dieses Binary ist {our_ver}. \
                 Es findet keine harte Pruefung statt — bitte selbst verifizieren.",
                dir.display()
            );
        }
    }
```

with:

```rust
    if let Some(min_ver) = meta.min_server_version.as_deref() {
        if let Some(msg) = server_version_warning(min_ver, env!("CARGO_PKG_VERSION")) {
            tracing::warn!("data-dir '{}': {msg}", dir.display());
        }
    }
```

- [ ] **Step 3: Update the `min_server_version` doc-comment**

Replace the doc-comment on the `min_server_version` field (lines 44-46, the three `///` lines directly above `#[serde(default)]` / `min_server_version: Option<String>`):

```rust
    /// Optionale Mindest-Server-Version — reine Warn-Schwelle, kein Stopp.
    /// Format: `major.minor.patch` (lex-vergleichbar reicht uns hier nicht;
    /// wir tracen nur, wir parsen es nicht weiter — entkoppelt von SemVer).
```

with:

```rust
    /// Optionale Mindest-Server-Version (SemVer). Reine Warn-Schwelle, kein
    /// Stopp: das Binary warnt nur, wenn seine eigene Version *kleiner* als
    /// dieser Wert ist (oder der String kein gueltiges SemVer ist). Vergleich
    /// via `semver`-Crate (Q0015 §1).
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p server --target-dir target-test server_version_warning`
Expected: PASS — all 6 tests green (`binary_newer_than_min_no_warning`, `binary_equal_to_min_no_warning`, `binary_older_than_min_warns`, `binary_older_minor_warns`, `malformed_min_ver_warns_and_skips`, `malformed_our_ver_warns_and_skips`).

- [ ] **Step 5: Run the existing Q0012 loader tests to confirm no regression**

Run: `cargo test -p server --target-dir target-test --test loader_data_dir_format`
Expected: PASS — the 4 existing `dataDirFormat` tests stay green (the hard gate is untouched).

- [ ] **Step 6: Commit**

```bash
git add server/src/example/loader.rs
git commit -m "fix(loader): minServerVersion as SemVer >= threshold (Q0015)

Replace exact-string compare with a pure server_version_warning helper that
warns only when the binary is older than the declared minimum. Warn-and-skip
on malformed input. Stays warn-only; the dataDirFormat hard gate is unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Sync the standalone skeleton doc (pure docs — no test)

**Files:**
- Modify: `docs/standalone-projekt-skeleton.md` — `scripts/` block (lines 53-55) and source line (line 59)

This is pure documentation. No test; the verification is a Glob check that the listed scripts match `examples/d2v/scripts/`.

- [ ] **Step 1: Verify the live script inventory matches the planned list**

Run: `Get-ChildItem examples/d2v/scripts/ -Name | Sort-Object`
Expected output (3 `.rhai` + 3 `.manifest.json` = 3 script pairs):
```
d2v_balance_validator.manifest.json
d2v_balance_validator.rhai
d2v_stack_filter.manifest.json
d2v_stack_filter.rhai
d2v_value_type_label.manifest.json
d2v_value_type_label.rhai
```
If the inventory differs from this, stop and reconcile the list below to match reality before editing the doc.

- [ ] **Step 2: Update the `scripts/` block**

Replace the `scripts/` block at lines 53-55:

```
└── scripts/
    ├── d2v_value_type_label.rhai
    └── d2v_value_type_label.manifest.json
```

with the 3-pair listing (alphabetical, matching the spec §2.2):

```
└── scripts/
    ├── d2v_balance_validator.{rhai,manifest.json}
    ├── d2v_stack_filter.{rhai,manifest.json}
    └── d2v_value_type_label.{rhai,manifest.json}
```

- [ ] **Step 3: Update the source line + add the one-line anti-drift note**

Replace the source line (lines 58-60):

```
Quelle: 1:1 Kopie der Schicht-3+4-Dateien aus `dblicious/examples/d2v/`
(verifiziert: 17 Entity-Typen, 1 Script). Die einzige strukturelle Neuerung
ist `[meta]` in `config.toml` und die `dblicious-version`-Plaintext-Pin.
```

with:

```
Quelle: 1:1 Kopie der Schicht-3+4-Dateien aus `dblicious/examples/d2v/`
(verifiziert: 17 Entity-Typen, 3 Scripts (Stand 2026-05-30)). Die einzige
strukturelle Neuerung ist `[meta]` in `config.toml` und die
`dblicious-version`-Plaintext-Pin.

> `scripts/` ist der lebende Bestand von `examples/d2v/scripts/` und waechst mit
> jedem Script-Pilot mit — Liste hier synchron halten, Bestand via Glob auf
> `examples/d2v/scripts/` verifizieren.
```

Note: the 17-entity tree above is confirmed STILL CORRECT (spec §2.1) — do **not** touch it. The `§7 Stdlib-Naht` chapter also needs no change (spec §2.2 / §2.3 closing note).

- [ ] **Step 4: Commit**

```bash
git add docs/standalone-projekt-skeleton.md
git commit -m "docs(skeleton): sync scripts block to 3 d2v scripts (Q0015)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Final verification (fmt + clippy + full server tests)

**Files:** none (verification only)

- [ ] **Step 1: Format check (pre-commit hook baseline)**

Run: `cargo fmt --check`
Expected: no output, exit 0. If it reports diffs, run `cargo fmt`, re-stage, and amend the relevant commit.

- [ ] **Step 2: Clippy (pre-push hook baseline)**

Run: `cargo clippy --workspace --all-targets --target-dir target-test -- -D warnings`
Expected: no warnings, exit 0.

- [ ] **Step 3: Full server test suite**

Run: `cargo test -p server --target-dir target-test`
Expected: PASS — including the 6 new `server_version_warning` unit tests and the 4 existing `loader_data_dir_format` tests.

- [ ] **Step 4: Confirm no out-of-scope changes**

Run: `git status` and `git diff --stat main...HEAD`
Expected: only `server/Cargo.toml`, `server/src/example/loader.rs`, `docs/standalone-projekt-skeleton.md` (and possibly an unchanged `Cargo.lock`). Confirm **no** `examples/**` mutation and **no** change to `shared/src/lib.rs::DATA_DIR_FORMAT` (spec §3, DoD §4).

---

## Definition of Done (from spec §4)

- [ ] `server/Cargo.toml`: `semver = "1"` as a direct dependency (resolves to locked 1.0.28, no bump) — Task 1.
- [ ] `loader.rs`: pure `server_version_warning(min_ver, our_ver) -> Option<String>` helper, call-site uses it, `>=` semantics, German messages per spec §1.5 — Task 3.
- [ ] `loader.rs`: `ConfigMeta::min_server_version` doc-comment rewritten to SemVer `>=` semantics (§1.6) — Task 3 Step 3.
- [ ] Tests: all 6 cases from §1.7 green; "binary newer → no warning" and "binary older → warning" both explicitly covered — Tasks 2 + 3.
- [ ] `docs/standalone-projekt-skeleton.md`: `scripts/` block + source line on 3 scripts (§2.2), one-line anti-drift note (§2.3) — Task 4.
- [ ] `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` green — Task 5.
- [ ] `cargo test -p server` green — Task 5.
- [ ] No `examples/**` mutation, no `DATA_DIR_FORMAT` bump — Task 5 Step 4.
