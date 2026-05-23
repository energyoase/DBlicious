//! Drag&Drop-Canvas fuer den Visual-Builder (Phase 1.4).
//!
//! Architektur:
//!   - liest [`UiTreeSignal`] und [`BuilderHistory`] aus dem Leptos-Context,
//!   - positioniert jeden Top-Level-`UiNode` absolut anhand seines
//!     [`Transform`],
//!   - faengt `pointerdown` auf jedem Knoten ab und startet einen Drag,
//!   - `pointermove`/`pointerup` haengen am Canvas-Container (gleicher
//!     Pattern wie [`crate::components::designer::canvas`]), damit der
//!     Cursor auch ausserhalb des Knotens sauber weitergezogen werden
//!     kann,
//!   - Toolbar oberhalb des Canvas: Add / Delete / Undo / Redo.
//!
//! **Granularitaet der History**: ein Drag entspricht **einer**
//! Snapshot-Aufnahme (am Drag-Start), nicht einer pro Pointermove. Add-
//! und Delete-Aktionen sind je ein eigener Snapshot.
//!
//! Scope der ersten Iteration: nur Top-Level-`UiNode`s. Hierarchische
//! Drag-Targets (Knoten in fremde Eltern verschieben) sind eine spaetere
//! Erweiterung — Phase 1.4 liefert das fundamentale Drag-Mechanik plus
//! die zugehoerige Route.

use leptos::prelude::*;
use shared::FieldType;

use super::history::{mutate_with_history, redo, undo};
use super::node::{BoundField, NodeId, Style as NodeStyle, Transform, UiNode};
use super::{use_history, use_ui_tree};
use crate::i18n::t;
use crate::styling::{use_design, ButtonVariant, SurfaceLevel, TextVariant};

/// Defaults fuer einen frisch erzeugten Knoten.
const DEFAULT_NODE_W: f64 = 160.0;
const DEFAULT_NODE_H: f64 = 80.0;
const STAGGER_OFFSET: f64 = 24.0;

/// Laufender Drag-Vorgang.
#[derive(Clone, Copy, Debug)]
struct DragState {
    node_id: NodeId,
    /// Cursor-zu-Origin-Versatz im Moment des `pointerdown` (CSS-Pixel).
    offset_x: f64,
    offset_y: f64,
}

/// Wurzelkomponente des Builder-Canvas.
///
/// Erwartet, dass [`super::provide_ui_tree`] und [`super::provide_history`]
/// vom Aufrufer (Builder-Route, siehe `routes::BuilderPage`) bereits im
/// Context bereitgestellt wurden.
#[component]
pub fn BuilderCanvas() -> impl IntoView {
    let tree_sig = use_ui_tree();
    let history = use_history();
    let design = use_design();

    let canvas_surface = design.surface(SurfaceLevel::Card).inline.clone();
    let muted = design.text(TextVariant::Muted).inline.clone();

    let selected: RwSignal<Option<NodeId>> = RwSignal::new(None);
    let drag: RwSignal<Option<DragState>> = RwSignal::new(None);

    // ----- Container-globale Drag-Handler -----
    let on_pointer_move = move |ev: web_sys::PointerEvent| {
        let Some(d) = drag.get_untracked() else { return };
        let x = (ev.client_x() as f64 - d.offset_x).max(0.0);
        let y = (ev.client_y() as f64 - d.offset_y).max(0.0);
        tree_sig.update(|t| {
            if let Some(node) = t.find_mut(d.node_id) {
                node.transform.x = x;
                node.transform.y = y;
            }
        });
    };
    let on_pointer_up = move |_| {
        if drag.get_untracked().is_some() {
            drag.set(None);
        }
    };
    let on_canvas_click = move |_| {
        selected.set(None);
    };

    // ----- Toolbar-Aktionen -----
    let on_add = move |_| {
        mutate_with_history(tree_sig, history, |t| {
            let id = t.allocate_id();
            // Versetzte Stapelung, damit neue Knoten nicht aufeinander liegen.
            let n = t.nodes.len() as f64;
            t.push_root(UiNode {
                id,
                transform: Transform {
                    x: 16.0 + n * STAGGER_OFFSET,
                    y: 16.0 + n * STAGGER_OFFSET,
                    w: DEFAULT_NODE_W,
                    h: DEFAULT_NODE_H,
                },
                style: NodeStyle {
                    token_ref: Some("table_cell".into()),
                },
                bound_field: Some(BoundField {
                    key: format!("field_{}", id.0),
                    field_type: Some(FieldType::Text),
                    label_key: None,
                    sortable: None,
                    filterable: None,
                }),
                event_trigger: None,
                draggable: true,
                children: Vec::new(),
                kind: Default::default(),
            });
            selected.set(Some(id));
        });
    };

    let on_delete = move |_| {
        let Some(id) = selected.get_untracked() else { return };
        mutate_with_history(tree_sig, history, |t| {
            t.remove(id);
        });
        selected.set(None);
    };

    let on_undo = move |_| {
        undo(tree_sig, history);
        selected.set(None);
    };
    let on_redo = move |_| {
        redo(tree_sig, history);
        selected.set(None);
    };

    let btn_primary = design.button(ButtonVariant::Primary).inline.clone();
    let btn_secondary = design.button(ButtonVariant::Secondary).inline.clone();
    let btn_ghost = design.button(ButtonVariant::Ghost).inline.clone();

    // Capture-Clones der Styles fuer reaktive Buttons.
    let btn_secondary_for_delete = btn_secondary.clone();
    let btn_ghost_for_undo = btn_ghost.clone();
    let btn_ghost_for_redo = btn_ghost.clone();

    view! {
        <div style="display: flex; flex-direction: column; gap: 0.75rem;">
            // ----- Toolbar -----
            <div style="display: flex; gap: 0.5rem; align-items: center;">
                <button style=btn_primary on:click=on_add>
                    {move || t("builder.action.add")}
                </button>
                <button
                    style=move || {
                        let mut s = btn_secondary_for_delete.clone();
                        if selected.get().is_none() {
                            s.push_str(" opacity: 0.5; pointer-events: none;");
                        }
                        s
                    }
                    on:click=on_delete
                >
                    {move || t("builder.action.delete")}
                </button>
                <button
                    style=move || {
                        let mut s = btn_ghost_for_undo.clone();
                        if !history.can_undo() {
                            s.push_str(" opacity: 0.5; pointer-events: none;");
                        }
                        s
                    }
                    on:click=on_undo
                >
                    {move || t("builder.action.undo")}
                </button>
                <button
                    style=move || {
                        let mut s = btn_ghost_for_redo.clone();
                        if !history.can_redo() {
                            s.push_str(" opacity: 0.5; pointer-events: none;");
                        }
                        s
                    }
                    on:click=on_redo
                >
                    {move || t("builder.action.redo")}
                </button>
                <span style=muted>
                    {move || {
                        let count = tree_sig.tree.with(|t| t.nodes.len());
                        crate::t!("builder.nodes_count", "n" => count.to_string())
                    }}
                </span>
            </div>

            // ----- Leinwand -----
            <div
                style=format!("{canvas_surface} position: relative; height: 60vh; overflow: auto;")
                on:pointermove=on_pointer_move
                on:pointerup=on_pointer_up
                on:click=on_canvas_click
            >
                {move || {
                    // Top-Level-Knoten reaktiv rendern. Kinder werden in dieser
                    // Iteration bewusst nicht visualisiert (Hierarchie-Drag ist
                    // Phase 1.4+).
                    tree_sig.tree.with(|t| {
                        t.nodes
                            .iter()
                            .cloned()
                            .map(|node| view! { <BuilderNodeView node selected drag/> })
                            .collect_view()
                    })
                }}
            </div>
        </div>
    }
}

/// Visuelle Repraesentation eines einzelnen `UiNode`. Bewusst flach
/// gehalten — die Phase-4-Codegen-Schicht wird hieraus spaeter konkrete
/// Komponenten-Views generieren; fuer den Designer reicht eine
/// Karten-Darstellung mit Binding-Hinweis.
#[component]
fn BuilderNodeView(
    node: UiNode,
    selected: RwSignal<Option<NodeId>>,
    drag: RwSignal<Option<DragState>>,
) -> impl IntoView {
    let design = use_design();
    let surface = design.surface(SurfaceLevel::Card).inline.clone();
    let caption = design.text(TextVariant::Caption).inline.clone();

    let node_id = node.id;
    let t_clone = node.transform;
    let label = node
        .bound_field
        .as_ref()
        .map(|b| b.key.clone())
        .unwrap_or_else(|| format!("#{}", node_id));
    let is_selected = move || selected.with(|s| s.as_ref() == Some(&node_id));

    let on_pointer_down = move |ev: web_sys::PointerEvent| {
        if ev.button() != 0 {
            return;
        }
        let s = DragState {
            node_id,
            offset_x: ev.client_x() as f64 - t_clone.x,
            offset_y: ev.client_y() as f64 - t_clone.y,
        };
        drag.set(Some(s));
        selected.set(Some(node_id));
        ev.stop_propagation();
        ev.prevent_default();
    };
    let on_node_click = move |ev: web_sys::MouseEvent| {
        selected.set(Some(node_id));
        ev.stop_propagation();
    };

    view! {
        <div
            style=move || {
                let outline = if is_selected() {
                    "outline: 2px solid #3b82f6;".to_string()
                } else {
                    "outline: 1px solid rgba(0,0,0,0.1);".to_string()
                };
                format!(
                    "{surf} position: absolute; left: {x:.0}px; top: {y:.0}px; \
                     width: {w:.0}px; height: {h:.0}px; user-select: none; \
                     cursor: move; padding: 0.5rem; box-sizing: border-box; {outline}",
                    surf = surface,
                    x = t_clone.x,
                    y = t_clone.y,
                    w = t_clone.w.max(40.0),
                    h = t_clone.h.max(24.0),
                    outline = outline,
                )
            }
            on:pointerdown=on_pointer_down
            on:click=on_node_click
        >
            <strong>{label}</strong>
            <div style=caption>{format!("id #{}", node_id)}</div>
        </div>
    }
}
