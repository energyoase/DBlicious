//! Typisierte Anwendungs-Fehler.
//!
//! Pendant zur C#-Hierarchie unter `JezitLibraryShared/Exceptions/`. Statt
//! einer Klassenhierarchie wird das in Rust idiomatisch als getaggter Enum
//! ausgedrueckt. Die Varianten transportieren ausschliesslich strukturelle
//! Information – Lokalisierung passiert beim Anzeigen, nicht in der Variante.
//!
//! Die Display-Texte sind bewusst englisch gehalten: sie sind fuer Logs und
//! Entwickler-Ausgaben gedacht. Auf der UI wird `AppError::message_key()`
//! benutzt, um einen Fluent-Schluessel zu erzeugen.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AppError {
    /// Dekodierung einer Antwort (JSON, GraphQL, …) fehlgeschlagen.
    Decode { detail: String },
    /// Eine Kennung (Entity-ID, Property-Name, …) ist ungueltig oder leer.
    InvalidIdentifier { value: String },
    /// Eine erforderliche Eingabe fehlt vollstaendig.
    UnexpectedEmpty { target: String },
    /// Ein Wert lag ausserhalb des erlaubten Bereichs.
    OutOfBound { target: String, detail: String },
    /// Format passt nicht zur Erwartung.
    UnexpectedFormat { target: String, detail: String },
    /// Wert war `null`, obwohl erforderlich.
    UnexpectedNull { target: String },
    /// Typ-Inkompatibilitaet.
    UnexpectedType {
        target: String,
        expected: String,
        actual: String,
    },
    /// Eine `match`- oder Branch-Tabelle haette ausgeloest sein muessen,
    /// erreichte aber einen unerwarteten Zustand.
    UnexpectedCode { detail: String },
    /// Genau eines aus einer Auswahl haette gesetzt sein muessen.
    OneParameterRequired { options: Vec<String> },
    /// Netzwerk-/Transportfehler – z.B. GraphQL-Endpunkt nicht erreichbar.
    Network { detail: String },
    /// Fachlicher Validation-Fehler. Detail-Liste lebt im
    /// [`crate::validation::ValidationResult`].
    Validation { messages: usize },
    /// Sammelvariante fuer alles, was nicht in eine der obigen Kategorien
    /// passt. Bewusst zurueckhaltend einzusetzen – wenn ein Fehler oft als
    /// `Other` auftritt, gehoert eine neue Variante in den Enum.
    Other { detail: String },
}

impl AppError {
    /// Liefert einen Fluent-Message-ID-Kandidaten fuer die UI-Anzeige.
    ///
    /// Aufrufer koennen das Ergebnis ohne Aufbereitung in
    /// [`crate::translatable::message_id_for_key`] geben.
    pub fn message_key(&self) -> &'static str {
        match self {
            AppError::Decode { .. } => "error.decode",
            AppError::InvalidIdentifier { .. } => "error.invalid_identifier",
            AppError::UnexpectedEmpty { .. } => "error.unexpected_empty",
            AppError::OutOfBound { .. } => "error.out_of_bound",
            AppError::UnexpectedFormat { .. } => "error.unexpected_format",
            AppError::UnexpectedNull { .. } => "error.unexpected_null",
            AppError::UnexpectedType { .. } => "error.unexpected_type",
            AppError::UnexpectedCode { .. } => "error.unexpected_code",
            AppError::OneParameterRequired { .. } => "error.one_parameter_required",
            AppError::Network { .. } => "error.network",
            AppError::Validation { .. } => "error.validation",
            AppError::Other { .. } => "error.other",
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Decode { detail } => write!(f, "decode error: {detail}"),
            AppError::InvalidIdentifier { value } => write!(f, "invalid identifier: {value}"),
            AppError::UnexpectedEmpty { target } => write!(f, "unexpected empty: {target}"),
            AppError::OutOfBound { target, detail } => {
                write!(f, "out of bound on {target}: {detail}")
            }
            AppError::UnexpectedFormat { target, detail } => {
                write!(f, "unexpected format on {target}: {detail}")
            }
            AppError::UnexpectedNull { target } => write!(f, "unexpected null: {target}"),
            AppError::UnexpectedType {
                target,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "unexpected type on {target}: expected {expected}, got {actual}"
                )
            }
            AppError::UnexpectedCode { detail } => write!(f, "unexpected code path: {detail}"),
            AppError::OneParameterRequired { options } => {
                write!(f, "exactly one required of: {}", options.join(", "))
            }
            AppError::Network { detail } => write!(f, "network error: {detail}"),
            AppError::Validation { messages } => {
                write!(f, "validation failed ({messages} message(s))")
            }
            AppError::Other { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;
