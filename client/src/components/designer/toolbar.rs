//! Aktionsleiste des Designers.
//!
//! Buendelt die Aktionen, die nicht direkt auf einer Tabellenkarte sitzen:
//!  - Schema-Name editieren,
//!  - neue Tabelle hinzufuegen,
//!  - Verknuepfungsmodus umschalten,
//!  - Schema an den Server senden (und Status anzeigen).
//!
//! Die eigentliche Mutation laeuft ueber `graphql::queries::save_db_schema`;
//! das Ergebnis landet im `save_status`-Signal des Modells und wird unten
//! an der Leinwand angezeigt.

use leptos::ev::Event;
use leptos::prelude::*;
use leptos::task::spawn_local;

use super::model::{use_designer_model, SaveStatus};
use crate::graphql::queries::save_db_schema;
use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant, TextVariant};

#[component]
pub fn DesignerToolbar() -> impl IntoView {
    let model = use_designer_model();
    let design = use_design();

    let toolbar_style = design.toolbar().inline.clone();
    let label_style = design.text(TextVariant::Caption).inline.clone();
    let input_style = design.input().inline.clone();
    let primary = design.button(ButtonVariant::Primary).inline.clone();
    let secondary = design.button(ButtonVariant::Secondary).inline.clone();
    let ghost = design.button(ButtonVariant::Ghost).inline.clone();
    let _ = ghost; // bereitgehalten falls spaeter zusaetzliche Ghost-Aktionen folgen

    let on_name_input = move |ev: Event| {
        let value: String = event_target_value(&ev);
        model.schema_name.set(value);
    };

    let on_add_table = move |_| {
        let n = model.tables.with_untracked(|t| t.len());
        let x = 80.0 + ((n as f64) % 4.0) * 280.0;
        let y = 80.0 + ((n as f64 / 4.0).floor()) * 240.0;
        let name = format!("Tabelle {}", n + 1);
        model.add_table_at(&name, x, y);
    };

    let toggle_link_mode = move |_| {
        let now = !model.link_mode.get_untracked();
        model.link_mode.set(now);
        if !now {
            model.cancel_pending_link();
        }
    };

    let on_save = move |_| {
        let snapshot = model.snapshot();
        model.save_status.set(SaveStatus::Saving);
        let status_signal = model.save_status;
        spawn_local(async move {
            match save_db_schema(&snapshot).await {
                Ok(res) => status_signal.set(if res.ok {
                    SaveStatus::Ok(res.message)
                } else {
                    SaveStatus::Err(res.message)
                }),
                Err(e) => status_signal.set(SaveStatus::Err(e.to_string())),
            }
        });
    };

    let link_label = move || {
        if model.link_mode.get() {
            t("designer.actions.link_mode_on")
        } else {
            t("designer.actions.link_mode_off")
        }
    };

    let save_disabled = move || matches!(model.save_status.get(), SaveStatus::Saving);

    view! {
        <div style=toolbar_style>
            <div style="display: flex; flex-direction: column; gap: 0.15rem;">
                <span style=label_style>{move || t("designer.fields.schema_name")}</span>
                <input
                    style=input_style
                    prop:value=move || model.schema_name.get()
                    on:input=on_name_input
                />
            </div>
            <button style=secondary.clone() on:click=on_add_table>
                {move || t("designer.actions.add_table")}
            </button>
            <button
                style=move || {
                    if model.link_mode.get() {
                        // Aktiver Zustand visuell hervorheben.
                        format!("{} background: #fef3c7; border-color: #facc15; color: #92400e;", secondary.clone())
                    } else {
                        secondary.clone()
                    }
                }
                on:click=toggle_link_mode
            >
                {link_label}
            </button>
            <div style="flex: 1;"></div>
            <button
                style=primary
                on:click=on_save
                prop:disabled=save_disabled
            >
                {move || match model.save_status.get() {
                    SaveStatus::Saving => t("designer.actions.saving"),
                    _ => t("designer.actions.save"),
                }}
            </button>
        </div>
    }
}

#[component]
pub fn DesignerStatusBanner() -> impl IntoView {
    let model = use_designer_model();
    let design = use_design();

    move || {
        let status = model.save_status.get();
        match status {
            SaveStatus::Idle | SaveStatus::Saving => view! { <span></span> }.into_any(),
            SaveStatus::Ok(msg) => {
                let style = design.designer_status(true).inline.clone();
                view! { <div style=style>{msg}</div> }.into_any()
            }
            SaveStatus::Err(msg) => {
                let style = design.designer_status(false).inline.clone();
                view! { <div style=style>{msg}</div> }.into_any()
            }
        }
    }
}
