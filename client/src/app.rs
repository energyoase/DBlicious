//! App-Wurzelkomponente, Router-Setup und Hauptlayout.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::auth::AuthContext;
use crate::commands::provide_command_registry;
use crate::components::field::provide_field_registry;
use crate::components::navigation::Navigation;
use crate::components::script_renderer::{provide_script_render_env, ScriptRenderEnv};
use crate::graphql::queries::{fetch_translatable, logout};
use crate::header::{provide_debounce_queue, provide_header_registry};
use crate::i18n::{
    bump_revision_if_available, detect_browser_locale, install_translatable_bundle,
    set_available_locales_from_bundle, t, I18nContext, Locale,
};
use crate::routes::{
    BuilderPage, DashboardPage, DesignerPage, EditorPage, EntityListPage, LoginPage, NotFoundPage,
};
use crate::script::registry::ScriptRegistry;
use crate::script::render_host::RenderHost;
use crate::styling::{
    provide_design_system, provide_style_overrides, use_design, use_style_overrides, ButtonVariant,
    SurfaceLevel, TextVariant,
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
        o.set_inline("table.toolbar.search", "max-width: 320px; flex: 1 1 auto;");
    });
    provide_field_registry();
    provide_validation_system();
    provide_header_registry();
    provide_debounce_queue();
    provide_tabs_state();
    provide_command_registry();

    let i18n = I18nContext::provide(detect_browser_locale());
    let auth = AuthContext::provide();

    // ScriptRenderEnv: Registry + Host bereitstellen, danach async vom Server laden.
    // Die Registry ist Arc<ScriptRegistry> mit Mutex-Innenleben, sodass der
    // async-Refresh nach provide_context die gleiche Instanz befuellt.
    let script_registry = std::sync::Arc::new(ScriptRegistry::new());
    let registry_for_refresh = script_registry.clone();
    provide_script_render_env(ScriptRenderEnv {
        registry: script_registry,
        host: std::sync::Arc::new(RenderHost),
        locale: i18n.locale.get_untracked().code().to_string(),
        user_id: auth.user.get_untracked().as_ref().map(|u| u.id.clone()),
        tenant_id: None,
    });

    // Registry befuellen (wasm32-only; auf nativen Test-Targets ist der
    // cfg-Block leer, der Env bleibt mit leerer Registry — FieldCell faellt zurueck).
    #[cfg(target_arch = "wasm32")]
    {
        spawn_local(async move {
            match registry_for_refresh.refresh_from_server(None).await {
                Ok(n) => log::info!("ScriptRegistry: {n} Skripte vom Server geladen."),
                Err(e) => log::warn!("ScriptRegistry konnte nicht geladen werden: {e:?}"),
            }
        });
    }
    // Sicherstellen, dass der Compiler `registry_for_refresh` nicht als ungenutzten
    // Move-Capture auf non-wasm-Targets bestaendig.
    #[cfg(not(target_arch = "wasm32"))]
    let _ = registry_for_refresh;

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
                        <Route path=path!("/builder/:entity_type") view=BuilderPage/>
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
                            Locale::Fr => "locale.fr",
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
