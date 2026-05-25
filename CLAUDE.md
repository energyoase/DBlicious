# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Rust workspace (`shared`, `server`, `client`, `cli`) implementing a Leptos CSR/WASM client that talks to an axum + async-graphql mock server, plus a `dblicious` admin CLI. Code comments and FTL strings are in German; identifiers are English. The README (German) is authoritative for architectural intent.

**No demo content lives in the server crate.** Navigation, entity columns/editor/settings, seed entities, users, groups and translatables are loaded at startup from a `--data-dir` folder (see `examples/shop/`). Without `--data-dir` the server refuses to start; with an empty/missing folder the server runs with no nav and an empty DB. The `dblicious` CLI does **not** need an example — it only mutates the DB.

**CCM läuft als Plugin in DBlicious** (seit 2026-05-25). Das `ccm@ccm`-Plugin wird über eine lokale Marketplace-`directory`-Source installiert; der `SessionStart`-Hook feuert und injiziert die `ccm-using`-Orientierung, die `ccm-*`-Skills erscheinen als `ccm:ccm-*`. Damit ist der in Q0001 §8 notierte **H4-Lösungspfad** (Marketplace-Registry + `installed_plugins.json` + Cache-Layout `cache/<marketplace>/<plugin>/<version>/`) realisiert — die frühere Skills-only-Junction-Lösung (`scripts/setup-ccm-symlinks.ps1`, inzwischen entfernt) ist abgelöst. Historie: [`docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md`](docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md) §8.

## Commands

Prereqs (one-time): `rustup target add wasm32-unknown-unknown` and `cargo install trunk`. The `wasm32-unknown-unknown` target is also pinned in `rust-toolchain.toml`. **Git-Hooks aktivieren** (einmalig pro Clone): `git config core.hooksPath .githooks` — `core.hooksPath` ist lokale Config, reist nicht im Repo mit; ohne diesen Schritt feuern pre-commit/pre-push still **nicht**.

- Run server (port from `config.toml` / `--bind`, GraphiQL at `/`, GraphQL at `POST /graphql`): `cargo run -p server -- --data-dir ./examples/shop`
- Run client (port 8080, proxies `/graphql` → `127.0.0.1:8000`): `cd client && trunk serve`
- Both must be running for the app to load data. The client expects the server's GraphQL endpoint at the same origin via the Trunk proxy (`client/Trunk.toml`).
- Build / lint / format: `cargo build`, `cargo clippy`, `cargo fmt`.
- Git-Hooks (`.githooks/`, via `core.hooksPath`): **pre-commit** = `rustfmt --check` der staged `.rs` (instant, kein Build, keine `target/`-Locks); **pre-push** = `cargo clippy --workspace --all-targets -- -D warnings`. Halten die fmt/clippy-Baseline gruen (Hintergrund: `docs/queue/done/Q0010`). Notfall-Umgehung: `git commit`/`git push --no-verify`.
- Tests: `cargo test --workspace` (oder gezielt `cargo test -p shared`, `cargo test -p server`). Wenn ein lokaler Dev-Server laufen koennte (`server.exe`-Datei-Lock unter Windows), nutze `cargo test --target-dir target-test ...` — `target-test/` ist in `.gitignore`.
- The release profile in the workspace `Cargo.toml` (`opt-level = "z"`, `lto`, `codegen-units = 1`, `strip`) is tuned for WASM size — don't loosen it without reason.

## Architecture

### Type contract (`shared/`)

`shared/src/lib.rs` defines the on-the-wire types (`NavigationNode`, `ColumnMeta`, `FieldType`, `Entity`, `EntityPage`, `Sort`, `FilterCriteria`). Both server and client depend on these via plain `serde`. **`FieldType` is a tagged enum** (`#[serde(tag = "kind", rename_all = "camelCase")]`) — values like `{"kind":"money","currency_code_field":"currency"}`. Achtung: `rename_all` greift hier nur fuer die Variantennamen (= `kind`-Wert), die **inneren Felder** einer Struct-Variante bleiben snake_case (`currency_code_field`, `case_insensitive` bei `FilterPredicate`). Pinned via `shared/tests/field_type_wire_format.rs`. Don't change the tag/case without updating both sides.

### Server (`server/`)

`async-graphql` cannot derive `SimpleObject` directly on the `shared` types (would require feature gating in the workspace), so `server/src/schema.rs` re-wraps them with thin local copies. `ColumnMeta.field_type` and `Entity.fields` are exposed as `async_graphql::Json<serde_json::Value>` — the client deserializes them back into the `shared` enums/maps. This is intentional: it avoids modeling tagged unions and dynamic field maps in GraphQL.

**Persistenz via SeaORM + SQLite.** `server/src/entity/` haelt typisierte SeaORM-Modelle pro Tabelle (Generic `entities` plus `users`/`groups`/`user_groups`/`sessions`/`translatable_*`/`metadata_*`/`db_schemas`). `server/src/db.rs::init` oeffnet beim Server-Start einen Pool, erzeugt das Schema via `Schema::create_table_from_entity` (idempotent durch `IF NOT EXISTS`) und seedet leere Tabellen aus dem aktuell installierten `example::ExampleSet`. Ist kein Set installiert, sind die Seed-Schritte no-op (CLI-Pfad). Datenbank-URL via Env-Variable:

- `DBLICIOUS_DATABASE_URL=sqlite::memory:` (Default) — alles pro Server-Start frisch.
- `DBLICIOUS_DATABASE_URL=sqlite://./dblicious.db` — Datei-DB, persistiert.

`schema.rs::QueryRoot::entities` akzeptiert `sort_by`, `sort_dir`, `filter` ignoriert sie aber. Auslese und CRUD sind heute generisch ueber die `entities`-Tabelle; entity-spezifische Tabellen koennen optional via Designer (`saveDbSchema`) zusaetzlich erzeugt werden (`ddl::try_apply_schema`).

**Beispiel-/Daten-Loader (`server/src/example/`).** Format-Dispatch (`format.rs`: `read_typed`, `find_file`, `SUPPORTED_EXTS`) ist absichtlich offen — heute werden `.json` und `.toml` unterstuetzt, neue Formate (YAML, Skripte) lassen sich durch je einen Match-Arm in `read_typed` plus einen Eintrag in `SUPPORTED_EXTS` erweitern. `loader.rs::load(dir)` liest das Verzeichnislayout (`config.{toml,json}`, `navigation.*`, `security/{users,groups}.*`, `translatables/{languages,entries,values}.*`, `entities/<type>/{columns,editor,settings,seed}.*`); fehlende Sub-Dateien sind kein Fehler, das fehlende Top-Verzeichnis schon. Das geladene Set wird per `example::install` in einen prozessweiten `RwLock`-Slot gelegt, den `data::*` synchron konsultiert. Tests laden `examples/shop/` ueber `setup_for_tests`.

**Tests**: `server/src/db.rs::reset()` setzt den Pool-Slot zurueck, damit `#[serial_test::serial]`-annotierte Tests pro Lauf mit einem frischen `sqlite::memory:`-Stand starten koennen. `lib.rs::fresh_test_setup()` ist der Test-Entrypoint; jeder `#[tokio::test]` startet mit einem `boot().await`. Loader-Tests in `server/tests/loader.rs` umgehen `setup_for_tests` und brauchen kein `#[serial]`, weil sie den prozessweiten `example::install`-Slot nicht anfassen. Shared-Tests in `shared/tests/` decken Wire-Format (`field_type_wire_format`), Header-Hash, Validierung, Security und das `DbSchema`-Roundtrip ab.

### Client (`client/`)

Module layout (`client/src/lib.rs`): `app`, `components`, `graphql`, `i18n`, `routes`, `styling`. The crate is `cdylib` + `rlib`; `main.rs` is the WASM entry that mounts `App`.

Three abstractions carry most of the architectural weight — preserve them when adding features:

1. **Design system (`styling/`).** Components never write CSS classes or style strings directly. They call `use_design()` and ask for semantic styles (`surface`, `text`, `nav_item`, `table_cell`, …) via the `DesignSystem` trait. The current impl is `InlineDesign` (CSS-in-Rust over tokens in `styling/tokens.rs`). To switch to Tailwind/Stylance, add a new impl returning `Style::class(...)` and change the single call site in `app.rs::App` (`provide_design_system()`). The `Style` struct carries both `inline` and `class`; components set both on elements so either backend works.

2. **Generic `EntityTable` (`components/table/`).** It only knows `Vec<ColumnMeta>` + `Rc<dyn DataSource>`. There is no per-entity-type code in the table. `column_set_for(entity_type)` in `column.rs` is **deliberately mirroring the signature of `graphql::queries::fetch_columns`** — replacing client-side column metadata with server-driven metadata is a one-line change in `routes/mod.rs::EntityListPage`. Sort/filter/pagination signals (`state.rs::TableState`) are fully wired to the UI and trigger reloads; the server just ignores those args today (see Server section). When server- or client-side sort/filter logic lands, `TableState` and the table view should not need changes.

3. **`DataSource` trait (`components/table/data_source.rs`).** The table is agnostic about whether data comes from the server or is processed locally. `RemoteSource` forwards to GraphQL. A `LocalSource` for client-side sort/filter is a documented extension point — implement the trait, swap the `Rc<dyn DataSource>` constructed in `EntityListPage`.

### GraphQL client (`client/src/graphql/`)

Hand-written, no codegen (`graphql_client`/`cynic` were intentionally avoided to keep the build to `cargo` + `trunk`). `mod.rs::execute` is the generic POST-and-decode helper; `queries.rs` holds the three query strings and their request/response structs. The `RawColumnMeta` → `ColumnMeta` step in `fetch_columns` is where the JSON-blob `fieldType` from the server is parsed into the `shared::FieldType` enum (falls back to `Text` on parse error).

### i18n (`client/src/i18n/`)

Project Fluent. The two `.ftl` files (`client/locales/{de,en}/main.ftl`) are embedded with `include_str!` at compile time — no runtime fetch. The active locale lives in a Leptos `RwSignal` in `I18nContext`; reading via `t(key)` / the `t!` macro subscribes the caller, so locale switches re-render automatically. Adding a language: add `locales/<code>/main.ftl`, extend the `Locale` enum, register a bundle in `bundles()`, and update `Locale::from_code` / `lang_id` / `ftl_source`. Number, date, and currency formatting goes through the browser `Intl` API (`i18n::format`) — don't reimplement locale-aware formatting in Rust.

### Routing

`leptos_router` 0.7 with two routes (`/` → `DashboardPage`, `/entities/:entity_type` → `EntityListPage`) plus a `NotFoundPage` fallback. Navigation links are produced from the GraphQL `navigation` query (`server/src/data.rs::navigation_tree`) and rendered recursively to arbitrary depth in `components/navigation.rs`.

## Conventions worth knowing

- `field_type` round-trips as JSON, not as a typed GraphQL union — keep it that way unless you're prepared to model the union in `async-graphql` and update both serializers.
- Server CORS is open (`Any`/`Any`/`Any`) for local dev only.
- The `SortDirection` enum on the wire is `lowercase` (`"asc"`/`"desc"`); other shared types use `camelCase`.
- Don't bypass `DesignSystem` with hard-coded styles in components — the small handful of inline `style="..."` strings in `app.rs` and `routes/mod.rs` are layout-only (grid templates, padding) and predate the trait, but new visual styling goes through the trait.
