//! Standard-Filter `number-range`: zwei Eingaben fuer min/max.

use leptos::prelude::*;
use shared::{ColumnMeta, FilterPredicate};

use super::helpers::{current_predicate, upsert_predicate};
use super::super::shell::use_shell;
use crate::styling::use_design;

pub const ID: &str = "number-range";

fn parse_optional_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        t.parse().ok()
    }
}

#[component]
pub fn NumberRangeFilter(column: ColumnMeta) -> impl IntoView {
    let shell = use_shell();
    let state = shell.state;
    let design = use_design();
    let style = design.input().inline.clone();

    let key = column.key.clone();
    let (min0, max0) = match current_predicate(state, &key) {
        Some(FilterPredicate::NumberRange { min, max }) => (
            min.map(|v| v.to_string()).unwrap_or_default(),
            max.map(|v| v.to_string()).unwrap_or_default(),
        ),
        _ => (String::new(), String::new()),
    };

    let min_sig = RwSignal::new(min0);
    let max_sig = RwSignal::new(max0);

    let key_for_effect = key.clone();
    Effect::new(move |_| {
        let lo = parse_optional_f64(&min_sig.get());
        let hi = parse_optional_f64(&max_sig.get());
        let pred = if lo.is_none() && hi.is_none() {
            None
        } else {
            Some(FilterPredicate::NumberRange { min: lo, max: hi })
        };
        upsert_predicate(state, &key_for_effect, pred);
    });

    let style_min = style.clone();
    let style_max = style.clone();
    view! {
        <div style="display: flex; gap: 0.25rem;">
            <input
                type="number"
                style=style_min
                placeholder="min"
                prop:value=move || min_sig.get()
                on:input=move |ev| min_sig.set(event_target_value(&ev))
            />
            <input
                type="number"
                style=style_max
                placeholder="max"
                prop:value=move || max_sig.get()
                on:input=move |ev| max_sig.set(event_target_value(&ev))
            />
        </div>
    }
}
