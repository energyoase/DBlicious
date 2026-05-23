//! Implementations-Registries (Phase 1.5.4).
//!
//! Pendant zur [`super::table::filters::FilterRegistry`], aber fuer andere
//! austauschbare Komponenten:
//!   - [`editor::EditorRegistry`]    — Edit-Controls pro `ColumnMeta`.
//!   - [`formatter::FormatterRegistry`] — Zellen-Render pro `ColumnMeta`.
//!   - [`action::ActionRegistry`]    — Row-Aktionen pro `ColumnMeta`.
//!
//! Jede Registry ist ein String-ID → Factory-Map. Aufloesung pro Spalte
//! laeuft ueber die [`resolve`]-Helper (Resolution-Kette analog Server:
//! ColumnMeta.X_id → EntitySettings.field_type_defaults → Client-Fallback).
//!
//! Diese Registries sind heute opt-in — der heutige `FieldCell` und
//! `FieldEditor` aus `components::field` rendern direkt ohne Registry-
//! Lookup. Die Registries werden ab Phase 1.5.5 vom
//! `ImplementationPicker` konsumiert; spaetere Iterationen koennen den
//! Field-Render-Pfad auf Registry-basierte Resolution umstellen.

pub mod action;
pub mod editor;
pub mod formatter;
pub mod picker;
pub mod resolve;

pub use action::{default_action_registry, ActionContext, ActionFactory, ActionRegistry};
pub use editor::{default_editor_registry, EditorContext, EditorFactory, EditorRegistry};
pub use formatter::{
    default_formatter_registry, FormatterContext, FormatterFactory, FormatterRegistry,
};
pub use picker::ImplementationPicker;
pub use resolve::resolve_implementation_id;
