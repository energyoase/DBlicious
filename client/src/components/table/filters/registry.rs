//! Skelett der Filter-Registry. Vollausbau in Phase 0.5.8.
//!
//! Die Registry kennt String-IDs (`"text-contains"`, `"number-range"`, …) und
//! liefert pro ID eine Komponente, die ein [`shared::ColumnFilter`] in den
//! Tabellen-State schreibt. Heute leer; Standard-Filter und das
//! `<Table>`-Konsumieren werden in 0.5.8 ergaenzt.

use std::collections::BTreeMap;
use std::sync::Arc;

use leptos::prelude::*;
use shared::ColumnMeta;

/// Wird einer registrierten Filter-Komponente uebergeben: die `ColumnMeta`
/// fuer die Spalte und eine Callback-Schnittstelle, um Aenderungen am
/// [`shared::FilterCriteria`] zu signalisieren.
#[derive(Clone)]
pub struct FilterContext {
    pub column: ColumnMeta,
}

/// Builder fuer eine konkrete Filter-Komponente.
pub type FilterFactory = Arc<dyn Fn(FilterContext) -> AnyView + Send + Sync>;

#[derive(Clone, Default)]
pub struct FilterRegistry {
    entries: BTreeMap<String, FilterFactory>,
}

impl FilterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriert eine Filter-Komponente unter einer String-ID. Existierende
    /// IDs werden ueberschrieben — der letzte Aufruf gewinnt.
    pub fn register(&mut self, id: impl Into<String>, factory: FilterFactory) {
        self.entries.insert(id.into(), factory);
    }

    pub fn get(&self, id: &str) -> Option<&FilterFactory> {
        self.entries.get(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}
