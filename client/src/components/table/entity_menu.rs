//! Container fuer Bulk-Aktionen (oben im TopMenu).
//!
//! Bulk-Aktionen brauchen einen Inhalt — `<EntityAction>`, `<DeleteAction>`,
//! `<EditAction>` werden hier als children eingehaengt. Heute reicht ein
//! schlanker Flex-Container.

use leptos::prelude::*;

#[component]
pub fn EntityMenu(children: Children) -> impl IntoView {
    view! {
        <div style="display: flex; gap: 0.25rem; align-items: center;">
            {children()}
        </div>
    }
}
