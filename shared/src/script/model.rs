//! Kernmodell: `Script`, `ScriptKind`, `ScriptState`, `ScriptId`.
//!
//! `ScriptKind::Wasm` ist heute *reserviert*. Sandbox + Engine lehnen
//! Wasm-Skripte mit `ScriptError::WasmEngineNotAvailable` ab — siehe
//! `server/src/script/engine/rhai.rs::compile`. Phase 2 fuellt die
//! Variante.

use serde::{Deserialize, Serialize};

use crate::script::error::ScriptError;
use crate::script::manifest::ScriptManifest;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(transparent)]
pub struct ScriptId(pub String);

impl From<String> for ScriptId {
    fn from(s: String) -> Self {
        ScriptId(s)
    }
}

impl From<&str> for ScriptId {
    fn from(s: &str) -> Self {
        ScriptId(s.into())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ProviderSlot {
    Formatter,
    Filter,
    Computed,
    Validator,
    RowAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptKind {
    Provider {
        slot: ProviderSlot,
    },
    Component {
        entry: String,
    },
    /// Phase-2-reserviert. Server lehnt `compile()` heute mit
    /// `ScriptError::WasmEngineNotAvailable` ab.
    Wasm {
        wasm_bytes: Vec<u8>,
        entry: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ScriptState {
    Draft,
    Active,
    Locked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Script {
    pub id: ScriptId,
    pub kind: ScriptKind,
    pub manifest: ScriptManifest,
    pub source: String,
    pub version: u32,
    pub state: ScriptState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<ScriptError>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Wire-Form fuer `UiNode::Script`: enthaelt das `script_id`-Ziel und
/// optional eine Version. Der Builder-Client baut darum den vollen
/// `UiNode`-Wrapper.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptNodeRef {
    pub script_id: ScriptId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_pin: Option<u32>,
}
