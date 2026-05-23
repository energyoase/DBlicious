//! Manifest = statische, deklarative Selbstauskunft eines Skripts.
//! Tier deckelt Capability-Set. `lift_capable` wird beim Save berechnet.

use serde::{Deserialize, Serialize};

use crate::script::capability::{CapabilityToken, ScriptTier};

pub const MANIFEST_VERSION_CURRENT: u8 = 1;

/// Whitelisted UI-Primitives — `ui.*`-Aufruf nicht in der Liste → Sandbox
/// schlaegt mit `UiPrimitiveDenied` fehl.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum UiPrimitive {
    Vstack,
    Hstack,
    Text,
    Table,
    Chart,
    If,
    ForEach,
    Action,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptManifest {
    /// Beginnt bei 1; spaetere Versionen duerfen Felder ergaenzen.
    pub manifest_version: u8,
    pub tier: ScriptTier,
    pub capabilities: Vec<CapabilityToken>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ui_primitives: Vec<UiPrimitive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_kb: Option<u32>,
    /// Vom Save-Step berechnet (Phase 3 / `lift::analyze_lift_capability`).
    /// Default `false`: erst die Analyse darf das Flag setzen.
    #[serde(default)]
    pub lift_capable: bool,
}

impl Default for ScriptManifest {
    fn default() -> Self {
        Self {
            manifest_version: MANIFEST_VERSION_CURRENT,
            tier: ScriptTier::Reader,
            capabilities: Vec::new(),
            ui_primitives: Vec::new(),
            timeout_ms: None,
            memory_kb: None,
            lift_capable: false,
        }
    }
}
