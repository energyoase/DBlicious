//! Entity- und Property-Einstellungen.
//!
//! Pendant zu `JezitLibraryShared/EntityProvider/Settings/`. Pflegt
//! Sichtbarkeit, Zugriff und Ladeverhalten pro Entity/Property — sowohl als
//! globaler Default (vom Server geliefert) als auch als persistierter
//! Nutzer-Override (z.B. ausgeblendete Spalten in der Tabelle).
//!
//! Die `Info`-Enums spiegeln 1:1 die C#-Vorlage (`Info/Access`,
//! `Info/Visibility`, `Info/LoadMethod`, `Info/PropertyAccess`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{FilterCriteria, Sort};

/// Zugriffsstufe einer Entitaet oder Property.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Access {
    #[default]
    Public,
    Internal,
    Protected,
    Admin,
}

/// Sichtbarkeit in UI-Listen und Editoren.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    /// Wird angezeigt, aber nicht editierbar.
    ReadOnly,
    /// Nur in Detail-Views, nicht in Listen.
    DetailOnly,
}

/// Wie/Wann werden zugehoerige Daten beschafft.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LoadMethod {
    /// Sofort mit dem Eltern-Objekt laden.
    #[default]
    Eager,
    /// Erst bei Zugriff laden.
    Lazy,
    /// Nur auf expliziten Trigger laden.
    Manual,
}

/// Lese-/Schreibzugriff auf eine Property.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum PropertyAccess {
    #[default]
    ReadWrite,
    ReadOnly,
    WriteOnly,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PropertySettings {
    /// Property-Key (gleicher Schluessel wie [`crate::ColumnMeta::key`]).
    pub key: String,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(default)]
    pub access: PropertyAccess,
    #[serde(default)]
    pub load_method: LoadMethod,
    /// Reihenfolge in der Tabelle/im Editor. Bei Gleichstand entscheidet
    /// die ursprueengliche Server-Reihenfolge.
    #[serde(default)]
    pub order: i32,
    /// Optionaler Override des Spalten-Labels (z.B. fuer Nutzer-Aliasing).
    #[serde(default)]
    pub label_override_key: Option<String>,
    /// Optionale Mindestbreite in CSS-Pixel (nur Tabelle).
    #[serde(default)]
    pub min_width: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntitySettings {
    pub entity_type: String,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub default_page_size: Option<u32>,
    #[serde(default)]
    pub default_sort: Option<Sort>,
    #[serde(default)]
    pub default_filter: Option<FilterCriteria>,
    /// Pro Property; nur diejenigen, deren Default-Verhalten ueberschrieben wird.
    #[serde(default)]
    pub properties: Vec<PropertySettings>,
}

impl EntitySettings {
    pub fn property(&self, key: &str) -> Option<&PropertySettings> {
        self.properties.iter().find(|p| p.key == key)
    }

    pub fn property_mut(&mut self, key: &str) -> Option<&mut PropertySettings> {
        self.properties.iter_mut().find(|p| p.key == key)
    }

    pub fn ensure_property(&mut self, key: &str) -> &mut PropertySettings {
        if !self.properties.iter().any(|p| p.key == key) {
            self.properties.push(PropertySettings {
                key: key.to_string(),
                ..Default::default()
            });
        }
        self.property_mut(key).expect("just inserted")
    }
}

/// Container fuer Einstellungen mehrerer Entity-Typen — wird typischerweise
/// als Block aus einem User-Profil geladen.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsBundle {
    #[serde(default)]
    pub entities: HashMap<String, EntitySettings>,
}

impl SettingsBundle {
    pub fn get(&self, entity_type: &str) -> Option<&EntitySettings> {
        self.entities.get(entity_type)
    }

    pub fn ensure(&mut self, entity_type: &str) -> &mut EntitySettings {
        self.entities
            .entry(entity_type.to_string())
            .or_insert_with(|| EntitySettings {
                entity_type: entity_type.to_string(),
                ..Default::default()
            })
    }
}
