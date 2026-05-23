//! Freitext-Suche fuer die globale Filterung (entspricht
//! [`shared::FilterCriteria::global_search`]).
//!
//! Konsumiert den `TableShellContext` und schreibt direkt in den State.
//! Pagination wird zurueckgesetzt, sobald sich der Suchbegriff aendert.

use leptos::prelude::*;

use super::shell::use_shell;
use crate::i18n::t;
use crate::styling::{styled, use_design};

#[component]
pub fn GlobalFilter() -> impl IntoView {
    let shell = use_shell();
    let design = use_design();
    let input_style = styled("table.toolbar.search", design.input())
        .inline
        .clone();

    let state = shell.state;
    let on_search = move |ev| {
        let value: String = event_target_value(&ev);
        let mut crit = state.filter.get();
        crit.global_search = if value.is_empty() { None } else { Some(value) };
        state.filter.set(crit);
        state.page.set(1);
    };

    view! {
        <input
            type="search"
            placeholder=move || t("table.actions.search")
            style=input_style
            on:input=on_search
        />
    }
}
