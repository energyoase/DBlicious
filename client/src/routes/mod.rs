//! Routen-Komponenten.
//!
//! Module:
//!   - `dashboard` und `entity_list`/`editor` leben hier inline
//!     (Datei wuerde sonst aufgeblaeht)
//!   - `LoginPage` ist die nicht-authentifizierte Einstiegsseite.

mod editor;
mod login;

use std::rc::Rc;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::auth::AuthContext;
use crate::components::designer::Designer;
use crate::components::table::{
    filters::default_registry, BottomMenu, DeleteAction, EditAction, EntityTableShell,
    GlobalFilter, PageSize, Pager, RemoteSource, RowActions, SelectionColumn, SelectionMode,
    TableView, TopMenu,
};
use crate::i18n::t;
use crate::styling::{use_design, SurfaceLevel, TextVariant};

pub use editor::EditorPage;
pub use login::LoginPage;

#[component]
pub fn DashboardPage() -> impl IntoView {
    let design = use_design();
    let card = design.surface(SurfaceLevel::Card).inline.clone();
    let h1 = design.text(TextVariant::H1).inline.clone();
    let muted = design.text(TextVariant::Muted).inline.clone();
    view! {
        <div style=card>
            <h1 style=h1>{move || t("nav.dashboard")}</h1>
            <p style=muted>"Wähle einen Eintrag aus der Navigation, um eine Entitätsliste zu öffnen."</p>
        </div>
    }
}

#[component]
pub fn EntityListPage() -> impl IntoView {
    use crate::graphql::queries::{fetch_columns, fetch_settings};

    let params = use_params_map();
    let design = use_design();
    let card = design.surface(SurfaceLevel::Card).inline.clone();
    let h1 = design.text(TextVariant::H1).inline.clone();
    let auth = AuthContext::use_context();

    // Spalten-Metadaten vom Server nachladen (zuvor hartkodiert via column_set_for).
    let columns_resource: LocalResource<Vec<shared::ColumnMeta>> = LocalResource::new(move || {
        let entity_type = params.read().get("entity_type").unwrap_or_default();
        async move {
            if entity_type.is_empty() {
                Vec::new()
            } else {
                fetch_columns(&entity_type).await.unwrap_or_default()
            }
        }
    });

    // Settings nachladen, damit Spalten gefiltert/sortiert werden koennen.
    let settings_resource: LocalResource<Option<shared::EntitySettings>> = LocalResource::new(move || {
        let entity_type = params.read().get("entity_type").unwrap_or_default();
        async move {
            if entity_type.is_empty() {
                None
            } else {
                fetch_settings(&entity_type).await.ok().flatten()
            }
        }
    });

    view! {
        <div style=card>
            {move || {
                let entity_type = params.read().get("entity_type").unwrap_or_default();
                let h1 = h1.clone();
                let entity_type_for_table = entity_type.clone();
                let can_read = auth.is_allowed(&entity_type, shared::PermissionOp::Read);

                // Beide Resources lesen; solange noch nicht geladen → Loading-State.
                let columns_loaded = columns_resource.get().map(|r| r.take());
                let settings_loaded = settings_resource.get();

                if !can_read {
                    return view! {
                        <div>
                            <h1 style=h1>{entity_type.clone()}</h1>
                            <p>{move || t("error.validation")}</p>
                        </div>
                    }.into_any();
                }

                let (Some(mut columns), Some(settings_opt)) =
                    (columns_loaded, settings_loaded) else {
                    return view! {
                        <div>
                            <h1 style=h1>{entity_type.clone()}</h1>
                            <p>{move || t("table.loading")}</p>
                        </div>
                    }.into_any();
                };

                let settings = settings_opt.take();

                // Settings auf Spalten anwenden (Visibility, Order, MinWidth).
                if let Some(s) = &settings {
                    apply_settings_to_columns(&mut columns, s);
                }

                let source = Rc::new(RemoteSource::new(entity_type.clone())) as Rc<dyn crate::components::table::DataSource>;

                if columns.is_empty() {
                    view! {
                        <div>
                            <h1 style=h1>{entity_type.clone()}</h1>
                            <p>{move || t("table.empty")}</p>
                        </div>
                    }.into_any()
                } else {
                    let settings_for_table = settings.clone();
                    let can_create = auth.is_allowed(&entity_type, shared::PermissionOp::Create);
                    let new_href = format!("/entities/{}/new", entity_type_for_table);
                    let primary_btn = use_design().button(crate::styling::ButtonVariant::Primary).inline.clone();
                    view! {
                        <div>
                            <h1 style=h1>{entity_type.clone()}</h1>
                            <EntityTableShell
                                columns=columns
                                source=source
                                entity_type=entity_type_for_table
                                settings=settings_for_table
                                filters=Rc::new(default_registry())
                            >
                                <TopMenu>
                                    <GlobalFilter/>
                                    {can_create.then(|| view! {
                                        <a href=new_href style=primary_btn>
                                            {move || t("table.actions.new")}
                                        </a>
                                    })}
                                </TopMenu>
                                <SelectionColumn mode=SelectionMode::Multi/>
                                <RowActions>
                                    <EditAction/>
                                    <DeleteAction/>
                                </RowActions>
                                <TableView/>
                                <BottomMenu>
                                    <Pager/>
                                    <PageSize/>
                                </BottomMenu>
                            </EntityTableShell>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

fn apply_settings_to_columns(
    columns: &mut Vec<shared::ColumnMeta>,
    settings: &shared::EntitySettings,
) {
    // Hidden filtern + sortieren nach `order`.
    columns.retain(|c| {
        match settings.property(&c.key).map(|p| p.visibility) {
            Some(shared::Visibility::Hidden) => false,
            Some(shared::Visibility::DetailOnly) => false,
            _ => true,
        }
    });
    columns.sort_by_key(|c| {
        settings.property(&c.key).map(|p| p.order).unwrap_or(i32::MAX)
    });
}

#[component]
pub fn DesignerPage() -> impl IntoView {
    let design = use_design();
    let card = design.surface(SurfaceLevel::Card).inline.clone();
    let h1 = design.text(TextVariant::H1).inline.clone();
    let auth = AuthContext::use_context();
    // Server-Mutation `saveDbSchema` verlangt `*`-Update; ohne dieses
    // Recht zeigen wir den Designer gar nicht erst.
    let allowed = auth.is_allowed("*", shared::PermissionOp::Update);
    view! {
        <div style=card>
            <h1 style=h1>{move || t("designer.title")}</h1>
            {if allowed {
                view! { <Designer/> }.into_any()
            } else {
                view! { <p>{move || t("designer.forbidden")}</p> }.into_any()
            }}
        </div>
    }
}

#[component]
pub fn BuilderPage() -> impl IntoView {
    use crate::builder::{
        load_tree, provide_history, provide_ui_tree_with, save_tree, BuilderCanvas, SaveOutcome,
        UiTree,
    };
    use crate::components::table::{
        filters::default_registry, BottomMenu, BuilderPreviewSource, EntityTableShell, Pager,
        TableView, DEFAULT_PREVIEW_ROWS,
    };
    use leptos::task::spawn_local;

    let params = use_params_map();
    let design = use_design();
    let card = design.surface(SurfaceLevel::Card).inline.clone();
    let h1 = design.text(TextVariant::H1).inline.clone();
    let muted = design.text(TextVariant::Muted).inline.clone();
    let primary_btn = design.button(crate::styling::ButtonVariant::Primary).inline.clone();
    let secondary_btn = design.button(crate::styling::ButtonVariant::Secondary).inline.clone();

    // Auth-Gate analog zum DesignerPage: ohne Update-Recht kein Builder.
    let auth = AuthContext::use_context();
    let allowed = auth.is_allowed("*", shared::PermissionOp::Update);

    // Builder-State + History fuer den gesamten Unterbaum bereitstellen.
    // Wir starten mit leerem Tree; bei erfolgreichem Load wird er ersetzt.
    let tree_sig = provide_ui_tree_with(UiTree::empty());
    let _history = provide_history();

    let entity_type = move || {
        params
            .with(|p| p.get("entity_type").map(|s| s.to_string()))
            .unwrap_or_default()
    };

    // Phase 1.6: aktuelle Server-Version (None = noch nicht geladen oder
    // Tabelle leer). Save-Mutation schickt das als expected_version mit.
    let current_version: RwSignal<Option<i32>> = RwSignal::new(None);
    // UI-Status: Idle | Loading | Saving | Saved(time) | Conflict(server_version) | Error(msg)
    #[derive(Clone)]
    enum SaveStatus {
        Idle,
        Loading,
        Saving,
        Saved(i32),
        Conflict(i32),
        Error(String),
    }
    let status: RwSignal<SaveStatus> = RwSignal::new(SaveStatus::Idle);

    // Beim Mount: aktive Version laden und in den Tree-Signal kippen.
    let entity_for_load = entity_type();
    if !entity_for_load.is_empty() && allowed {
        status.set(SaveStatus::Loading);
        spawn_local(async move {
            match load_tree(&entity_for_load).await {
                Ok(Some((design, tree))) => {
                    tree_sig.tree.set(tree);
                    current_version.set(Some(design.version));
                    status.set(SaveStatus::Saved(design.version));
                }
                Ok(None) => {
                    // Server hat noch nichts — bleibt Idle, expected_version None.
                    status.set(SaveStatus::Idle);
                }
                Err(e) => {
                    status.set(SaveStatus::Error(format!("load: {e}")));
                }
            }
        });
    }

    let on_save = {
        let entity_for_save = entity_type();
        move |_| {
            let entity = entity_for_save.clone();
            let tree = tree_sig.tree.get();
            let expected = current_version.get();
            status.set(SaveStatus::Saving);
            spawn_local(async move {
                match save_tree(&entity, &tree, expected).await {
                    Ok(SaveOutcome::Ok { design }) => {
                        current_version.set(Some(design.version));
                        status.set(SaveStatus::Saved(design.version));
                    }
                    Ok(SaveOutcome::Conflict { current }) => {
                        current_version.set(Some(current.version));
                        status.set(SaveStatus::Conflict(current.version));
                    }
                    Ok(SaveOutcome::Locked { current }) => {
                        if let Some(c) = &current {
                            current_version.set(Some(c.version));
                        }
                        status.set(SaveStatus::Error("locked".into()));
                    }
                    Ok(SaveOutcome::Error(e)) => {
                        status.set(SaveStatus::Error(e));
                    }
                    Err(e) => {
                        status.set(SaveStatus::Error(format!("network: {e}")));
                    }
                }
            });
        }
    };

    let on_reload_from_server = {
        let entity_for_reload = entity_type();
        move |_| {
            let entity = entity_for_reload.clone();
            status.set(SaveStatus::Loading);
            spawn_local(async move {
                match load_tree(&entity).await {
                    Ok(Some((design, tree))) => {
                        tree_sig.tree.set(tree);
                        current_version.set(Some(design.version));
                        status.set(SaveStatus::Saved(design.version));
                    }
                    Ok(None) => status.set(SaveStatus::Idle),
                    Err(e) => status.set(SaveStatus::Error(format!("reload: {e}"))),
                }
            });
        }
    };

    let status_muted = muted.clone();
    let status_view = {
        let secondary_btn = secondary_btn.clone();
        move || {
            let muted = status_muted.clone();
            match status.get() {
                SaveStatus::Idle => view! {
                    <span style=muted>{move || t("builder.status.idle")}</span>
                }
                .into_any(),
                SaveStatus::Loading => view! {
                    <span style=muted>{move || t("builder.status.loading")}</span>
                }
                .into_any(),
                SaveStatus::Saving => view! {
                    <span style=muted>{move || t("builder.status.saving")}</span>
                }
                .into_any(),
                SaveStatus::Saved(v) => view! {
                    <span style="color: #16a34a;">
                        {move || crate::t!("builder.status.saved", "version" => v as i64)}
                    </span>
                }
                .into_any(),
                SaveStatus::Conflict(v) => {
                    let reload = on_reload_from_server.clone();
                    let style = secondary_btn.clone();
                    view! {
                        <div style="display: flex; gap: 0.5rem; align-items: center; color: #b91c1c;">
                            <span>
                                {move || crate::t!("builder.status.conflict", "version" => v as i64)}
                            </span>
                            <button style=style on:click=reload>
                                {move || t("builder.action.reload")}
                            </button>
                        </div>
                    }
                    .into_any()
                }
                SaveStatus::Error(msg) => view! {
                    <span style="color: #b91c1c;">
                        {move || crate::t!("builder.status.error", "message" => msg.clone())}
                    </span>
                }
                .into_any(),
            }
        }
    };

    view! {
        <div style=card>
            <h1 style=h1>{move || t("builder.title")}</h1>
            <p style=muted.clone()>
                {move || crate::t!("builder.subtitle", "entity" => entity_type())}
            </p>
            {move || if !allowed {
                view! { <p>{move || t("builder.forbidden")}</p> }.into_any()
            } else {
                let primary = primary_btn.clone();
                let on_save = on_save.clone();
                let status_view = status_view.clone();
                view! {
                    <div style="display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.75rem;">
                        <button style=primary on:click=on_save>
                            {move || t("builder.action.save")}
                        </button>
                        {status_view}
                    </div>
                    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1rem;">
                        <BuilderCanvas/>
                        <div style="display: flex; flex-direction: column; gap: 0.5rem;">
                            <h2 style="margin: 0;">{move || t("builder.preview.title")}</h2>
                            // Reaktive Re-Konstruktion der EntityTableShell, sobald sich der Tree
                            // aendert. Rc<dyn DataSource> ist !Send, daher direkt in der Render-
                            // Closure konstruieren (kein Leptos-Memo).
                            {move || {
                                let cols = tree_sig.tree.with(|t| crate::builder::project_columns(t));
                                let src: Rc<dyn crate::components::table::DataSource> = Rc::new(
                                    BuilderPreviewSource::from_columns(&cols, DEFAULT_PREVIEW_ROWS),
                                );
                                let entity = entity_type();
                                let filters = Rc::new(default_registry());
                                view! {
                                    <EntityTableShell
                                        entity_type=entity
                                        columns=cols
                                        source=src
                                        filters=filters
                                    >
                                        <TableView/>
                                        <BottomMenu>
                                            <Pager/>
                                        </BottomMenu>
                                    </EntityTableShell>
                                }
                            }}
                        </div>
                    </div>
                }.into_any()
            }}
        </div>
    }
}

#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <div style="padding: 2rem;">
            <h1>"404"</h1>
            <p>"Diese Seite gibt es nicht."</p>
        </div>
    }
}
