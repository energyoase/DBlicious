//! Tier- und Capability-Token-Definitionen.
//!
//! `CapabilityToken` ist ein getaggter Enum mit `#[serde(tag = "kind",
//! rename_all = "camelCase")]`. Inner-Field-Konvention wie bei `FieldType`:
//! die Felder einer Struct-Variante bleiben snake_case.

use serde::{Deserialize, Serialize};

/// Berechtigungs-Stufe eines Skripts. Deckel — nicht Default — fuer die
/// Tokens, die das Manifest deklarieren darf.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum ScriptTier {
    Reader,
    Author,
    Developer,
    Admin,
}

/// Eine spezifische Capability, die das Manifest deklarieren muss, damit
/// das Skript sie nutzen darf. Die Liste ist abschliessend: ein
/// Exhaustiveness-Match-Test in `script_wire_format.rs` enumeriert jede
/// Variante.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CapabilityToken {
    ReadOwnEntities,
    ReadAllEntitiesWhereAllowed,
    WriteEntity { validated: bool },
    ComputeOnly,
    ReadI18n,
    EmitUiNode { scope: UiScope },
    EmitWorkflowAction,
    LoadOtherScript,
    ReadAuditLog { own_only: bool },
    WriteAuditLog,
    RegisterHostFunction,
    ScheduleJob,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum UiScope {
    Leaf,
    Composite,
}

/// Liefert den maximalen Default-Token-Set fuer einen Tier (gemaess Spec §4.1).
/// Manifest darf nur Tokens daraus deklarieren.
pub fn default_tokens_for_tier(tier: ScriptTier) -> Vec<CapabilityToken> {
    use CapabilityToken::*;
    let mut out = vec![
        ReadOwnEntities,
        ReadI18n,
        ComputeOnly,
        EmitUiNode { scope: UiScope::Leaf },
    ];
    if tier >= ScriptTier::Author {
        out.push(ReadAllEntitiesWhereAllowed);
        out.push(EmitUiNode { scope: UiScope::Composite });
        out.push(ReadAuditLog { own_only: true });
    }
    if tier >= ScriptTier::Developer {
        out.push(WriteEntity { validated: true });
        out.push(EmitWorkflowAction);
        out.push(LoadOtherScript);
    }
    if tier >= ScriptTier::Admin {
        out.push(WriteAuditLog);
        out.push(RegisterHostFunction);
        out.push(ScheduleJob);
    }
    out
}
