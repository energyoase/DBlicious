//! DTOs fuer schreibende Entity-Operationen.
//!
//! Pendant zu `JezitLibraryShared/EntityProvider/UpdateRequest/`. Drei Shapes
//! decken den klassischen CRUD-Schreibpfad ab:
//!   - [`EntityCreate`] — neue Entitaet, ID kann optional vorgegeben werden
//!   - [`EntityUpdate`] — partielles Update bestehender Felder
//!   - [`EntityDelete`] — Loeschung anhand der ID
//!
//! Die Felder sind als `serde_json::Map<String, Value>` modelliert, damit
//! dieselbe Struktur fuer jeden Entity-Typ funktioniert — analog zu
//! [`crate::Entity`]. Validierungsfehler werden in [`EntityChangeResult`]
//! gemeldet und referenzieren Properties ueber [`crate::validation::ValidationMessage::target`].

use serde::{Deserialize, Serialize};

use crate::{validation::ValidationResult, Entity};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityCreate {
    pub entity_type: String,
    /// Optional vorgegebene ID. Bleibt das Feld leer, vergibt der Server eine.
    #[serde(default)]
    pub id: Option<String>,
    pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityUpdate {
    pub entity_type: String,
    pub id: String,
    /// Nur die Felder, die sich geaendert haben. Felder, die hier nicht
    /// auftauchen, bleiben unveraendert.
    pub fields: serde_json::Map<String, serde_json::Value>,
    /// Optionaler Hash-Wert aus [`crate::header::EntityHeader::hash`] zum
    /// optimistischen Concurrency-Check. Wenn gesetzt und vom Server-Wert
    /// abweichend, lehnt der Server die Aenderung ab.
    #[serde(default)]
    pub expected_hash: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EntityDelete {
    pub entity_type: String,
    pub id: String,
    #[serde(default)]
    pub expected_hash: Option<u64>,
}

/// Antwort einer schreibenden Operation.
///
/// `entity` ist bei `delete` leer; bei `create`/`update` enthaelt es den
/// aktualisierten Server-Stand inkl. eventueller serverseitig generierter
/// Felder (IDs, Timestamps, …).
///
/// `validation` listet sowohl Fehler (die zur Ablehnung gefuehrt haben) als
/// auch Warnungen (die *trotz* erfolgreicher Speicherung gemeldet werden).
/// Aufrufer pruefen zuerst `ok`, dann `validation.has_blocking()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityChangeResult {
    pub ok: bool,
    #[serde(default)]
    pub entity: Option<Entity>,
    #[serde(default)]
    pub validation: ValidationResult,
}

impl EntityChangeResult {
    pub fn success(entity: Entity) -> Self {
        Self { ok: true, entity: Some(entity), validation: ValidationResult::default() }
    }

    pub fn failure(validation: ValidationResult) -> Self {
        Self { ok: false, entity: None, validation }
    }
}
