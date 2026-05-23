//! Wurzelkomponente des Designers: Toolbar + Leinwand.
//!
//! Die Leinwand besteht aus zwei Layern:
//!  1. einem `<svg>`-Layer fuer die Beziehungs-Bezier-Kurven,
//!  2. einem HTML-Layer mit absolut positionierten `<TableBox>`-Karten.
//!
//! Beide Layer leben in einem gemeinsamen, scrollbaren Container, dessen
//! Mindestmasse aus den aeusseren Tabellenkoordinaten errechnet werden.
//! Drag-Move/Drag-Up werden hier zentral abgegriffen, damit der Cursor auch
//! ausserhalb der Karte sauber weiter gezogen werden kann.

use leptos::prelude::*;
use shared::DbTable;

use super::connector::{anchors_for, bezier_path, bounds};
use super::model::{provide_designer_model, use_designer_model};
use super::table_box::TableBox;
use super::toolbar::{DesignerStatusBanner, DesignerToolbar};
use crate::i18n::t;
use crate::styling::{use_design, TextVariant};

#[component]
pub fn Designer() -> impl IntoView {
    // Modell bereitstellen.
    let model = provide_designer_model();
    let design = use_design();
    let canvas_style = design.designer_canvas().inline.clone();
    let muted = design.text(TextVariant::Muted).inline.clone();

    // ----- Globale Drag-Handler -----
    let on_pointer_move = move |ev: web_sys::PointerEvent| {
        if let Some(drag) = model.drag.get_untracked() {
            let x = ev.client_x() as f64 - drag.offset_x;
            let y = ev.client_y() as f64 - drag.offset_y;
            // Negativ-Drift verhindern, damit die Karte nicht aus der Leinwand rutscht.
            let x = x.max(0.0);
            let y = y.max(0.0);
            model.set_position(&drag.table_id, x, y);
        }
    };
    let on_pointer_up = move |_| {
        if model.drag.get_untracked().is_some() {
            model.drag.set(None);
        }
    };

    // Klick auf leere Leinwand: Auswahl aufheben + ggf. Linkmodus-Pending zuruecknehmen.
    let on_canvas_click = move |_| {
        model.selected_table.set(None);
        model.cancel_pending_link();
    };

    let canvas_size = move || {
        let tables = model.tables.get();
        let (w, h) = bounds(&tables);
        format!("min-width: {w:.0}px; min-height: {h:.0}px;")
    };

    view! {
        <div style="display: flex; flex-direction: column; gap: 0.75rem;">
            <DesignerToolbar/>
            <p style=muted.clone()>{move || t("designer.hint")}</p>
            <DesignerStatusBanner/>

            // Aeusserer scrollbarer Container, Innen-Container fuer absolute Positionierung.
            <div style=format!("{canvas_style} height: 70vh;")>
                <div
                    style=move || format!(
                        "position: relative; {size}",
                        size = canvas_size()
                    )
                    on:pointermove=on_pointer_move
                    on:pointerup=on_pointer_up
                    on:click=on_canvas_click
                >
                    // --- SVG-Layer: Verbindungen ---
                    <RelationsLayer/>

                    // --- HTML-Layer: Tabellen ---
                    {move || {
                        let tables = model.tables.get();
                        tables.into_iter().map(|t| view! { <PositionedTable table=t /> }).collect_view()
                    }}
                </div>
            </div>
        </div>
    }
}

#[component]
fn PositionedTable(table: DbTable) -> impl IntoView {
    let model = use_designer_model();
    let table_id = table.id.clone();
    let pos_style = move || {
        let id = table_id.clone();
        model.tables.with(|tables| {
            tables
                .iter()
                .find(|t| t.id == id)
                .map(|t| {
                    format!(
                        "position: absolute; left: {}px; top: {}px;",
                        t.position.x, t.position.y
                    )
                })
                .unwrap_or_default()
        })
    };
    view! {
        <div style=pos_style>
            <TableBox table=table />
        </div>
    }
}

#[component]
fn RelationsLayer() -> impl IntoView {
    let model = use_designer_model();

    let width = move || {
        let tables = model.tables.get();
        format!("{:.0}", bounds(&tables).0)
    };
    let height = move || {
        let tables = model.tables.get();
        format!("{:.0}", bounds(&tables).1)
    };

    view! {
        <svg
            // Pointer-Events ausschliesslich auf den Pfaden, sonst „durchklickbar".
            style="position: absolute; left: 0; top: 0; pointer-events: none;"
            width=width
            height=height
        >
            // Pfeilspitze als Wiederverwendung.
            <defs>
                <marker
                    id="designer-arrow"
                    viewBox="0 0 10 10"
                    refX="9"
                    refY="5"
                    markerWidth="6"
                    markerHeight="6"
                    orient="auto-start-reverse"
                >
                    <path d="M 0 0 L 10 5 L 0 10 z" fill="#3b82f6"></path>
                </marker>
            </defs>

            {move || {
                let tables = model.tables.get();
                let relations = model.relations.get();
                relations.into_iter().filter_map(|rel| {
                    let src = tables.iter().find(|t| t.id == rel.source_table_id)?;
                    let tgt = tables.iter().find(|t| t.id == rel.target_table_id)?;
                    let pair = rel.column_pairs.first()?;
                    let (s, t) = anchors_for(src, &pair.source_column_id, tgt, &pair.target_column_id)?;
                    let d = bezier_path(s, t);
                    let rel_id = rel.id.clone();
                    let on_click = move |ev: web_sys::MouseEvent| {
                        ev.stop_propagation();
                        model.remove_relation(&rel_id);
                    };
                    Some(view! {
                        <path
                            d=d
                            fill="none"
                            stroke="#3b82f6"
                            stroke-width="2"
                            marker-end="url(#designer-arrow)"
                            style="pointer-events: stroke; cursor: pointer;"
                            on:click=on_click
                        >
                            <title>{move || crate::i18n::t("designer.relation.tooltip")}</title>
                        </path>
                    })
                }).collect_view()
            }}
        </svg>
    }
}
