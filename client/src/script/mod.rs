//! Client-seitige Skript-Sprachen-Integration (Q0009 Phase 4).
//!
//! Spiegelt den Server-Pfad (`server::script`) mit identischen Trait-Impls,
//! anderem Backing-Stack:
//!   - `engine::RhaiEngine` — derselbe `Engine::new_raw()` + `configure_strict`-
//!     Aufbau wie auf dem Server (Spec §11: Rhai-Symbole leben einzig in
//!     `engine::rhai`).
//!   - `sandbox` — gleiche `gate(token, body)`-Logik. Deadline-Pruefung
//!     wechselt zwischen `Instant` (native) und `web_sys::Performance::now()`
//!     (WASM).
//!
//! Weitere Submodule (`host`, `audit_queue`, `data_source`,
//! `ClientHostApiRegistry`) folgen in den naechsten Phase-4-Teilcommits.

pub mod engine;
pub mod sandbox;
