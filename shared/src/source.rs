//! Source-Architektur-Wire-Typen.
//!
//! Server-Seite implementiert die `Source`-Trait selbst (siehe
//! `server/src/source/`). Hier nur die Typen, die ueber GraphQL/JSON
//! reisen: Bindings, IDs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Trennzeichen fuer Composite-Keys im Wire-Format.
pub const COMPOSITE_KEY_SEPARATOR: &str = "::";

/// Logische ID einer Entitaet. Single-PK ist der haeufige Fall;
/// Composite-PK gibt es bei fremden DB-Schemas (z.B. D2V).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityId {
    Single(String),
    Composite(Vec<String>),
}

impl EntityId {
    /// Serialisiert in einen `String`, der ueber das Wire-Format
    /// `Entity.id` reist.
    ///
    /// - Single → der Inhalt 1:1.
    /// - Composite ohne Sonderzeichen → `"part1::part2"`.
    /// - Composite mit einer Komponente, die `::` enthaelt, leer ist,
    ///   oder mit Whitespace an den Raendern → JSON-Array-Form
    ///   `"[\"part1\",\"part2\"]"`.
    pub fn encode(&self) -> String {
        match self {
            EntityId::Single(s) => s.clone(),
            EntityId::Composite(parts) => {
                if parts.iter().any(needs_escalation) {
                    serde_json::to_string(parts).expect("Vec<String> serializes")
                } else {
                    parts.join(COMPOSITE_KEY_SEPARATOR)
                }
            }
        }
    }

    /// Dekodiert die Wire-Form zurueck.
    ///
    /// Erkennung:
    /// - String mit `'['` als erstem Zeichen UND valides JSON-Array-of-
    ///   Strings → Composite.
    /// - String mit eingebettetem `"::"` (mind. einer) und nicht JSON →
    ///   Composite (Split am Trennzeichen).
    /// - sonst → Single.
    pub fn decode(s: &str) -> Self {
        if s.starts_with('[') {
            if let Ok(parts) = serde_json::from_str::<Vec<String>>(s) {
                return EntityId::Composite(parts);
            }
            // ungueltiges JSON → als Single behandeln
        }
        if s.contains(COMPOSITE_KEY_SEPARATOR) {
            return EntityId::Composite(
                s.split(COMPOSITE_KEY_SEPARATOR).map(str::to_string).collect(),
            );
        }
        EntityId::Single(s.to_string())
    }
}

fn needs_escalation(part: &String) -> bool {
    part.contains(COMPOSITE_KEY_SEPARATOR)
        || part.is_empty()
        || part.starts_with(char::is_whitespace)
        || part.ends_with(char::is_whitespace)
}

/// Eine Entitaet-Type-Bindung: wo lebt die Entitaet, mit welchem PK,
/// und welche Spalten-Namen sind in der Source vs. im UI/Wire-Format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EntityBinding {
    pub source: String,
    pub locator: BindingLocator,
    pub primary_key: Vec<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub column_map: BTreeMap<String, String>,
}

/// Wo in der Source liegt die Entitaet konkret.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BindingLocator {
    /// Echte relationale Tabelle (foreign-sqlite, postgres, …).
    Table { table: String },
    /// Heutiges Verhalten: Zeile in der gemeinsamen `entities`-Tabelle.
    GenericEntityRow { entity_type: String },
    /// REST-Endpunkt (Phase B3 — Trait kennt es, Implementation folgt).
    RestEndpoint { path: String },
}
