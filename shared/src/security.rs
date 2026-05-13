//! Security-Modell (User / Group / Permission).
//!
//! Pendant zu `JezitLibraryShared/EntityProvider/Security/` plus den
//! JSON-Fixtures aus `JezitLibraryServer/Fixtures/`. Bewusst klein
//! gehalten: ein Nutzer gehoert zu n Gruppen, eine Gruppe haelt n
//! [`Permission`]s, und die wirksamen Permissions eines Nutzers sind die
//! Vereinigung ueber alle seine Gruppen.
//!
//! Eine Permission referenziert eine Entitaet ueber `entity_type` (gleicher
//! String wie [`crate::ColumnMeta`]-Kontext). `entity_type = "*"` bedeutet
//! "alle Entitaeten" — fuer Admin-Gruppen.

use serde::{Deserialize, Serialize};

use crate::settings::Access;

/// Eine einzelne Berechtigung: was darf in welchem Scope getan werden.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    /// Entity-Typ oder `"*"` fuer alle.
    pub entity_type: String,
    #[serde(default)]
    pub can_read: bool,
    #[serde(default)]
    pub can_create: bool,
    #[serde(default)]
    pub can_update: bool,
    #[serde(default)]
    pub can_delete: bool,
    /// Erforderliche Zugriffsstufe; default `Public`.
    #[serde(default)]
    pub min_access: Access,
    /// Optionale, pro Property granulare Overrides.
    ///
    /// Wenn leer, gilt die `can_*`-Stufe oben fuer alle Properties. Sonst
    /// schraenkt jeder Eintrag *nur* die genannte Property weiter ein —
    /// Properties, die nicht vorkommen, behalten den Entity-Default.
    #[serde(default)]
    pub property_overrides: Vec<PropertyPermission>,
}

/// Property-spezifische Einschraenkung.
///
/// Pendant zur `PropertyAccess`-Enum aus `JezitLibrary` (NoAccess / Read /
/// WriteBeforePersist / Write). Die Semantik ist *restriktiv*: ein Override
/// kann nur weniger Rechte vergeben, niemals mehr als die Entity-Permission.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPermission {
    pub property: String,
    pub access: PropertyAccessLevel,
}

/// Mehrstufige Property-Zugriffsstufe.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum PropertyAccessLevel {
    /// Property ist nicht sichtbar, weder in Liste noch im Editor.
    NoAccess,
    /// Read-only.
    Read,
    /// Schreibbar nur waehrend `create` (z.B. Identity-PK).
    WriteBeforePersist,
    /// Voller Schreibzugriff (Default, identisch zur Entity-Permission).
    #[default]
    Write,
}

impl Permission {
    pub fn matches(&self, entity_type: &str) -> bool {
        self.entity_type == "*" || self.entity_type == entity_type
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroup {
    pub id: String,
    /// Fluent-Schluessel; konkrete Anzeige laeuft ueber den i18n-Stack.
    pub name_key: String,
    #[serde(default)]
    pub description_key: Option<String>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityUser {
    pub id: String,
    pub username: String,
    /// Fluent-Schluessel oder fertiger Anzeigename. Konvention: Fluent-Schluessel
    /// wenn er mit `user.` beginnt, sonst Klartext.
    pub display_name: String,
    /// Bevorzugte UI-Sprache als BCP-47-Code (`de`, `en-US`, …). Optional;
    /// bei Abwesenheit greift die Browser-Erkennung.
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default = "default_active")]
    pub active: bool,
    /// Argon2-Hash des Passworts (PHC-String). Wird ueber GraphQL **nicht**
    /// ausgeliefert (siehe `server::schema::map_user`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_hash: Option<String>,
}

fn default_active() -> bool {
    true
}

/// Verbinder-Eintrag, falls die n:n-Beziehung Nutzer↔Gruppe normalisiert
/// ueber die Leitung kommt (siehe C#-Fixture `SecurityUser2Group`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityUser2Group {
    pub user_id: String,
    pub group_id: String,
}

/// Effektive Permissions eines Nutzers, berechnet aus seinen Gruppen.
pub fn effective_permissions<'a>(
    user: &SecurityUser,
    groups: &'a [SecurityGroup],
) -> Vec<&'a Permission> {
    groups
        .iter()
        .filter(|g| user.group_ids.iter().any(|gid| gid == &g.id))
        .flat_map(|g| g.permissions.iter())
        .collect()
}

/// Convenience-Pruefung fuer eine konkrete Operation.
pub fn is_allowed(
    user: &SecurityUser,
    groups: &[SecurityGroup],
    entity_type: &str,
    op: PermissionOp,
) -> bool {
    if !user.active {
        return false;
    }
    effective_permissions(user, groups)
        .into_iter()
        .filter(|p| p.matches(entity_type))
        .any(|p| match op {
            PermissionOp::Read => p.can_read,
            PermissionOp::Create => p.can_create,
            PermissionOp::Update => p.can_update,
            PermissionOp::Delete => p.can_delete,
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionOp {
    Read,
    Create,
    Update,
    Delete,
}

/// Liefert die effektive Property-Zugriffsstufe fuer einen User auf einer
/// konkreten Entity-Property.
///
/// Auswertung:
///   1. Alle Permissions zusammensuchen, die auf den Entity-Typ greifen.
///   2. Pro Permission die Property-Override-Liste konsultieren —
///      Override gewinnt, falls vorhanden.
///   3. Aus allen wirksamen Stufen wird das *Maximum* gebildet
///      (Union-Semantik: hoechster Zugriff zaehlt).
///
/// Wenn keine Permission den Entity-Typ trifft, ist das Ergebnis
/// `NoAccess`. Wenn fuer den Entity-Typ Permissions existieren, aber
/// keine Overrides eingreifen, ist das Ergebnis `Write` (entspricht dem
/// Entity-Default).
pub fn property_access_for(
    user: &SecurityUser,
    groups: &[SecurityGroup],
    entity_type: &str,
    property: &str,
) -> PropertyAccessLevel {
    if !user.active {
        return PropertyAccessLevel::NoAccess;
    }
    let perms = effective_permissions(user, groups);
    let mut best: Option<PropertyAccessLevel> = None;
    let mut any_entity_match = false;
    for p in perms {
        if !p.matches(entity_type) {
            continue;
        }
        any_entity_match = true;
        let level = p
            .property_overrides
            .iter()
            .find(|o| o.property == property)
            .map(|o| o.access)
            .unwrap_or(PropertyAccessLevel::Write);
        best = Some(match best {
            Some(prev) => max_access(prev, level),
            None => level,
        });
    }
    match (any_entity_match, best) {
        (false, _) => PropertyAccessLevel::NoAccess,
        (_, Some(l)) => l,
        (_, None) => PropertyAccessLevel::NoAccess,
    }
}

fn max_access(a: PropertyAccessLevel, b: PropertyAccessLevel) -> PropertyAccessLevel {
    fn weight(l: PropertyAccessLevel) -> u8 {
        match l {
            PropertyAccessLevel::NoAccess => 0,
            PropertyAccessLevel::Read => 1,
            PropertyAccessLevel::WriteBeforePersist => 2,
            PropertyAccessLevel::Write => 3,
        }
    }
    if weight(a) >= weight(b) { a } else { b }
}

// =============================================================================
// Auth-Wire-Typen
// =============================================================================
//
// `AuthSession` ist das, was der Server nach erfolgreichem Login zurueckgibt.
// `token` ist ein zufaelliger, lang genug bemessener String (>= 32 Bytes,
// base64-kodiert), den der Client als `Authorization: Bearer <token>` an alle
// nachfolgenden Requests anhaengt. Server-seitige Validierung lebt im
// `server::auth`-Modul.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub token: String,
    pub user: SecurityUser,
    /// Effektive Permissions zur Session-Zeit. Der Client kann ohne Re-Fetch
    /// von `groups` UI gating durchfuehren.
    pub permissions: Vec<Permission>,
    /// ISO-8601-Zeitstempel; nach diesem Punkt MUSS der Client neu loggen.
    /// `None` heisst "keine harte Ablaufgrenze" (Mock-Default).
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// Fehler-Antwort fuer Login.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuthFailure {
    /// Username unbekannt oder Passwort falsch — gleiche Variante absichtlich,
    /// damit kein User-Enumeration-Leak entsteht.
    InvalidCredentials,
    /// Nutzer ist als `active = false` markiert.
    Inactive,
    /// Servern-interner Fehler.
    Internal,
}
