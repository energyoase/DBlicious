//! Editor-Seite fuer einen einzelnen Datensatz.
//!
//! Bedient sowohl `/entities/:type/new` als auch `/entities/:type/:id`.
//! Verzahnt:
//!   - [`crate::graphql::queries::fetch_editor`] fuer Property-Schema
//!   - [`crate::graphql::queries::fetch_entity_by_id`] fuer den Edit-Pfad
//!   - [`crate::components::field::FieldEditor`] pro Property
//!   - [`crate::validation::ValidationSystem`] mit `import_required_from`
//!   - [`crate::header::HeaderRegistry`] fuer Dirty-Tracking + expected_hash
//!   - [`crate::components::table::SavableSource`]/[`DeletableSource`]
//!     fuer das eigentliche Schreiben

use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use serde_json::Value;
use shared::auth::Op;
use shared::{compute_hash, EditorMeta, Entity};

use crate::auth::AuthContext;
use crate::components::field::FieldEditor;
use crate::components::table::{DataSource, RemoteSource};
use crate::graphql::queries::{fetch_editor, fetch_entity_by_id};
use crate::header::use_header_registry;
use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant, SurfaceLevel, TextVariant};
use crate::validation::use_validation_system;

#[component]
pub fn EditorPage() -> impl IntoView {
    let params = use_params_map();
    let design = use_design();
    let card = design.surface(SurfaceLevel::Card).inline.clone();
    let h1 = design.text(TextVariant::H1).inline.clone();
    let muted = design.text(TextVariant::Muted).inline.clone();
    let primary = design.button(ButtonVariant::Primary).inline.clone();
    let secondary = design.button(ButtonVariant::Secondary).inline.clone();
    let ghost = design.button(ButtonVariant::Ghost).inline.clone();
    let auth = AuthContext::use_context();

    view! {
        <div style=card>
            {move || {
                let p = params.read();
                let entity_type = p.get("entity_type").unwrap_or_default();
                let id = p.get("id").unwrap_or_default();
                let is_new = id == "new" || id.is_empty();
                let perm_op = if is_new { Op::Create } else { Op::Update };
                if !auth.can_entity_type(&entity_type, perm_op) {
                    return view! {
                        <div>
                            <h1 style=h1.clone()>{entity_type.clone()}</h1>
                            <p>{move || t("error.validation")}</p>
                        </div>
                    }.into_any();
                }

                view! {
                    <EditorBody
                        entity_type=entity_type
                        id=if is_new { None } else { Some(id) }
                        h1_style=h1.clone()
                        muted_style=muted.clone()
                        primary_style=primary.clone()
                        secondary_style=secondary.clone()
                        ghost_style=ghost.clone()
                    />
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn EditorBody(
    entity_type: String,
    id: Option<String>,
    h1_style: String,
    muted_style: String,
    primary_style: String,
    secondary_style: String,
    ghost_style: String,
) -> impl IntoView {
    let auth = AuthContext::use_context();
    let validation = use_validation_system();
    let header_registry = use_header_registry();

    let entity_type_signal = RwSignal::new(entity_type.clone());
    let id_signal: RwSignal<Option<String>> = RwSignal::new(id.clone());
    let fields_signal: RwSignal<serde_json::Map<String, Value>> =
        RwSignal::new(serde_json::Map::new());
    let editor_meta: RwSignal<Option<EditorMeta>> = RwSignal::new(None);
    let validation_messages: RwSignal<Vec<shared::ValidationMessage>> = RwSignal::new(Vec::new());
    let saving = RwSignal::new(false);
    let server_message: RwSignal<Option<String>> = RwSignal::new(None);

    // EditorMeta + Entity laden.
    let entity_type_for_load = entity_type.clone();
    let id_for_load = id.clone();
    let validation_for_load = validation.clone();
    let header_for_load = header_registry.clone();
    let _load = LocalResource::new(move || {
        let entity_type = entity_type_for_load.clone();
        let id_inner = id_for_load.clone();
        let validation = validation_for_load.clone();
        let header = header_for_load.clone();
        async move {
            let meta = fetch_editor(&entity_type).await.ok().flatten();
            if let Some(m) = meta.clone() {
                editor_meta.set(Some(m.clone()));
                validation.update(|sys| sys.import_required_from(&m));
            }
            if let Some(id) = id_inner {
                if let Ok(Some(entity)) = fetch_entity_by_id(&entity_type, &id).await {
                    fields_signal.set(entity.fields.clone());
                    header.update(|hr| hr.upsert_loaded(&entity_type, &entity));
                }
            }
        }
    });

    // Save-/Delete-Handler als Leptos-`Callback` (Copy + FnMut-fest).
    let validation_for_save = validation.clone();
    let header_for_save = header_registry.clone();
    let on_save: Callback<()> = Callback::new(move |_: ()| {
        if saving.get() {
            return;
        }
        let entity_type = entity_type_signal.get();
        let fields = fields_signal.get();

        let result = validation_for_save.run(&entity_type, &fields);
        validation_messages.set(result.messages.clone());
        if result.has_blocking() {
            return;
        }

        saving.set(true);
        let source = Rc::new(RemoteSource::new(entity_type.clone())) as Rc<dyn DataSource>;
        let id_now = id_signal.get();
        let header_reg = header_for_save.clone();
        spawn_local(async move {
            let outcome = if let Some(id) = id_now.clone() {
                let expected =
                    header_reg.with(|hr| hr.get(&entity_type, &id).map(|h| h.original_hash));
                if let Some(savable) = source.savable() {
                    Some(savable.update(id.clone(), fields.clone(), expected).await)
                } else {
                    None
                }
            } else if let Some(savable) = source.savable() {
                Some(savable.create(fields.clone()).await)
            } else {
                None
            };

            match outcome {
                Some(Ok(res)) => {
                    validation_messages.set(res.validation.messages.clone());
                    if res.ok {
                        if let Some(entity) = res.entity {
                            id_signal.set(Some(entity.id.clone()));
                            fields_signal.set(entity.fields.clone());
                            let entity_clone = entity.clone();
                            header_reg.update(|hr| {
                                hr.upsert_loaded(&entity_type, &entity_clone);
                                hr.baseline(&entity_type, &entity_clone.id);
                            });
                        }
                        server_message.set(Some("editor.state.saved".into()));
                    } else if res.validation.has_blocking() {
                        server_message.set(Some("error.validation".into()));
                    } else {
                        server_message.set(Some("error.other".into()));
                    }
                }
                Some(Err(e)) => {
                    log::error!("Save fehlgeschlagen: {e}");
                    server_message.set(Some("error.network".into()));
                }
                None => {
                    server_message.set(Some("error.other".into()));
                }
            }
            saving.set(false);
        });
    });

    let header_for_delete = header_registry.clone();
    let on_delete: Callback<()> = Callback::new(move |_: ()| {
        let Some(id) = id_signal.get() else { return };
        let entity_type = entity_type_signal.get();
        if !auth.can_entity_type(&entity_type, Op::Delete) {
            return;
        }
        if let Some(win) = web_sys::window() {
            let confirmed = win
                .confirm_with_message(&t("editor.confirm.delete"))
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        let source = Rc::new(RemoteSource::new(entity_type.clone())) as Rc<dyn DataSource>;
        let expected = header_for_delete
            .with(|hr| hr.get(&entity_type, &id).map(|h| h.original_hash));
        spawn_local(async move {
            if let Some(deletable) = source.deletable() {
                match deletable.delete(id.clone(), expected).await {
                    Ok(res) => {
                        if res.ok {
                            if let Some(win) = web_sys::window() {
                                let _ = win
                                    .location()
                                    .set_href(&format!("/entities/{entity_type}"));
                            }
                        } else {
                            validation_messages.set(res.validation.messages.clone());
                            server_message.set(Some("error.validation".into()));
                        }
                    }
                    Err(e) => {
                        log::error!("Delete fehlgeschlagen: {e}");
                        server_message.set(Some("error.network".into()));
                    }
                }
            }
        });
    });

    let header_for_dirty = header_registry.clone();
    let is_dirty = move || {
        let Some(id) = id_signal.get() else { return false };
        let live = compute_hash(&Entity {
            id: id.clone(),
            fields: fields_signal.get(),
        });
        let original = header_for_dirty
            .with(|hr| hr.get(&entity_type_signal.get(), &id).map(|h| h.original_hash));
        original.map(|orig| orig != live).unwrap_or(false)
    };

    let entity_type_for_title = entity_type.clone();
    let title = move || {
        if id_signal.get().is_some() {
            crate::t!("editor.title.edit", "type" => entity_type_for_title.clone())
        } else {
            t("editor.title.new")
        }
    };

    let entity_type_for_back = entity_type.clone();
    let back_href = move || format!("/entities/{}", entity_type_for_back);

    view! {
        <div>
            <div style="display: flex; align-items: baseline; justify-content: space-between; margin-bottom: 0.75rem;">
                <h1 style=h1_style.clone()>{title}</h1>
                <a href=back_href style=ghost_style.clone()>{move || t("editor.actions.back")}</a>
            </div>

            // Dirty-Indikator
            {move || is_dirty().then(|| view! {
                <div style=muted_style.clone()>{move || t("editor.state.dirty")}</div>
            })}
            {move || server_message.get().map(|k| view! {
                <div style="color:#047857; font-size:0.85rem; margin: 0.25rem 0;">
                    {move || t(&k)}
                </div>
            })}

            // Property-Liste
            {
                let header_for_view = header_registry.clone();
                move || editor_meta.get().map(|meta| {
                    let entity_type = entity_type_signal.get();
                    let props = meta.properties.clone();
                    let messages = validation_messages.get();
                    let header_for_props = header_for_view.clone();
                    view! {
                        <div style="display: flex; flex-direction: column;">
                            {props.into_iter().map(|prop| {
                                let key = prop.key.clone();
                                let value = fields_signal.with(|f| f.get(&key).cloned().unwrap_or(Value::Null));
                                let prop_messages: Vec<_> = messages
                                    .iter()
                                    .filter(|m| m.target.as_deref() == Some(key.as_str()))
                                    .cloned()
                                    .collect();
                                let key_for_change = key.clone();
                                let entity_type_for_change = entity_type.clone();
                                let header_for_change = header_for_props.clone();
                                let on_change: Callback<Value> = Callback::new(move |v: Value| {
                                    fields_signal.update(|f| {
                                        f.insert(key_for_change.clone(), v);
                                    });
                                    if let Some(id) = id_signal.get() {
                                        let entity = Entity {
                                            id,
                                            fields: fields_signal.get(),
                                        };
                                        header_for_change.update(|hr| hr.touch(&entity_type_for_change, &entity));
                                    }
                                });
                                view! {
                                    <FieldEditor meta=prop value=value on_change=on_change messages=prop_messages />
                                }
                            }).collect_view()}
                        </div>
                    }
                })
            }

            // Actions
            <div style="display: flex; gap: 0.5rem; margin-top: 1rem;">
                <button
                    style=primary_style.clone()
                    disabled=move || saving.get()
                    on:click=move |_| on_save.run(())
                >
                    {move || if saving.get() {
                        t("editor.actions.saving")
                    } else {
                        t("editor.actions.save")
                    }}
                </button>
                {move || id_signal.get().is_some().then(|| {
                    let secondary = secondary_style.clone();
                    view! {
                        <button
                            style=secondary
                            on:click=move |_| on_delete.run(())
                        >
                            {move || t("editor.actions.delete")}
                        </button>
                    }.into_any()
                })}
            </div>
        </div>
    }
}
