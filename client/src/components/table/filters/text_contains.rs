//! Standard-Filter `text-contains`: Substring-Suche (Case-insensitive).

use leptos::prelude::*;
use shared::{ColumnMeta, FilterPredicate};

use super::super::shell::use_shell;
use super::helpers::{current_predicate, upsert_predicate};
use crate::i18n::t;
use crate::styling::use_design;

pub const ID: &str = "text-contains";

#[component]
pub fn TextContainsFilter(column: ColumnMeta) -> impl IntoView {
    let shell = use_shell();
    let state = shell.state;
    let design = use_design();
    let style = design.input().inline.clone();

    let key = column.key.clone();
    let initial = current_predicate(state, &key)
        .and_then(|p| match p {
            FilterPredicate::TextContains { value, .. } => Some(value),
            _ => None,
        })
        .unwrap_or_default();

    let key_for_event = key.clone();
    let on_input = move |ev| {
        let value: String = event_target_value(&ev);
        let pred = if value.is_empty() {
            None
        } else {
            Some(FilterPredicate::TextContains {
                value,
                case_insensitive: true,
            })
        };
        upsert_predicate(state, &key_for_event, pred);
    };

    view! {
        <input
            type="search"
            style=style
            placeholder=move || t("table.actions.filter")
            prop:value=initial
            on:input=on_input
        />
    }
}
