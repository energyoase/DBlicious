//! Convenience-Wrapper um die dekomponierten Bausteine.
//!
//! Phase 0.5.7 hat die Tabelle in einen Shell- + Baustein-Baum zerlegt
//! (siehe `mod.rs`). Diese Komponente bleibt fuer Bestandsaufrufer als
//! one-shot Wrapper bestehen und stellt die alte Default-Komposition
//! (Toolbar mit Search + New-Button, Sortierung, Edit/Delete-Aktionen,
//! Pagination) bereit.
//!
//! Neue Aufrufer sollten `<EntityTableShell>` + Bausteine direkt benutzen.

use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::{ColumnMeta, Entity, PermissionOp, SortDirection};

use super::data_source::{DataRequest, DataSource};
use super::formatters::FieldCell;
use super::state::TableState;
use crate::auth::AuthContext;
use crate::header::use_header_registry;
use crate::i18n::t;
use crate::styling::{styled, use_design, ButtonVariant};

#[deprecated(note = "use EntityTableShell + composable blocks (TopMenu, TableView, BottomMenu, …)")]
#[component]
pub fn EntityTable(
    columns: Vec<ColumnMeta>,
    source: Rc<dyn DataSource>,
    #[prop(default = String::new())] entity_type: String,
    #[prop(default = None)] settings: Option<shared::EntitySettings>,
) -> impl IntoView {
    let state = TableState::new();
    if let Some(s) = &settings {
        state.apply_settings(s);
    }
    let auth = AuthContext::use_context();
    let can_create = if entity_type.is_empty() {
        false
    } else {
        auth.is_allowed(&entity_type, PermissionOp::Create)
    };
    let can_delete = if entity_type.is_empty() {
        false
    } else {
        auth.is_allowed(&entity_type, PermissionOp::Delete)
    };
    let can_update = if entity_type.is_empty() {
        false
    } else {
        auth.is_allowed(&entity_type, PermissionOp::Update)
    };

    // Lade-Resource: reagiert auf Aenderungen von page/page_size/sort/filter.
    let page = state.page;
    let page_size = state.page_size;
    let sort = state.sort;
    let filter = state.filter;

    let source_for_resource = source.clone();
    let data = LocalResource::new(move || {
        let req = DataRequest {
            page: page.get(),
            page_size: page_size.get(),
            sort: sort.get(),
            filter: filter.get(),
        };
        let src = source_for_resource.clone();
        async move { src.fetch(req).await }
    });

    let design = use_design();
    let table_style = design.table().inline.clone();
    let scroll_container_style = design.table_scroll_container().inline.clone();
    let header_row_style = design.table_header_row().inline.clone();
    let cell_style_for_actions = design.table_cell().inline.clone();
    let header_cell_actions = design.table_header_cell().inline.clone();

    let columns_for_header = columns.clone();
    let columns_for_body = columns.clone();
    let state_for_header = state.clone();
    let trigger_reload = state.clone();
    let entity_type_for_toolbar: String = entity_type.clone();
    let entity_type_for_actions: String = entity_type.clone();
    let has_actions = can_update || can_delete;

    view! {
        <div>
            // ---------- Toolbar mit Filter-/Such-Stubs ----------
            <Toolbar
                state=state.clone()
                entity_type=entity_type_for_toolbar
                can_create=can_create
            />

            // ---------- Eigentliche Tabelle ----------
            <div style=scroll_container_style>
                <table style=table_style>
                    <thead>
                        <tr style=header_row_style>
                            {columns_for_header.into_iter().map(|c| {
                                view! { <HeaderCell column=c state=state_for_header.clone() /> }
                            }).collect_view()}
                            {has_actions.then(|| view! {
                                <th style=header_cell_actions.clone()></th>
                            })}
                        </tr>
                    </thead>
                    <tbody>
                        <Suspense fallback=move || view! {
                            <tr><td colspan="100">{move || t("table.loading")}</td></tr>
                        }>
                            {move || data.get().map(|res| match res.take() {
                                Ok(resp) => {
                                    if resp.items.is_empty() {
                                        view! {
                                            <tr><td colspan="100" style="padding:1rem; text-align:center;">
                                                {move || t("table.empty")}
                                            </td></tr>
                                        }.into_any()
                                    } else {
                                        let cols = columns_for_body.clone();
                                        let entity_type = entity_type_for_actions.clone();
                                        let trigger = trigger_reload.clone();
                                        let cell_style_actions = cell_style_for_actions.clone();
                                        view! {
                                            <For
                                                each={move || resp.items.clone().into_iter().enumerate().collect::<Vec<_>>()}
                                                key={|(_, e)| e.id.clone()}
                                                children={move |(idx, entity): (usize, Entity)| {
                                                    let cols_inner = cols.clone();
                                                    let entity_type = entity_type.clone();
                                                    let trigger = trigger.clone();
                                                    let cell_style_actions = cell_style_actions.clone();
                                                    view! {
                                                        <BodyRow
                                                            entity=entity
                                                            even={idx % 2 == 0}
                                                            columns=cols_inner
                                                            entity_type=entity_type
                                                            can_update=can_update
                                                            can_delete=can_delete
                                                            trigger=trigger
                                                            actions_cell_style=cell_style_actions
                                                        />
                                                    }
                                                }}
                                            />
                                        }.into_any()
                                    }
                                }
                                Err(e) => {
                                    let msg = e.to_string();
                                    view! {
                                        <tr><td colspan="100" style="padding:1rem; color:#b91c1c;">
                                            {move || crate::t!("app.error", "message" => msg.clone())}
                                        </td></tr>
                                    }.into_any()
                                }
                            })}
                        </Suspense>
                    </tbody>
                </table>
            </div>

            // ---------- Pagination ----------
            <Pagination state=state.clone() data=data />
        </div>
    }
}

#[component]
fn Toolbar(
    state: TableState,
    entity_type: String,
    can_create: bool,
) -> impl IntoView {
    let design = use_design();
    let style = design.toolbar().inline.clone();
    // Demo: Search-Input akzeptiert Per-Element-Overrides ueber `styled()`.
    let input_style = styled("table.toolbar.search", design.input()).inline.clone();
    let primary = design.button(ButtonVariant::Primary).inline.clone();

    let on_search = move |ev| {
        let value: String = event_target_value(&ev);
        let mut crit = state.filter.get();
        crit.global_search = if value.is_empty() { None } else { Some(value) };
        state.filter.set(crit);
        state.page.set(1);
    };

    let new_href = format!("/entities/{}/new", entity_type);

    view! {
        <div style=style>
            <input
                type="search"
                placeholder=move || t("table.actions.search")
                style=input_style
                on:input=on_search
            />
            {can_create.then(|| view! {
                <a href=new_href style=primary>
                    {move || t("table.actions.new")}
                </a>
            }.into_any())}
        </div>
    }
}

#[component]
fn HeaderCell(column: ColumnMeta, state: TableState) -> impl IntoView {
    let design = use_design();
    let style = design.table_header_cell().inline.clone();
    let key = column.key.clone();
    let label_key = column.label_key.clone();
    let sortable = column.sortable;

    let sort_indicator = move || {
        let s = state.sort.get();
        match s {
            Some(s) if s.field == key => match s.direction {
                SortDirection::Asc => " ▲",
                SortDirection::Desc => " ▼",
            },
            _ => "",
        }
    };

    let key_for_click = column.key.clone();
    let cursor = if sortable { "pointer" } else { "default" };
    let combined_style = format!("{style} cursor: {cursor}; user-select: none;");

    let on_click = move |_| {
        if sortable {
            state.toggle_sort(&key_for_click);
        }
    };

    view! {
        <th style=combined_style on:click=on_click>
            {move || t(&label_key)}{sort_indicator}
        </th>
    }
}

#[component]
fn BodyRow(
    entity: Entity,
    even: bool,
    columns: Vec<ColumnMeta>,
    entity_type: String,
    can_update: bool,
    can_delete: bool,
    trigger: TableState,
    actions_cell_style: String,
) -> impl IntoView {
    use super::data_source::RemoteSource;

    let design = use_design();
    let row_style = design.table_row(even).inline.clone();
    let cell_style = design.table_cell().inline.clone();
    let ghost = design.button(ButtonVariant::Ghost).inline.clone();
    let entity_id_for_edit = entity.id.clone();
    let entity_id_for_delete = entity.id.clone();
    let entity_type_for_edit = entity_type.clone();
    let entity_type_for_delete = entity_type.clone();
    let edit_href = format!("/entities/{}/{}", entity_type_for_edit, entity_id_for_edit);
    let has_actions = can_update || can_delete;

    // Wenn der Datensatz schon mal geladen war (z.B. ueber den Editor), liest
    // die HeaderRegistry den `original_hash` und stellt ihn der Delete-
    // Mutation als `expected_hash` zur Verfuegung. Sonst senden wir `None` —
    // dann wirkt kein Concurrency-Check.
    let header_for_delete = use_header_registry();
    // Damit die HeaderRegistry beim ersten Render schon einen Eintrag hat,
    // tragen wir die geladene Entitaet hier ein.
    header_for_delete.update(|hr| hr.upsert_loaded(&entity_type, &entity));
    let on_delete = move |_| {
        if let Some(win) = web_sys::window() {
            let confirmed = win
                .confirm_with_message(&t("editor.confirm.delete"))
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        let id = entity_id_for_delete.clone();
        let entity_type = entity_type_for_delete.clone();
        let trigger = trigger.clone();
        let expected = header_for_delete
            .with(|hr| hr.get(&entity_type, &id).map(|h| h.original_hash));
        spawn_local(async move {
            let source = RemoteSource::new(entity_type);
            use super::data_source::DataSource as _;
            if let Some(deletable) = source.deletable() {
                if let Ok(res) = deletable.delete(id, expected).await {
                    if res.ok {
                        // Bumper, damit die Tabelle neu laedt.
                        trigger.page.set(trigger.page.get());
                        trigger.filter.update(|_| {});
                    }
                }
            }
        });
    };

    view! {
        <tr style=row_style>
            {columns.into_iter().map(|col| {
                let value = entity.fields.get(&col.key).cloned().unwrap_or(serde_json::Value::Null);
                let fields = entity.fields.clone();
                let cell_style = cell_style.clone();
                view! {
                    <td style=cell_style>
                        <FieldCell
                            field_type=col.field_type
                            key=col.key
                            value=value
                            fields=fields
                        />
                    </td>
                }
            }).collect_view()}
            {has_actions.then(|| view! {
                <td style=actions_cell_style>
                    <div style="display: flex; gap: 0.25rem;">
                        {can_update.then(|| view! {
                            <a href=edit_href style=ghost.clone()>
                                {move || t("table.actions.edit")}
                            </a>
                        }.into_any())}
                        {can_delete.then(|| view! {
                            <button style=ghost.clone() on:click=on_delete>
                                {move || t("table.actions.delete")}
                            </button>
                        }.into_any())}
                    </div>
                </td>
            }.into_any())}
        </tr>
    }
}

#[component]
fn Pagination(
    state: TableState,
    data: LocalResource<Result<super::data_source::DataResponse, crate::graphql::GqlError>>,
) -> impl IntoView {
    let design = use_design();
    let bar = design.pagination_bar().inline.clone();
    let prev = design.button(ButtonVariant::Secondary).inline.clone();
    let next = design.button(ButtonVariant::Secondary).inline.clone();

    let total_count = move || {
        data.get()
            .and_then(|r| r.take().ok())
            .map(|d| d.total_count)
            .unwrap_or(0)
    };
    let total_pages = move || {
        let ps = state.page_size.get().max(1) as u64;
        ((total_count() + ps - 1) / ps) as u32
    };

    let summary = move || {
        let p = state.page.get();
        let tp = total_pages().max(1);
        crate::t!("table.pagination.summary", "page" => p as i64, "total" => tp as i64)
    };

    let on_prev = {
        let state = state.clone();
        move |_| state.prev_page()
    };
    let on_next = {
        let state = state.clone();
        move |_| state.next_page(total_pages())
    };

    view! {
        <div style=bar>
            <button style=prev on:click=on_prev>{move || t("table.pagination.prev")}</button>
            <span>{summary}</span>
            <button style=next on:click=on_next>{move || t("table.pagination.next")}</button>
        </div>
    }
}
