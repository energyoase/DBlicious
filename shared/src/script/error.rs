//! Skript-Fehlerklassen.
//!
//! Aufgeteilt in:
//!   - Compile-Time (Save akzeptiert mit state=Draft)
//!   - Run-Time (geht in `script_audit_log.outcome`)
//!   - Validation (Provider mit Slot=Validator)
//!
//! `unmaskable()` markiert die Fehler, die Skripte mit Rhai-`try`/`catch`
//! NICHT fangen koennen (Spec §10).

use serde::{Deserialize, Serialize};

use crate::script::capability::{CapabilityToken, ScriptTier};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestError {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptError {
    // Compile-Time
    ParseFailed {
        line: u32,
        col: u32,
        msg: String,
    },
    ManifestInvalid {
        reason: ManifestError,
    },
    TierExceeded {
        declared: ScriptTier,
        user: ScriptTier,
    },
    /// Phase-2-Reservation: Wasm-Engine ist im Server/Client noch nicht
    /// eingebunden. Wird vom Rhai-Compile-Pfad bei `ScriptKind::Wasm`
    /// geworfen.
    WasmEngineNotAvailable,

    // Run-Time
    CapabilityDenied {
        token: CapabilityToken,
    },
    UiPrimitiveDenied {
        primitive: String,
    },
    ServerOnlyFunction {
        name: String,
    },
    Timeout {
        limit_ms: u32,
    },
    MemoryExceeded {
        limit_kb: u32,
    },
    InternalPanic {
        backtrace: String,
    },
    HostError {
        source: String,
    },

    // Validation
    ValidationFailed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field: Option<String>,
        msg_key: String,
        args: serde_json::Value,
    },
}

impl ScriptError {
    /// Spec §10: diese Fehler sind in Rhai NICHT per `try`/`catch` fangbar.
    pub fn unmaskable(&self) -> bool {
        matches!(
            self,
            ScriptError::CapabilityDenied { .. }
                | ScriptError::UiPrimitiveDenied { .. }
                | ScriptError::Timeout { .. }
                | ScriptError::MemoryExceeded { .. }
        )
    }
}
