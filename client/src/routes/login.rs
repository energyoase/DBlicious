//! Login-Seite.
//!
//! Anonyme Route. Bei Erfolg wird der [`AuthContext`] gefuellt und mit
//! `window.location` zur Dashboard-Route navigiert (statt
//! `Router::navigate` — letzteres haette zur Folge, dass der
//! `provide_context`-Setup im `App` nicht neu durchlaeuft).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::auth::AuthContext;
use crate::graphql::queries::{login, LoginOutcome};
use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant, SurfaceLevel, TextVariant};

#[component]
pub fn LoginPage() -> impl IntoView {
    let design = use_design();
    let card = design.surface(SurfaceLevel::Card).inline.clone();
    let h1 = design.text(TextVariant::H1).inline.clone();
    let muted = design.text(TextVariant::Muted).inline.clone();
    let input = design.input().inline.clone();
    let primary = design.button(ButtonVariant::Primary).inline.clone();

    let auth = AuthContext::use_context();

    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error_key: RwSignal<Option<String>> = RwSignal::new(None);
    let pending = RwSignal::new(false);

    let on_submit = move || {
        if pending.get() {
            return;
        }
        let u = username.get();
        let p = password.get();
        if u.is_empty() || p.is_empty() {
            error_key.set(Some("login.error.invalidCredentials".into()));
            return;
        }
        pending.set(true);
        error_key.set(None);
        spawn_local(async move {
            match login(&u, &p).await {
                Ok(LoginOutcome::Success(session)) => {
                    auth.apply_session(session);
                    if let Some(win) = web_sys::window() {
                        let _ = win.location().set_href("/");
                    }
                }
                Ok(LoginOutcome::Failed(code)) => {
                    error_key.set(Some(format!("login.error.{code}")));
                }
                Err(e) => {
                    log::error!("Login-RPC fehlgeschlagen: {e}");
                    error_key.set(Some("login.error.internal".into()));
                }
            }
            pending.set(false);
        });
    };

    let on_submit_form = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        on_submit();
    };

    let card_style = format!("{card} max-width: 360px; margin: 4rem auto;");

    view! {
        <div style=card_style>
            <h1 style=h1>{move || t("login.title")}</h1>
            <p style=muted>{move || t("login.hint")}</p>
            <form on:submit=on_submit_form style="display: flex; flex-direction: column; gap: 0.75rem; margin-top: 1rem;">
                <label>
                    {move || t("login.username")}
                    <input
                        style=input.clone()
                        type="text"
                        autocomplete="username"
                        on:input=move |ev| username.set(event_target_value(&ev))
                    />
                </label>
                <label>
                    {move || t("login.password")}
                    <input
                        style=input.clone()
                        type="password"
                        autocomplete="current-password"
                        on:input=move |ev| password.set(event_target_value(&ev))
                    />
                </label>
                {move || error_key.get().map(|k| view! {
                    <div style="color: #b91c1c; font-size: 0.85rem;">
                        {move || t(&k)}
                    </div>
                })}
                <button
                    type="submit"
                    style=primary.clone()
                    disabled=move || pending.get()
                >
                    {move || if pending.get() {
                        t("editor.actions.saving")
                    } else {
                        t("login.submit")
                    }}
                </button>
            </form>
        </div>
    }
}
