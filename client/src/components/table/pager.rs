//! Pagination-Controls (Prev/Next + Summary).

use leptos::prelude::*;

use super::shell::use_shell;
use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant};

#[component]
pub fn Pager() -> impl IntoView {
    let shell = use_shell();
    let design = use_design();
    let prev_style = design.button(ButtonVariant::Secondary).inline.clone();
    let next_style = design.button(ButtonVariant::Secondary).inline.clone();

    let state = shell.state;
    let data = shell.data;

    let total_count = move || {
        data.get()
            .and_then(|r| r.take().ok())
            .map(|d| d.total_count)
            .unwrap_or(0)
    };
    let total_pages = move || {
        let ps = state.page_size.get().max(1) as u64;
        total_count().div_ceil(ps) as u32
    };

    let summary = move || {
        let p = state.page.get();
        let tp = total_pages().max(1);
        crate::t!("table.pagination.summary", "page" => p as i64, "total" => tp as i64)
    };

    let on_prev = move |_| state.prev_page();
    let on_next = move |_| state.next_page(total_pages());

    view! {
        <button style=prev_style on:click=on_prev>{move || t("table.pagination.prev")}</button>
        <span>{summary}</span>
        <button style=next_style on:click=on_next>{move || t("table.pagination.next")}</button>
    }
}
