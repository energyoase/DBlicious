//! Q0005 — In-Place Column-Editor als Popover neben dem Spalten-Header.

use leptos::prelude::*;
use shared::view::ViewPropertyOverride;
use shared::{ColumnMeta, Visibility};

use crate::components::table::filters::default_registry;
use crate::components::table::formatters::compatible_formatters_for;
use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant, SurfaceLevel, TextVariant};

#[component]
pub fn ColumnEditorPopover(
    column: ColumnMeta,
    current_override: Signal<Option<ViewPropertyOverride>>,
    #[prop(into)] on_change: Callback<ViewPropertyOverride>,
    #[prop(into)] on_reset:  Callback<()>,
    #[prop(into)] on_close:  Callback<()>,
    /// Absolute Position relativ zum Viewport (px).
    top:  f64,
    left: f64,
) -> impl IntoView {
    let design = use_design();
    let card = design.surface(SurfaceLevel::Card).inline.clone();
    let h3 = design.text(TextVariant::H2).inline.clone();
    let muted = design.text(TextVariant::Muted).inline.clone();
    let input_style = design.input().inline.clone();
    let secondary = design.button(ButtonVariant::Secondary).inline.clone();

    // Stabile, clonbare Referenzen auf Column-Metadaten
    let col_key = StoredValue::new(column.key.clone());
    let col_label_key = StoredValue::new(column.label_key.clone());
    let col_field_type = column.field_type.clone();
    let col_field_type_for_preview = col_field_type.clone();
    let col_sortable_default = column.sortable;

    // Accessor-Helper: liefert den aktuellen Override (oder Default mit dem key)
    let ov = move || current_override.get().unwrap_or_else(|| ViewPropertyOverride {
        key: col_key.get_value(),
        ..Default::default()
    });

    let visible    = move || ov().visibility.unwrap_or(Visibility::Visible) == Visibility::Visible;
    let order_val  = move || ov().order.unwrap_or(0);
    let width_val  = move || ov().min_width.unwrap_or(0);
    let label_val  = move || ov().label_override_key.unwrap_or_default();
    let sortable_v = move || ov().sortable.unwrap_or(col_sortable_default);
    let filter_id_v = move || ov().filter_id_override.unwrap_or_default();
    let format_id_v = move || ov().formatter_id_override.unwrap_or_default();

    let filters    = StoredValue::new(default_registry().compatible_for(&col_field_type));
    let formatters = StoredValue::new(compatible_formatters_for(&col_field_type));

    let style = format!("{card} position: absolute; top: {top}px; left: {left}px; width: 280px; z-index: 1000;");

    view! {
        <div style=style tabindex="-1" on:keydown=move |ev| {
            if ev.key() == "Escape" { on_close.run(()); }
        }>
            <div style="display: flex; justify-content: space-between; align-items: center;">
                <h3 style=h3>
                    {move || crate::t!("column-editor-title", "name" => col_label_key.get_value())}
                </h3>
                <div>
                    <button style=secondary.clone() on:click=move |_| on_reset.run(())>
                        {move || t("column-editor-reset")}
                    </button>
                    <button style=secondary.clone() on:click=move |_| on_close.run(())>"✕"</button>
                </div>
            </div>

            // Visibility
            <label style="display: flex; align-items: center; gap: 0.5rem; margin-top: 0.5rem;">
                <input
                    type="checkbox"
                    prop:checked=move || visible()
                    on:change=move |ev| {
                        let checked = event_target_checked(&ev);
                        let next_vis = if checked { Visibility::Visible } else { Visibility::Hidden };
                        let mut next = current_override.get_untracked().unwrap_or_else(|| ViewPropertyOverride {
                            key: col_key.get_value(), ..Default::default()
                        });
                        next.visibility = Some(next_vis);
                        on_change.run(next);
                    }
                />
                {move || t("column-editor-visibility")}
            </label>

            // Position
            <label style="display: flex; gap: 0.5rem; margin-top: 0.5rem;">
                <span>{move || t("column-editor-position")}</span>
                <input
                    type="number"
                    style=input_style.clone()
                    prop:value=move || order_val()
                    on:input=move |ev| {
                        let Ok(o) = event_target_value(&ev).parse::<i32>() else { return; };
                        let mut next = current_override.get_untracked().unwrap_or_else(|| ViewPropertyOverride {
                            key: col_key.get_value(), ..Default::default()
                        });
                        next.order = Some(o);
                        on_change.run(next);
                    }
                />
            </label>

            // Min-Width
            <label style="display: flex; gap: 0.5rem; margin-top: 0.5rem;">
                <span>{move || t("column-editor-min-width")}</span>
                <input
                    type="number"
                    style=input_style.clone()
                    prop:value=move || width_val()
                    on:input=move |ev| {
                        let Ok(w) = event_target_value(&ev).parse::<u32>() else { return; };
                        let mut next = current_override.get_untracked().unwrap_or_else(|| ViewPropertyOverride {
                            key: col_key.get_value(), ..Default::default()
                        });
                        next.min_width = Some(w);
                        on_change.run(next);
                    }
                />
                <span>"px"</span>
            </label>

            // Label
            <label style="display: flex; gap: 0.5rem; margin-top: 0.5rem;">
                <span>{move || t("column-editor-label")}</span>
                <input
                    type="text"
                    style=input_style.clone()
                    prop:value=move || label_val()
                    on:input=move |ev| {
                        let s = event_target_value(&ev);
                        let mut next = current_override.get_untracked().unwrap_or_else(|| ViewPropertyOverride {
                            key: col_key.get_value(), ..Default::default()
                        });
                        next.label_override_key = if s.is_empty() { None } else { Some(s) };
                        on_change.run(next);
                    }
                />
            </label>

            // Sortable
            <label style="display: flex; align-items: center; gap: 0.5rem; margin-top: 0.5rem;">
                <input
                    type="checkbox"
                    prop:checked=move || sortable_v()
                    on:change=move |ev| {
                        let c = event_target_checked(&ev);
                        let mut next = current_override.get_untracked().unwrap_or_else(|| ViewPropertyOverride {
                            key: col_key.get_value(), ..Default::default()
                        });
                        next.sortable = Some(c);
                        on_change.run(next);
                    }
                />
                {move || t("column-editor-sortable")}
            </label>

            // Filter dropdown
            <label style="display: flex; gap: 0.5rem; margin-top: 0.5rem;">
                <span>{move || t("column-editor-filter")}</span>
                <select
                    style=input_style.clone()
                    on:change=move |ev| {
                        let id = event_target_value(&ev);
                        let mut next = current_override.get_untracked().unwrap_or_else(|| ViewPropertyOverride {
                            key: col_key.get_value(), ..Default::default()
                        });
                        next.filter_id_override = if id.is_empty() { None } else { Some(id) };
                        on_change.run(next);
                    }
                >
                    <option value="" selected=move || filter_id_v().is_empty()>"—"</option>
                    {move || filters.with_value(|fs| fs.iter().map(|f| {
                        let id = f.id.clone();
                        let label = f.label_key.clone();
                        let selected = filter_id_v() == id;
                        view! { <option value=id.clone() selected=selected>{label}</option> }
                    }).collect_view())}
                </select>
            </label>

            // Formatter dropdown
            <label style="display: flex; gap: 0.5rem; margin-top: 0.5rem;">
                <span>{move || t("column-editor-format")}</span>
                <select
                    style=input_style.clone()
                    on:change=move |ev| {
                        let id = event_target_value(&ev);
                        let mut next = current_override.get_untracked().unwrap_or_else(|| ViewPropertyOverride {
                            key: col_key.get_value(), ..Default::default()
                        });
                        next.formatter_id_override = if id.is_empty() { None } else { Some(id) };
                        on_change.run(next);
                    }
                >
                    <option value="" selected=move || format_id_v().is_empty()>"—"</option>
                    {move || formatters.with_value(|fs| fs.iter().map(|f| {
                        let id = f.id.clone();
                        let label = f.label_key.clone();
                        let selected = format_id_v() == id;
                        view! { <option value=id.clone() selected=selected>{label}</option> }
                    }).collect_view())}
                </select>
            </label>

            // Preview
            <div style=format!("{muted} margin-top: 0.75rem; padding: 6px 8px; border-top: 1px solid #374151;")>
                {move || t("column-editor-preview")} ": "
                <code>{move || format!("{:?}", col_field_type_for_preview)}</code>
            </div>
        </div>
    }
}
