//! Validierungs-Modell.
//!
//! Pendant zu `JezitLibraryShared/Validation/`. Drei-stufige
//! Klassifikation (Error / Warning / Info), eine Sammelstruktur und ein
//! zielbezogenes Property-Referenzschema.
//!
//! Konkrete Validatoren leben im Client unter
//! `client/src/validation/` und werden ueber eine Registry pro Entity-Typ
//! verdrahtet. Diese Datei beschreibt nur das *Ergebnis*-Schema – damit es
//! sowohl im Mutation-Response (Server → Client) als auch in clientseitigen
//! Live-Validierungen identisch verwendet werden kann.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Blockiert die Aktion (Speichern, Mutation, …).
    Error,
    /// Wird angezeigt, blockiert aber nicht.
    Warning,
    /// Rein informativ; meist Hinweise auf abgeleitete Werte.
    Info,
}

impl Severity {
    pub fn blocks(self) -> bool {
        matches!(self, Severity::Error)
    }
}

/// Eine einzelne Validation-Meldung.
///
/// `target` zeigt auf die Property der Entitaet (gleicher Schluessel wie in
/// [`crate::ColumnMeta::key`]). `None` bedeutet entity-weit (z.B.
/// "Mindestens eines der drei Felder muss gesetzt sein").
///
/// `message_key` ist immer ein Fluent-Schluessel — der konkrete Text wird
/// in der UI uebersetzt. `args` traegt die Substitutionswerte fuer Fluent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationMessage {
    pub severity: Severity,
    pub message_key: String,
    #[serde(default)]
    pub target: Option<String>,
    /// Werte fuer Fluent-Substitution. Nur einfache Skalare (String/Zahl/Bool).
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
}

impl ValidationMessage {
    pub fn error(target: impl Into<String>, message_key: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message_key: message_key.into(),
            target: Some(target.into()),
            args: serde_json::Map::new(),
        }
    }

    pub fn warning(target: impl Into<String>, message_key: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message_key: message_key.into(),
            target: Some(target.into()),
            args: serde_json::Map::new(),
        }
    }

    pub fn info(target: impl Into<String>, message_key: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            message_key: message_key.into(),
            target: Some(target.into()),
            args: serde_json::Map::new(),
        }
    }

    pub fn with_arg(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.args.insert(key.into(), value.into());
        self
    }
}

/// Sammelergebnis einer Validierung.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    #[serde(default)]
    pub messages: Vec<ValidationMessage>,
}

impl ValidationResult {
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn has_blocking(&self) -> bool {
        self.messages.iter().any(|m| m.severity.blocks())
    }

    pub fn push(&mut self, msg: ValidationMessage) {
        self.messages.push(msg);
    }

    pub fn extend(&mut self, other: ValidationResult) {
        self.messages.extend(other.messages);
    }

    /// Alle Meldungen, die zur uebergebenen Property gehoeren.
    pub fn for_target<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a ValidationMessage> {
        self.messages
            .iter()
            .filter(move |m| m.target.as_deref() == Some(key))
    }

    /// Meldungen ohne `target` (Entity-weit).
    pub fn entity_level(&self) -> impl Iterator<Item = &ValidationMessage> {
        self.messages.iter().filter(|m| m.target.is_none())
    }
}

/// Aufgaben-Vertrag fuer eine einzelne Validation-Regel.
///
/// Bewusst minimal: der konkrete Task lebt im Client (kein dyn-Closure ins
/// `shared`-Crate ueber GraphQL-Grenzen). Hier wird nur dokumentiert, wie das
/// Ergebnis aussieht.
///
/// Im C#-Original entspricht das `ValidationSystem` + `ValidationSystemMethod`
/// + `ValidationTask`. In Rust trennen wir Schema (hier) und Ausfuehrung
/// (`client/src/validation/`).
pub mod tasks {
    //! Marker fuer eingebaute Standard-Validierungen.
    //!
    //! Anders als in C# (Reflection-getrieben) werden diese im Client
    //! exhaustiv in einer Registry zugeordnet — siehe
    //! `client/src/validation/mod.rs::ValidationSystem`.

    /// "Wert darf nicht leer sein."
    pub const REQUIRED: &str = "required";
    /// "Wert liegt unter dem Minimum."
    pub const MIN_LENGTH: &str = "min_length";
    /// "Wert liegt ueber dem Maximum."
    pub const MAX_LENGTH: &str = "max_length";
    /// "Zahl ausserhalb des erlaubten Bereichs."
    pub const NUMBER_RANGE: &str = "number_range";
    /// "Wert passt nicht zum erwarteten Muster (Regex)."
    pub const PATTERN: &str = "pattern";
    /// "Wert nicht in der Liste erlaubter Werte."
    pub const ENUM_VALUE: &str = "enum_value";
}
