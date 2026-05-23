//! Standard-Filter `enum-in`: Mehrfachauswahl ueber `FieldType::Enum`-Werte.
//!
//! Bewusst minimal: ein einzelnes `<select multiple>` reicht fuer kleine
//! Wertelisten. Bei mehr Werten lohnt eine eigene Komponente, die per ID
//! `enum-in-large` o.ae. die Registry uebersteuert.

use leptos::prelude::*;
use shared::{ColumnMeta, FieldType, FilterPredicate};
use wasm_bindgen::JsCast;

use super::super::shell::use_shell;
use super::helpers::{current_predicate, upsert_predicate};
use crate::styling::use_design;

pub const ID: &str = "enum-in";

#[component]
pub fn EnumInFilter(column: ColumnMeta) -> impl IntoView {
    let shell = use_shell();
    let state = shell.state;
    let design = use_design();
    let style = design.input().inline.clone();

    let values = match &column.field_type {
        FieldType::Enum { values } => values.clone(),
        _ => Vec::new(),
    };

    let key = column.key.clone();
    let initial: Vec<String> = match current_predicate(state, &key) {
        Some(FilterPredicate::EnumIn { values }) => values,
        _ => Vec::new(),
    };

    let key_for_event = key.clone();
    let on_change = move |ev: web_sys::Event| {
        let target = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok());
        let Some(select) = target else { return };
        let options = select.selected_options();
        let mut picked: Vec<String> = Vec::new();
        for i in 0..options.length() {
            if let Some(opt) = options
                .item(i)
                .and_then(|n| n.dyn_into::<web_sys::HtmlOptionElement>().ok())
            {
                picked.push(opt.value());
            }
        }
        let pred = if picked.is_empty() {
            None
        } else {
            Some(FilterPredicate::EnumIn { values: picked })
        };
        upsert_predicate(state, &key_for_event, pred);
    };

    view! {
        <select multiple=true style=style on:change=on_change>
            {values.into_iter().map(|v| {
                let selected = initial.contains(&v);
                let v_label = v.clone();
                view! { <option value=v selected=selected>{v_label}</option> }
            }).collect_view()}
        </select>
    }
}
