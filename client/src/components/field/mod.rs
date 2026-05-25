//! Renderer-Registry fuer Feldtypen + generischer Field-Editor.
//!
//! Zwei Bausteine in einem Modul:
//!   - [`FieldRegistry`] (heute View-only): rendert einen Wert anhand seines
//!     [`FieldType`].
//!   - [`FieldEditor`] (siehe Komponente unten): rendert ein Eingabe-Control
//!     anhand einer [`shared::EditorPropertyMeta`] und sendet Aenderungen
//!     ueber einen `on_change`-Callback an den Aufrufer.
//!
//!
//! Spiegelt das `DesignSystem`-Pattern: Komponenten holen sich die Registry
//! ueber [`use_field_registry`] und delegieren das Zeichnen eines Wertes an
//! sie. Eine alternative Registry (z.B. mit Custom-Editoren fuer Geld,
//! Lookups fuer Referenzen) wird in einer einzigen Zeile in
//! [`provide_field_registry`] gewechselt.
//!
//! Im Vergleich zur C#-Vorlage (`IControl`/`IEditor`/`IViewer`,
//! `[Editor]`/`[Viewer]`-Attribute, `ImplControl`-Persistenz, Selektion
//! ueber `ModelSelectionType.ByType` + Reflection) ersetzt der hier
//! verwendete Trait-Dispatch den gesamten Reflection-Apparat: jede
//! [`FieldType`]-Variante wird im Default-Renderer exhaustiv abgedeckt,
//! Compile-Fehler erzwingen die Pflege.
//!
//! Der Mode (`View` vs. `Edit`) ist absichtlich bereits Teil des Vertrags,
//! auch wenn die heutigen Renderer ausschliesslich `View` ausfuellen — die
//! `Edit`-Aufrufstelle wird durch denselben Trait beliefert, sobald ein
//! Edit-Workflow existiert.

use std::sync::Arc;

use leptos::prelude::*;
use serde_json::Value;
use shared::FieldType;

use crate::i18n::{format, Locale};
use crate::styling::use_design;

/// Anzeige-Modus eines Feldes. `Delete` aus dem C#-Original ist im neuen
/// System eine Tabellen-Aktion, kein Feld-Modus, und faellt deshalb weg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMode {
    View,
    Edit,
}

/// Render-Kontext fuer eine einzelne Zelle.
///
/// Enthaelt nicht nur den eigenen Wert, sondern auch das gesamte
/// `fields`-Map der Entitaet — damit kreuzfeldabhaengige Renderer (z.B.
/// `Money` mit `currency_code_field`) ohne Sonderpfad funktionieren.
pub struct FieldContext<'a> {
    pub mode: FieldMode,
    pub field: &'a FieldType,
    pub key: &'a str,
    pub value: &'a Value,
    pub fields: &'a serde_json::Map<String, Value>,
    pub locale: Locale,
    /// Aufgeloestes Display-Label fuer Reference-Felder. `Some(label)` wenn der
    /// Server ein `reference_labels`-Mapping fuer diese Spalte und Zeile
    /// geliefert hat; `None` → Fallback auf die rohe ID.
    pub reference_label: Option<&'a str>,
}

/// Vertrag fuer alle Renderer-Registries.
pub trait FieldRegistry: Send + Sync {
    fn render(&self, ctx: FieldContext<'_>) -> AnyView;
}

// =============================================================================
// Default-Implementierung
// =============================================================================

/// Eingebaute Registry. Deckt alle [`FieldType`]-Varianten ab. Eigene
/// Registries koennen sie als Fallback nutzen, indem sie sie als Feld
/// halten und nur fuer ausgewaehlte Varianten umlenken.
#[derive(Default, Clone, Copy)]
pub struct DefaultFieldRegistry;

impl FieldRegistry for DefaultFieldRegistry {
    fn render(&self, ctx: FieldContext<'_>) -> AnyView {
        // `Edit`-Mode liefert vorerst dieselbe Darstellung wie `View`. Sobald
        // ein Edit-Workflow eingezogen wird, schalten konkrete Varianten hier
        // auf Eingabe-Controls um.
        match (ctx.field, ctx.value) {
            // -------- Skalar-Typen --------
            (FieldType::Text, Value::String(s)) => render_text(s),
            (FieldType::Text, _) => render_empty(),

            (FieldType::Integer, Value::Number(n)) => {
                let v = n.as_i64().unwrap_or(0);
                view! { <span>{format::integer(v, ctx.locale)}</span> }.into_any()
            }

            (FieldType::Decimal { precision }, Value::Number(n)) => {
                let v = n.as_f64().unwrap_or(0.0);
                let p = *precision;
                view! { <span>{format::decimal(v, p, ctx.locale)}</span> }.into_any()
            }

            (FieldType::Boolean, Value::Bool(b)) => {
                let key = if *b {
                    "value.bool.true"
                } else {
                    "value.bool.false"
                };
                view! { <span>{crate::i18n::t(key)}</span> }.into_any()
            }

            (FieldType::Date, Value::String(s)) => {
                view! { <span>{format::date(s, ctx.locale)}</span> }.into_any()
            }
            (FieldType::DateTime, Value::String(s)) => {
                view! { <span>{format::datetime(s, ctx.locale)}</span> }.into_any()
            }

            (
                FieldType::Money {
                    currency_code_field,
                },
                Value::Number(_),
            ) => {
                let amount = ctx
                    .fields
                    .get(ctx.key)
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let currency = currency_code_field
                    .as_deref()
                    .and_then(|cf| ctx.fields.get(cf))
                    .and_then(Value::as_str)
                    .unwrap_or("EUR")
                    .to_string();
                view! { <span>{format::money(amount, &currency, ctx.locale)}</span> }.into_any()
            }

            (FieldType::Enum { .. }, Value::String(s)) => render_text(s),
            // IntEnum/DirectionalEnum liefern nach der Server-Grenz-Konvertierung
            // einen wire_name-String — wie Enum als Text rendern (statt
            // Placeholder).
            (FieldType::IntEnum { .. }, Value::String(s)) => render_text(s),
            (FieldType::DirectionalEnum { .. }, Value::String(s)) => render_text(s),

            // -------- Reference: zeigt das aufgeloeste Display-Label (wenn vom
            // Server via `reference_labels` geliefert) oder – als Fallback –
            // die rohe ID des verknuepften Datensatzes.
            (FieldType::Reference { .. }, Value::String(s)) if !s.is_empty() => {
                match ctx.reference_label {
                    Some(label) => render_text(label),
                    None => render_reference(s),
                }
            }
            (FieldType::Reference { .. }, Value::Number(n)) => render_reference(&n.to_string()),
            (FieldType::Reference { .. }, Value::Null) => render_empty(),
            (FieldType::Reference { .. }, _) => placeholder("table.placeholder.reference"),

            // -------- Collection: bei kleinen, rein skalaren Arrays inline
            // joinen ("a, b, c"); sonst Count-Platzhalter.
            (FieldType::Collection { .. }, Value::Array(arr)) => render_collection(arr),
            (FieldType::Collection { .. }, _) => {
                placeholder_count("table.placeholder.collection", 0)
            }

            _ => placeholder("table.placeholder.complex"),
        }
    }
}

fn render_text(s: &str) -> AnyView {
    let owned = s.to_string();
    view! { <span>{owned}</span> }.into_any()
}

fn render_empty() -> AnyView {
    view! { <span></span> }.into_any()
}

/// Rendert die ID eines verknuepften Datensatzes mit dezenter Akzentuierung
/// (monospaced, leichter Hintergrund), damit der User auf den ersten Blick
/// erkennt: das ist eine technische Referenz, kein Anzeige-Wert.
fn render_reference(id: &str) -> AnyView {
    let owned = id.to_string();
    let style = "font-family: ui-monospace, monospace; font-size: 0.85em; \
                 padding: 0.05em 0.35em; border-radius: 0.25rem; \
                 background: rgba(0,0,0,0.05); color: #1f2937;";
    view! { <span style=style>{owned}</span> }.into_any()
}

/// Inline-Repraesentation einer Collection. Bei reinen Skalar-Arrays mit
/// max. 3 Eintraegen werden die Werte komma-separiert dargestellt; ansonsten
/// faellt der Renderer auf den Count-Platzhalter zurueck.
fn render_collection(arr: &[Value]) -> AnyView {
    const INLINE_LIMIT: usize = 3;
    let count = arr.len() as i64;

    let all_scalar = arr
        .iter()
        .all(|v| matches!(v, Value::String(_) | Value::Number(_) | Value::Bool(_)));

    if arr.len() <= INLINE_LIMIT && all_scalar && !arr.is_empty() {
        let joined = arr
            .iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        return view! { <span>{joined}</span> }.into_any();
    }

    placeholder_count("table.placeholder.collection", count)
}

fn placeholder(key: &'static str) -> AnyView {
    let design = use_design();
    let style = design.placeholder().inline.clone();
    view! { <span style=style>{move || crate::i18n::t(key)}</span> }.into_any()
}

fn placeholder_count(key: &'static str, count: i64) -> AnyView {
    let design = use_design();
    let style = design.placeholder().inline.clone();
    view! {
        <span style=style>
            {move || crate::t!(key, "count" => count)}
        </span>
    }
    .into_any()
}

// =============================================================================
// Context-Plumbing
// =============================================================================

#[derive(Clone)]
pub struct FieldRegistryHandle(pub Arc<dyn FieldRegistry>);

impl FieldRegistryHandle {
    pub fn new<R: FieldRegistry + 'static>(r: R) -> Self {
        Self(Arc::new(r))
    }
}

impl std::ops::Deref for FieldRegistryHandle {
    type Target = dyn FieldRegistry;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

pub fn provide_field_registry() {
    provide_context(FieldRegistryHandle::new(DefaultFieldRegistry));
}

pub fn use_field_registry() -> FieldRegistryHandle {
    use_context::<FieldRegistryHandle>()
        .expect("Keine FieldRegistry im Context (provide_field_registry fehlt?)")
}

// =============================================================================
// FieldEditor
// =============================================================================
//
// Generisches Eingabe-Control fuer eine [`shared::EditorPropertyMeta`].
// Liefert pro [`FieldType`] das passende HTML-Element und meldet
// Aenderungen ueber `on_change(serde_json::Value)` an den Aufrufer.
//
// Validierungsfehler (typensicher als [`shared::ValidationMessage`]) werden
// optional als rote Note unter dem Control angezeigt.

use fluent::{FluentArgs, FluentValue};
use shared::{ControlKind, EditorPropertyMeta, ValidationMessage};

/// Mini-Editor fuer Listen-Felder.
///
/// Rendert eine einfache Liste der vorhandenen Eintraege (als JSON-String pro
/// Eintrag) mit "Entfernen"-Buttons und einem "Hinzufuegen"-Button am Ende.
/// Das ist bewusst minimalistisch — der erste Anwendungsfall wuerde hier
/// einen entity-spezifischen Sub-Editor anhaengen.
fn render_inline_list(initial: Value, on_change: Callback<Value>) -> AnyView {
    let items = RwSignal::new(match initial {
        Value::Array(a) => a,
        _ => Vec::new(),
    });

    Effect::new(move |_| {
        on_change.run(Value::Array(items.get()));
    });

    let on_add = move |_| {
        items.update(|v| v.push(Value::Object(serde_json::Map::new())));
    };

    view! {
        <div style="border: 1px solid #e5e7eb; border-radius: 4px; padding: 0.5rem; display: flex; flex-direction: column; gap: 0.25rem;">
            <For
                each={move || items.get().into_iter().enumerate().collect::<Vec<_>>()}
                key={|(idx, _)| *idx}
                children={move |(idx, item): (usize, Value)| {
                    let preview = match &item {
                        Value::Object(o) => serde_json::to_string(&o).unwrap_or_default(),
                        other => other.to_string(),
                    };
                    let on_remove = move |_| {
                        items.update(|v| {
                            if idx < v.len() {
                                v.remove(idx);
                            }
                        });
                    };
                    view! {
                        <div style="display: flex; gap: 0.5rem; align-items: center; font-family: monospace; font-size: 0.8rem;">
                            <span style="flex: 1; overflow: hidden; text-overflow: ellipsis;">{preview}</span>
                            <button type="button" on:click=on_remove>"×"</button>
                        </div>
                    }
                }}
            />
            <button type="button" on:click=on_add style="align-self: flex-start; font-size: 0.85rem;">
                {move || crate::i18n::t("table.actions.new")}
            </button>
        </div>
    }
    .into_any()
}

/// Uebersetzt einen Validation-Schluessel und ueberreicht die `args`-Map
/// als Fluent-Substitutionen — `{ $min }` etc. werden dadurch korrekt
/// ersetzt.
fn format_validation(key: &str, args: &serde_json::Map<String, serde_json::Value>) -> String {
    if args.is_empty() {
        return crate::i18n::t(key);
    }
    let mut fluent_args = FluentArgs::new();
    for (k, v) in args {
        let fv: Option<FluentValue<'static>> = match v {
            serde_json::Value::String(s) => Some(FluentValue::from(s.clone())),
            serde_json::Value::Number(n) => n.as_f64().map(FluentValue::from),
            serde_json::Value::Bool(b) => {
                Some(FluentValue::from(if *b { "true" } else { "false" }))
            }
            _ => None,
        };
        if let Some(fv) = fv {
            fluent_args.set(k.clone(), fv);
        }
    }
    crate::i18n::t_with(key, &fluent_args)
}

#[component]
pub fn FieldEditor(
    meta: EditorPropertyMeta,
    value: Value,
    /// Callback bei jeder Aenderung (auf Tasten- / Click-Event).
    on_change: Callback<Value>,
    /// Validierungs-Meldungen, die zu diesem Feld gehoeren.
    #[prop(default = vec![])]
    messages: Vec<ValidationMessage>,
) -> impl IntoView {
    let design = use_design();
    let input_style = design.input().inline.clone();
    let readonly = meta.readonly;
    let placeholder_key = meta.placeholder_key.clone().unwrap_or_default();
    let label_key = meta.label_key.clone();
    let help_key = meta.help_key.clone().unwrap_or_default();
    let field_type = meta.field_type.clone();
    let control = meta.control;

    let value_str = match &value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => value.to_string(),
    };
    let checked = value.as_bool().unwrap_or(false);

    // Validation-Bereich – wertet Fluent-Args aus, damit z.B.
    // `validation.min_length` mit `{ $min }` korrekt substituiert wird.
    let messages_clone = messages.clone();
    let validation_block = move || {
        if messages_clone.is_empty() {
            None
        } else {
            let items: Vec<_> = messages_clone
                .iter()
                .map(|m| {
                    let color = match m.severity {
                        shared::Severity::Error => "#b91c1c",
                        shared::Severity::Warning => "#b45309",
                        shared::Severity::Info => "#2563eb",
                    };
                    let key = m.message_key.clone();
                    let args = m.args.clone();
                    view! {
                        <div style=format!("color: {color}; font-size: 0.8rem;")>
                            {move || format_validation(&key, &args)}
                        </div>
                    }
                })
                .collect();
            Some(view! { <div>{items}</div> })
        }
    };

    // Control-Auswahl: Auto leitet aus FieldType ab, sonst explizit.
    let effective = if matches!(control, ControlKind::Auto) {
        match field_type {
            FieldType::Boolean => ControlKind::Toggle,
            FieldType::Enum { .. } => ControlKind::Select,
            FieldType::Date | FieldType::DateTime => ControlKind::DatePicker,
            FieldType::Reference { .. } => ControlKind::Lookup,
            FieldType::Collection { .. } => ControlKind::InlineList,
            _ => ControlKind::Input,
        }
    } else {
        control
    };

    let on_change_value = on_change;
    let control_view = match effective {
        ControlKind::Toggle => view! {
            <input
                type="checkbox"
                prop:checked=checked
                disabled=readonly
                on:change=move |ev| {
                    let v = event_target_checked(&ev);
                    on_change_value.run(Value::Bool(v));
                }
            />
        }
        .into_any(),
        ControlKind::Select => {
            let values = match &meta.field_type {
                FieldType::Enum { values } => values.clone(),
                _ => Vec::new(),
            };
            view! {
                <select
                    style=input_style.clone()
                    disabled=readonly
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        on_change_value.run(Value::String(v));
                    }
                >
                    {values.into_iter().map(|v| {
                        let selected = v == value_str;
                        let v_label = v.clone();
                        view! { <option value=v selected=selected>{v_label}</option> }
                    }).collect_view()}
                </select>
            }
            .into_any()
        }
        ControlKind::DatePicker => view! {
            <input
                type="date"
                style=input_style.clone()
                prop:value=value_str.clone()
                readonly=readonly
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    on_change_value.run(Value::String(v));
                }
            />
        }
        .into_any(),
        ControlKind::TextArea => view! {
            <textarea
                style=input_style.clone()
                readonly=readonly
                on:input=move |ev| {
                    let v = event_target_value(&ev);
                    on_change_value.run(Value::String(v));
                }
            >{value_str.clone()}</textarea>
        }
        .into_any(),
        ControlKind::Lookup => view! {
            <span style="color: #6b7280; font-style: italic;">
                {move || crate::i18n::t("editor.placeholder.complex")}
            </span>
        }
        .into_any(),
        ControlKind::InlineList => render_inline_list(value, on_change_value).into_any(),
        _ /* Input + Fallback */ => {
            // Zahleneingabe fuer numerische Typen, sonst Text.
            let is_number = matches!(
                meta.field_type,
                FieldType::Integer | FieldType::Decimal { .. } | FieldType::Money { .. }
            );
            let input_type = if is_number { "number" } else { "text" };
            view! {
                <input
                    type=input_type
                    style=input_style.clone()
                    prop:value=value_str.clone()
                    readonly=readonly
                    placeholder=move || crate::i18n::t(&placeholder_key)
                    on:input=move |ev| {
                        let v = event_target_value(&ev);
                        let parsed = if is_number {
                            v.parse::<f64>()
                                .ok()
                                .and_then(|n| serde_json::Number::from_f64(n).map(Value::Number))
                                .unwrap_or(Value::Null)
                        } else {
                            Value::String(v)
                        };
                        on_change_value.run(parsed);
                    }
                />
            }
            .into_any()
        }
    };

    view! {
        <div style="display: flex; flex-direction: column; gap: 0.25rem; padding: 0.4rem 0;">
            <label style="font-size: 0.8rem; color: #374151;">
                {move || crate::i18n::t(&label_key)}
            </label>
            {control_view}
            {(!help_key.is_empty()).then(|| {
                let k = help_key.clone();
                view! {
                    <small style="color: #6b7280;">
                        {move || crate::i18n::t(&k)}
                    </small>
                }
            })}
            {validation_block}
        </div>
    }
}
