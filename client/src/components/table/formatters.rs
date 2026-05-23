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

// ---------------------------------------------------------------------------
// Q0005 — Formatter-Discovery
// ---------------------------------------------------------------------------

/// Beschreibung eines Formatters fuer die UI-Auswahl (Named-Views-Popover).
///
/// Die `id` wird in `ViewDefinition.column_formatters` gespeichert;
/// `label_key` ist der Fluent-Schluessel fuer die Anzeige im Dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatterDescriptor {
    pub id: String,
    pub label_key: String,
}

/// Gibt alle fuer `ft` sinnvollen Formatter-Descriptor-Eintraege zurueck.
///
/// Die Liste ist rein deklarativ — die eigentlichen Render-Implementierungen
/// leben in der [`FieldRegistry`](crate::components::field::FieldRegistry).
/// K1-Popover ruft diese Funktion auf, um das Format-Dropdown zu befuellen.
pub fn compatible_formatters_for(ft: &shared::FieldType) -> Vec<FormatterDescriptor> {
    use shared::FieldType::*;
    let raw: &[(&str, &str)] = match ft {
        Money { .. } => &[
            ("money-symbol", "formatter.money.symbol"),
            ("money-code", "formatter.money.code"),
            ("money-decimals", "formatter.money.decimals"),
        ],
        Decimal { .. } => &[
            ("decimal-default", "formatter.decimal.default"),
            ("decimal-2", "formatter.decimal.2"),
        ],
        Date => &[
            ("date-iso", "formatter.date.iso"),
            ("date-local", "formatter.date.local"),
        ],
        DateTime => &[
            ("datetime-iso", "formatter.datetime.iso"),
            ("datetime-local", "formatter.datetime.local"),
        ],
        Integer => &[("int-default", "formatter.int.default")],
        Boolean => &[("bool-yesno", "formatter.bool.yesno")],
        Text => &[("text-plain", "formatter.text.plain")],
        _ => &[],
    };
    raw.iter()
        .map(|(id, key)| FormatterDescriptor {
            id: (*id).into(),
            label_key: (*key).into(),
        })
        .collect()
}

#[cfg(test)]
mod compatible_tests {
    use super::*;
    use shared::FieldType;

    #[test]
    fn money_has_three_formatters() {
        let d = compatible_formatters_for(&FieldType::Money {
            currency_code_field: None,
        });
        assert_eq!(d.len(), 3);
    }

    #[test]
    fn boolean_has_yesno_formatter() {
        let d = compatible_formatters_for(&FieldType::Boolean);
        assert_eq!(
            d.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["bool-yesno"]
        );
    }
}
