//! Menue-Aktions-Taxonomie.
//!
//! Pendant zu `JezitLibraryShared/UI/Navigation/`. [`crate::NavigationNode`]
//! traegt heute ein `route`-Feld; eine `MenuAction` ist die generalisierte
//! Form davon: ein Klick auf einen Navigationsknoten kann
//!   - nichts tun (rein Gruppen-Knoten),
//!   - zu einer Route navigieren,
//!   - einen Tab oeffnen,
//!   - oder einen Code-Trigger ausloesen (z.B. "Export jetzt starten").
//!
//! Das Tag-`type`-Schema ist getaggt analog zu [`crate::FieldType`].

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MenuAction {
    /// Keine Aktion – reiner Gruppenknoten.
    #[default]
    None,
    /// Navigation zu einer In-App-Route (gleicher String wie heute in `route`).
    Link { route: String },
    /// Oeffnet einen Tab im [`crate::tabs::TabsState`].
    Tab {
        tab_id: String,
        /// Bei `true` wird der existierende Tab aktiviert statt eines neuen.
        #[serde(default = "default_focus_existing")]
        focus_existing: bool,
    },
    /// Triggert einen clientseitig registrierten Code-Handler. `command`
    /// referenziert einen Eintrag in der Command-Registry des Clients.
    Code {
        command: String,
        #[serde(default)]
        args: serde_json::Map<String, serde_json::Value>,
    },
}

fn default_focus_existing() -> bool {
    true
}

impl MenuAction {
    /// Bequeme Migration: ein altes `route: Option<String>` wird in eine
    /// [`MenuAction::Link`] (oder `None`) ueberfuehrt.
    pub fn from_route(route: Option<String>) -> Self {
        match route {
            Some(r) => MenuAction::Link { route: r },
            None => MenuAction::None,
        }
    }

    pub fn route(&self) -> Option<&str> {
        match self {
            MenuAction::Link { route } => Some(route.as_str()),
            _ => None,
        }
    }
}
