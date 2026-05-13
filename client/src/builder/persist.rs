//! Builder-Persistenz-Helfer (Phase 1.6 Client-Teil).
//!
//! Brueckt das `UiTree`-Wire-Format zum Server-`state`-Blob (siehe
//! ROADMAP-Spec 1.6):
//!
//! ```json
//! {
//!   "schemaVersion": 1,
//!   "tree":  { "nodes": [...], "nextId": 5 },
//!   "projection": { "columns": [...], "settings": null, "editor": null }
//! }
//! ```
//!
//! Server-API:
//!   - [`crate::graphql::queries::fetch_entity_design`]      — Load
//!   - [`crate::graphql::queries::save_entity_design`]       — Save
//!   - [`crate::graphql::queries::revert_entity_design`]     — Revert
//!
//! `save_tree` baut den Blob aus dem aktuellen `UiTree`, ruft die Mutation
//! und liefert ein Ergebnis-Enum, das die UI-Reaktion (Konflikt-Banner,
//! Erfolgs-Meldung) typensicher macht.

use crate::builder::{project_columns, UiTree, TREE_SCHEMA_VERSION};
use crate::graphql::queries::{
    fetch_entity_design, save_entity_design, EntityDesign, SaveEntityDesignResult,
};
use crate::graphql::GqlError;

/// Erfolgs-/Fehlerfall der Save-Operation.
pub enum SaveOutcome {
    /// Neue Version wurde geschrieben.
    Ok { design: EntityDesign },
    /// Server-Seite hat eine neuere Version — der Client muss seinen
    /// `expected_version`-Stand aktualisieren oder die Server-Version laden.
    Conflict { current: EntityDesign },
    /// Plugin ist als `locked` markiert (Phase-4-Codegen-Snapshot).
    Locked { current: Option<EntityDesign> },
    /// Sonstige Fehler.
    Error(String),
}

/// Baut den `state`-Blob aus dem aktuellen Tree-Stand.
///
/// `projection` enthaelt heute nur `columns` — `settings` und `editor`
/// sind heute `null` (werden in spaeteren Iterationen aus dem Builder
/// gespeist).
pub fn build_state_blob(tree: &UiTree) -> serde_json::Value {
    let projection = serde_json::json!({
        "columns": project_columns(tree),
        "settings": serde_json::Value::Null,
        "editor": serde_json::Value::Null,
    });
    serde_json::json!({
        "schemaVersion": TREE_SCHEMA_VERSION,
        "tree": tree,
        "projection": projection,
    })
}

/// Speichert den aktuellen Tree-Stand fuer `entity_type`. `expected_version`
/// stammt aus der zuletzt geladenen Version (`None` = "ich denke, es
/// gibt noch nichts" — selten richtig, weil der Server beim Boot eine
/// version=0 anlegt).
pub async fn save_tree(
    entity_type: &str,
    tree: &UiTree,
    expected_version: Option<i32>,
) -> Result<SaveOutcome, GqlError> {
    let state = build_state_blob(tree);
    let res = save_entity_design(
        entity_type,
        TREE_SCHEMA_VERSION as i32,
        state,
        expected_version,
    )
    .await?;
    Ok(classify(res))
}

/// Laedt die aktive Version. Tree wird aus `state.tree` deserialisiert;
/// bei deserialisierungsfehler wird ein leerer Tree zurueckgeliefert
/// (der Aufrufer sollte das nur tun, wenn `design.state` der erwartete
/// `schemaVersion` ist).
pub async fn load_tree(entity_type: &str) -> Result<Option<(EntityDesign, UiTree)>, GqlError> {
    let Some(design) = fetch_entity_design(entity_type).await? else {
        return Ok(None);
    };
    let tree = design
        .state
        .get("tree")
        .and_then(|v| serde_json::from_value::<UiTree>(v.clone()).ok())
        .unwrap_or_else(UiTree::empty);
    Ok(Some((design, tree)))
}

fn classify(res: SaveEntityDesignResult) -> SaveOutcome {
    if res.ok {
        return SaveOutcome::Ok {
            design: res.design.expect("ok=true must carry design"),
        };
    }
    match res.error.as_deref() {
        Some("concurrent_design_modification") => {
            if let Some(current) = res.conflict_current {
                SaveOutcome::Conflict { current }
            } else {
                SaveOutcome::Error("conflict without current state".to_string())
            }
        }
        Some("locked") => SaveOutcome::Locked {
            current: res.conflict_current,
        },
        Some(code) => SaveOutcome::Error(code.to_string()),
        None => SaveOutcome::Error("unknown".to_string()),
    }
}
