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
    filters::default_registry, ActiveDrag, BottomMenu, BuilderPreviewSource, ColumnEditorPopover,
    DeleteAction, DragReorderCtx, EditAction, EntityTableShell, GlobalFilter, PageSize, Pager,
    RemoteSource, RowActions, SelectionColumn, SelectionMode, TableView, TopMenu,
    DEFAULT_PREVIEW_ROWS,
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

/// Sucht in einem Navigations-Baum (auch geschachtelt) den ersten Knoten,
/// dessen aufgeloeste `Link`-Route exakt `route` ist, und liefert dessen
/// `labelKey`. Wird vom H1-Title-Lookup der `EntityListPage` benutzt, damit
/// der Seiten-Titel mit der Sidebar-Beschriftung uebereinstimmt (Q0006).
fn find_nav_label_key(nodes: &[shared::NavigationNode], route: &str) -> Option<String> {
    for n in nodes {
        if let shared::MenuAction::Link { route: r } = n.resolved_action() {
            if r == route {
                return Some(n.label_key.clone());
            }
        }
        if let Some(found) = find_nav_label_key(&n.children, route) {
            return Some(found);
        }
    }
    None
}

/// H1-Titel einer Entity-Listen-Seite: aus Nav-`labelKey` aufgeloest,
/// Fallback auf den `entity_type`-Slug solange Nav noch laedt oder die
/// Entity dort nicht verlinkt ist.
fn entity_list_h1_title(
    entity_type: &str,
    nav_resource: LocalResource<Vec<shared::NavigationNode>>,
) -> String {
    if entity_type.is_empty() {
        return String::new();
    }
    let route = format!("/entities/{entity_type}");
    match nav_resource
        .get()
        .and_then(|r| find_nav_label_key(&r.take(), &route))
    {
        Some(k) => t(&k),
        None => entity_type.to_string(),
    }
}

#[component]
pub fn EntityListPage() -> impl IntoView {
    use std::collections::HashMap;
    use crate::graphql::queries::{fetch_columns, fetch_navigation, fetch_settings};
    use shared::view::{ViewLayer, ViewPropertyOverride};

    let params = use_params_map();
    let design = use_design();
    let card = design.surface(SurfaceLevel::Card).inline.clone();
    let h1 = design.text(TextVariant::H1).inline.clone();
    let auth = AuthContext::use_context();

    // Aktiver View-Name aus dem Query-Parameter ?view=… (Default: "default").
    let view_name = move || {
        leptos_router::hooks::use_query_map()
            .with(|q| q.get("view").map(|s| s.to_string()))
            .unwrap_or_else(|| "default".into())
    };

    // View-Edit-State-Signale (werden in L2/L3/L4 genutzt).
    let edit_mode: RwSignal<bool> = RwSignal::new(false);
    let edit_layer: RwSignal<ViewLayer> = RwSignal::new(ViewLayer::Global);
    let pending_overrides: RwSignal<HashMap<String, ViewPropertyOverride>> =
        RwSignal::new(HashMap::new());
    let open_popover_for: RwSignal<Option<(String, f64, f64)>> = RwSignal::new(None);
    let current_view_version: RwSignal<Option<i32>> = RwSignal::new(None);

    // Aktuelle Named View vom Server nachladen.
    let current_view: LocalResource<Option<shared::view::EntityView>> =
        LocalResource::new(move || {
            let et = params.read().get("entity_type").unwrap_or_default();
            let vn = view_name();
            async move {
                if et.is_empty() {
                    None
                } else {
                    crate::graphql::queries::fetch_entity_view(&et, &vn)
                        .await
                        .ok()
                        .flatten()
                }
            }
        });

    // Spiegelt die Server-Version in das current_view_version-Signal,
    // damit der Save-Flow (L4) optimistic locking machen kann.
    Effect::new(move |_| {
        if let Some(wrapper) = current_view.get() {
            if let Some(v) = wrapper.take() {
                current_view_version.set(Some(v.version));
            }
        }
    });

    // Kontext fuer L3 (ColumnEditorPopover) und L2 (TopMenu edit-mode toggle).
    provide_context::<RwSignal<bool>>(edit_mode);
    provide_context::<RwSignal<Option<(String, f64, f64)>>>(open_popover_for);

    // Q0007: Drag-Reorder-Kontext fuer Spaltenheader. Commit-Callback
    // schreibt eine `order`-Override fuer jede umgeordnete Spalte in
    // `pending_overrides`. Save-Flow (L4) persistiert wie bei den anderen
    // Header-Popover-Edits.
    let drag_reorder_active: RwSignal<Option<ActiveDrag>> = RwSignal::new(None);
    let drag_commit = Callback::new(move |updates: Vec<(String, i32)>| {
        pending_overrides.update(|m| {
            for (key, order) in updates {
                let entry = m
                    .entry(key.clone())
                    .or_insert_with(|| shared::view::ViewPropertyOverride {
                        key: key.clone(),
                        ..Default::default()
                    });
                entry.order = Some(order);
            }
        });
    });
    provide_context(DragReorderCtx {
        active_drag: drag_reorder_active,
        commit:      drag_commit,
    });

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

    // Navigations-Baum fuer den H1-Titel-Lookup (Q0006): aktive Route
    // → Nav-Knoten → labelKey → t(). Solange Nav noch laedt, faellt der
    // Title auf den entity_type-Slug zurueck.
    let nav_resource: LocalResource<Vec<shared::NavigationNode>> =
        LocalResource::new(|| async { fetch_navigation().await.unwrap_or_default() });

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

    // Save-Callback: schickt pending_overrides an den Server und räumt den
    // Edit-Mode auf — Konflikt-Fall zeigt Browser-Alert und aktualisiert
    // die gespeicherte Version für den nächsten Save-Versuch.
    let on_save: Callback<()> = Callback::new(move |_| {
        let et = params.read().get("entity_type").unwrap_or_default();
        let vn = view_name();
        let expected = current_view_version.get();
        let overrides: Vec<shared::view::ViewPropertyOverride> =
            pending_overrides.with(|p| p.values().cloned().collect());
        let payload = serde_json::json!({
            "properties": overrides,
            "defaultFilter": serde_json::Value::Null,
            "defaultSort":   serde_json::Value::Null,
            "defaultPageSize": serde_json::Value::Null,
        });
        leptos::task::spawn_local(async move {
            let res = crate::graphql::queries::save_entity_view(
                crate::graphql::queries::SaveEntityViewInputClient {
                    entity_type: &et,
                    view_name:   &vn,
                    layer:       shared::view::ViewLayer::Global,
                    owner_id:    None,
                    payload,
                    expected_version: expected,
                }
            ).await;
            match res {
                Ok(outcome) if outcome.kind == "OK" => {
                    pending_overrides.update(|p| p.clear());
                    edit_mode.set(false);
                    open_popover_for.set(None);
                    current_view.refetch();
                }
                Ok(outcome) if outcome.kind == "CONFLICT" => {
                    if let Some(server_view) = outcome.view.as_ref() {
                        current_view_version.set(Some(server_view.version));
                    }
                    let _ = web_sys::window()
                        .and_then(|w| w.alert_with_message(
                            &crate::i18n::t("table-view-conflict")
                        ).ok());
                }
                Ok(outcome) => {
                    log::warn!("save_entity_view: {}: {:?}", outcome.kind, outcome.message);
                }
                Err(e) => log::error!("save_entity_view RPC: {e}"),
            }
        });
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
                    let et_h1 = entity_type.clone();
                    return view! {
                        <div>
                            <h1 style=h1>{move || entity_list_h1_title(&et_h1, nav_resource)}</h1>
                            <p>{move || t("error.validation")}</p>
                        </div>
                    }.into_any();
                }

                let (Some(mut columns), Some(settings_opt)) =
                    (columns_loaded, settings_loaded) else {
                    let et_h1 = entity_type.clone();
                    return view! {
                        <div>
                            <h1 style=h1>{move || entity_list_h1_title(&et_h1, nav_resource)}</h1>
                            <p>{move || t("table.loading")}</p>
                        </div>
                    }.into_any();
                };

                let settings = settings_opt.take();

                // Settings auf Spalten anwenden (Visibility, Order, MinWidth).
                if let Some(s) = &settings {
                    apply_settings_to_columns(&mut columns, s);
                }

                // Server-gespeicherte View-Overrides auf ColumnMeta-seitige Felder
                // anwenden (sortable / filter_id / formatter_id). Die
                // PropertySettings-seitigen Felder (visibility, order, min_width,
                // label) laufen bereits über apply_settings_to_columns.
                if let Some(view_wrapped) = current_view.get() {
                    if let Some(view) = view_wrapped.take() {
                        for ov in &view.properties {
                            if let Some(col) = columns.iter_mut().find(|c| c.key == ov.key) {
                                if let Some(s) = ov.sortable {
                                    col.sortable = s;
                                }
                                if let Some(id) = &ov.filter_id_override {
                                    col.filter_id = Some(id.clone());
                                }
                                if let Some(id) = &ov.formatter_id_override {
                                    col.formatter_id = Some(id.clone());
                                }
                            }
                        }
                    }
                }

                // Pending Overrides aus dem Edit-Mode auf Spalten + Settings anwenden.
                {
                    use crate::components::table::apply_pending_overrides;
                    let mut settings_for_overrides = settings.clone().unwrap_or_default();
                    apply_pending_overrides(
                        &mut columns,
                        &mut settings_for_overrides,
                        &pending_overrides.get(),
                    );
                }

                let source: Rc<dyn crate::components::table::DataSource> = if edit_mode.get() {
                    let preview_columns = columns.clone();
                    Rc::new(BuilderPreviewSource::from_columns(&preview_columns, DEFAULT_PREVIEW_ROWS))
                } else {
                    Rc::new(RemoteSource::new(entity_type.clone()))
                };

                if columns.is_empty() {
                    let et_h1 = entity_type.clone();
                    view! {
                        <div>
                            <h1 style=h1>{move || entity_list_h1_title(&et_h1, nav_resource)}</h1>
                            <p>{move || t("table.empty")}</p>
                        </div>
                    }.into_any()
                } else {
                    let settings_for_table = settings.clone();
                    let can_create = auth.is_allowed(&entity_type, shared::PermissionOp::Create);
                    let can_update = auth.is_allowed(&entity_type, shared::PermissionOp::Update);
                    let new_href = format!("/entities/{}/new", entity_type_for_table);
                    let builder_href = format!("/builder/{}", entity_type_for_table);
                    let design = use_design();
                    let primary_btn = design.button(crate::styling::ButtonVariant::Primary).inline.clone();
                    let secondary_btn = design.button(crate::styling::ButtonVariant::Secondary).inline.clone();
                    let muted = design.text(TextVariant::Muted).inline.clone();
                    let et_h1 = entity_type.clone();
                    view! {
                        <div>
                            <h1 style=h1>{move || entity_list_h1_title(&et_h1, nav_resource)}</h1>
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
                                        <a href=new_href style=primary_btn.clone()>
                                            {move || t("table.actions.new")}
                                        </a>
                                    })}
                                    {can_update.then(|| view! {
                                        <a href=builder_href style=secondary_btn.clone()>
                                            {move || t("table.actions.builder")}
                                        </a>
                                    })}
                                    {can_update.then(|| {
                                        let edit_mode_btn = secondary_btn.clone();
                                        view! {
                                            <button
                                                style=edit_mode_btn
                                                on:click=move |_| edit_mode.update(|b| *b = !*b)
                                            >
                                                {move || if edit_mode.get() {
                                                    t("table.actions.discard-view")
                                                } else {
                                                    t("table.actions.edit-mode")
                                                }}
                                            </button>
                                        }
                                    })}
                                    {move || (edit_mode.get() && !pending_overrides.with(|p| p.is_empty())).then(|| {
                                        let save_btn = primary_btn.clone();
                                        view! {
                                            <button style=save_btn on:click=move |_| on_save.run(())>
                                                {move || t("table.actions.save-view")}
                                            </button>
                                        }
                                    })}
                                    {move || edit_mode.get().then(|| {
                                        let muted_pill = muted.clone();
                                        view! {
                                            <span style=muted_pill>
                                                {move || crate::t!("table.status.edit-layer", "layer" => format!("{:?}", edit_layer.get()))}
                                                " · "
                                                {move || crate::t!("table.status.pending", "n" => pending_overrides.with(|p| p.len()) as i64)}
                                            </span>
                                        }
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
            // Popover: erscheint wenn ein Spalten-Header im Edit-Mode geklickt wird.
            {move || open_popover_for.get().map(|(key, top, left)| {
                use shared::view::ViewPropertyOverride;
                // Aktuelle Spalten-Metadaten aus der Resource lesen (ohne pending Overrides —
                // der Popover zeigt immer die Original-ColumnMeta als Basis).
                let opt_columns = columns_resource.get().map(|r| r.take()).unwrap_or_default();
                let Some(col) = opt_columns.iter().find(|c| c.key == key).cloned() else {
                    return view! { <span style="display:none;"/> }.into_any();
                };
                let key_for_sig    = key.clone();
                let key_for_change = key.clone();
                let key_for_reset  = key.clone();
                // Fallback-Kette: pending Override → server-gespeicherter
                // View-Override → None. Sonst zeigt der Popover „leer", obwohl
                // der Server bereits einen Override hält (Q0008).
                let ov_sig: Signal<Option<ViewPropertyOverride>> = Signal::derive(move || {
                    if let Some(ov) = pending_overrides.with(|p| p.get(&key_for_sig).cloned()) {
                        return Some(ov);
                    }
                    current_view
                        .get()
                        .and_then(|w| w.take())
                        .and_then(|v| v.properties.into_iter().find(|ov| ov.key == key_for_sig))
                });
                view! {
                    <ColumnEditorPopover
                        column=col
                        current_override=ov_sig
                        on_change=Callback::new(move |ov: ViewPropertyOverride| {
                            pending_overrides.update(|m| { m.insert(key_for_change.clone(), ov); });
                        })
                        on_reset=Callback::new(move |_: ()| {
                            pending_overrides.update(|m| { m.remove(&key_for_reset); });
                        })
                        on_close=Callback::new(move |_: ()| open_popover_for.set(None))
                        top=top
                        left=left
                    />
                }.into_any()
            })}
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

    // Auth-Gate analog zum EditorPage: Update-Recht auf die konkrete Entity
    // genuegt — wer eine Entity editieren darf, darf auch ihr Layout
    // (Builder) bearbeiten.
    let auth = AuthContext::use_context();
    let allowed_entity = params
        .with(|p| p.get("entity_type").map(|s| s.to_string()))
        .unwrap_or_default();
    let allowed = !allowed_entity.is_empty()
        && auth.is_allowed(&allowed_entity, shared::PermissionOp::Update);

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
