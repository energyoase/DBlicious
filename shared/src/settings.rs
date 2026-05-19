//! Entity- und Property-Einstellungen.
//!
//! Pendant zu `JezitLibraryShared/EntityProvider/Settings/`. Pflegt
//! Sichtbarkeit, Zugriff und Ladeverhalten pro Entity/Property — sowohl als
//! globaler Default (vom Server geliefert) als auch als persistierter
//! Nutzer-Override (z.B. ausgeblendete Spalten in der Tabelle).
//!
//! Die `Info`-Enums spiegeln 1:1 die C#-Vorlage (`Info/Access`,
//! `Info/Visibility`, `Info/LoadMethod`, `Info/PropertyAccess`).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::{FilterCriteria, Sort};

/// Implementations-IDs als Default fuer einen `FieldType` (Phase 1.5.1).
///
/// Dient der Resolution-Kette: wenn `ColumnMeta.X_id` `None` ist, fragt
/// der Resolver hier nach. Pro Feld kann es mehrere erlaubte Implementierungen
/// geben — die `allowed_*`-Listen schraenken die Wahl ein, ohne eine zu
/// erzwingen.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FieldTypeDefaults {
    /// Vorgewaehlte Filter-Implementation, falls die Spalte keine eigene
    /// `filter_id` setzt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatter_id: Option<String>,
    /// Empfohlene Row-Action-IDs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_ids: Vec<String>,
    /// Falls > 1 erlaubt: weitere zulaessige Filter-Alternativen (zum
    /// Beispiel `["text-contains", "text-starts-with"]`). User darf eine
    /// davon waehlen, sofern die Choose-Permission greift.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_filter_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_editor_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_formatter_ids: Vec<String>,
}

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
    #[serde(default)]
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
    /// Phase 1.5.1: Default-Implementations-IDs pro [`crate::FieldType`].
    /// Map-Key ist die String-Diskriminante des FieldType — `"text"`,
    /// `"integer"`, `"decimal"`, `"boolean"`, `"date"`, `"dateTime"`,
    /// `"money"`, `"reference"`, `"collection"`, `"enum"`. Resolution-
    /// Stufe 2 nach `ColumnMeta.X_id`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_type_defaults: BTreeMap<String, FieldTypeDefaults>,
    /// Phase 0.6: Source-Binding fuer diesen Entity-Type. `None` => Default-
    /// Binding (managed-sqlite, generische entities-Tabelle).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<crate::source::EntityBinding>,
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
