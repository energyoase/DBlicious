//! Tab-Verwaltung (Manager + UI-Komponente).
//!
//! Pendant zu `JezitLibraryShared/UI/Tabs/` + `JezitLibraryBlazor/UI/Pages/TabPage`.
//! Anders als die Blazor-Variante (Pages/Components ueber Reflection)
//! reicht im Leptos-Modell ein reaktiver Vector + ein aktiver Index:
//!
//!   - [`TabsState`] ist die globale Verwaltung — als Context bereitgestellt.
//!   - [`TabBar`] ist die UI-Komponente, die die Tabs anzeigt und auf
//!     Klicks/Schliessen reagiert.
//!
//! `SplittablePage` aus dem Blazor-Vorbild lebt heute *nicht* — der Bedarf
//! ist offen.

use leptos::prelude::*;
use shared::TabInfo;

use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant};

const LS_TABS_KEY: &str = "dblicious.tabs";
const LS_ACTIVE_KEY: &str = "dblicious.tabs.active";

#[derive(Clone, Copy)]
pub struct TabsState {
    pub tabs: RwSignal<Vec<TabInfo>>,
    pub active: RwSignal<Option<String>>,
}

impl TabsState {
    pub fn new() -> Self {
        let state = Self {
            tabs: RwSignal::new(Vec::new()),
            active: RwSignal::new(None),
        };
        state.hydrate_from_storage();
        state
    }

    fn hydrate_from_storage(&self) {
        let Some(storage) = local_storage() else { return };
        if let Ok(Some(tabs_json)) = storage.get_item(LS_TABS_KEY) {
            if let Ok(tabs) = serde_json::from_str::<Vec<TabInfo>>(&tabs_json) {
                self.tabs.set(tabs);
            }
        }
        if let Ok(Some(active)) = storage.get_item(LS_ACTIVE_KEY) {
            if !active.is_empty() {
                self.active.set(Some(active));
            }
        }
    }

    fn persist(&self) {
        let Some(storage) = local_storage() else { return };
        if let Ok(json) = serde_json::to_string(&self.tabs.get()) {
            let _ = storage.set_item(LS_TABS_KEY, &json);
        }
        let _ = storage.set_item(LS_ACTIVE_KEY, self.active.get().as_deref().unwrap_or(""));
    }

    pub fn open(&self, tab: TabInfo, focus_existing: bool) {
        let existing = self.tabs.with(|t| t.iter().any(|x| x.id == tab.id));
        if !existing {
            let id = tab.id.clone();
            let route = tab.route.clone();
            self.tabs.update(|t| t.push(tab));
            self.active.set(Some(id));
            self.persist();
            if let Some(route) = route {
                navigate_to(&route);
            }
        } else if focus_existing {
            let route = self.tabs.with(|t| {
                t.iter().find(|x| x.id == tab.id).and_then(|x| x.route.clone())
            });
            self.active.set(Some(tab.id));
            self.persist();
            if let Some(route) = route {
                navigate_to(&route);
            }
        }
    }

    pub fn close(&self, tab_id: &str) {
        let removed_id = tab_id.to_string();
        self.tabs.update(|t| t.retain(|x| x.id != removed_id));
        self.active.update(|a| {
            if a.as_deref() == Some(tab_id) {
                *a = self.tabs.with(|t| t.last().map(|x| x.id.clone()));
            }
        });
        self.persist();
    }

    pub fn activate(&self, tab_id: &str) {
        let entry = self
            .tabs
            .with(|t| t.iter().find(|x| x.id == tab_id).cloned());
        if let Some(tab) = entry {
            self.active.set(Some(tab.id.clone()));
            self.persist();
            if let Some(route) = tab.route {
                navigate_to(&route);
            }
        }
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Navigiert per `window.location` (sicherste, Router-unabhaengige Variante).
fn navigate_to(route: &str) {
    if let Some(win) = web_sys::window() {
        // Wenn der Pfad bereits stimmt, nicht erneut navigieren.
        let same = win
            .location()
            .pathname()
            .map(|p| p == route)
            .unwrap_or(false);
        if !same {
            let _ = win.location().set_href(route);
        }
    }
}

impl Default for TabsState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn provide_tabs_state() -> TabsState {
    let state = TabsState::new();
    provide_context(state);
    state
}

pub fn use_tabs_state() -> TabsState {
    use_context::<TabsState>().expect("Kein TabsState im Context (provide_tabs_state fehlt?)")
}

// =============================================================================
// TabBar-Komponente
// =============================================================================

#[component]
pub fn TabBar() -> impl IntoView {
    let state = use_tabs_state();
    let design = use_design();
    let bar_style = design.toolbar().inline.clone();
    let close_btn = design.button(ButtonVariant::Ghost).inline.clone();

    view! {
        <div style=bar_style>
            <For
                each={move || state.tabs.get()}
                key={|t| t.id.clone()}
                children={move |tab: TabInfo| {
                    let id = tab.id.clone();
                    let id_for_close = tab.id.clone();
                    let label_key = tab.label_key.clone();
                    let closable = tab.closable;
                    let close_btn = close_btn.clone();

                    let active_style = move || {
                        let active = state.active.get().as_deref() == Some(id.as_str());
                        if active {
                            "padding: 0.25rem 0.75rem; border-bottom: 2px solid #3b82f6; cursor: pointer;"
                        } else {
                            "padding: 0.25rem 0.75rem; cursor: pointer; opacity: 0.7;"
                        }
                    };

                    let on_click_id = id_for_close.clone();
                    view! {
                        <span style=active_style on:click=move |_| state.activate(&on_click_id)>
                            {move || t(&label_key)}
                            {closable.then(|| {
                                let close_id = id_for_close.clone();
                                view! {
                                    <button
                                        style=close_btn.clone()
                                        on:click=move |ev| {
                                            ev.stop_propagation();
                                            state.close(&close_id);
                                        }
                                    >"×"</button>
                                }
                            })}
                        </span>
                    }
                }}
            />
        </div>
    }
}
