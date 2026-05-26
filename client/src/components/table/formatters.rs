//! Render-Bruecke zur [`FieldRegistry`](crate::components::field::FieldRegistry).
//!
//! Die eigentliche Renderlogik pro [`FieldType`] lebt zentral in der
//! Registry. Diese Datei haelt nur noch die Komponenten-Fassade, damit der
//! Aufrufer keinen direkten Kontakt zum Registry-Trait braucht.

use leptos::prelude::*;
use serde_json::Value;
use shared::FieldType;

use crate::components::field::{use_field_registry, FieldContext, FieldMode};
use crate::components::script_renderer::ScriptRenderEnv;
use crate::i18n::I18nContext;
use crate::script::provider_lookup::{
    lookup_provider, script_value_to_display, LookupResult, SCRIPT_PREFIX,
};

/// Zellen-Renderer fuer eine einzelne Tabellenzelle.
///
/// `fields` enthaelt das gesamte `Entity.fields`-Map, damit
/// kreuzfeldabhaengige Renderer (z.B. Geldbetraege mit Verweis auf das
/// Currency-Code-Feld) ohne Sonderpfad funktionieren.
///
/// `formatter_id` ist die optionale Formatter-Implementierungs-ID aus
/// [`shared::ColumnMeta`]. Hat sie den `script:`-Prefix und ist eine
/// [`ScriptRenderEnv`] im Leptos-Context, wird das Provider-Skript
/// ausgefuehrt. Andernfalls – oder bei jedem Fehler – faellt der Renderer
/// auf den statischen Default-Pfad zurueck.
#[component]
pub fn FieldCell(
    field_type: FieldType,
    key: String,
    value: Value,
    fields: serde_json::Map<String, Value>,
    #[prop(default = None)] formatter_id: Option<String>,
    /// Aufgeloestes Display-Label fuer Reference-Felder. Wird von `BodyRow`
    /// aus der `DataResponse.reference_labels`-Map pro Zelle befuellt.
    #[prop(default = None)]
    reference_label: Option<String>,
) -> impl IntoView {
    let locale = I18nContext::use_context().locale;
    let registry = use_field_registry();

    // Script-Pfad: nur wenn formatter_id einen `script:`-Prefix traegt und
    // eine ScriptRenderEnv im Leptos-Context vorhanden ist.
    let script_out = {
        let value_for_script = value.clone();
        let fields_for_script = fields.clone();
        let formatter_id_for_script = formatter_id.clone();
        move || -> Option<String> {
            let fid = formatter_id_for_script.clone()?;
            if !fid.starts_with(SCRIPT_PREFIX) {
                return None;
            }
            let env = use_context::<ScriptRenderEnv>()?;
            // Reaktive Subskription auf den Bump-Counter: sobald
            // `app.rs` nach `refresh_from_server` erhoht, laeuft diese
            // Closure neu und ruft das jetzt verfuegbare Script auf.
            let _ = env.scripts_version.get();
            let host = env.host.clone();
            let ctx = env.make_ctx();
            let inputs = shared::script::engine::ScriptInputs {
                value: value_for_script.clone(),
                fields: fields_for_script.clone(),
            };
            match lookup_provider(
                &fid,
                shared::script::model::ProviderSlot::Formatter,
                &env.registry,
                host,
                ctx,
                inputs,
            ) {
                LookupResult::Ok { value } => Some(script_value_to_display(&value)),
                _ => None,
            }
        }
    };

    view! {
        {move || {
            if let Some(s) = script_out() {
                view! { <span>{s}</span> }.into_any()
            } else {
                let l = locale.get();
                registry.render(FieldContext {
                    mode: FieldMode::View,
                    field: &field_type,
                    key: &key,
                    value: &value,
                    fields: &fields,
                    locale: l,
                    reference_label: reference_label.as_deref(),
                }).into_any()
            }
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

/// Prueft, ob eine `formatter_id` den `script:`-Prefix traegt. Keine
/// `ScriptRenderEnv` noetig — reine ID-Klassifikation. Aufgerufen von
/// `FieldCell` und auch direkt testbar.
pub fn formatter_id_is_script(id: &Option<String>) -> bool {
    id.as_deref()
        .map(|s| s.starts_with(SCRIPT_PREFIX))
        .unwrap_or(false)
}

/// Hilfsfunktion: liefert das aufgeloeste Label fuer eine Zelle aus der
/// `reference_labels`-Map, oder `None`. Extrahiert aus `BodyRow`, damit es
/// separat testbar ist.
pub fn resolve_reference_label<'a>(
    reference_labels: &'a std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, String>,
    >,
    col_key: &str,
    row_id: &str,
) -> Option<&'a str> {
    // Server-Achse: { row_id → { col_key → label } }
    reference_labels
        .get(row_id)
        .and_then(|by_col| by_col.get(col_key))
        .map(String::as_str)
}

#[cfg(test)]
mod reference_label_tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Baut eine Map in der korrekten Server-Achse: { row_id → { col_key → label } }
    fn make_labels() -> BTreeMap<String, BTreeMap<String, String>> {
        let mut row1 = BTreeMap::new();
        row1.insert("category_id".into(), "Werkzeug".into());
        let mut row2 = BTreeMap::new();
        row2.insert("category_id".into(), "Material".into());
        let mut outer = BTreeMap::new();
        outer.insert("row-1".into(), row1);
        outer.insert("row-2".into(), row2);
        outer
    }

    #[test]
    fn resolve_reference_label_hit() {
        let labels = make_labels();
        assert_eq!(
            resolve_reference_label(&labels, "category_id", "row-1"),
            Some("Werkzeug")
        );
        assert_eq!(
            resolve_reference_label(&labels, "category_id", "row-2"),
            Some("Material")
        );
    }

    #[test]
    fn resolve_reference_label_miss_unknown_col() {
        let labels = make_labels();
        assert_eq!(
            resolve_reference_label(&labels, "nonexistent", "row-1"),
            None
        );
    }

    #[test]
    fn resolve_reference_label_miss_unknown_row() {
        let labels = make_labels();
        assert_eq!(
            resolve_reference_label(&labels, "category_id", "row-999"),
            None
        );
    }

    #[test]
    fn resolve_reference_label_empty_map() {
        let labels = BTreeMap::new();
        assert_eq!(
            resolve_reference_label(&labels, "category_id", "row-1"),
            None
        );
    }

    /// Verifiziert, dass eine Map, die EXAKT der Server-Ausgabe entspricht
    /// ({ row_id → { col_key → label } }), korrekt aufgeloest wird.
    #[test]
    fn server_shaped_map_resolves_correctly() {
        // Server baut: out[row.id][col_key] = label  (reference.rs:65-67)
        let mut inner = BTreeMap::new();
        inner.insert("customer".into(), "Erika Mustermann".into());
        let mut server_map = BTreeMap::new();
        server_map.insert("o-0001".into(), inner);

        assert_eq!(
            resolve_reference_label(&server_map, "customer", "o-0001"),
            Some("Erika Mustermann"),
            "Server-Achse {{ row_id → {{ col_key → label }} }} muss korrekt aufgeloest werden"
        );
        // Falsche Reihenfolge (transponiert) liefert None
        assert_eq!(
            resolve_reference_label(&server_map, "o-0001", "customer"),
            None,
            "Transponierte Achse darf nicht aufgeloest werden"
        );
    }
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

#[cfg(test)]
mod script_branch_tests {
    use super::*;

    #[test]
    fn none_formatter_id_is_not_script() {
        assert!(!formatter_id_is_script(&None));
    }

    #[test]
    fn static_formatter_id_is_not_script() {
        assert!(!formatter_id_is_script(&Some("money-symbol".into())));
        assert!(!formatter_id_is_script(&Some("text-plain".into())));
        assert!(!formatter_id_is_script(&Some("date-iso".into())));
    }

    #[test]
    fn script_prefix_is_recognised() {
        assert!(formatter_id_is_script(&Some("script:my-formatter".into())));
        assert!(formatter_id_is_script(&Some("script:".into())));
    }
}
