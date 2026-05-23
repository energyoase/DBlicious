//! Fehlerklassen — wird in Task 1.5 ausgebaut.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptError {
    ParseFailed { line: u32, col: u32, msg: String },
    /// Placeholder, der in Task 1.5 entfernt wird, sobald die vollen
    /// Varianten gesetzt sind.
    Placeholder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestError {
    pub reason: String,
}
