//! Reaktiver Zustand der Tabelle.
//!
//! Sort, Filter und Pagination sind als vollwertige Schnittstellen vorhanden,
//! damit die UI bereits korrekt verdrahtet ist. Die Aktualisierungslogik
//! arbeitet, die Daten werden auf jeder Aenderung neu geladen.
//!
//! In der ersten Iteration werden `sort` und `filter` von `RemoteSource`
//! noch nicht ausgewertet (Server ignoriert die Argumente). Sobald
//! Server- oder Client-Logik dazukommt, sind keine Aenderungen am
//! Tabellen-State noetig.

use leptos::prelude::*;
use shared::{EntitySettings, FilterCriteria, Sort, SortDirection};

#[derive(Clone, Copy)]
pub struct TableState {
    pub page: RwSignal<u32>,
    pub page_size: RwSignal<u32>,
    pub sort: RwSignal<Option<Sort>>,
    pub filter: RwSignal<FilterCriteria>,
}

impl TableState {
    pub fn new() -> Self {
        Self {
            page: RwSignal::new(1),
            page_size: RwSignal::new(10),
            sort: RwSignal::new(None),
            filter: RwSignal::new(FilterCriteria::default()),
        }
    }

    /// Wendet die Defaults aus einem [`EntitySettings`] an: page_size,
    /// default_sort und default_filter. Wird *einmal* nach dem Laden der
    /// Settings aufgerufen — danach uebernehmen User-Eingaben.
    pub fn apply_settings(&self, settings: &EntitySettings) {
        if let Some(ps) = settings.default_page_size {
            self.page_size.set(ps);
        }
        if let Some(s) = &settings.default_sort {
            self.sort.set(Some(s.clone()));
        }
        if let Some(f) = &settings.default_filter {
            self.filter.set(f.clone());
        }
    }

    /// Wird vom Klick auf einen Spaltenkopf aufgerufen.
    /// Stub-Implementierung: rotiert Asc → Desc → keine Sortierung.
    pub fn toggle_sort(&self, field: &str) {
        let current = self.sort.get();
        let next = match current {
            Some(s) if s.field == field && s.direction == SortDirection::Asc => {
                Some(Sort { field: field.into(), direction: SortDirection::Desc })
            }
            Some(s) if s.field == field && s.direction == SortDirection::Desc => None,
            _ => Some(Sort { field: field.into(), direction: SortDirection::Asc }),
        };
        self.sort.set(next);
        self.page.set(1);
    }

    pub fn next_page(&self, total_pages: u32) {
        let p = self.page.get();
        if p < total_pages {
            self.page.set(p + 1);
        }
    }

    pub fn prev_page(&self) {
        let p = self.page.get();
        if p > 1 {
            self.page.set(p - 1);
        }
    }
}

impl Default for TableState {
    fn default() -> Self {
        Self::new()
    }
}
