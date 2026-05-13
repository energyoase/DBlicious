//! Property-Filter-Komponenten und Filter-Registry.
//!
//! Filter-Komponenten sind eigenstaendige Leptos-Komponenten mit einer
//! `column: ColumnMeta`-Prop. Sie konsumieren den
//! [`super::shell::TableShellContext`] und schreiben Aenderungen direkt
//! in [`shared::FilterCriteria::predicates`].
//!
//! Resolution pro Spalte (siehe [`resolve_filter_id`]):
//!   1. `ColumnMeta.filter_id` (Server-Vorgabe).
//!   2. Client-Default pro [`shared::FieldType`].
//!   3. Kein Filter-UI.

pub mod bool_equals;
pub mod date_range;
pub mod enum_in;
pub mod helpers;
pub mod number_range;
pub mod registry;
pub mod text_contains;

pub use bool_equals::BoolEqualsFilter;
pub use date_range::DateRangeFilter;
pub use enum_in::EnumInFilter;
pub use number_range::NumberRangeFilter;
pub use registry::{FilterContext, FilterFactory, FilterRegistry};
pub use text_contains::TextContainsFilter;

use std::sync::Arc;

use leptos::prelude::*;
use shared::{ColumnMeta, FieldType};

/// Liefert die Filter-ID, die fuer eine Spalte gerendert werden soll.
/// `None` bedeutet "kein Filter-UI fuer diese Spalte".
pub fn resolve_filter_id(column: &ColumnMeta, registry: &FilterRegistry) -> Option<&'static str> {
    // 1) Server-Vorgabe gewinnt, wenn registriert.
    if let Some(id) = column.filter_id.as_deref() {
        if registry.get(id).is_some() {
            // Wir wissen die ID ist registriert; geben aber die owned String
            // via 'static-Lookup zurueck. Pragmatisch: wir erlauben nur die
            // bekannten Standard-IDs als &'static, alle anderen -> Default.
            // Daher hier auf Default fallback, wenn ID nicht einer der
            // bekannten Standard-IDs entspricht.
            return match id {
                text_contains::ID => Some(text_contains::ID),
                number_range::ID => Some(number_range::ID),
                bool_equals::ID => Some(bool_equals::ID),
                enum_in::ID => Some(enum_in::ID),
                date_range::ID => Some(date_range::ID),
                _ => default_id_for(&column.field_type),
            };
        }
    }
    // 2) Default pro FieldType.
    default_id_for(&column.field_type)
}

fn default_id_for(ft: &FieldType) -> Option<&'static str> {
    match ft {
        FieldType::Text => Some(text_contains::ID),
        FieldType::Integer | FieldType::Decimal { .. } | FieldType::Money { .. } => {
            Some(number_range::ID)
        }
        FieldType::Boolean => Some(bool_equals::ID),
        FieldType::Enum { .. } => Some(enum_in::ID),
        FieldType::Date | FieldType::DateTime => Some(date_range::ID),
        FieldType::Reference { .. } | FieldType::Collection { .. } => None,
    }
}

/// Standard-Registry, die alle eingebauten Filter unter ihren IDs anbietet.
/// Aufruf-Code kann darauf weitere Custom-Filter ergaenzen oder bestehende
/// IDs ueberschreiben.
pub fn default_registry() -> FilterRegistry {
    let mut r = FilterRegistry::new();
    register_factory(&mut r, text_contains::ID, |ctx| {
        view! { <TextContainsFilter column=ctx.column/> }.into_any()
    });
    register_factory(&mut r, number_range::ID, |ctx| {
        view! { <NumberRangeFilter column=ctx.column/> }.into_any()
    });
    register_factory(&mut r, bool_equals::ID, |ctx| {
        view! { <BoolEqualsFilter column=ctx.column/> }.into_any()
    });
    register_factory(&mut r, enum_in::ID, |ctx| {
        view! { <EnumInFilter column=ctx.column/> }.into_any()
    });
    register_factory(&mut r, date_range::ID, |ctx| {
        view! { <DateRangeFilter column=ctx.column/> }.into_any()
    });
    r
}

fn register_factory(
    registry: &mut FilterRegistry,
    id: &str,
    factory: impl Fn(FilterContext) -> AnyView + Send + Sync + 'static,
) {
    registry.register(id, Arc::new(factory));
}
