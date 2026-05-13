//! Render-Bruecke zur [`FieldRegistry`](crate::components::field::FieldRegistry).
//!
//! Die eigentliche Renderlogik pro [`FieldType`] lebt zentral in der
//! Registry. Diese Datei haelt nur noch die Komponenten-Fassade, damit der
//! Aufrufer keinen direkten Kontakt zum Registry-Trait braucht.

use leptos::prelude::*;
use serde_json::Value;
use shared::FieldType;

use crate::components::field::{use_field_registry, FieldContext, FieldMode};
use crate::i18n::I18nContext;

/// Zellen-Renderer fuer eine einzelne Tabellenzelle.
///
/// `fields` enthaelt das gesamte `Entity.fields`-Map, damit
/// kreuzfeldabhaengige Renderer (z.B. Geldbetraege mit Verweis auf das
/// Currency-Code-Feld) ohne Sonderpfad funktionieren.
#[component]
pub fn FieldCell(
    field_type: FieldType,
    key: String,
    value: Value,
    fields: serde_json::Map<String, Value>,
) -> impl IntoView {
    let locale = I18nContext::use_context().locale;
    let registry = use_field_registry();

    view! {
        {move || {
            let l = locale.get();
            registry.render(FieldContext {
                mode: FieldMode::View,
                field: &field_type,
                key: &key,
                value: &value,
                fields: &fields,
                locale: l,
            })
        }}
    }
}
