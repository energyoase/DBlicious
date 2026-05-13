//! Editor-Registry (Phase 1.5.4).
//!
//! Stellt Edit-Controls fuer Spalten bereit. Eine Factory bekommt den
//! aktuellen `value` plus einen `on_change`-Callback und rendert ein
//! beliebiges Control.

use std::collections::BTreeMap;
use std::sync::Arc;

use leptos::prelude::*;
use serde_json::Value;
use shared::ColumnMeta;

/// Kontext fuer eine Editor-Factory.
#[derive(Clone)]
pub struct EditorContext {
    pub column: ColumnMeta,
    pub value: Value,
    pub on_change: Callback<Value>,
}

/// Builder fuer einen Editor.
pub type EditorFactory = Arc<dyn Fn(EditorContext) -> AnyView + Send + Sync>;

#[derive(Clone, Default)]
pub struct EditorRegistry {
    entries: BTreeMap<String, EditorFactory>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, id: impl Into<String>, factory: EditorFactory) {
        self.entries.insert(id.into(), factory);
    }

    pub fn get(&self, id: &str) -> Option<&EditorFactory> {
        self.entries.get(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

// =============================================================================
// Default-Editoren
// =============================================================================

/// Liefert die eingebauten Editor-Implementierungen unter Standard-IDs:
/// - `text-input`        — `<input type="text">`
/// - `textarea`          — `<textarea>`
/// - `number-input`      — `<input type="number">`
/// - `checkbox`          — `<input type="checkbox">`
/// - `date-picker`       — `<input type="date">`
pub fn default_editor_registry() -> EditorRegistry {
    let mut r = EditorRegistry::new();
    r.register("text-input", Arc::new(render_text_input));
    r.register("textarea", Arc::new(render_textarea));
    r.register("number-input", Arc::new(render_number_input));
    r.register("checkbox", Arc::new(render_checkbox));
    r.register("date-picker", Arc::new(render_date_picker));
    r
}

fn render_text_input(ctx: EditorContext) -> AnyView {
    let v = ctx.value.as_str().unwrap_or("").to_string();
    let cb = ctx.on_change;
    view! {
        <input
            type="text"
            prop:value=v
            on:input=move |ev| cb.run(Value::String(event_target_value(&ev)))
        />
    }
    .into_any()
}

fn render_textarea(ctx: EditorContext) -> AnyView {
    let v = ctx.value.as_str().unwrap_or("").to_string();
    let cb = ctx.on_change;
    view! {
        <textarea
            on:input=move |ev| cb.run(Value::String(event_target_value(&ev)))
        >{v}</textarea>
    }
    .into_any()
}

fn render_number_input(ctx: EditorContext) -> AnyView {
    let v = ctx.value.as_f64().map(|n| n.to_string()).unwrap_or_default();
    let cb = ctx.on_change;
    view! {
        <input
            type="number"
            prop:value=v
            on:input=move |ev| {
                let raw = event_target_value(&ev);
                let parsed = raw
                    .parse::<f64>()
                    .ok()
                    .and_then(|n| serde_json::Number::from_f64(n).map(Value::Number))
                    .unwrap_or(Value::Null);
                cb.run(parsed);
            }
        />
    }
    .into_any()
}

fn render_checkbox(ctx: EditorContext) -> AnyView {
    let v = ctx.value.as_bool().unwrap_or(false);
    let cb = ctx.on_change;
    view! {
        <input
            type="checkbox"
            prop:checked=v
            on:change=move |ev| cb.run(Value::Bool(event_target_checked(&ev)))
        />
    }
    .into_any()
}

fn render_date_picker(ctx: EditorContext) -> AnyView {
    let v = ctx.value.as_str().unwrap_or("").to_string();
    let cb = ctx.on_change;
    view! {
        <input
            type="date"
            prop:value=v
            on:change=move |ev| cb.run(Value::String(event_target_value(&ev)))
        />
    }
    .into_any()
}
