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
    BottomMenu, DeleteAction, EditAction, EntityTableShell, GlobalFilter, PageSize, Pager,
    RemoteSource, RowActions, SelectionColumn, SelectionMode, TableView, TopMenu,
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
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <div style="padding: 2rem;">
            <h1>"404"</h1>
            <p>"Diese Seite gibt es nicht."</p>
        </div>
    }
}
