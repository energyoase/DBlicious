//! Entity-Header und Dirty-Tracking-Primitiven.
//!
//! Pendant zu `JezitLibraryShared/EntityProvider/Header/`. Ein `EntityHeader`
//! ist ein leichtgewichtiger Begleiter zur eigentlichen [`crate::Entity`],
//! der den Lade-Zustand, einen Inhalts-Hash und einen "Original"-Hash zur
//! Differenz-Erkennung haelt.
//!
//! Der Hash ist deterministisch ueber die serialisierte JSON-Repraesentation
//! der Felder berechnet — das genuegt, um zu erkennen, ob sich eine
//! geladene Entitaet gegenueber dem zuletzt gespeicherten Stand veraendert
//! hat. Optimistic-Concurrency-Checks gegenueber dem Server laufen ueber
//! [`crate::mutation::EntityUpdate::expected_hash`].

use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::Entity;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum EntityLoadState {
    #[default]
    Unloaded,
    Loading,
    Loaded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityHeader {
    pub id: String,
    pub entity_type: String,
    /// Aktueller Inhalts-Hash – wird bei jeder Aenderung im Client neu berechnet.
    pub hash: u64,
    /// Hash zum Zeitpunkt des letzten erfolgreichen Ladens/Speicherns. Dient
    /// als Referenz fuer "ist die Entitaet dirty?".
    pub original_hash: u64,
    #[serde(default)]
    pub load_state: EntityLoadState,
    /// Optionale Anzeige-Repraesentation – z.B. fuer Tab-Titel.
    #[serde(default)]
    pub display: Option<String>,
}

impl EntityHeader {
    pub fn new_loaded(entity_type: impl Into<String>, entity: &Entity) -> Self {
        let h = compute_hash(entity);
        Self {
            id: entity.id.clone(),
            entity_type: entity_type.into(),
            hash: h,
            original_hash: h,
            load_state: EntityLoadState::Loaded,
            display: None,
        }
    }

    /// Hash gegenueber dem neuen Stand neu berechnen.
    pub fn touch(&mut self, entity: &Entity) {
        self.hash = compute_hash(entity);
    }

    /// Aktuellen Stand als "Original" markieren (nach erfolgreichem Save).
    pub fn baseline(&mut self) {
        self.original_hash = self.hash;
    }

    pub fn is_dirty(&self) -> bool {
        self.hash != self.original_hash
    }
}

/// Stabiler Hash einer Entitaet ueber ihre Felder.
///
/// `serde_json` garantiert keine kanonische Reihenfolge bei
/// `serde_json::Map` (verwendet `BTreeMap`-aehnliches Verhalten nur bei
/// `preserve_order = false` — Standard ist sortiert), deshalb gibt die
/// JSON-Serialisierung bei gleichen Inhalten dieselbe Bytefolge.
pub fn compute_hash(entity: &Entity) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let bytes = serde_json::to_vec(&entity.fields).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    entity.id.hash(&mut hasher);
    bytes.hash(&mut hasher);
    hasher.finish()
}
