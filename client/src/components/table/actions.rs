//! Wiederverwendbare Action-Komponenten fuer Tabellen-Zeilen und Bulk-
//! Aktionen.
//!
//! Per-Row-Aktionen ([`EditAction`], [`DeleteAction`]) lesen ihren
//! Bezug ueber den [`RowContext`] aus dem Leptos-Kontext, der von
//! [`super::table_view::TableView`] pro Zeile vor dem Rendern der
//! `<RowActions>`-Children gesetzt wird.
//!
//! Bulk-Aktionen ([`EntityAction`]) leben innerhalb eines
//! [`super::entity_menu::EntityMenu`] und beziehen sich auf die aktuelle
//! Selektion ([`super::selection::SelectionState`]).

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::Entity;

use super::shell::use_shell;
use crate::header::use_header_registry;
use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant};

/// Pro-Zeile-Kontext. Wird von der TableView pro `<For>`-Iteration vor dem
/// Rendern der `<RowActions>`-Children ins Context geschoben.
#[derive(Clone)]
pub struct RowContext {
    pub entity: Entity,
}

pub fn use_row() -> RowContext {
    use_context::<RowContext>()
        .expect("kein RowContext — <EditAction>/<DeleteAction> nur innerhalb <RowActions> nutzen")
}

#[component]
pub fn EditAction() -> impl IntoView {
    let shell = use_shell();
    let row = use_row();
    let design = use_design();
    let ghost = design.button(ButtonVariant::Ghost).inline.clone();

    if !shell.caps.can_update {
        return view! { <></> }.into_any();
    }

    let href = format!("/entities/{}/{}", shell.entity_type(), row.entity.id);
    view! {
        <a href=href style=ghost>
            {move || t("table.actions.edit")}
        </a>
    }
    .into_any()
}

#[component]
pub fn DeleteAction() -> impl IntoView {
    let shell = use_shell();
    let row = use_row();
    let design = use_design();
    let ghost = design.button(ButtonVariant::Ghost).inline.clone();

    if !shell.caps.can_delete {
        return view! { <></> }.into_any();
    }

    // HeaderRegistry mit dem Datensatz vorpopulieren, damit ein Concurrency-
    // Check (expected_hash) bei der Delete-Mutation greift, ohne dass der
    // User die Zeile erst im Editor geoeffnet haben muss.
    let header_for_delete = use_header_registry();
    let entity_type = shell.entity_type();
    header_for_delete.update(|hr| hr.upsert_loaded(&entity_type, &row.entity));

    let entity_id = row.entity.id.clone();
    let source = shell.source();
    let state = shell.state;

    let on_delete = move |_| {
        if let Some(win) = web_sys::window() {
            let confirmed = win
                .confirm_with_message(&t("editor.confirm.delete"))
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        let id = entity_id.clone();
        let et = entity_type.clone();
        let source = source.clone();
        let expected = header_for_delete
            .with(|hr| hr.get(&et, &id).map(|h| h.original_hash));
        spawn_local(async move {
            if let Some(deletable) = source.deletable() {
                if let Ok(res) = deletable.delete(id, expected).await {
                    if res.ok {
                        // Reload erzwingen, indem ein Signal aus dem Resource-
                        // Trigger angefasst wird.
                        state.page.set(state.page.get());
                        state.filter.update(|_| {});
                    }
                }
            }
        });
    };

    view! {
        <button style=ghost on:click=on_delete>
            {move || t("table.actions.delete")}
        </button>
    }
    .into_any()
}

/// Generische Bulk-Aktion fuer die aktuelle Selektion. `enabled_when` wird
/// jedes Mal evaluiert, wenn sich die Selektion aendert.
#[component]
pub fn EntityAction(
    /// Label-Key fuer den Button-Text.
    label_key: &'static str,
    /// Callback mit den selektierten IDs.
    on_click: Callback<Vec<String>>,
    /// Aktivierungs-Praedikat anhand der Anzahl selektierter Zeilen.
    /// Default: aktiviert, sobald mindestens eine Zeile selektiert ist.
    #[prop(default = std::sync::Arc::new(|n: usize| n > 0))]
    enabled_when: std::sync::Arc<dyn Fn(usize) -> bool + Send + Sync>,
) -> impl IntoView {
    let shell = use_shell();
    let design = use_design();
    let style = design.button(ButtonVariant::Secondary).inline.clone();
    let selection = shell.selection;

    let enabled_for_click = enabled_when.clone();
    let click = move |_| {
        if enabled_for_click(selection.count()) {
            on_click.run(selection.ids());
        }
    };
    let disabled = move || !enabled_when(selection.count());

    view! {
        <button style=style on:click=click disabled=disabled>
            {move || t(label_key)}
        </button>
    }
}
