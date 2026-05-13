//! Tab-Modell.
//!
//! Pendant zu `JezitLibraryShared/UI/Tabs/`. Beschreibt nur das Datenschema
//! — die eigentliche Tab-Verwaltung (Signals, Komponenten) lebt im Client
//! unter `client/src/tabs/`.

use serde::{Deserialize, Serialize};

/// Beschreibung eines einzelnen Tabs.
///
/// `route` ist die optionale URL, die beim Aktivieren des Tabs angesteuert
/// wird. `payload` traegt frei strukturierte Metadaten – z.B. die ID einer
/// Entitaet, die in dem Tab editiert wird – und ist absichtlich
/// `serde_json::Value`, damit das Schema sich pro Tab-Typ
/// unterscheiden darf.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    pub id: String,
    /// Fluent-Message-ID fuer den Tab-Titel.
    pub label_key: String,
    #[serde(default)]
    pub route: Option<String>,
    #[serde(default = "default_closable")]
    pub closable: bool,
    /// Optionaler Icon-Hinweis (z.B. "cart", "user").
    #[serde(default)]
    pub icon: Option<String>,
    /// Frei strukturierte Nutz-Metadaten.
    #[serde(default)]
    pub payload: serde_json::Value,
}

fn default_closable() -> bool {
    true
}

impl TabInfo {
    pub fn new(id: impl Into<String>, label_key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label_key: label_key.into(),
            route: None,
            closable: true,
            icon: None,
            payload: serde_json::Value::Null,
        }
    }

    pub fn with_route(mut self, route: impl Into<String>) -> Self {
        self.route = Some(route.into());
        self
    }

    pub fn pinned(mut self) -> Self {
        self.closable = false;
        self
    }
}
