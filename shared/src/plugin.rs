//! Plugin-Manifest und Trigger-Vertraege (Phase 2.2).
//!
//! Wire-Format zwischen Plugin-Hochlader und Server. Die hier definierten
//! Typen werden auf Server-Seite zur Validierung und Capability-Pruefung
//! benutzt; die Client-Seite (Admin-UI) zeigt sie zur Information.
//!
//! Der eigentliche Plugin-Code ist eine WASM-Binary, die zusammen mit dem
//! Manifest hochgeladen wird. Das Manifest steckt in der `plugin.toml`
//! innerhalb des WASM-Bundles (Extism-Konvention), wird beim Upload
//! geparst und in der `plugins`-Tabelle als JSON persistiert.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Wurzel-Manifest. Format mirrors the ROADMAP-Spec (`plugin.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Eindeutige Plugin-ID, z.B. `"com.example.slug-generator"`.
    /// Identitaet ueber Reverse-DNS-Style, damit Marketplace-Kollisionen
    /// unwahrscheinlich werden.
    pub id: String,
    /// SemVer-Version des Plugins (`"1.2.3"`).
    pub version: String,
    /// Host-API-Version, gegen die das Plugin geschrieben wurde.
    /// Server lehnt inkompatible Versions ab.
    pub api_version: u32,

    #[serde(default)]
    pub compatibility: Compatibility,
    #[serde(default)]
    pub capabilities: Capabilities,
    /// Funktionsnamen → Trigger-Mapping. Schluessel = Funktionsname im
    /// WASM-Export, Wert = wann und wofuer der Aufruf greifen soll.
    #[serde(default)]
    pub functions: BTreeMap<String, FunctionDef>,
    /// Optional: Signatur fuer Marketplace-Distribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing: Option<Signing>,
}

/// Kompatibilitaets-Bereich des Plugins.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Compatibility {
    /// SemVer-Range gegen den dblicious-Server-Build, z.B. `">=0.2, <0.3"`.
    /// Leer = "irgendeine Version" (nur fuer Dev empfohlen).
    #[serde(default)]
    pub dblicious: String,
    /// Plugin-Dependencies in der Form `"<id> <range>"`, z.B.
    /// `"com.example.locale-utils ^1"`. Topologisch geladen; Zyklen oder
    /// fehlende Versionen disablen das Plugin.
    #[serde(default)]
    pub plugins: Vec<String>,
}

/// Trigger-Kategorie. Bestimmt, **wann** eine Plugin-Funktion gerufen wird.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum TriggerKind {
    /// Vor einem Create/Update — kann `fields_after` mutieren und/oder
    /// Validation-Fehler werfen. Sync, Latency-Budget per Manifest.
    BeforeSave,
    /// Nach einem erfolgreichen Create/Update. Async (fire-and-forget),
    /// Audit-only.
    AfterSave,
    /// Vor einem Delete. Kann ablehnen.
    BeforeDelete,
    /// Berechnet ein Feld aus anderen Feldern derselben Entitaet
    /// (`target_field`, `from_field`).
    DeriveField,
    /// Reine Validierungs-Funktion. Sync, gibt Fehler-Liste zurueck.
    Validate,
    /// Vom Client explizit getriggerte Aktion (z.B. Toolbar-Knopf).
    CustomAction,
}

/// Vom Plugin angeforderte Capabilities. Whitelist — was nicht hier steht,
/// darf das Plugin nicht.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// Welche Trigger das Plugin abonniert.
    #[serde(default)]
    pub triggers: Vec<TriggerKind>,
    /// Entity-Typen mit Lese-Capability. `"*"` als Eintrag = alle.
    #[serde(default)]
    pub db_read: Vec<String>,
    /// Entity-Typen mit Schreib-Capability.
    #[serde(default)]
    pub db_write: Vec<String>,
    /// Erlaubte HTTP-Hosts/-Pfade (Glob-Pattern).
    #[serde(default)]
    pub http_fetch: Vec<String>,
    /// Erlaubte Filesystem-Pfade (heute nicht enforced, Schema-Vorbereitung).
    #[serde(default)]
    pub fs_paths: Vec<String>,
    /// Extism-Memory-Limit in 64 KiB-Pages. Default 16 (= 1 MiB).
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    /// Hartes Timeout pro Trigger-Aufruf. Default 200 ms (sync).
    #[serde(default = "default_max_runtime_ms")]
    pub max_runtime_ms: u32,
}

fn default_max_pages() -> u32 {
    16
}
fn default_max_runtime_ms() -> u32 {
    200
}

/// Mapping von Funktion → Trigger + Kontext.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDef {
    pub trigger: TriggerKind,
    /// Wenn `trigger = DeriveField`: Zielfeld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_field: Option<String>,
    /// Wenn `trigger = DeriveField`: Quellfeld (optional, default = alle Felder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_field: Option<String>,
    /// Auf welchen Entity-Typ greift der Trigger. Leer = alle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_types: Vec<String>,
}

/// Optionale Plugin-Signatur fuer Marketplace-Distribution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Signing {
    /// Algorithmus-Praefix + Schluesselmaterial, z.B. `"ed25519:<base64>"`.
    pub public_key: String,
    /// Signatur ueber das WASM-Modul (base64).
    pub signature: String,
}

// =============================================================================
// Trigger-Input/Output-Vertraege
// =============================================================================
//
// Plugin liest stdin als JSON, schreibt stdout als JSON. Diese Strukturen
// definieren das Wire-Format pro Trigger. Pluggins muessen die `*Input`-
// Struktur akzeptieren und die `*Output`-Struktur emittieren.

/// Input fuer `BeforeSave`-Trigger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BeforeSaveInput {
    pub entity_type: String,
    /// Stand vor dem Save (None bei Create).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields_before: Option<serde_json::Map<String, serde_json::Value>>,
    /// Stand nach dem Save (vom Client geliefert).
    pub fields_after: serde_json::Map<String, serde_json::Value>,
    /// User-ID des Aufrufers.
    pub user: String,
}

/// Output fuer `BeforeSave`. Wenn `fields_after` `Some`, ersetzt der Server
/// die Felder; wenn `validation` `Some` mit Fehlern, wird der Save
/// abgebrochen.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BeforeSaveOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields_after: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationFromPlugin>,
}

/// Input fuer `DeriveField`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeriveFieldInput {
    pub entity_type: String,
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub target_field: String,
}

/// Output: der berechnete Wert fuer das Zielfeld.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeriveFieldOutput {
    pub value: serde_json::Value,
}

/// Input fuer `Validate`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidateInput {
    pub entity_type: String,
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub user: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFromPlugin {
    pub errors: Vec<ValidationErrorFromPlugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationErrorFromPlugin {
    /// Property-Key (None = entity-level Fehler).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Maschinen-lesbarer Code, z.B. `"slug_collision"`.
    pub code: String,
    /// Menschlich-lesbare Nachricht (optional, vorzugsweise i18n-Key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Generischer Plugin-Fehler. Wird vom Host in einen GraphQL-Error mit
/// `extensions.code = error.code` umgewandelt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Wrapper fuer Plugin-Output: entweder `data` oder `error`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginResponse<T> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<PluginError>,
}
