//! Validierungs-Runtime fuer Editoren.
//!
//! Pendant zu `JezitLibraryShared/Validation/ValidationSystem`. Anders als
//! die C#-Variante (Reflection-getrieben, attributbasiert) ist hier alles
//! explizit:
//!   - Validatoren sind `Fn`-Closures, gehalten in einer Registry.
//!   - Eine Registry-Instanz wird per `provide_context` global zugaenglich
//!     gemacht — analog zu `DesignSystem` und `FieldRegistry`.
//!   - Pro Entity-Typ wird **eine** Liste von Tasks gepflegt; die Auswertung
//!     iteriert sequenziell und sammelt die [`shared::ValidationMessage`]s in
//!     einem [`shared::ValidationResult`].
//!
//! Die eingebauten Standard-Tasks (Required, MinLength, ...) sind als
//! Konstruktoren in [`builtin`] verfuegbar und werden aus
//! [`EditorMeta`] heraus automatisch abgeleitet (`from_editor_meta`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use leptos::prelude::*;
use serde_json::Value;
use shared::{EditorMeta, ValidationMessage, ValidationResult};

/// Ergebnis-Typ eines einzelnen Validators.
pub type TaskFn = Arc<
    dyn Fn(&str, &Value, &serde_json::Map<String, Value>) -> Option<ValidationMessage>
        + Send
        + Sync,
>;

/// Ein Validator, gebunden an eine konkrete Property eines Entity-Typs.
#[derive(Clone)]
pub struct ValidationTask {
    pub target: String,
    pub task: TaskFn,
}

#[derive(Default, Clone)]
pub struct ValidationSystem {
    /// Tasks pro Entity-Typ.
    tasks: HashMap<String, Vec<ValidationTask>>,
}

impl ValidationSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entity_type: &str, task: ValidationTask) {
        self.tasks.entry(entity_type.into()).or_default().push(task);
    }

    /// Bequeme Variante zur Registrierung mehrerer Tasks.
    pub fn extend(&mut self, entity_type: &str, tasks: impl IntoIterator<Item = ValidationTask>) {
        self.tasks
            .entry(entity_type.into())
            .or_default()
            .extend(tasks);
    }

    /// Wertet alle Tasks fuer `entity_type` gegen die uebergebenen `fields` aus.
    pub fn run(
        &self,
        entity_type: &str,
        fields: &serde_json::Map<String, Value>,
    ) -> ValidationResult {
        let mut result = ValidationResult::default();
        let Some(tasks) = self.tasks.get(entity_type) else {
            return result;
        };
        for t in tasks {
            let value = fields.get(&t.target).cloned().unwrap_or(Value::Null);
            if let Some(msg) = (t.task)(&t.target, &value, fields) {
                result.push(msg);
            }
        }
        result
    }

    /// Leitet aus [`EditorMeta::required`] automatisch `required`-Tasks ab.
    /// Damit braucht die UI fuer den einfachsten Fall keine separate
    /// Validator-Registrierung — `EditorMeta` ist Single-Source-of-Truth.
    pub fn import_required_from(&mut self, meta: &EditorMeta) {
        for prop in &meta.properties {
            if prop.required {
                let key = prop.key.clone();
                self.register(
                    &meta.entity_type,
                    ValidationTask {
                        target: prop.key.clone(),
                        task: Arc::new(move |_target, value, _fields| {
                            if is_empty_value(value) {
                                Some(ValidationMessage::error(
                                    key.clone(),
                                    format!("validation.{}", shared::validation::tasks::REQUIRED),
                                ))
                            } else {
                                None
                            }
                        }),
                    },
                );
            }
        }
    }
}

fn is_empty_value(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

// =============================================================================
// Eingebaute Tasks
// =============================================================================

pub mod builtin {
    use super::*;

    /// `required`: Wert darf nicht leer/null sein.
    pub fn required(target: &str) -> ValidationTask {
        let target_owned = target.to_string();
        ValidationTask {
            target: target.into(),
            task: Arc::new(move |_t, value, _fields| {
                if is_empty_value(value) {
                    Some(ValidationMessage::error(
                        target_owned.clone(),
                        format!("validation.{}", shared::validation::tasks::REQUIRED),
                    ))
                } else {
                    None
                }
            }),
        }
    }

    /// `min_length`: nur fuer textartige Werte.
    pub fn min_length(target: &str, min: usize) -> ValidationTask {
        let target_owned = target.to_string();
        ValidationTask {
            target: target.into(),
            task: Arc::new(move |_t, value, _fields| {
                let len = value.as_str().map(|s| s.chars().count()).unwrap_or(0);
                if len < min {
                    Some(
                        ValidationMessage::error(
                            target_owned.clone(),
                            format!("validation.{}", shared::validation::tasks::MIN_LENGTH),
                        )
                        .with_arg("min", min as i64),
                    )
                } else {
                    None
                }
            }),
        }
    }

    /// `number_range`: Zahl muss zwischen `min` und `max` liegen.
    pub fn number_range(target: &str, min: Option<f64>, max: Option<f64>) -> ValidationTask {
        let target_owned = target.to_string();
        ValidationTask {
            target: target.into(),
            task: Arc::new(move |_t, value, _fields| {
                let n = value.as_f64()?;
                let below = min.is_some_and(|lo| n < lo);
                let above = max.is_some_and(|hi| n > hi);
                if below || above {
                    let mut m = ValidationMessage::error(
                        target_owned.clone(),
                        format!("validation.{}", shared::validation::tasks::NUMBER_RANGE),
                    );
                    if let Some(lo) = min {
                        m = m.with_arg("min", lo);
                    }
                    if let Some(hi) = max {
                        m = m.with_arg("max", hi);
                    }
                    Some(m)
                } else {
                    None
                }
            }),
        }
    }
}

// =============================================================================
// Context-Plumbing
// =============================================================================

/// Geteilter Handle auf das ValidationSystem.
///
/// Die Registry ist nicht reaktiv: sie wird beim Setup gefuellt und beim
/// Save-Event abgefragt. `Arc<Mutex<_>>` (statt `Rc<RefCell<_>>`) ist hier
/// notwendig, weil Leptos `provide_context` `Send + Sync` verlangt — auch im
/// single-threaded WASM-Modell.
#[derive(Clone)]
pub struct ValidationSystemHandle(pub Arc<Mutex<ValidationSystem>>);

impl ValidationSystemHandle {
    pub fn with<R>(&self, f: impl FnOnce(&ValidationSystem) -> R) -> R {
        f(&self.0.lock().expect("ValidationSystem mutex poisoned"))
    }

    pub fn update(&self, f: impl FnOnce(&mut ValidationSystem)) {
        f(&mut self.0.lock().expect("ValidationSystem mutex poisoned"));
    }

    pub fn run(
        &self,
        entity_type: &str,
        fields: &serde_json::Map<String, Value>,
    ) -> ValidationResult {
        self.0
            .lock()
            .expect("ValidationSystem mutex poisoned")
            .run(entity_type, fields)
    }
}

pub fn provide_validation_system() {
    provide_context(ValidationSystemHandle(Arc::new(Mutex::new(
        ValidationSystem::new(),
    ))));
}

pub fn use_validation_system() -> ValidationSystemHandle {
    use_context::<ValidationSystemHandle>()
        .expect("Kein ValidationSystem im Context (provide_validation_system fehlt?)")
}
