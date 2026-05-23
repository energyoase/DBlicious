//! Host-Module — Client-Pendant zu `server::script::host::*`. Identische
//! Trait-Surface (`shared::script::engine::HostApi`), unterschiedliche
//! Backings:
//!   - `i18n` ruft die Project-Fluent-Lookup-API des Clients
//!     (`crate::i18n::t`) auf.
//!   - `ctx` reicht den `ScriptCtx` raw durch — der Renderer (Phase 5) baut
//!     diesen aus den Leptos-Signals zusammen.
//!   - `db` schickt GraphQL-Queries ueber `crate::graphql::execute`; Schreib-
//!     Operationen lehnen mit `ScriptError::ServerOnlyFunction` ab (Spec §5.3
//!     — `WriteEntity` ist `server_only`).
//!   - `ui` produziert JSON-Subtrees in derselben Form wie das Server-Pendant
//!     (`{"type": "vstack", ...}`); der Leptos-Renderer fuer
//!     `UiNode::Script` lebt in Phase 5.
//!   - `audit` schreibt in eine prozesslokale Queue (`audit_queue`), die der
//!     Renderer spaeter ueber den naechsten Heartbeat zum Server schiebt.
//!
//! Sandbox-Gating (Capability-Token-Check) passiert NICHT hier — der
//! `Sandbox::gate(...)`-Aufruf wickelt diese Funktionen ein. So sind die
//! Host-Module testbar, ohne den Sandbox-Pfad nachzubilden.

pub mod audit;
pub mod ctx;
pub mod db;
pub mod i18n;
pub mod ui;
