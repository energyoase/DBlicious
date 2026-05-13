//! Gemeinsame Helfer fuer alle Standard-Filter.
//!
//! Filter-Komponenten lesen `TableState.filter`, mutieren den Eintrag fuer
//! ihre Spalte und resetten `page` auf 1. Das wiederkehrende Pattern liegt
//! hier zentral.

use leptos::prelude::*;
use shared::{ColumnFilter, FilterPredicate};

use super::super::state::TableState;

/// Setzt den Praedikat-Eintrag fuer eine Spalte. `predicate = None` entfernt
/// den Eintrag. Pagination wird auf Seite 1 zurueckgesetzt.
pub fn upsert_predicate(state: TableState, key: &str, predicate: Option<FilterPredicate>) {
    state.filter.update(|c| {
        c.predicates.retain(|p| p.key != key);
        if let Some(pred) = predicate {
            c.predicates.push(ColumnFilter {
                key: key.to_string(),
                predicate: pred,
            });
        }
    });
    state.page.set(1);
}

/// Liefert das aktuelle Praedikat fuer eine Spalte (falls vorhanden).
pub fn current_predicate(state: TableState, key: &str) -> Option<FilterPredicate> {
    state
        .filter
        .with(|c| c.predicates.iter().find(|p| p.key == key).map(|p| p.predicate.clone()))
}
