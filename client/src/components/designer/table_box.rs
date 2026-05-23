//! Karte einer einzelnen Tabelle auf der Designer-Leinwand.
//!
//! Verantwortlich fuer:
//!  - Darstellung (Header, Spaltenliste) ueber das `DesignSystem`,
//!  - Drag-Initialisierung (der eigentliche Move/Up-Handler liegt im Canvas),
//!  - Inline-Editieren von Tabellen-/Spaltennamen,
//!  - Steuerung des Verknuepfungsmodus ueber die linken/rechten „Ports".

use leptos::ev::{Event, MouseEvent};
use leptos::prelude::*;
use shared::{DbColumn, DbColumnType, DbTable};
use wasm_bindgen::JsCast;
use web_sys::HtmlSelectElement;

use super::model::{use_designer_model, DragState, PortAddress, PortSide};
use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant, TextVariant};

#[component]
pub fn TableBox(table: DbTable) -> impl IntoView {
    let model = use_designer_model();
    let design = use_design();
    let table_id = table.id.clone();

    let selected = move || {
        let id = table_id.clone();
        model
            .selected_table
            .with(|s| s.as_deref() == Some(id.as_str()))
    };
    let table_style_for = {
        let design = design.clone();
        move || design.designer_table(selected()).inline
    };
    let header_style = design.designer_table_header().inline.clone();
    let muted_style = design.text(TextVariant::Caption).inline.clone();

    // ----- Drag-Init -----
    let table_id_for_drag = table.id.clone();
    let on_header_pointer_down = move |ev: web_sys::PointerEvent| {
        // Nur primaere Maustaste / Touch.
        if ev.button() != 0 {
            return;
        }
        let pos = model.position_of(&table_id_for_drag);
        let drag = DragState {
            table_id: table_id_for_drag.clone(),
            offset_x: ev.client_x() as f64 - pos.x,
            offset_y: ev.client_y() as f64 - pos.y,
        };
        model.drag.set(Some(drag));
        model.selected_table.set(Some(table_id_for_drag.clone()));
        ev.stop_propagation();
        ev.prevent_default();
    };

    // ----- Tabellen-Aktionen -----
    let table_id_for_select = table.id.clone();
    let on_card_click = move |_| {
        model.selected_table.set(Some(table_id_for_select.clone()));
    };

    let table_id_for_rename = table.id.clone();
    let on_name_input = move |ev: Event| {
        let value: String = event_target_value(&ev);
        model.rename_table(&table_id_for_rename, value);
    };

    let table_id_for_add_col = table.id.clone();
    let on_add_column = move |_| {
        let name = format!("spalte_{}", model.next_id.get_untracked());
        model.add_column(
            &table_id_for_add_col,
            &name,
            DbColumnType::Text,
            false,
            true,
            false,
        );
    };

    let table_id_for_remove = table.id.clone();
    let on_remove_table = move |ev: MouseEvent| {
        ev.stop_propagation();
        model.remove_table(&table_id_for_remove);
    };

    let table_id_for_columns = table.id.clone();
    let columns_signal = move || {
        let id = table_id_for_columns.clone();
        model.tables.with(|tables| {
            tables
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.columns.clone())
                .unwrap_or_default()
        })
    };

    let name_value = {
        let id = table.id.clone();
        move || {
            let id = id.clone();
            model.tables.with(|tables| {
                tables
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default()
            })
        }
    };

    let table_id_for_view = table.id.clone();

    view! {
        <div
            style=table_style_for
            on:pointerdown=on_card_click
        >
            <div style=header_style.clone() on:pointerdown=on_header_pointer_down>
                <input
                    prop:value=name_value
                    on:input=on_name_input
                    on:pointerdown=move |ev: web_sys::PointerEvent| ev.stop_propagation()
                    style="background: transparent; color: inherit; border: none; \
                           font: inherit; font-weight: 600; outline: none; width: 100%; \
                           padding: 0; margin-right: 0.5rem;"
                />
                <button
                    on:click=on_remove_table
                    title=move || t("designer.actions.remove_table")
                    style="background: transparent; color: inherit; border: none; \
                           cursor: pointer; font-size: 1rem; line-height: 1; padding: 0 0.25rem;"
                >
                    "×"
                </button>
            </div>

            <div>
                {move || {
                    let table_id_inner = table_id_for_view.clone();
                    columns_signal()
                        .into_iter()
                        .map(|c| view! { <ColumnRow table_id=table_id_inner.clone() column=c /> })
                        .collect_view()
                }}
            </div>

            <div style="padding: 0.4rem 0.5rem; border-top: 1px solid #e5e7eb; display: flex; justify-content: space-between; align-items: center;">
                <span style=muted_style.clone()>
                    {move || t("designer.column.add_hint")}
                </span>
                <button
                    on:click=on_add_column
                    style=design.button(ButtonVariant::Secondary).inline
                >
                    {move || t("designer.actions.add_column")}
                </button>
            </div>
        </div>
    }
}

#[component]
fn ColumnRow(table_id: String, column: DbColumn) -> impl IntoView {
    let model = use_designer_model();
    let design = use_design();

    let column_id = column.id.clone();
    let column_id_for_left = column_id.clone();
    let column_id_for_right = column_id.clone();
    let column_id_for_rename = column_id.clone();
    let column_id_for_type = column_id.clone();
    let column_id_for_pk = column_id.clone();
    let column_id_for_remove = column_id.clone();

    let table_id_for_left = table_id.clone();
    let table_id_for_right = table_id.clone();
    let table_id_for_rename = table_id.clone();
    let table_id_for_type = table_id.clone();
    let table_id_for_pk = table_id.clone();
    let table_id_for_remove = table_id.clone();

    let column_name = column.name.clone();
    let column_data_type = column.data_type.clone();
    let column_pk = column.primary_key;

    let pending_match = {
        let table_id = table_id.clone();
        let column_id = column_id.clone();
        move |side: PortSide| {
            model.pending_source.with(|p| {
                p.as_ref()
                    .map(|p| p.table_id == table_id && p.column_id == column_id && p.side == side)
                    .unwrap_or(false)
            })
        }
    };
    let pending_match_left = pending_match.clone();
    let pending_match_right = pending_match.clone();

    let selected_row = {
        let table_id = table_id.clone();
        let column_id = column_id.clone();
        move || {
            model.pending_source.with(|p| {
                p.as_ref()
                    .map(|p| p.table_id == table_id && p.column_id == column_id)
                    .unwrap_or(false)
            })
        }
    };

    let row_style = {
        let design = design.clone();
        move || design.designer_column_row(selected_row()).inline
    };
    let design_for_left = design.clone();
    let design_for_right = design.clone();

    let on_left_port = move |ev: MouseEvent| {
        ev.stop_propagation();
        model.handle_port_click(PortAddress {
            table_id: table_id_for_left.clone(),
            column_id: column_id_for_left.clone(),
            side: PortSide::Left,
        });
    };
    let on_right_port = move |ev: MouseEvent| {
        ev.stop_propagation();
        model.handle_port_click(PortAddress {
            table_id: table_id_for_right.clone(),
            column_id: column_id_for_right.clone(),
            side: PortSide::Right,
        });
    };

    let on_name_input = move |ev: Event| {
        let value: String = event_target_value(&ev);
        model.rename_column(&table_id_for_rename, &column_id_for_rename, value);
    };

    let on_type_change = move |ev: Event| {
        let target = ev
            .target()
            .and_then(|t| t.dyn_into::<HtmlSelectElement>().ok());
        let Some(select) = target else { return };
        let new_type = parse_column_type(&select.value());
        model.set_column_type(&table_id_for_type, &column_id_for_type, new_type);
    };

    let on_pk_click = move |ev: MouseEvent| {
        ev.stop_propagation();
        model.toggle_pk(&table_id_for_pk, &column_id_for_pk);
    };

    let on_remove = move |ev: MouseEvent| {
        ev.stop_propagation();
        model.remove_column(&table_id_for_remove, &column_id_for_remove);
    };

    let current_type_value = column_type_value(&column_data_type).to_string();

    view! {
        <div
            style=row_style
            on:pointerdown=move |ev: web_sys::PointerEvent| ev.stop_propagation()
        >
            <span
                style=move || design_for_left.designer_port(pending_match_left(PortSide::Left)).inline
                on:click=on_left_port
                title=move || t("designer.port.tooltip")
            ></span>

            <div style="display: flex; flex-direction: column; gap: 0.15rem; min-width: 0;">
                <input
                    prop:value=column_name
                    on:input=on_name_input
                    style="background: transparent; border: none; outline: none; \
                           font: inherit; padding: 0; width: 100%;"
                />
                <select
                    on:change=on_type_change
                    prop:value=current_type_value
                    style="background: transparent; border: none; outline: none; \
                           font: inherit; font-size: 0.75rem; color: #6b7280; padding: 0;"
                >
                    <option value="text">"text"</option>
                    <option value="integer">"integer"</option>
                    <option value="bigInt">"bigInt"</option>
                    <option value="decimal">"decimal(12,2)"</option>
                    <option value="boolean">"boolean"</option>
                    <option value="date">"date"</option>
                    <option value="dateTime">"dateTime"</option>
                    <option value="uuid">"uuid"</option>
                    <option value="json">"json"</option>
                    <option value="foreignKey">"foreignKey"</option>
                </select>
            </div>

            <div style="display: flex; gap: 0.25rem; align-items: center;">
                <button
                    on:click=on_pk_click
                    title=move || t("designer.column.toggle_pk")
                    style=move || {
                        let active = column_pk;
                        let bg = if active { "#fde68a" } else { "transparent" };
                        let color = if active { "#92400e" } else { "#6b7280" };
                        format!(
                            "border: 1px solid #e5e7eb; background: {bg}; color: {color}; \
                             border-radius: 4px; font-size: 0.7rem; padding: 0 0.3rem; cursor: pointer;"
                        )
                    }
                >
                    "PK"
                </button>
                <button
                    on:click=on_remove
                    title=move || t("designer.actions.remove_column")
                    style="background: transparent; border: none; color: #b91c1c; \
                           font-size: 0.9rem; cursor: pointer; padding: 0 0.2rem;"
                >
                    "×"
                </button>
            </div>

            <span
                style=move || design_for_right.designer_port(pending_match_right(PortSide::Right)).inline
                on:click=on_right_port
                title=move || t("designer.port.tooltip")
            ></span>
        </div>
    }
}

fn column_type_value(t: &DbColumnType) -> &'static str {
    t.kind()
}

fn parse_column_type(s: &str) -> DbColumnType {
    match s {
        "text" => DbColumnType::Text,
        "integer" => DbColumnType::Integer,
        "bigInt" => DbColumnType::BigInt,
        "decimal" => DbColumnType::Decimal {
            precision: 12,
            scale: 2,
        },
        "boolean" => DbColumnType::Boolean,
        "date" => DbColumnType::Date,
        "dateTime" => DbColumnType::DateTime,
        "uuid" => DbColumnType::Uuid,
        "json" => DbColumnType::Json,
        "foreignKey" => DbColumnType::ForeignKey,
        _ => DbColumnType::Text,
    }
}
