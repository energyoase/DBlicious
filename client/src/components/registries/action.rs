//! Action-Registry (Phase 1.5.4).
//!
//! Per-Row-Aktionen, die ueber `ColumnMeta.action_ids` einer Spalte
//! zugeordnet werden. Beispiele: `edit-link`, `delete-button`, `copy-id`,
//! `export-row`.

use std::collections::BTreeMap;
use std::sync::Arc;

use leptos::prelude::*;
use shared::Entity;

#[derive(Clone)]
pub struct ActionContext {
    pub entity_type: String,
    pub entity: Entity,
}

pub type ActionFactory = Arc<dyn Fn(ActionContext) -> AnyView + Send + Sync>;

#[derive(Clone, Default)]
pub struct ActionRegistry {
    entries: BTreeMap<String, ActionFactory>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, id: impl Into<String>, factory: ActionFactory) {
        self.entries.insert(id.into(), factory);
    }
    pub fn get(&self, id: &str) -> Option<&ActionFactory> {
        self.entries.get(id)
    }
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

/// Standard-Actions:
/// - `edit-link`     — Link `/entities/<type>/<id>`.
/// - `show-id`       — Reine Anzeige der ID (monospaced).
///
/// `copy-to-clipboard` ist absichtlich nicht im Default — die Browser-
/// Clipboard-API ist Permission-pflichtig und der Fallback-Pfad
/// (document.execCommand) ist deprecated. Wer Clipboard-Support braucht,
/// registriert eine eigene Factory mit konkretem Browser-Adapter.
pub fn default_action_registry() -> ActionRegistry {
    let mut r = ActionRegistry::new();
    r.register("edit-link", Arc::new(render_edit_link));
    r.register("show-id", Arc::new(render_show_id));
    r
}

fn render_edit_link(ctx: ActionContext) -> AnyView {
    let href = format!("/entities/{}/{}", ctx.entity_type, ctx.entity.id);
    view! {
        <a href=href>{move || crate::i18n::t("table.actions.edit")}</a>
    }
    .into_any()
}

fn render_show_id(ctx: ActionContext) -> AnyView {
    let id = ctx.entity.id.clone();
    view! {
        <span style="font-family: ui-monospace, monospace; font-size: 0.85em;">{id}</span>
    }
    .into_any()
}
