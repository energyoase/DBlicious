//! Run-Pipeline (Q0009 Phase 3.4).
//!
//! `run_and_persist` ist die einzige Tuer fuer "Skript ausfuehren + Audit
//! schreiben". Sie zaehlt:
//!   1. Engine instantiieren + Source compilen.
//!   2. Sandbox aufsetzen (Token-Buffer + Deadline + Panic-Catch).
//!   3. Den User-Body laufen lassen — der wickelt seine Host-Calls in
//!      `sandbox.gate()` ein, damit der Audit-Buffer gefuellt wird.
//!   4. Outcome auswerten + 1 Row in `script_audit_log` schreiben.
//!
//! Heutige Engine-Integration (Phase 3.4) ist absichtlich minimal: der
//! Engine-`run`-Pfad ruft *noch keine* Host-Funktionen durch die Sandbox,
//! daher wird die `body`-Closure als expliziter Hook angeboten. Phase 4-6
//! ziehen den Host-Call-Pfad zentral durch den Engine-Adapter und machen
//! diese Closure ueberfluessig.

use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use ulid::Ulid;

use shared::script::engine::{HostApi, ScriptCtx, ScriptEngine, ScriptValue};
use shared::script::error::ScriptError;
use shared::script::model::Script;

use crate::entity::script_audit_log;
use crate::script::engine::rhai::RhaiEngine;
use crate::script::sandbox::{Sandbox, TokenOutcome, TokenUse};

/// Outcome-Tag fuer die `script_audit_log.outcome`-Spalte. Wir mappen
/// `ScriptError::*` auf den jeweiligen camelCase-Tag der Variante (vgl.
/// `error.rs`-Wire-Form).
fn outcome_tag(err: &Option<ScriptError>) -> &'static str {
    match err {
        None => "ok",
        Some(e) => match e {
            ScriptError::ParseFailed { .. } => "parseFailed",
            ScriptError::ManifestInvalid { .. } => "manifestInvalid",
            ScriptError::TierExceeded { .. } => "tierExceeded",
            ScriptError::WasmEngineNotAvailable => "wasmEngineNotAvailable",
            ScriptError::CapabilityDenied { .. } => "capabilityDenied",
            ScriptError::UiPrimitiveDenied { .. } => "uiPrimitiveDenied",
            ScriptError::ServerOnlyFunction { .. } => "serverOnlyFunction",
            ScriptError::Timeout { .. } => "timeout",
            ScriptError::MemoryExceeded { .. } => "memoryExceeded",
            ScriptError::InternalPanic { .. } => "internalPanic",
            ScriptError::HostError { .. } => "hostError",
            ScriptError::ValidationFailed { .. } => "validationFailed",
        },
    }
}

/// Serialisiert eine Token-Use-Liste in das JSON-Format, das in
/// `script_audit_log.tokens_used` landet.
fn tokens_used_to_json(uses: &[TokenUse]) -> Result<String, sea_orm::DbErr> {
    let arr: Vec<serde_json::Value> = uses
        .iter()
        .map(|tu| {
            serde_json::json!({
                "token": tu.token,
                "outcome": match tu.outcome {
                    TokenOutcome::Ok => "ok",
                    TokenOutcome::Denied => "denied",
                },
            })
        })
        .collect();
    serde_json::to_string(&arr).map_err(|e| sea_orm::DbErr::Custom(e.to_string()))
}

/// Result eines Runs. `value` ist `Some` bei Erfolg, `error` bei Misserfolg.
/// `audit_id` ist der DB-Primary-Key der frisch geschriebenen Audit-Zeile.
#[derive(Debug)]
pub struct RunRecord {
    pub run_id: String,
    pub audit_id: i64,
    pub outcome: String,
    pub value: Option<ScriptValue>,
    pub error: Option<ScriptError>,
}

/// Result eines server-seitigen Preview-Runs (Q0009 Phase 6, Spec §9).
///
/// Identische Outcome-Berechnung wie [`run_and_persist`], aber **ohne**
/// `script_audit_log`-Insert. Wird vom `previewScriptRun`-Mutator benutzt,
/// damit Builder/Inspector ein Skript probefahren koennen, ohne dass
/// jeder Klick einen Audit-Eintrag erzeugt.
///
/// Zusatz: `WriteEntity`-Capability ist im Preview hart verboten. Der
/// Aufrufer muss vor `run_preview` selbst pruefen — der Run-Pfad gated
/// gegen das Manifest, was hier nicht reicht (ein boeses Skript koennte
/// `WriteEntity` deklarieren). Der GraphQL-Mutator [`schema::MutationRoot::preview_script_run`]
/// rejected daher Skripte mit `WriteEntity` schon vor dem Compile.
#[derive(Debug, Clone)]
pub struct PreviewRecord {
    pub run_id: String,
    pub outcome: String,
    pub value: Option<ScriptValue>,
    pub error: Option<ScriptError>,
    /// Tokens-Used als JSON-Array (gleiches Format wie `script_audit_log.tokens_used`).
    pub tokens_used: serde_json::Value,
    /// Dauer in Millisekunden zwischen `started_at` und `finished_at`.
    pub duration_ms: i64,
}

/// Wickelt einen Script-Run end-to-end. Die `body`-Closure bekommt die
/// Engine, den AST und die Sandbox — die ist der "richtige" Ort, an dem
/// das aufrufende Modul seine gate()-Aufrufe macht (Phase 3.4-Provisorium,
/// vgl. Modul-Header).
///
/// Outcome-Mapping: `Ok(_)` -> "ok", `Err(e)` -> `outcome_tag(Some(e))`.
pub async fn run_and_persist<F>(
    db: &DatabaseConnection,
    script: &Script,
    ctx: ScriptCtx,
    _host: &dyn HostApi,
    body: F,
) -> Result<RunRecord, sea_orm::DbErr>
where
    F: for<'s, 'm> FnOnce(
        &RhaiEngine,
        &<RhaiEngine as ScriptEngine>::Ast,
        &mut Sandbox<'m>,
        &ScriptCtx,
    ) -> Result<ScriptValue, ScriptError>,
{
    let engine = RhaiEngine::new();
    let started_at = chrono::Utc::now().to_rfc3339();
    let run_id = Ulid::new().to_string();

    // ---- Compile ----
    let ast_result = engine.compile(&script.source, &script.manifest);

    // Bei Compile-Fehler erzeugen wir trotzdem einen Audit-Eintrag,
    // damit jeder Run-Versuch nachvollziehbar bleibt.
    let (outcome_str, value, error, token_uses) = match ast_result {
        Err(e) => (
            outcome_tag(&Some(e.clone())).to_string(),
            None,
            Some(e),
            Vec::new(),
        ),
        Ok(ast) => {
            let mut sb = Sandbox::new(&script.manifest);
            match body(&engine, &ast, &mut sb, &ctx) {
                Ok(v) => ("ok".to_string(), Some(v), None, sb.token_uses().to_vec()),
                Err(e) => (
                    outcome_tag(&Some(e.clone())).to_string(),
                    None,
                    Some(e),
                    sb.token_uses().to_vec(),
                ),
            }
        }
    };

    let finished_at = chrono::Utc::now().to_rfc3339();
    let tokens_used_json = tokens_used_to_json(&token_uses)?;

    // ---- Audit-Row append ----
    let am = script_audit_log::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        script_id: Set(script.id.0.clone()),
        script_version: Set(script.version as i32),
        run_id: Set(run_id.clone()),
        user_id: Set(ctx.user_id.clone()),
        started_at: Set(started_at),
        finished_at: Set(finished_at),
        outcome: Set(outcome_str.clone()),
        tokens_used: Set(tokens_used_json),
        custom_events: Set("[]".into()),
    };
    let inserted = am.insert(db).await?;

    Ok(RunRecord {
        run_id,
        audit_id: inserted.id,
        outcome: outcome_str,
        value,
        error,
    })
}

/// Server-seitiger Preview-Run (Q0009 Phase 6).
///
/// Wickelt dasselbe Compile/Sandbox/Body-Triple wie [`run_and_persist`], aber
/// schreibt KEINE Audit-Zeile. Der Aufrufer (`preview_script_run`-Mutator)
/// bekommt das volle Outcome inklusive Tokens-Used als JSON zurueck — fuer
/// die UI-Anzeige reicht das, fuer eine richtige Run-Spur nimmt der Aufrufer
/// `run_and_persist`.
pub fn run_preview<F>(
    script: &Script,
    ctx: ScriptCtx,
    _host: &dyn HostApi,
    body: F,
) -> PreviewRecord
where
    F: for<'s, 'm> FnOnce(
        &RhaiEngine,
        &<RhaiEngine as ScriptEngine>::Ast,
        &mut Sandbox<'m>,
        &ScriptCtx,
    ) -> Result<ScriptValue, ScriptError>,
{
    let engine = RhaiEngine::new();
    let started_at = chrono::Utc::now();
    let run_id = Ulid::new().to_string();

    let ast_result = engine.compile(&script.source, &script.manifest);

    let (outcome_str, value, error, token_uses) = match ast_result {
        Err(e) => (
            outcome_tag(&Some(e.clone())).to_string(),
            None,
            Some(e),
            Vec::new(),
        ),
        Ok(ast) => {
            let mut sb = Sandbox::new(&script.manifest);
            match body(&engine, &ast, &mut sb, &ctx) {
                Ok(v) => ("ok".to_string(), Some(v), None, sb.token_uses().to_vec()),
                Err(e) => (
                    outcome_tag(&Some(e.clone())).to_string(),
                    None,
                    Some(e),
                    sb.token_uses().to_vec(),
                ),
            }
        }
    };

    let finished_at = chrono::Utc::now();
    let duration_ms = (finished_at - started_at).num_milliseconds();

    // Token-Use-Liste in dasselbe JSON-Format wie `script_audit_log.tokens_used`
    // serialisieren — wir parsen es danach gleich wieder zu serde_json::Value
    // zurueck, damit GraphQL Json<Value> ohne Doppelserialisierung serialisieren
    // kann.
    let tokens_json_str = tokens_used_to_json(&token_uses).unwrap_or_else(|_| "[]".to_string());
    let tokens_used: serde_json::Value =
        serde_json::from_str(&tokens_json_str).unwrap_or(serde_json::Value::Array(Vec::new()));

    PreviewRecord {
        run_id,
        outcome: outcome_str,
        value,
        error,
        tokens_used,
        duration_ms,
    }
}

// Manueller Clone fuer TokenUse, weil das Tupel `(token, outcome)`
// dieselben Felder hat — die Source-Datei traegt schon `Clone` mit `derive`.
// (Wir nutzen das nur falls noetig — kein zusaetzlicher Code hier.)
