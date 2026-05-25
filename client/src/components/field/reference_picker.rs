//! FK-Referenz-Picker (U1, Editor-Control fuer FieldType::Reference).
//! Sucht Kandidaten der Ziel-Entity (fetch_entities) und filtert sie
//! client-seitig ueber das display_field.

use leptos::prelude::*;
use serde_json::Value;

use crate::graphql::queries;
use crate::i18n::t;
use crate::styling::{use_design, TextVariant, Tokens};

/// Filtert Kandidaten-Zeilen client-seitig ueber das display_field
/// (case-insensitive contains). Leerer Query → alle.
pub fn filter_candidates<'a>(
    rows: &'a [shared::Entity],
    display_field: &str,
    query: &str,
) -> Vec<&'a shared::Entity> {
    let q = query.trim().to_lowercase();
    rows.iter()
        .filter(|e| {
            q.is_empty()
                || e.fields
                    .get(display_field)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase().contains(&q))
                    .unwrap_or(false)
        })
        .collect()
}

/// Liefert das display_field einer Ziel-Entity. Strategie:
/// 1. `fetch_settings(entity_type).display_field` — vom Server konfiguriert.
/// 2. Fallback: erster Text-aehnlicher Spalten-Key, der nicht "id" ist.
/// 3. Letzter Fallback: leerer String (Picker zeigt dann nur IDs).
async fn resolve_display_field(entity_type: &str) -> String {
    // Stufe 1: settings.display_field (vom Server via GraphQL)
    if let Ok(Some(settings)) = queries::fetch_settings(entity_type).await {
        if let Some(df) = settings.display_field {
            if !df.is_empty() {
                return df;
            }
        }
    }
    // Stufe 2: erstes Text-/Integer-Feld, das nicht "id" ist
    match queries::fetch_columns(entity_type).await {
        Ok(cols) => cols
            .into_iter()
            .find(|c| {
                c.key != "id"
                    && matches!(
                        c.field_type,
                        shared::FieldType::Text | shared::FieldType::Integer
                    )
            })
            .map(|c| c.key)
            .unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// Kombiniertes Async-Laden: display_field + Kandidaten-Page.
#[derive(Clone)]
struct PickerData {
    display_field: String,
    candidates: Vec<shared::Entity>,
}

async fn load_picker_data(entity_type: String) -> PickerData {
    let display_field = resolve_display_field(&entity_type).await;
    let candidates = match queries::fetch_entities(&entity_type, 1, 200).await {
        Ok(page) => page.items,
        Err(_) => Vec::new(),
    };
    PickerData {
        display_field,
        candidates,
    }
}

/// Suchbares FK-Editor-Control fuer `FieldType::Reference`.
///
/// Laedt Kandidaten der `target_entity` und filtert sie client-seitig per
/// Texteingabe. Klick auf einen Eintrag ruft `on_change` mit der gewaehlten
/// ID auf.
#[component]
pub fn ReferencePicker(
    /// Entity-Typ des FK-Ziels (z.B. "customer").
    target_entity: String,
    /// Aktuell gespeicherte ID (oder None, wenn leer).
    current_id: Option<String>,
    /// Callback bei Auswahl eines Kandidaten; erhaelt die neue ID als String-Value.
    on_change: Callback<Value>,
) -> impl IntoView {
    let design = use_design();
    let input_style = design.input().inline.clone();
    let text_muted = design.text(TextVariant::Muted).inline.clone();
    let text_caption = design.text(TextVariant::Caption).inline.clone();

    // Suchtext-Signal
    let query = RwSignal::new(String::new());
    // Aktuell gewaehlt (startet mit current_id)
    let selected_id: RwSignal<Option<String>> = RwSignal::new(current_id);

    // Async-Lade: display_field + Kandidaten
    let entity_type_for_resource = target_entity.clone();
    let picker_resource: LocalResource<PickerData> =
        LocalResource::new(move || load_picker_data(entity_type_for_resource.clone()));

    // Auswahl-Label aus geladenen Kandidaten aufloesen
    let selected_label = move || -> Option<String> {
        let id = selected_id.get()?;
        let resource_val = picker_resource.get()?;
        let data = resource_val.take();
        if data.display_field.is_empty() {
            return Some(id.clone());
        }
        // Label aus Kandidaten suchen; Fallback: id
        let label = data
            .candidates
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.fields.get(&data.display_field))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| id.clone());
        Some(label)
    };

    view! {
        <div style="display: flex; flex-direction: column; gap: 0.25rem;">
            // Suchfeld
            <input
                type="text"
                style=input_style.clone()
                placeholder=move || t("picker.search.placeholder")
                prop:value=move || query.get()
                on:input=move |ev| {
                    query.set(event_target_value(&ev));
                }
            />

            // Kandidaten-Liste
            {move || {
                let resource_val = picker_resource.get();
                match resource_val {
                    None => {
                        // Noch am Laden
                        let muted = text_muted.clone();
                        view! {
                            <div style=format!("{muted} padding: 0.25rem 0;")>
                                {move || t("picker.loading")}
                            </div>
                        }.into_any()
                    }
                    Some(data) => {
                        let data = data.take();
                        let display_field = data.display_field.clone();
                        let candidates = data.candidates;
                        let q = query.get();
                        let filtered = filter_candidates(&candidates, &display_field, &q);
                        let sel = selected_id.get();

                        if filtered.is_empty() {
                            let muted = text_muted.clone();
                            view! {
                                <div style=format!("{muted} font-style: italic; padding: 0.25rem 0;")>
                                    {move || t("picker.no_results")}
                                </div>
                            }.into_any()
                        } else {
                            let list_style = format!(
                                "border: 1px solid {border}; border-radius: {radius}; \
                                 max-height: 12rem; overflow-y: auto;",
                                border = Tokens::COLOR_BORDER,
                                radius = Tokens::RADIUS_SM,
                            );
                            let items: Vec<_> = filtered
                                .into_iter()
                                .map(|e| {
                                    let id = e.id.clone();
                                    let label = if display_field.is_empty() {
                                        e.id.clone()
                                    } else {
                                        e.fields
                                            .get(&display_field)
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| e.id.clone())
                                    };
                                    let is_selected = sel.as_deref() == Some(&e.id);
                                    let item_style = if is_selected {
                                        format!(
                                            "padding: 0.3rem 0.5rem; cursor: pointer; \
                                             border-radius: {radius}; font-size: 0.9rem; \
                                             background: rgba(59,130,246,0.12); font-weight: 600;",
                                            radius = Tokens::RADIUS_SM,
                                        )
                                    } else {
                                        format!(
                                            "padding: 0.3rem 0.5rem; cursor: pointer; \
                                             border-radius: {radius}; font-size: 0.9rem; \
                                             background: transparent;",
                                            radius = Tokens::RADIUS_SM,
                                        )
                                    };
                                    let id_for_click = id.clone();
                                    let on_click = move |_| {
                                        selected_id.set(Some(id_for_click.clone()));
                                        on_change.run(Value::String(id_for_click.clone()));
                                    };
                                    view! {
                                        <div
                                            style=item_style
                                            on:click=on_click
                                        >
                                            {label}
                                        </div>
                                    }
                                })
                                .collect();
                            view! {
                                <div style=list_style>
                                    {items}
                                </div>
                            }.into_any()
                        }
                    }
                }
            }}

            // Aktuelle Auswahl anzeigen — zeigt das Display-Label, nicht die rohe ID
            {move || {
                selected_label().map(|label| {
                    let caption = text_caption.clone();
                    view! {
                        <div style=format!("{caption} color: {};", Tokens::COLOR_TEXT_PRIMARY)>
                            {move || t("picker.selected_label")}
                            {": "}
                            <strong>{label.clone()}</strong>
                        </div>
                    }
                })
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ent(id: &str, name: &str) -> shared::Entity {
        let mut m = serde_json::Map::new();
        m.insert("displayName".into(), json!(name));
        shared::Entity {
            id: id.into(),
            fields: m,
        }
    }

    #[test]
    fn filters_by_display_field_case_insensitive() {
        let rows = vec![
            ent("c-1", "Max Mustermann"),
            ent("c-2", "Erika"),
            ent("c-3", "maximilian"),
        ];
        let hits = filter_candidates(&rows, "displayName", "max");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn empty_query_returns_all() {
        let rows = vec![ent("c-1", "A"), ent("c-2", "B")];
        assert_eq!(filter_candidates(&rows, "displayName", "  ").len(), 2);
    }

    #[test]
    fn missing_display_field_does_not_match() {
        let rows = vec![ent("c-1", "A")];
        assert!(filter_candidates(&rows, "nonexistent", "a").is_empty());
    }
}
