//! Plugin-Manager (Phase 2.1).
//!
//! Bauteile:
//!   - [`install_plugin`]: Manifest validieren + WASM persistieren.
//!   - [`list_plugins`] / [`get_plugin`] / [`set_enabled`] / [`delete_plugin`]:
//!     CRUD ueber die `plugins`-Tabelle.
//!   - [`call_function`]: laedt das WASM-Blob ueber Extism, ruft eine
//!     Function mit JSON-Input und liefert JSON-Output.
//!   - [`record_invocation`]: schreibt einen Eintrag in `plugin_invocations`
//!     fuer Audit.
//!
//! Trigger-Integration in den CRUD-Resolvern kommt in Phase 2.3.

use std::time::Instant;

use chrono::Utc;
use extism::{Manifest as ExtismManifest, Plugin as ExtismPlugin, Wasm};
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};
use serde_json::Value;
use shared::plugin::PluginManifest;

use crate::db::conn;
use crate::entity;

#[derive(Debug)]
pub enum PluginError {
    InvalidManifest(String),
    InvalidWasm(String),
    NotFound,
    Disabled,
    Timeout,
    Capability(String),
    Execution(String),
    Db(sea_orm::DbErr),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::InvalidManifest(m) => write!(f, "invalid_manifest: {m}"),
            PluginError::InvalidWasm(m) => write!(f, "invalid_wasm: {m}"),
            PluginError::NotFound => write!(f, "not_found"),
            PluginError::Disabled => write!(f, "disabled"),
            PluginError::Timeout => write!(f, "timeout"),
            PluginError::Capability(m) => write!(f, "capability_violation: {m}"),
            PluginError::Execution(m) => write!(f, "execution: {m}"),
            PluginError::Db(e) => write!(f, "db: {e}"),
        }
    }
}

impl std::error::Error for PluginError {}

impl From<sea_orm::DbErr> for PluginError {
    fn from(e: sea_orm::DbErr) -> Self {
        PluginError::Db(e)
    }
}

/// Installiert ein neues Plugin oder ersetzt eine bestehende Version mit
/// gleicher ID. Validiert das Manifest (Pflichtfelder + api_version) und
/// versucht, das WASM-Blob via Extism zu laden, um sicherzustellen, dass
/// es gueltig ist. WASM-Wegwerf-Plugin nur als Validations-Probe; das
/// echte Laden pro Call passiert in [`call_function`].
pub async fn install_plugin(
    manifest: PluginManifest,
    wasm: Vec<u8>,
) -> Result<entity::plugins::Model, PluginError> {
    // 1) Manifest-Plausibilitaet
    if manifest.id.is_empty() {
        return Err(PluginError::InvalidManifest("id is empty".into()));
    }
    if manifest.version.is_empty() {
        return Err(PluginError::InvalidManifest("version is empty".into()));
    }
    if manifest.api_version != 1 {
        return Err(PluginError::InvalidManifest(format!(
            "unsupported api_version {} (only 1 known)",
            manifest.api_version
        )));
    }

    // 2) WASM-Smoke-Test: laedt die Binary einmal ein. Wenn das wirft, ist
    //    sie nicht installierbar.
    let probe_manifest = ExtismManifest::new([Wasm::data(wasm.clone())]);
    ExtismPlugin::new(&probe_manifest, [], true)
        .map_err(|e| PluginError::InvalidWasm(e.to_string()))?;

    // 3) Persistieren (upsert by id).
    let manifest_json = serde_json::to_string(&manifest)
        .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;
    let now = Utc::now().to_rfc3339();
    let db = conn();
    let existing = entity::plugins::Entity::find_by_id(manifest.id.clone())
        .one(&db)
        .await?;
    let model = match existing {
        Some(prev) => {
            let mut am: entity::plugins::ActiveModel = prev.into();
            am.version = ActiveValue::Set(manifest.version.clone());
            am.manifest_json = ActiveValue::Set(manifest_json);
            am.wasm_blob = ActiveValue::Set(wasm);
            am.installed_at = ActiveValue::Set(now);
            am.update(&db).await?
        }
        None => entity::plugins::ActiveModel {
            id: ActiveValue::Set(manifest.id.clone()),
            version: ActiveValue::Set(manifest.version.clone()),
            manifest_json: ActiveValue::Set(manifest_json),
            wasm_blob: ActiveValue::Set(wasm),
            enabled: ActiveValue::Set(true),
            installed_at: ActiveValue::Set(now),
        }
        .insert(&db)
        .await?,
    };
    Ok(model)
}

pub async fn list_plugins() -> Result<Vec<entity::plugins::Model>, PluginError> {
    Ok(entity::plugins::Entity::find().all(&conn()).await?)
}

pub async fn get_plugin(id: &str) -> Result<Option<entity::plugins::Model>, PluginError> {
    Ok(entity::plugins::Entity::find_by_id(id.to_string())
        .one(&conn())
        .await?)
}

pub async fn set_enabled(id: &str, enabled: bool) -> Result<bool, PluginError> {
    let Some(model) = get_plugin(id).await? else { return Err(PluginError::NotFound) };
    let mut am: entity::plugins::ActiveModel = model.into();
    am.enabled = ActiveValue::Set(enabled);
    am.update(&conn()).await?;
    Ok(true)
}

pub async fn delete_plugin(id: &str) -> Result<bool, PluginError> {
    let res = entity::plugins::Entity::delete_by_id(id.to_string())
        .exec(&conn())
        .await?;
    Ok(res.rows_affected > 0)
}

/// Ruft eine WASM-Funktion eines installierten Plugins mit JSON-Input.
///
/// Heute minimal: kein Capability-Check, kein Timeout-Enforcement, keine
/// Host-Functions. Phase 2.3/2.4 erweitern das.
pub async fn call_function(
    plugin_id: &str,
    function_name: &str,
    input: &Value,
) -> Result<Value, PluginError> {
    let Some(model) = get_plugin(plugin_id).await? else { return Err(PluginError::NotFound) };
    if !model.enabled {
        return Err(PluginError::Disabled);
    }
    let manifest_extism = ExtismManifest::new([Wasm::data(model.wasm_blob.clone())]);
    let mut plugin = ExtismPlugin::new(&manifest_extism, [], true)
        .map_err(|e| PluginError::InvalidWasm(e.to_string()))?;
    let input_bytes = serde_json::to_vec(input)
        .map_err(|e| PluginError::Execution(format!("input encode: {e}")))?;
    let raw = plugin
        .call::<&[u8], &[u8]>(function_name, &input_bytes)
        .map_err(|e| PluginError::Execution(e.to_string()))?;
    let parsed: Value = serde_json::from_slice(raw)
        .map_err(|e| PluginError::Execution(format!("output decode: {e}")))?;
    Ok(parsed)
}

/// Schreibt einen Eintrag ins plugin_invocations-Audit-Log. Best-effort —
/// Fehler werden als tracing::warn geloggt und nicht propagiert.
pub async fn record_invocation(
    plugin_id: &str,
    function_name: &str,
    trigger_kind: &str,
    entity_type: Option<&str>,
    actor_user_id: Option<&str>,
    started_at: chrono::DateTime<Utc>,
    duration_ms: i32,
    outcome: &str,
    input_hash: Option<&str>,
    error: Option<(&str, &str)>,
) {
    use sea_orm::ActiveModelTrait;
    let am = entity::plugin_invocations::ActiveModel {
        id: ActiveValue::NotSet,
        plugin_id: ActiveValue::Set(plugin_id.to_string()),
        function_name: ActiveValue::Set(function_name.to_string()),
        trigger_kind: ActiveValue::Set(trigger_kind.to_string()),
        entity_type: ActiveValue::Set(entity_type.map(str::to_string)),
        actor_user_id: ActiveValue::Set(actor_user_id.map(str::to_string)),
        started_at: ActiveValue::Set(started_at.to_rfc3339()),
        duration_ms: ActiveValue::Set(duration_ms),
        outcome: ActiveValue::Set(outcome.to_string()),
        input_hash: ActiveValue::Set(input_hash.map(str::to_string)),
        error_code: ActiveValue::Set(error.map(|(c, _)| c.to_string())),
        error_message: ActiveValue::Set(error.map(|(_, m)| m.to_string())),
    };
    if let Err(e) = am.insert(&conn()).await {
        tracing::warn!(target: "server::plugins", "plugin_invocations insert failed: {e}");
    }
}

/// Conveniance-Helper: misst die Dauer, ruft die Funktion, loggt das
/// Resultat ins Audit-Log. Wird ab Phase 2.3 von den Trigger-Punkten
/// gerufen.
pub async fn call_with_audit(
    plugin_id: &str,
    function_name: &str,
    trigger_kind: &str,
    entity_type: Option<&str>,
    actor_user_id: Option<&str>,
    input: &Value,
) -> Result<Value, PluginError> {
    let started = Utc::now();
    let inst = Instant::now();
    let result = call_function(plugin_id, function_name, input).await;
    let dur = inst.elapsed().as_millis() as i32;
    let outcome = match &result {
        Ok(_) => "ok",
        Err(PluginError::Timeout) => "timeout",
        Err(PluginError::Capability(_)) => "capability_violation",
        Err(_) => "error",
    };
    let err_info = match &result {
        Err(e) => Some(("execution", e.to_string())),
        Ok(_) => None,
    };
    record_invocation(
        plugin_id,
        function_name,
        trigger_kind,
        entity_type,
        actor_user_id,
        started,
        dur,
        outcome,
        None,
        err_info.as_ref().map(|(c, m)| (*c, m.as_str())),
    )
    .await;
    result
}

/// Manifest aus dem JSON-Text der DB-Zeile deserialisieren.
pub fn parse_manifest(model: &entity::plugins::Model) -> Result<PluginManifest, PluginError> {
    serde_json::from_str(&model.manifest_json)
        .map_err(|e| PluginError::InvalidManifest(e.to_string()))
}
