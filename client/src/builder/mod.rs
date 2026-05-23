//! Visual-Builder — reaktiver UI-State (Phase 1).
//!
//! Stand: Phasen 1.1 und 1.2 — Datenstrukturen und Signal-Wrapper. Drag&
//! Drop-Canvas (1.4), Live-Preview (1.5), Persistenz (1.6) und Undo/Redo
//! (1.7) bauen auf diesem Modul auf, ohne dass dessen Form sich aendern
//! muss.
//!
//! Geometrie:
//!   - [`tree::UiTree`]  — die Wurzel-Datenstruktur (Multi-Root,
//!     rekursive `children`).
//!   - [`node::UiNode`]  — der eigentliche Baustein mit Transform/Style/
//!     Binding/Trigger.
//!   - [`tree::UiTreeSignal`] — `RwSignal`-Wrapper, der ueber Leptos-Context
//!     an alle Builder-Komponenten ausgespielt wird.
//!
//! Verhaeltnis zu `shared`: die Event-Vertraege ([`shared::EventTrigger`],
//! [`shared::GuardExpr`]) leben in `shared::builder`, weil sie auch von
//! Server-Triggern (Phase 2) konsumiert werden. Tree und Knoten sind heute
//! reine Client-Strukturen — Phase 1.6 entscheidet, ob sie mit nach
//! `shared` wandern.

pub mod canvas;
pub mod history;
pub mod node;
pub mod persist;
pub mod project;
pub mod script_node;
pub mod tree;

pub use canvas::BuilderCanvas;
pub use history::{
    mutate_with_history, provide_history, redo, undo, use_history, BuilderHistory,
    DEFAULT_CAPACITY,
};
pub use node::{BoundField, NodeId, Style, Transform, UiNode};
pub use persist::{build_state_blob, load_tree, save_tree, SaveOutcome};
pub use project::{default_filterable, default_sortable, project_columns, project_columns_from_node};
pub use tree::{UiTree, UiTreeSignal, TREE_SCHEMA_VERSION};

use leptos::prelude::*;

/// Installiert einen frischen [`UiTreeSignal`] im Leptos-Context der
/// aktuellen Owner-Scope. Aufrufer (z.B. Phase 1.4 Canvas-Route) rufen
/// diese Funktion in der Route-Komponente; Kind-Komponenten holen sich den
/// Signal-Wrapper via [`use_ui_tree`].
pub fn provide_ui_tree() -> UiTreeSignal {
    let signal = UiTreeSignal::new();
    provide_context(signal);
    signal
}

/// Installiert einen vorgefuellten [`UiTreeSignal`] (z.B. vom Server
/// geladen) im Leptos-Context.
pub fn provide_ui_tree_with(initial: UiTree) -> UiTreeSignal {
    let signal = UiTreeSignal::from_tree(initial);
    provide_context(signal);
    signal
}

/// Holt den aktuellen [`UiTreeSignal`] aus dem Leptos-Context.
///
/// Panikt, wenn vorher kein [`provide_ui_tree`] aufgerufen wurde — das ist
/// ein Programmierfehler des Aufrufers (Builder-Komponente ausserhalb der
/// Builder-Route).
pub fn use_ui_tree() -> UiTreeSignal {
    expect_context::<UiTreeSignal>()
}
