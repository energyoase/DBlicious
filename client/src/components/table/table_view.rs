//! Die eigentliche Tabellen-Darstellung (Header + Body). Konsumiert
//! ausschliesslich den [`super::shell::TableShellContext`]. Selektion und
//! Per-Row-Aktionen werden eingeblendet, sobald die zugehoerigen Marker-
//! Komponenten ([`super::selection_column::SelectionColumn`],
//! [`super::row_actions::RowActions`]) in den Context geschrieben haben.

use leptos::prelude::*;
use shared::{ColumnMeta, Entity, SortDirection};

use super::actions::RowContext;
use super::column_editor::{compute_reorder, ActiveDrag, DragReorderCtx, HeaderRect};
use super::filters::{resolve_filter_id, FilterContext};
use super::formatters::FieldCell;
use super::selection::SelectionMode;
use super::shell::use_shell;
use super::state::TableState;
use crate::i18n::t;
use crate::styling::use_design;

#[component]
pub fn TableView() -> impl IntoView {
    let shell = use_shell();
    let design = use_design();
    let table_style = design.table().inline.clone();
    let scroll_container_style = design.table_scroll_container().inline.clone();
    let header_row_style = design.table_header_row().inline.clone();
    let header_cell_actions = design.table_header_cell().inline.clone();
    // Styles vorab clonen, damit nachfolgende Closures `design` nicht
    // mehrfach konsumieren.
    let header_cell_selection = design.table_header_cell().inline.clone();
    let header_cell_filter = design.table_header_cell().inline.clone();
    let header_cell_filter_sel = design.table_header_cell().inline.clone();
    let header_cell_filter_actions = design.table_header_cell().inline.clone();

    let columns = shell.columns();
    let state = shell.state;
    let data = shell.data;
    let selection = shell.selection;
    let row_actions_trigger = shell.row_actions_trigger;
    let filters = shell.filters();

    // Pruefen, ob die Filter-Reihe ueberhaupt gerendert werden soll: nur wenn
    // mindestens eine Spalte einen Filter aufloest.
    let any_filterable = columns
        .iter()
        .any(|c| c.filterable && resolve_filter_id(c, &filters).is_some());

    let columns_for_header = columns.clone();
    let columns_for_filter_row = columns.clone();
    let filters_for_row = filters.clone();
    view! {
        <div style=scroll_container_style>
            <table style=table_style>
                <thead>
                    <tr style=header_row_style>
                        // Selektions-Header (sichtbar, sobald SelectionColumn aktiv).
                        {move || {
                            let mode = selection.mode.get();
                            (mode != SelectionMode::None).then(|| {
                                view! {
                                    <th style=header_cell_selection.clone()>
                                        // Multi: "alle in Page" toggle. Single: leer.
                                        {(mode == SelectionMode::Multi).then(|| view! {
                                            <SelectionHeaderCheckbox/>
                                        })}
                                    </th>
                                }
                            })
                        }}
                        // Eigentliche Spaltenkoepfe.
                        {columns_for_header.iter().cloned().map(|c| {
                            view! { <HeaderCell column=c state=state /> }
                        }).collect_view()}
                        // Aktions-Header (leere TH, sobald RowActions aktiv).
                        {move || {
                            let _ = row_actions_trigger.get();
                            let has = shell.with_row_actions(|s| s.borrow().is_some());
                            has.then(|| view! { <th style=header_cell_actions.clone()></th> })
                        }}
                    </tr>
                    // Filter-Reihe: nur wenn mind. eine Spalte einen Filter
                    // aufloest. Selection- und Action-Spalten bleiben leer.
                    {any_filterable.then(|| {
                        let cell_style = header_cell_filter.clone();
                        let cell_style_sel = header_cell_filter_sel.clone();
                        let cell_style_actions = header_cell_filter_actions.clone();
                        let cols = columns_for_filter_row.clone();
                        let filters = filters_for_row.clone();
                        view! {
                            <tr>
                                {move || {
                                    let mode = selection.mode.get();
                                    (mode != SelectionMode::None).then(|| {
                                        view! { <th style=cell_style_sel.clone()></th> }
                                    })
                                }}
                                {cols.iter().map(|c| {
                                    let cell_style = cell_style.clone();
                                    let filter_view = if c.filterable {
                                        resolve_filter_id(c, &filters).and_then(|id| {
                                            filters.get(id).map(|factory| {
                                                factory(FilterContext { column: c.clone() })
                                            })
                                        })
                                    } else {
                                        None
                                    };
                                    view! { <th style=cell_style>{filter_view}</th> }
                                }).collect_view()}
                                {move || {
                                    let _ = row_actions_trigger.get();
                                    let has = shell.with_row_actions(|s| s.borrow().is_some());
                                    has.then(|| {
                                        view! { <th style=cell_style_actions.clone()></th> }
                                    })
                                }}
                            </tr>
                        }
                    })}
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
                                    // Vec extrahieren (statt Rc<Vec>), damit der
                                    // For-children-Closure Send-bar wird.
                                    let cols: Vec<ColumnMeta> = (*shell.columns()).clone();
                                    view! {
                                        <For
                                            each={move || resp.items.clone().into_iter().enumerate().collect::<Vec<_>>()}
                                            key={|(_, e)| e.id.clone()}
                                            children={move |(idx, entity): (usize, Entity)| {
                                                view! {
                                                    <BodyRow
                                                        entity=entity
                                                        even={idx % 2 == 0}
                                                        columns=cols.clone()
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
    }
}

#[component]
fn SelectionHeaderCheckbox() -> impl IntoView {
    let shell = use_shell();
    let data = shell.data;
    let selection = shell.selection;

    let page_ids = move || -> Vec<String> {
        data.get()
            .and_then(|r| r.take().ok())
            .map(|d| d.items.iter().map(|e| e.id.clone()).collect())
            .unwrap_or_default()
    };

    let all_checked = move || {
        let ids = page_ids();
        !ids.is_empty() && ids.iter().all(|id| selection.is_selected(id))
    };

    let on_change = move |_| {
        let ids = page_ids();
        selection.toggle_all(&ids);
    };

    view! {
        <input
            type="checkbox"
            prop:checked=all_checked
            on:change=on_change
        />
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
                SortDirection::Asc => " \u{25B2}",
                SortDirection::Desc => " \u{25BC}",
            },
            _ => "",
        }
    };

    // Edit-Mode-Kontext: wird von EntityListPage bereitgestellt (L1/L3).
    let edit_mode_ctx = use_context::<RwSignal<bool>>();
    let open_popover_ctx = use_context::<RwSignal<Option<(String, f64, f64)>>>();
    let reorder_ctx = use_context::<DragReorderCtx>();

    let key_for_click = column.key.clone();
    let key_for_popover = column.key.clone();
    let key_for_drag = column.key.clone();
    let cursor = if sortable { "pointer" } else { "default" };
    let combined_style =
        format!("{style} cursor: {cursor}; user-select: none; touch-action: none;");

    // Marker, damit `pointerdown` weiss, ob er einen Drag gestartet hat
    // und der spaetere `click` deshalb keinen Popover oeffnen darf.
    let drag_started: RwSignal<bool> = RwSignal::new(false);
    // Mindest-Distanz in px, ab der ein pointermove als Drag gilt — sonst
    // ist es ein normaler Klick (Sort/Popover).
    const DRAG_THRESHOLD_PX: f64 = 4.0;

    let on_click = move |ev: web_sys::MouseEvent| {
        // Click direkt nach Drag schluckt das Event.
        if drag_started.get() {
            drag_started.set(false);
            return;
        }
        // Popover-Logik: nur im Edit-Mode.
        if let (Some(em), Some(pop)) = (edit_mode_ctx, open_popover_ctx) {
            if em.get() {
                use wasm_bindgen::JsCast;
                let (top, left) = ev
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                    .map(|el| {
                        let r = el.get_bounding_client_rect();
                        (r.bottom(), r.left())
                    })
                    .unwrap_or((0.0, 0.0));
                pop.set(Some((key_for_popover.clone(), top, left)));
                return; // im Edit-Mode kein Sort auslösen
            }
        }
        // Normaler Sort-Click (nur außerhalb Edit-Mode).
        if sortable {
            state.toggle_sort(&key_for_click);
        }
    };

    // pointerdown: nur wirken, wenn Edit-Mode aktiv und Reorder-Ctx providet.
    let key_pd = key_for_drag.clone();
    let on_pointer_down = move |ev: web_sys::PointerEvent| {
        let Some(em) = edit_mode_ctx else { return };
        if !em.get() {
            return;
        }
        let Some(ctx) = reorder_ctx else { return };
        use wasm_bindgen::JsCast;
        // Eigene <th>-Element holen, daraus den umliegenden <tr> finden und
        // alle Daten-Headers (= mit data-col) einsammeln. Selection- und
        // Action-Headers tragen das Attribut nicht.
        let Some(target) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        let Some(row) = target.parent_element() else {
            return;
        };
        let cells = row.query_selector_all("th[data-col]").ok();
        let Some(cells) = cells else { return };
        let mut keys: Vec<String> = Vec::with_capacity(cells.length() as usize);
        let mut rects: Vec<HeaderRect> = Vec::with_capacity(cells.length() as usize);
        let mut from_index: Option<usize> = None;
        for i in 0..cells.length() {
            let Some(node) = cells.item(i) else { continue };
            let Ok(el) = node.dyn_into::<web_sys::Element>() else {
                continue;
            };
            let col_key = el.get_attribute("data-col").unwrap_or_default();
            let r = el.get_bounding_client_rect();
            if col_key == key_pd {
                from_index = Some(keys.len());
            }
            keys.push(col_key);
            rects.push(HeaderRect {
                left: r.left(),
                right: r.right(),
            });
        }
        let Some(from_index) = from_index else { return };
        ctx.active_drag.set(Some(ActiveDrag {
            from_index,
            keys,
            rects,
            pointer_x: ev.client_x() as f64,
        }));
        // Pointer-Capture: nachfolgende move/up gehen an dieses Element,
        // egal wo die Maus ist (Standard-Web-Pattern).
        let _ = target.set_pointer_capture(ev.pointer_id());
    };

    let on_pointer_move = move |ev: web_sys::PointerEvent| {
        let Some(ctx) = reorder_ctx else { return };
        ctx.active_drag.update(|opt| {
            if let Some(d) = opt {
                let dx = (ev.client_x() as f64 - d.pointer_x).abs();
                if dx >= DRAG_THRESHOLD_PX {
                    drag_started.set(true);
                }
                d.pointer_x = ev.client_x() as f64;
            }
        });
    };

    let on_pointer_up = move |_ev: web_sys::PointerEvent| {
        let Some(ctx) = reorder_ctx else { return };
        let Some(active) = ctx.active_drag.get() else {
            return;
        };
        ctx.active_drag.set(None);
        // Wenn kein wirklicher Drag entstanden ist (Threshold unterschritten),
        // ueberlassen wir das Event dem `on_click`.
        if !drag_started.get() {
            return;
        }
        let order = compute_reorder(&active.rects, &active.drag_state());
        // Vergleich mit Identitaet — kein Commit, wenn unveraendert.
        let unchanged = order.iter().enumerate().all(|(i, j)| i == *j);
        if unchanged {
            return;
        }
        let updates: Vec<(String, i32)> = order
            .into_iter()
            .enumerate()
            .map(|(new_pos, orig_idx)| (active.keys[orig_idx].clone(), new_pos as i32))
            .collect();
        ctx.commit.run(updates);
    };

    view! {
        <th
            attr:data-col=column.key.clone()
            style=combined_style
            on:click=on_click
            on:pointerdown=on_pointer_down
            on:pointermove=on_pointer_move
            on:pointerup=on_pointer_up
        >
            {move || t(&label_key)}{sort_indicator}
        </th>
    }
}

#[component]
fn BodyRow(entity: Entity, even: bool, columns: Vec<ColumnMeta>) -> impl IntoView {
    let shell = use_shell();
    let design = use_design();
    let row_style = design.table_row(even).inline.clone();
    let cell_style = design.table_cell().inline.clone();

    let selection = shell.selection;
    let row_actions_trigger = shell.row_actions_trigger;

    let entity_id = entity.id.clone();

    // RowContext pro Zeile in den Kontext schieben, damit EditAction/
    // DeleteAction und ggf. Custom-Actions ihre Bezugs-Entitaet finden.
    provide_context(RowContext {
        entity: entity.clone(),
    });

    let cells = columns
        .into_iter()
        .map(|col| {
            let value = entity
                .fields
                .get(&col.key)
                .cloned()
                .unwrap_or(serde_json::Value::Null);
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
        })
        .collect_view();

    let selection_cell = move || {
        let mode = selection.mode.get();
        let entity_id = entity_id.clone();
        (mode != SelectionMode::None).then(|| {
            let id_for_check = entity_id.clone();
            let id_for_toggle = entity_id;
            let is_checked = move || selection.is_selected(&id_for_check);
            let on_change = move |_| selection.toggle(&id_for_toggle);
            view! {
                <td style="width: 2.5rem; text-align: center;">
                    <input type="checkbox" prop:checked=is_checked on:change=on_change/>
                </td>
            }
        })
    };

    let actions_cell = move || {
        let _ = row_actions_trigger.get();
        let view = shell.with_row_actions(|slot| slot.borrow().as_ref().map(|r| (r.0)()));
        view.map(|v| {
            let style = design.table_cell().inline.clone();
            view! {
                <td style=style>
                    <div style="display: flex; gap: 0.25rem;">
                        {v}
                    </div>
                </td>
            }
        })
    };

    view! {
        <tr style=row_style>
            {selection_cell}
            {cells}
            {actions_cell}
        </tr>
    }
}
