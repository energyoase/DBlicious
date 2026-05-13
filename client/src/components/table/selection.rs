//! Reaktive Selektion ueber Tabellenzeilen.
//!
//! [`SelectionState`] haelt die Menge aktuell selektierter Entity-IDs. Sie
//! lebt im [`super::shell::TableShellContext`], damit alle Bausteine
//! (Selection-Spalte, Bulk-Aktionen, Row-Aktionen) ohne Props darauf zugreifen.
//!
//! `mode` legt fest, ob die UI Single- oder Multi-Selektion erlaubt; intern
//! ist die Datenstruktur identisch (eine Menge).

use std::collections::BTreeSet;

use leptos::prelude::*;

/// Selektions-Modus, wird auf der [`super::selection_column::SelectionColumn`]
/// pro Tabelle gesetzt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// Keine UI fuer Selektion. Bulk-Aktionen sehen immer eine leere Menge.
    #[default]
    None,
    /// Genau eine Zeile darf selektiert sein.
    Single,
    /// Beliebig viele Zeilen.
    Multi,
}

#[derive(Clone, Copy)]
pub struct SelectionState {
    pub mode: RwSignal<SelectionMode>,
    selected: RwSignal<BTreeSet<String>>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            mode: RwSignal::new(SelectionMode::None),
            selected: RwSignal::new(BTreeSet::new()),
        }
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.with(|s| s.contains(id))
    }

    pub fn count(&self) -> usize {
        self.selected.with(|s| s.len())
    }

    pub fn ids(&self) -> Vec<String> {
        self.selected.with(|s| s.iter().cloned().collect())
    }

    pub fn clear(&self) {
        self.selected.update(|s| s.clear());
    }

    /// Toggle einer einzelnen Zeile. Im Single-Mode ersetzt das die ganze
    /// Auswahl, im Multi-Mode wird ergaenzt/entfernt.
    pub fn toggle(&self, id: &str) {
        let mode = self.mode.get();
        match mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected.update(|s| {
                    if s.contains(id) {
                        s.clear();
                    } else {
                        s.clear();
                        s.insert(id.to_string());
                    }
                });
            }
            SelectionMode::Multi => {
                self.selected.update(|s| {
                    if !s.insert(id.to_string()) {
                        s.remove(id);
                    }
                });
            }
        }
    }

    /// "Alle in der aktuellen Seite selektieren / abwaehlen".
    pub fn toggle_all(&self, page_ids: &[String]) {
        if !matches!(self.mode.get(), SelectionMode::Multi) {
            return;
        }
        self.selected.update(|s| {
            let all_selected = page_ids.iter().all(|id| s.contains(id));
            if all_selected {
                for id in page_ids {
                    s.remove(id);
                }
            } else {
                for id in page_ids {
                    s.insert(id.clone());
                }
            }
        });
    }
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}
