//! Oberer Menue-Bereich der Tabelle. Reiner Container — Inhalt wird vom
//! Aufrufer komponiert (z.B. `<GlobalFilter/>` + `<EntityMenu>` mit Bulk-
//! Aktionen).

use leptos::prelude::*;

use crate::styling::use_design;

#[component]
pub fn TopMenu(children: Children) -> impl IntoView {
    let design = use_design();
    let style = design.toolbar().inline.clone();
    view! {
        <div style=style>
            {children()}
        </div>
    }
}
