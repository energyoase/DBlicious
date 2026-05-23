//! Wire-Format-Typen fuer die eingebettete Skript-Sprache (Q0009).
//!
//! Beide Crates (server, client) konsumieren diese Typen ueber plain
//! `serde`. Die getaggten Enums (`ScriptKind`, `ScriptState`,
//! `CapabilityToken`, `ScriptError`) folgen dem `FieldType`-Vertrag:
//! `#[serde(tag = "kind", rename_all = "camelCase")]` auf der Enum-Ebene
//! benennt die Varianten in camelCase; innere Felder einer Struct-Variante
//! bleiben snake_case (vgl. `shared/tests/field_type_wire_format.rs`).

pub mod capability;
pub mod engine;
pub mod error;
pub mod host_api;
pub mod manifest;
pub mod model;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use capability::{default_tokens_for_tier, CapabilityToken, ScriptTier, UiScope};
pub use error::{ManifestError, ScriptError};
pub use manifest::{ScriptManifest, UiPrimitive, MANIFEST_VERSION_CURRENT};
pub use model::{ProviderSlot, Script, ScriptId, ScriptKind, ScriptNodeRef, ScriptState};
