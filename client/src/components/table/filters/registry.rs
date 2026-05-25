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

    /// Prueft, ob eine Filter-ID in der Registry hinterlegt ist.
    pub fn has_factory(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }
}

/// Q0005 — UI-Discovery der pro FieldType passenden Filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterDescriptor {
    pub id: String,
    pub label_key: String,
}

impl FilterRegistry {
    /// Liefert die fuer den gegebenen `FieldType` registrierten Filter
    /// mit ihrer ID + Fluent-Label-Key. Stabil sortiert nach `id`.
    pub fn compatible_for(&self, ft: &shared::FieldType) -> Vec<FilterDescriptor> {
        use shared::FieldType::*;
        let candidates: &[&str] = match ft {
            Text => &["text-contains"],
            Integer | Decimal { .. } | Money { .. } => &["number-range"],
            Boolean => &["bool-equals"],
            Date | DateTime => &["date-range"],
            Reference { .. }
            | Collection { .. }
            | Enum { .. }
            | IntEnum { .. }
            | DirectionalEnum { .. } => &["enum-in"],
        };
        let mut out: Vec<FilterDescriptor> = candidates
            .iter()
            .filter(|&id| self.has_factory(id))
            .map(|id| FilterDescriptor {
                id: (*id).into(),
                label_key: format!("filter.{id}"),
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }
}

#[cfg(test)]
mod compatible_tests {
    use super::*;
    use shared::FieldType;

    fn reg() -> FilterRegistry {
        crate::components::table::filters::default_registry()
    }

    #[test]
    fn money_offers_number_range() {
        let d = reg().compatible_for(&FieldType::Money {
            currency_code_field: None,
        });
        let ids: Vec<&str> = d.iter().map(|f| f.id.as_str()).collect();
        assert!(
            ids.contains(&"number-range"),
            "expected number-range, got {ids:?}"
        );
    }

    #[test]
    fn text_offers_text_contains() {
        let d = reg().compatible_for(&FieldType::Text);
        let ids: Vec<&str> = d.iter().map(|f| f.id.as_str()).collect();
        assert!(
            ids.contains(&"text-contains"),
            "expected text-contains, got {ids:?}"
        );
    }
}
