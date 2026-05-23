//! Standard-Filter `date-range`: zwei Datums-Eingaben.

use leptos::prelude::*;
use shared::{ColumnMeta, FilterPredicate};

use super::super::shell::use_shell;
use super::helpers::{current_predicate, upsert_predicate};
use crate::styling::use_design;

pub const ID: &str = "date-range";

fn opt(s: String) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[component]
pub fn DateRangeFilter(column: ColumnMeta) -> impl IntoView {
    let shell = use_shell();
    let state = shell.state;
    let design = use_design();
    let style = design.input().inline.clone();

    let key = column.key.clone();
    let (from0, to0) = match current_predicate(state, &key) {
        Some(FilterPredicate::DateRange { from, to }) => {
            (from.unwrap_or_default(), to.unwrap_or_default())
        }
        _ => (String::new(), String::new()),
    };

    let from_sig = RwSignal::new(from0);
    let to_sig = RwSignal::new(to0);

    let key_for_effect = key.clone();
    Effect::new(move |_| {
        let from = opt(from_sig.get());
        let to = opt(to_sig.get());
        let pred = if from.is_none() && to.is_none() {
            None
        } else {
            Some(FilterPredicate::DateRange { from, to })
        };
        upsert_predicate(state, &key_for_effect, pred);
    });

    let style_from = style.clone();
    let style_to = style.clone();
    view! {
        <div style="display: flex; gap: 0.25rem;">
            <input
                type="date"
                style=style_from
                prop:value=move || from_sig.get()
                on:input=move |ev| from_sig.set(event_target_value(&ev))
            />
            <input
                type="date"
                style=style_to
                prop:value=move || to_sig.get()
                on:input=move |ev| to_sig.set(event_target_value(&ev))
            />
        </div>
    }
}
