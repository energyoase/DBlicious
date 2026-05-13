//! Per-Element-Style-Overrides.
//!
//! Pendant zu `StyleOverwrite` / `ActionStyleOverwrite` aus dem
//! Blazor-Vorbild. Eine Komponente kann an einer `style_id` einen Override
//! registrieren; Konsumenten kombinieren den semantischen Stil aus dem
//! Design-System mit dem registrierten Override (Inline-CSS wird einfach
//! konkateniert, das spaeter genannte gewinnt).
//!
//! Beispiel: irgendwo zentral wird `register("toolbar.search-input",
//! "max-width: 320px;")` aufgerufen; jeder `<input>` mit derselben ID
//! erhaelt zusaetzlich diesen Suffix.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use leptos::prelude::*;

use super::Style;

#[derive(Default)]
pub struct StyleOverrides {
    inline: HashMap<String, String>,
    class: HashMap<String, String>,
}

impl StyleOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_inline(&mut self, style_id: impl Into<String>, inline: impl Into<String>) {
        self.inline.insert(style_id.into(), inline.into());
    }

    pub fn set_class(&mut self, style_id: impl Into<String>, class: impl Into<String>) {
        self.class.insert(style_id.into(), class.into());
    }

    pub fn clear(&mut self, style_id: &str) {
        self.inline.remove(style_id);
        self.class.remove(style_id);
    }

    /// Liefert den `base`-Stil mit registriertem Override gemerged.
    pub fn apply(&self, style_id: &str, base: Style) -> Style {
        let mut out = base;
        if let Some(extra_inline) = self.inline.get(style_id) {
            if out.inline.is_empty() {
                out.inline = extra_inline.clone();
            } else {
                // Inline-CSS wird konkateniert; spaetere Eigenschaften
                // gewinnen — der Browser nimmt den letzten Wert pro Property.
                out.inline.push(' ');
                out.inline.push_str(extra_inline);
            }
        }
        if let Some(extra_class) = self.class.get(style_id) {
            if out.class.is_empty() {
                out.class = extra_class.clone();
            } else {
                out.class.push(' ');
                out.class.push_str(extra_class);
            }
        }
        out
    }
}

#[derive(Clone)]
pub struct StyleOverridesHandle(pub Arc<Mutex<StyleOverrides>>);

impl StyleOverridesHandle {
    pub fn update(&self, f: impl FnOnce(&mut StyleOverrides)) {
        f(&mut self.0.lock().expect("StyleOverrides mutex poisoned"));
    }

    pub fn apply(&self, style_id: &str, base: Style) -> Style {
        self.0
            .lock()
            .expect("StyleOverrides mutex poisoned")
            .apply(style_id, base)
    }
}

pub fn provide_style_overrides() {
    provide_context(StyleOverridesHandle(Arc::new(Mutex::new(
        StyleOverrides::new(),
    ))));
}

pub fn use_style_overrides() -> StyleOverridesHandle {
    use_context::<StyleOverridesHandle>()
        .expect("Keine StyleOverrides im Context (provide_style_overrides fehlt?)")
}

/// Bequemer Helper: kombiniert eine semantische `Style`-Quelle mit dem ggf.
/// registrierten Override unter `style_id`. Wenn kein Override existiert,
/// kommt `base` unveraendert zurueck — kein Performance-Overhead.
///
/// Beispiel:
/// ```ignore
/// let s = styled("toolbar.search-input", design.input());
/// view! { <input style=s.inline class=s.class /> }
/// ```
pub fn styled(style_id: &str, base: Style) -> Style {
    use_style_overrides().apply(style_id, base)
}
