//! Engine-Adapter (Rhai). Spec-Garantie (§11): Rhai-Symbole tauchen **nur**
//! in `engine::rhai`-Submodul auf — niemand sonst importiert `rhai::*`.
//!
//! Pendant zu `server::script::engine` mit identischem Aufbau.

pub mod rhai;

pub use rhai::RhaiEngine;
