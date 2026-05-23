//! Save-Pipeline (Q0009 Phase 3.3).
//!
//! `save_script` ist die einzige Tuer fuer "neue Script-Version persistieren".
//! Sie macht den ganzen Reigen:
//!   1. Compile (Rhai) — Parse-Fehler ⇒ `state=Draft` mit `last_error`.
//!   2. Manifest-Validierung (Tier-Deck, Capability-Subset, UI-Subset,
//!      Timeout-/Memory-Caps).
//!   3. Lift-Analyse (`analyze_lift_capability`) — schreibt das Resultat in
//!      `manifest.lift_capable` zurueck.
//!   4. Version-Monotonie pruefen.
//!   5. Persistenz: `scripts` (Head-Zeile) + `script_versions` (Historie).
//!
//! Alle Steps sind in-memory; das Schreiben in die DB erledigt
//! [`persist_save`]. Tests koennen die reinen Analyse-Schritte ueber
//! [`prepare_save`] ohne DB nutzen.

use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};

use shared::script::engine::ScriptEngine;
use shared::script::error::{ManifestError, ScriptError};
use shared::script::manifest::ScriptManifest;
use shared::script::model::{Script, ScriptId, ScriptKind, ScriptState};
use shared::script::{
    capability::{default_tokens_for_tier, ScriptTier},
    CapabilityToken,
};

use crate::entity::{script as script_entity, script_version};
use crate::script::engine::rhai::{analyze_lift_capability, RhaiEngine};

/// Eingaben fuer den Save-Schritt.
pub struct SaveInput {
    pub id: ScriptId,
    pub source: String,
    pub manifest: ScriptManifest,
    pub kind: ScriptKind,
    pub user: ScriptTier,
    pub user_id: String,
    /// Bestehende Version (`None`, wenn das Skript neu ist). Neue Version
    /// muss `prev_version + 1` sein.
    pub prev_version: Option<u32>,
}

/// Resultat der reinen In-Memory-Analyse — vor dem DB-Write.
#[derive(Debug, Clone)]
pub struct PreparedSave {
    pub manifest: ScriptManifest,
    pub state: ScriptState,
    pub last_error: Option<ScriptError>,
    pub version: u32,
}

/// Validiert das Manifest gegen den User-Tier-Deckel und gegen die statisch
/// erlaubte Capability-Liste pro Tier. Zusaetzliche Caps darf nur ein hoher
/// Tier deklarieren.
fn validate_manifest(
    manifest: &ScriptManifest,
    user: ScriptTier,
) -> Result<(), ScriptError> {
    if manifest.tier > user {
        return Err(ScriptError::TierExceeded {
            declared: manifest.tier,
            user,
        });
    }
    let allowed = default_tokens_for_tier(manifest.tier);
    for token in &manifest.capabilities {
        if !allowed.iter().any(|a| token_eq(a, token)) {
            return Err(ScriptError::ManifestInvalid {
                reason: ManifestError {
                    reason: format!(
                        "capability {token:?} not allowed for tier {:?}",
                        manifest.tier
                    ),
                },
            });
        }
    }
    if let Some(t) = manifest.timeout_ms {
        // Cap: kein Skript darf laenger als 60s laufen, unabhaengig vom Tier.
        if t > 60_000 {
            return Err(ScriptError::ManifestInvalid {
                reason: ManifestError {
                    reason: format!("timeout_ms {t} exceeds 60_000 cap"),
                },
            });
        }
    }
    if let Some(k) = manifest.memory_kb {
        // Cap: 64 MB.
        if k > 64 * 1024 {
            return Err(ScriptError::ManifestInvalid {
                reason: ManifestError {
                    reason: format!("memory_kb {k} exceeds 65_536 cap"),
                },
            });
        }
    }
    Ok(())
}

/// Vergleicht zwei `CapabilityToken`s strukturell — gleichwertig zu `==`,
/// aber expliziter, damit das Default-Set ein Match-Verhalten hat.
fn token_eq(a: &CapabilityToken, b: &CapabilityToken) -> bool {
    use CapabilityToken::*;
    matches!(
        (a, b),
        (ReadOwnEntities, ReadOwnEntities)
            | (ReadAllEntitiesWhereAllowed, ReadAllEntitiesWhereAllowed)
            | (WriteEntity { .. }, WriteEntity { .. })
            | (ComputeOnly, ComputeOnly)
            | (ReadI18n, ReadI18n)
            | (EmitUiNode { .. }, EmitUiNode { .. })
            | (EmitWorkflowAction, EmitWorkflowAction)
            | (LoadOtherScript, LoadOtherScript)
            | (ReadAuditLog { .. }, ReadAuditLog { .. })
            | (WriteAuditLog, WriteAuditLog)
            | (RegisterHostFunction, RegisterHostFunction)
            | (ScheduleJob, ScheduleJob)
    )
}

/// Reine Analyse-Pipeline ohne DB-Zugriff: liefert ein `PreparedSave`, das
/// `persist_save` dann verbraucht. Idempotent — `manifest.lift_capable` wird
/// hier final ueberschrieben.
pub fn prepare_save(input: &SaveInput) -> PreparedSave {
    let mut manifest = input.manifest.clone();
    let next_version = input.prev_version.map(|v| v + 1).unwrap_or(1);

    // Wasm: heute hart abgelehnt (Spec §2).
    if let ScriptKind::Wasm { .. } = &input.kind {
        return PreparedSave {
            manifest,
            state: ScriptState::Draft,
            last_error: Some(ScriptError::WasmEngineNotAvailable),
            version: next_version,
        };
    }

    // Schritt 1: Compile.
    let engine = RhaiEngine::new();
    let ast = match engine.compile(&input.source, &manifest) {
        Ok(a) => a,
        Err(e) => {
            return PreparedSave {
                manifest,
                state: ScriptState::Draft,
                last_error: Some(e),
                version: next_version,
            }
        }
    };

    // Schritt 2: Manifest-Validierung.
    if let Err(e) = validate_manifest(&manifest, input.user) {
        return PreparedSave {
            manifest,
            state: ScriptState::Draft,
            last_error: Some(e),
            version: next_version,
        };
    }

    // Schritt 3: Lift-Analyse — und ins Manifest zurueckschreiben.
    manifest.lift_capable = analyze_lift_capability(&ast);

    PreparedSave {
        manifest,
        state: ScriptState::Active,
        last_error: None,
        version: next_version,
    }
}

/// Persistiert den vorbereiteten Save in beide Tabellen. `scripts` ist die
/// Head-Zeile (upsert: existiert eine Zeile, wird sie ueberschrieben).
/// `script_versions` ist append-only — die neue Version wird einfach
/// eingefuegt.
pub async fn persist_save(
    db: &DatabaseConnection,
    input: SaveInput,
    prepared: PreparedSave,
) -> Result<Script, sea_orm::DbErr> {
    let manifest_json = serde_json::to_string(&prepared.manifest)
        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
    let last_error_json = prepared
        .last_error
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

    let kind_value = serde_json::to_value(&input.kind)
        .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
    let kind_tag = kind_value
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("component")
        .to_string();

    let state_str = match prepared.state {
        ScriptState::Draft => "draft",
        ScriptState::Active => "active",
        ScriptState::Locked => "locked",
    }
    .to_string();

    let now = chrono::Utc::now().to_rfc3339();
    let script_id = input.id.0.clone();

    // ---- Head-Row upsert ----
    let existing = script_entity::Entity::find_by_id(script_id.clone())
        .one(db)
        .await?;
    let created_at = if let Some(r) = existing.as_ref() {
        r.created_at.clone()
    } else {
        now.clone()
    };
    if let Some(r) = existing {
        let mut am: script_entity::ActiveModel = r.into();
        am.kind = Set(kind_tag.clone());
        am.manifest_json = Set(manifest_json.clone());
        am.source = Set(input.source.clone());
        am.version = Set(prepared.version as i32);
        am.state = Set(state_str.clone());
        am.last_error = Set(last_error_json.clone());
        am.updated_at = Set(now.clone());
        am.update(db).await?;
    } else {
        script_entity::ActiveModel {
            id: Set(script_id.clone()),
            kind: Set(kind_tag.clone()),
            manifest_json: Set(manifest_json.clone()),
            source: Set(input.source.clone()),
            version: Set(prepared.version as i32),
            state: Set(state_str.clone()),
            last_error: Set(last_error_json.clone()),
            created_by: Set(input.user_id.clone()),
            created_at: Set(now.clone()),
            updated_at: Set(now.clone()),
        }
        .insert(db)
        .await?;
    }

    // ---- Versions-Row append ----
    script_version::ActiveModel {
        script_id: Set(script_id.clone()),
        version: Set(prepared.version as i32),
        manifest_json: Set(manifest_json.clone()),
        source: Set(input.source.clone()),
        state_at_save: Set(state_str.clone()),
        last_error: Set(last_error_json.clone()),
        saved_by: Set(input.user_id.clone()),
        saved_at: Set(now.clone()),
    }
    .insert(db)
    .await?;

    Ok(Script {
        id: input.id,
        kind: input.kind,
        manifest: prepared.manifest,
        source: input.source,
        version: prepared.version,
        state: prepared.state,
        last_error: prepared.last_error,
        created_by: input.user_id,
        created_at,
        updated_at: now,
    })
}

/// High-Level-API: prepare + persist. Lehnt nicht-monoton wachsende
/// Versionen mit einem `sea_orm::DbErr::Custom` ab (das ist eine
/// Programmierer-Fehlbenutzung, kein Script-Fehler).
pub async fn save_script(
    db: &DatabaseConnection,
    input: SaveInput,
) -> Result<Script, sea_orm::DbErr> {
    if let Some(prev) = input.prev_version {
        // Pruefen, dass die existierende DB-Row tatsaechlich `prev` ist.
        let existing = script_entity::Entity::find_by_id(input.id.0.clone())
            .one(db)
            .await?;
        if let Some(row) = existing {
            if row.version as u32 != prev {
                return Err(sea_orm::DbErr::Custom(format!(
                    "version conflict: prev_version={prev} but DB has version={}",
                    row.version
                )));
            }
        } else {
            return Err(sea_orm::DbErr::Custom(format!(
                "prev_version={prev} angefordert, aber Script '{}' existiert nicht",
                input.id.0
            )));
        }
    }
    let prepared = prepare_save(&input);
    persist_save(db, input, prepared).await
}
