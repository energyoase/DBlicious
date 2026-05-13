//! Beispiel-/Daten-Ordner-Layer.
//!
//! Der Server enthaelt selbst **keinen** fachlichen Inhalt. Stattdessen wird
//! beim Start ein Verzeichnis (`--data-dir`) eingelesen und in ein
//! [`ExampleSet`] uebersetzt. Saemtliche Lese-Pfade in [`crate::data`]
//! konsultieren das im Prozess installierte Set (siehe [`install`] /
//! [`current`]).
//!
//! Erweiterbarkeit
//! ---------------
//! Format-Dispatch lebt in [`format`]. Heute werden `*.json` und `*.toml`
//! unterstuetzt — neue Formate (YAML, Skript, ...) werden dort als zusaetzliche
//! Match-Arme hinzugefuegt; das Verzeichnislayout muss nicht aendern.
//!
//! Verzeichnislayout (`examples/<name>/`):
//!   config.{json,toml}                  optional, sonst Defaults
//!   navigation.{json,toml}              optional
//!   security/
//!     users.{json,toml}                 Liste von [`UserSeed`]
//!     groups.{json,toml}                Liste von [`shared::SecurityGroup`]
//!   translatables/
//!     languages.{json,toml}
//!     entries.{json,toml}
//!     values.{json,toml}
//!   entities/
//!     <entity-type>/
//!       columns.{json,toml}             [`shared::ColumnMeta`]-Liste
//!       editor.{json,toml}              [`shared::EditorMeta`] (optional)
//!       settings.{json,toml}            [`shared::EntitySettings`] (optional)
//!       seed.{json,toml}                Liste von [`shared::Entity`] (optional)

pub mod format;
pub mod loader;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

pub use loader::load;

/// Default-Bind fuer den GraphQL-Endpunkt, falls die `config`-Datei fehlt
/// oder das Feld nicht setzt.
pub const DEFAULT_BIND: &str = "127.0.0.1:8000";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleConfig {
    /// Anzeigename des Beispiels (nur fuer Logs).
    pub name: String,
    /// HTTP-Bind-Adresse (`host:port`).
    pub bind: String,
}

impl Default for ExampleConfig {
    fn default() -> Self {
        Self {
            name: "unnamed".into(),
            bind: DEFAULT_BIND.into(),
        }
    }
}

/// Ein Seed-User mit Klartext-Passwort. Wird beim DB-Seed in einen
/// Argon2-Hash umgewandelt (siehe [`crate::auth::hash_password`]). Eigene
/// Struktur, weil [`shared::SecurityUser`] nur den Hash kennt.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSeed {
    pub id: String,
    pub username: String,
    pub display_name: String,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default = "default_active")]
    pub active: bool,
    #[serde(default)]
    pub password_plain: Option<String>,
}

fn default_active() -> bool {
    true
}

#[derive(Clone, Debug, Default)]
pub struct EntityTypeSet {
    pub columns: Vec<shared::ColumnMeta>,
    pub editor: Option<shared::EditorMeta>,
    pub settings: Option<shared::EntitySettings>,
    pub seeds: Vec<shared::Entity>,
}

#[derive(Clone, Debug)]
pub struct ExampleSet {
    pub root: PathBuf,
    pub config: ExampleConfig,
    pub navigation: Vec<shared::NavigationNode>,
    pub users: Vec<UserSeed>,
    pub groups: Vec<shared::SecurityGroup>,
    pub translatables: shared::TranslatableBundle,
    pub entities: BTreeMap<String, EntityTypeSet>,
    /// Phase 0.7: neues Permission-Modell. Wird parallel zum alten
    /// `groups.permissions`-Pfad geladen; Enforcement-Schicht ist
    /// 0.7.4. Leer, wenn keine Loader-Dateien vorhanden sind — kein
    /// Bruch fuer bestehende Beispiele.
    #[doc(hidden)]
    pub permissions: Vec<shared::auth::Permission>,
    pub roles: Vec<shared::auth::Role>,
    pub role_assignments: Vec<shared::auth::RoleAssignment>,
}

impl ExampleSet {
    pub fn entity_types(&self) -> impl Iterator<Item = &str> {
        self.entities.keys().map(String::as_str)
    }
}

// =============================================================================
// Globaler Prozess-Slot
// =============================================================================
//
// Identisch zum Pattern in `db::CONN`: einmal beim Start oder Test-Setup
// gefuellt, danach nur noch gelesen. `parking_lot::RwLock` (nicht std), damit
// ein Panic in einem Test den Lock nicht poisoned.

static CURRENT: OnceLock<RwLock<Option<ExampleSet>>> = OnceLock::new();

fn slot() -> &'static RwLock<Option<ExampleSet>> {
    CURRENT.get_or_init(|| RwLock::new(None))
}

pub fn install(set: ExampleSet) {
    *slot().write() = Some(set);
}

pub fn current() -> Option<ExampleSet> {
    slot().read().clone()
}

/// Setzt den Slot zurueck (nur fuer Tests / Reset-Pfade).
pub fn reset() {
    *slot().write() = None;
}

/// In-Place-Mutation des installierten Sets (nur Tests / Designer-Updates).
/// Liefert `false`, wenn kein Set installiert ist.
pub fn mutate<F>(f: F) -> bool
where
    F: FnOnce(&mut ExampleSet),
{
    let mut guard = slot().write();
    let Some(set) = guard.as_mut() else { return false };
    f(set);
    true
}
