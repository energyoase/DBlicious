//! Abstraktionsschicht ueber das visuelle Design.
//!
//! Komponenten holen sich ein `DesignSystem` ueber `use_design()` und
//! verwenden dessen Methoden, statt CSS-Klassen oder Style-Strings direkt
//! zu schreiben. Damit kann die konkrete Implementierung (aktuell:
//! `InlineDesign` mit CSS-in-Rust) spaeter durch eine Tailwind-Variante
//! oder ein anderes Utility-System ersetzt werden, ohne Komponenten
//! anzupassen.

pub mod inline;
pub mod overrides;
pub mod tokens;

use std::sync::Arc;

use leptos::prelude::*;

pub use inline::InlineDesign;
pub use overrides::{provide_style_overrides, styled, use_style_overrides, StyleOverrides};
pub use tokens::*;

/// Ein einzelner Styling-Output.
///
/// Beide Felder werden parallel unterstuetzt:
///   - `inline` fuer CSS-in-Rust-Implementierungen
///   - `class`  fuer Klassen-basierte Implementierungen (Tailwind, Stylance, ...)
///
/// In Komponenten werden beide Felder auf das HTML-Element gesetzt,
/// das nicht genutzte Feld ist einfach leer.
#[derive(Debug, Clone, Default)]
pub struct Style {
    pub inline: String,
    pub class: String,
}

impl Style {
    pub fn inline(s: impl Into<String>) -> Self {
        Self {
            inline: s.into(),
            class: String::new(),
        }
    }
    pub fn class(s: impl Into<String>) -> Self {
        Self {
            inline: String::new(),
            class: s.into(),
        }
    }
}

/// Zustaende einer interaktiven UI-Komponente.
///
/// Pendant zu `ActionStyle` aus dem C#-Original. Renderer koennen damit
/// pseudo-Element-aehnliche Varianten anbieten, ohne CSS-Pseudoklassen
/// einzusetzen (was im Inline-Modus nicht ginge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionState {
    Default,
    Hover,
    Pressed,
    Focused,
    Disabled,
}

/// Fertige Varianten fuer alle [`ActionState`]s.
///
/// Komponenten waehlen anhand des aktuellen Zustands die richtige
/// `Style`-Instanz. Aufrufer koennen einzelne Felder durch [`Style::default`]
/// belassen, wenn der Renderer keinen distinkten Look fuer den Zustand hat.
#[derive(Debug, Clone, Default)]
pub struct ActionStyle {
    pub default: Style,
    pub hover: Style,
    pub pressed: Style,
    pub focused: Style,
    pub disabled: Style,
}

impl ActionStyle {
    pub fn pick(&self, state: ActionState) -> &Style {
        match state {
            ActionState::Default => &self.default,
            ActionState::Hover => &self.hover,
            ActionState::Pressed => &self.pressed,
            ActionState::Focused => &self.focused,
            ActionState::Disabled => &self.disabled,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SurfaceLevel {
    App,
    Sidebar,
    Card,
    Toolbar,
}

#[derive(Debug, Clone, Copy)]
pub enum TextVariant {
    H1,
    H2,
    Body,
    Caption,
    Muted,
}

#[derive(Debug, Clone, Copy)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
}

/// Vertrag fuer alle Design-System-Implementierungen.
///
/// Die Methoden sind absichtlich grob granular gehalten – sie beschreiben
/// semantische Rollen, keine technischen Details. Eine zukuenftige
/// Tailwind-Implementierung muss exakt dieselben Methoden ausfuellen,
/// kann aber andere Klassen liefern.
pub trait DesignSystem: Send + Sync {
    fn root(&self) -> Style;
    fn surface(&self, level: SurfaceLevel) -> Style;
    fn text(&self, variant: TextVariant) -> Style;
    fn button(&self, variant: ButtonVariant) -> Style;
    /// Aktion-State-aware Button-Variante. Default-Impl reicht den
    /// `state = Default`-Look in alle Slots — Implementierungen koennen
    /// pro Slot variieren.
    fn button_action(&self, variant: ButtonVariant) -> ActionStyle {
        let s = self.button(variant);
        ActionStyle {
            default: s.clone(),
            hover: s.clone(),
            pressed: s.clone(),
            focused: s.clone(),
            disabled: s,
        }
    }
    fn input(&self) -> Style;
    fn nav_item(&self, depth: usize, active: bool) -> Style;
    fn nav_group(&self, depth: usize) -> Style;
    fn table(&self) -> Style;
    /// Wrapper-Element um `<table>`. Begrenzt Hoehe und erzeugt einen
    /// eigenen Scroll-Bereich, damit der horizontale Scrollbalken am
    /// Tabellen-Boden im Viewport sichtbar bleibt.
    fn table_scroll_container(&self) -> Style;
    fn table_header_row(&self) -> Style;
    fn table_header_cell(&self) -> Style;
    fn table_row(&self, even: bool) -> Style;
    fn table_cell(&self) -> Style;
    fn placeholder(&self) -> Style;
    fn pagination_bar(&self) -> Style;
    fn toolbar(&self) -> Style;

    // ---- Designer ----
    /// Aeussere Leinwand des Datenbank-Designers (Hintergrund, Raster, …).
    fn designer_canvas(&self) -> Style;
    /// Karten-Look einer modellierten Tabelle auf der Leinwand.
    fn designer_table(&self, selected: bool) -> Style;
    /// Kopfleiste einer Designer-Tabelle (Drag-Handle).
    fn designer_table_header(&self) -> Style;
    /// Einzelne Spaltenzeile innerhalb einer Designer-Tabelle.
    fn designer_column_row(&self, selected: bool) -> Style;
    /// Kleiner "Port"-Punkt am linken/rechten Rand einer Spalte.
    fn designer_port(&self, active: bool) -> Style;
    /// Status-Banner unterhalb des Save-Buttons.
    fn designer_status(&self, ok: bool) -> Style;
}

/// Geteiltes Handle auf die aktuelle Implementierung.
#[derive(Clone)]
pub struct DesignHandle(pub Arc<dyn DesignSystem>);

impl DesignHandle {
    pub fn new<D: DesignSystem + 'static>(d: D) -> Self {
        Self(Arc::new(d))
    }
}

impl std::ops::Deref for DesignHandle {
    type Target = dyn DesignSystem;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

/// Stellt das Standard-Design bereit. Aktuell `InlineDesign`. Spaeter
/// genuegt es, hier eine andere Implementierung zu setzen, um die gesamte
/// App umzustylen.
pub fn provide_design_system() {
    provide_context(DesignHandle::new(InlineDesign::default()));
}

pub fn use_design() -> DesignHandle {
    use_context::<DesignHandle>()
        .expect("Kein DesignSystem im Context (provide_design_system fehlt?)")
}
