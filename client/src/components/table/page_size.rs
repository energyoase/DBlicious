//! Wahl der Seitengroesse.

use leptos::prelude::*;

use super::shell::use_shell;
use crate::styling::use_design;

const OPTIONS: &[u32] = &[10, 25, 50, 100];

#[component]
pub fn PageSize() -> impl IntoView {
    let shell = use_shell();
    let design = use_design();
    let input_style = design.input().inline.clone();
    let state = shell.state;

    let on_change = move |ev| {
        if let Ok(v) = event_target_value(&ev).parse::<u32>() {
            state.page_size.set(v);
            state.page.set(1);
        }
    };

    view! {
        <select style=input_style on:change=on_change>
            {OPTIONS.iter().map(|&n| {
                view! {
                    <option
                        value=n.to_string()
                        selected=move || state.page_size.get() == n
                    >{n}</option>
                }
            }).collect_view()}
        </select>
    }
}
