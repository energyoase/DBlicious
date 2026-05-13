//! Editor-Metadaten.
//!
//! Pendant zu `JezitLibraryShared/EntityProvider/Editor/`. Beschreibt, wie
//! eine Property in einem Editor-Formular dargestellt wird — analog zu
//! [`crate::ColumnMeta`] fuer Tabellen.
//!
//! Quelle der Metadaten ist heute der Server (siehe `entityEditor` Query),
//! gepflegt wird sie pro Entity-Typ. Der Client nutzt sie, um einen
//! generischen `<FieldEditor>` aufzubauen, ohne entity-spezifischen Code.

use serde::{Deserialize, Serialize};

use crate::{settings::Visibility, FieldType};

/// Welches Control-Pattern soll im Editor verwendet werden?
///
/// Spiegel zu `ModelControlType` aus dem C#-Original. `Auto` ueberlaesst dem
/// Renderer die Wahl anhand von [`FieldType`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ControlKind {
    #[default]
    Auto,
    /// Einzeiliger Text.
    Input,
    /// Mehrzeiliger Text (Textarea).
    TextArea,
    /// Auswahl aus einer Werteliste (Dropdown/Radio – Renderer entscheidet).
    Select,
    /// Datumswaehler.
    DatePicker,
    /// Lookup auf eine andere Entitaet (Referenz).
    Lookup,
    /// Inline-Editor fuer Sammlungen (1:n).
    InlineList,
    /// Checkbox/Toggle.
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorPropertyMeta {
    pub key: String,
    pub label_key: String,
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub visibility: Visibility,
    /// Reihenfolge im Formular. Bei Gleichstand entscheidet die Listenposition.
    #[serde(default)]
    pub order: i32,
    /// Optionaler Fluent-Schluessel fuer einen Hilfetext unter dem Control.
    #[serde(default)]
    pub help_key: Option<String>,
    /// Optionaler Fluent-Schluessel fuer einen Placeholder.
    #[serde(default)]
    pub placeholder_key: Option<String>,
    /// Optionale Gruppierung (Tab/Sektion). Renderer kann sie ignorieren.
    #[serde(default)]
    pub group_key: Option<String>,
    #[serde(default)]
    pub control: ControlKind,

    // ---- Validation-Constraints ----
    /// Mindestlaenge (Zeichen) fuer textartige Werte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u32>,
    /// Maximallaenge (Zeichen) fuer textartige Werte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    /// Untere Grenze fuer numerische Werte (inklusiv).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Obere Grenze fuer numerische Werte (inklusiv).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// RegEx-Muster (in `regex`-Syntax). Wenn gesetzt, muss ein
    /// textartiger Wert die Pattern matchen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorMeta {
    pub entity_type: String,
    pub properties: Vec<EditorPropertyMeta>,
}

impl EditorMeta {
    /// Properties in der vorgegebenen Reihenfolge, gruppiert nach `group_key`.
    /// Gruppen erscheinen in der Reihenfolge ihres ersten Vorkommens.
    pub fn grouped(&self) -> Vec<(Option<&str>, Vec<&EditorPropertyMeta>)> {
        let mut sorted: Vec<&EditorPropertyMeta> = self.properties.iter().collect();
        sorted.sort_by_key(|p| p.order);

        let mut order: Vec<Option<&str>> = Vec::new();
        let mut buckets: std::collections::HashMap<Option<&str>, Vec<&EditorPropertyMeta>> =
            std::collections::HashMap::new();
        for p in sorted {
            let key = p.group_key.as_deref();
            if !buckets.contains_key(&key) {
                order.push(key);
            }
            buckets.entry(key).or_default().push(p);
        }
        order
            .into_iter()
            .map(|k| (k, buckets.remove(&k).unwrap_or_default()))
            .collect()
    }
}
