//! App-Wurzelkomponente, Router-Setup und Hauptlayout.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::auth::AuthContext;
use crate::commands::provide_command_registry;
use crate::components::field::provide_field_registry;
use crate::components::navigation::Navigation;
use crate::graphql::queries::{fetch_translatable, logout};
use crate::header::{provide_debounce_queue, provide_header_registry};
use crate::i18n::{
    bump_revision_if_available, detect_browser_locale, install_translatable_bundle,
    set_available_locales_from_bundle, t, I18nContext, Locale,
};
use crate::routes::{
    DashboardPage, DesignerPage, EditorPage, EntityListPage, LoginPage, NotFoundPage,
};
use crate::styling::{
    provide_design_system, provide_style_overrides, use_design, use_style_overrides,
    ButtonVariant, SurfaceLevel, TextVariant,
};
use crate::tabs::{provide_tabs_state, TabBar};
use crate::validation::provide_validation_system;

#[component]
pub fn App() -> impl IntoView {
    provide_design_system();
    provide_style_overrides();

    // Default-Overrides: zeigen, dass der Mechanismus wirksam ist.
    // Anwendung kann hier projekt-spezifische Tweaks registrieren, ohne den
    // DesignSystem-Trait zu beruehren. Beispiel: das Search-Input in der
    // Tabellen-Toolbar bekommt eine sanftere Max-Breite.
    use_style_overrides().update(|o| {
        o.set_inline(
            "table.toolbar.search",
            "max-width: 320px; flex: 1 1 auto;",
        );
    });
    provide_field_registry();
    provide_validation_system();
    provide_header_registry();
    provide_debounce_queue();
    provide_tabs_state();
    provide_command_registry();

    I18nContext::provide(detect_browser_locale());
    let auth = AuthContext::provide();

    // DB-Translatable nachziehen + bump.
    let _hydrate = LocalResource::new(|| async {
        match fetch_translatable().await {
            Ok(bundle) => {
                install_translatable_bundle(&bundle);
                set_available_locales_from_bundle(&bundle);
                bump_revision_if_available();
            }
            Err(e) => log::warn!("TranslatableBundle konnte nicht geladen werden: {e}"),
        }
    });

    view! {
        <Router>
            // Route-Switch: anonym → LoginPage; authentifiziert → AppLayout.
            {move || {
                let _ = _hydrate.get();
                if auth.is_authenticated() {
                    view! { <AppLayout/> }.into_any()
                } else {
                    view! {
                        <Routes fallback=LoginPage>
                            <Route path=path!("/") view=LoginPage/>
                        </Routes>
                    }.into_any()
                }
            }}
        </Router>
    }
}

#[component]
fn AppLayout() -> impl IntoView {
    let design = use_design();
    let root_style = design.root().inline.clone();
    let app_surface = design.surface(SurfaceLevel::App).inline.clone();
    let sidebar_surface = design.surface(SurfaceLevel::Sidebar).inline.clone();
    let toolbar_surface = design.surface(SurfaceLevel::Toolbar).inline.clone();

    view! {
        <div style=root_style>
            <div style=format!("display: grid; grid-template-columns: 280px 1fr; grid-template-rows: auto 1fr; min-height: 100vh; {app_surface}")>
                <header style=format!("grid-column: 1 / span 2; {toolbar_surface}")>
                    <Topbar/>
                </header>
                <aside style=sidebar_surface>
                    <Navigation/>
                </aside>
                <main style="padding: 1.5rem; overflow: auto;">
                    <TabBar/>
                    <Routes fallback=NotFoundPage>
                        <Route path=path!("/") view=DashboardPage/>
                        <Route path=path!("/entities/:entity_type") view=EntityListPage/>
                        <Route path=path!("/entities/:entity_type/:id") view=EditorPage/>
                        <Route path=path!("/designer") view=DesignerPage/>
                    </Routes>
                </main>
            </div>
        </div>
    }
}

#[component]
fn Topbar() -> impl IntoView {
    let design = use_design();
    let title_style = design.text(TextVariant::H2).inline.clone();
    let primary_btn = design.button(ButtonVariant::Ghost).inline.clone();
    let primary_for_loop = primary_btn.clone();
    let primary_for_logout = primary_btn;

    let i18n = I18nContext::use_context();
    let auth = AuthContext::use_context();

    let on_logout = move |_| {
        let auth_for_clear = auth;
        spawn_local(async move {
            let _ = logout().await;
            auth_for_clear.clear();
            if let Some(win) = web_sys::window() {
                let _ = win.location().set_href("/");
            }
        });
    };

    view! {
        <div style="display: flex; align-items: center; justify-content: space-between; padding: 0.75rem 1.5rem;">
            <span style=title_style>{move || t("app.title")}</span>
            <div style="display: flex; gap: 0.5rem; align-items: center;">
                {move || auth.user.get().map(|u| {
                    let name = u.display_name.clone();
                    view! {
                        <span style="font-size: 0.85rem; color: rgba(255,255,255,0.8);">
                            {move || crate::t!("topbar.user", "name" => name.clone())}
                        </span>
                    }
                })}
                // Sprach-Buttons werden aus `i18n.available` gespeist.
                {move || {
                    let style = primary_for_loop.clone();
                    i18n.available.get().into_iter().map(move |loc| {
                        let label_key = match loc {
                            Locale::De => "locale.de",
                            Locale::En => "locale.en",
                        };
                        let btn_style = style.clone();
                        view! {
                            <button
                                style=btn_style
                                on:click=move |_| i18n.locale.set(loc)
                            >{move || t(label_key)}</button>
                        }
                    }).collect_view()
                }}
                <button
                    style=primary_for_logout
                    on:click=on_logout
                >{move || t("topbar.logout")}</button>
            </div>
        </div>
    }
}
