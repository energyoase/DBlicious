//! Implementation-Picker (Phase 1.5.5).
//!
//! Komponente, die fuer eine `(entity_type, property, registry)`-Kombination
//! die vom Server erlaubten Implementations-IDs holt und als `<select>`
//! anzeigt. Bei Selektion wird die `setImplementationChoice`-Mutation
//! gerufen — sobald der Server zustimmt, ist die Wahl persistiert.
//!
//! Anzeige der aktuell wirksamen ID stammt aus `resolveImplementation`
//! (Server-authoritative, inkl. Per-User-Choice).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::graphql::queries::{
    allowed_implementations, resolve_implementation, set_implementation_choice,
};

#[component]
pub fn ImplementationPicker(
    entity_type: String,
    property: String,
    /// `"filter"`, `"editor"`, `"formatter"`, …
    registry: String,
) -> impl IntoView {
    let allowed: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let current: RwSignal<Option<String>> = RwSignal::new(None);
    let saving: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);

    let et = entity_type.clone();
    let p = property.clone();
    let r = registry.clone();
    spawn_local(async move {
        match allowed_implementations(&et, &p, &r).await {
            Ok(list) => allowed.set(list),
            Err(e) => error.set(Some(format!("allowed: {e}"))),
        }
        match resolve_implementation(&et, &p, &r, None).await {
            Ok(c) => current.set(c),
            Err(e) => error.set(Some(format!("resolve: {e}"))),
        }
    });

    let et_for_change = entity_type.clone();
    let p_for_change = property.clone();
    let r_for_change = registry.clone();
    let on_change = move |ev| {
        let chosen = event_target_value(&ev);
        if chosen.is_empty() {
            return;
        }
        saving.set(true);
        error.set(None);
        let et = et_for_change.clone();
        let p = p_for_change.clone();
        let r = r_for_change.clone();
        let chosen_for_signal = chosen.clone();
        spawn_local(async move {
            match set_implementation_choice(&et, &p, &r, &chosen).await {
                Ok(true) => {
                    current.set(Some(chosen_for_signal));
                }
                Ok(false) => {
                    error.set(Some("server-rejected".into()));
                }
                Err(e) => {
                    error.set(Some(format!("set: {e}")));
                }
            }
            saving.set(false);
        });
    };

    view! {
        <div style="display: flex; gap: 0.25rem; align-items: center; font-size: 0.85rem;">
            <select on:change=on_change disabled=move || saving.get()>
                {move || {
                    let cur = current.get();
                    let ids = allowed.get();
                    let mut html = vec![
                        view! { <option value="" disabled=true selected=cur.is_none()>"—"</option> }
                            .into_any()
                    ];
                    for id in ids {
                        let is_current = cur.as_deref() == Some(id.as_str());
                        let label = id.clone();
                        let value = id.clone();
                        html.push(
                            view! {
                                <option value=value selected=is_current>{label}</option>
                            }
                            .into_any(),
                        );
                    }
                    html
                }}
            </select>
            {move || saving.get().then(|| view! { <span style="color:#6b7280;">"…"</span> })}
            {move || error.get().map(|e| view! {
                <span style="color:#b91c1c;">{e}</span>
            })}
        </div>
    }
}
