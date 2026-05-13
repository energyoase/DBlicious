//! Unterer Menue-Bereich der Tabelle. Reiner Container.

use leptos::prelude::*;

use crate::styling::use_design;

#[component]
pub fn BottomMenu(children: Children) -> impl IntoView {
    let design = use_design();
    let style = design.pagination_bar().inline.clone();
    view! {
        <div style=style>
            {children()}
        </div>
    }
}
