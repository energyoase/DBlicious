//! Formatter-Registry (Phase 1.5.4).
//!
//! Read-Only-Zellen-Renderer. Heute wird der Default-Pfad ueber
//! `components::field::DefaultFieldRegistry` abgewickelt — die hier
//! definierten Factories sind ID-basiert und werden vom
//! `ImplementationPicker` und kuenftigen Registry-getriebenen Renderern
//! konsumiert.

use std::collections::BTreeMap;
use std::sync::Arc;

use leptos::prelude::*;
use serde_json::Value;
use shared::ColumnMeta;

use crate::i18n::format;
use crate::i18n::I18nContext;

#[derive(Clone)]
pub struct FormatterContext {
    pub column: ColumnMeta,
    pub value: Value,
    /// Gesamte `Entity.fields`-Map — relevant fuer kreuzfeldabhaengige
    /// Formatter wie `money` (Currency-Lookup auf anderem Feld).
    pub fields: serde_json::Map<String, Value>,
}

pub type FormatterFactory = Arc<dyn Fn(FormatterContext) -> AnyView + Send + Sync>;

#[derive(Clone, Default)]
pub struct FormatterRegistry {
    entries: BTreeMap<String, FormatterFactory>,
}

impl FormatterRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, id: impl Into<String>, factory: FormatterFactory) {
        self.entries.insert(id.into(), factory);
    }
    pub fn get(&self, id: &str) -> Option<&FormatterFactory> {
        self.entries.get(id)
    }
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

/// Standard-Formatter-IDs:
/// - `text`           — Wert als String.
/// - `integer`        — i18n-formatierter Integer.
/// - `decimal-2`      — 2 Dezimalstellen.
/// - `boolean`        — Yes/No via `value.bool.*`.
/// - `date`           — i18n-Datum.
/// - `datetime`       — i18n-Datum+Zeit.
/// - `money`          — i18n-Geld mit Currency aus `money.currency_code_field`.
pub fn default_formatter_registry() -> FormatterRegistry {
    let mut r = FormatterRegistry::new();
    r.register("text", Arc::new(render_text));
    r.register("integer", Arc::new(render_integer));
    r.register("decimal-2", Arc::new(|ctx| render_decimal(ctx, 2)));
    r.register("boolean", Arc::new(render_boolean));
    r.register("date", Arc::new(render_date));
    r.register("datetime", Arc::new(render_datetime));
    r.register("money", Arc::new(render_money));
    r
}

fn locale() -> crate::i18n::Locale {
    I18nContext::use_context().locale.get()
}

fn render_text(ctx: FormatterContext) -> AnyView {
    let s = ctx.value.as_str().unwrap_or("").to_string();
    view! { <span>{s}</span> }.into_any()
}

fn render_integer(ctx: FormatterContext) -> AnyView {
    let v = ctx.value.as_i64().unwrap_or(0);
    let l = locale();
    view! { <span>{format::integer(v, l)}</span> }.into_any()
}

fn render_decimal(ctx: FormatterContext, precision: u8) -> AnyView {
    let v = ctx.value.as_f64().unwrap_or(0.0);
    let l = locale();
    view! { <span>{format::decimal(v, precision, l)}</span> }.into_any()
}

fn render_boolean(ctx: FormatterContext) -> AnyView {
    let b = ctx.value.as_bool().unwrap_or(false);
    let key = if b {
        "value.bool.true"
    } else {
        "value.bool.false"
    };
    view! { <span>{crate::i18n::t(key)}</span> }.into_any()
}

fn render_date(ctx: FormatterContext) -> AnyView {
    let s = ctx.value.as_str().unwrap_or("").to_string();
    let l = locale();
    view! { <span>{format::date(&s, l)}</span> }.into_any()
}

fn render_datetime(ctx: FormatterContext) -> AnyView {
    let s = ctx.value.as_str().unwrap_or("").to_string();
    let l = locale();
    view! { <span>{format::datetime(&s, l)}</span> }.into_any()
}

fn render_money(ctx: FormatterContext) -> AnyView {
    let amount = ctx.value.as_f64().unwrap_or(0.0);
    let currency = match &ctx.column.field_type {
        shared::FieldType::Money {
            currency_code_field: Some(cf),
        } => ctx
            .fields
            .get(cf)
            .and_then(|v| v.as_str())
            .unwrap_or("EUR")
            .to_string(),
        _ => "EUR".to_string(),
    };
    let l = locale();
    view! { <span>{format::money(amount, &currency, l)}</span> }.into_any()
}
