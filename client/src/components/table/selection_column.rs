//! Marker-Komponente: schaltet den Selektions-Modus der Tabelle.
//!
//! Rendert selbst nichts; `<TableView>` liest den Modus aus dem
//! [`super::selection::SelectionState`] und blendet eine Checkbox-Spalte
//! ein, wenn `mode != None`.

use leptos::prelude::*;

use super::selection::SelectionMode;
use super::shell::use_shell;

#[component]
pub fn SelectionColumn(
    #[prop(default = SelectionMode::Multi)] mode: SelectionMode,
) -> impl IntoView {
    let shell = use_shell();
    shell.selection.mode.set(mode);
    view! { <></> }
}
