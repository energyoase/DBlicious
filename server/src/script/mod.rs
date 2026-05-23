//! Server-seitige Skript-Sprachen-Integration (Q0009).
//!
//! Architektur-Garantie (Spec §11): Rhai-Symbole tauchen ausschliesslich im
//! `engine::rhai`-Submodul auf. Andere Module sprechen ueber die Traits in
//! `shared::script::*` (`ScriptEngine`, `HostApi`, `HostApiRegistry`).

pub mod engine;
pub mod host;
pub mod sandbox;
