//! Standard-Filter `bool-equals`: Tri-state (any/true/false).

use leptos::prelude::*;
use shared::{ColumnMeta, FilterPredicate};

use super::helpers::{current_predicate, upsert_predicate};
use super::super::shell::use_shell;
use crate::styling::use_design;

pub const ID: &str = "bool-equals";

#[component]
pub fn BoolEqualsFilter(column: ColumnMeta) -> impl IntoView {
    let shell = use_shell();
    let state = shell.state;
    let design = use_design();
    let style = design.input().inline.clone();

    let key = column.key.clone();
    let initial = match current_predicate(state, &key) {
        Some(FilterPredicate::BoolEquals { value }) => {
            if value { "true" } else { "false" }
        }
        _ => "",
    };

    let key_for_event = key.clone();
    let on_change = move |ev| {
        let value = event_target_value(&ev);
        let pred = match value.as_str() {
            "true" => Some(FilterPredicate::BoolEquals { value: true }),
            "false" => Some(FilterPredicate::BoolEquals { value: false }),
            _ => None,
        };
        upsert_predicate(state, &key_for_event, pred);
    };

    view! {
        <select style=style on:change=on_change>
            <option value="" selected=initial == "">""</option>
            <option value="true" selected=initial == "true">"true"</option>
            <option value="false" selected=initial == "false">"false"</option>
        </select>
    }
}
