//! Header-Registry und Debounce-Queue fuer Dirty-Tracking.
//!
//! Pendant zu `JezitLibraryShared/EntityProvider/Header/`. Zwei
//! orthogonale Bausteine:
//!
//!   - [`HeaderRegistry`] haelt pro `(entity_type, id)` einen
//!     [`shared::EntityHeader`] und ermoeglicht "ist dirty?" / "baseline
//!     setzen" / "Hash auffrischen". Wird als Leptos-Context bereitgestellt.
//!
//!   - [`DebounceQueue`] verzoegert einen Aufruf um N ms, faltet schnelle
//!     Folgeaufrufe auf einen einzigen zusammen. Beispiel: bei jedem
//!     Tastendruck Header neu hashen — aber nicht oefter als alle 200 ms.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use shared::{compute_hash, Entity, EntityHeader, EntityLoadState};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

// =============================================================================
// HeaderRegistry
// =============================================================================

type Key = (String, String); // (entity_type, id)

#[derive(Default)]
pub struct HeaderRegistry {
    headers: HashMap<Key, EntityHeader>,
}

impl HeaderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_loaded(&mut self, entity_type: &str, entity: &Entity) {
        let key = (entity_type.to_string(), entity.id.clone());
        self.headers
            .insert(key, EntityHeader::new_loaded(entity_type, entity));
    }

    pub fn mark_loading(&mut self, entity_type: &str, id: &str) {
        let key = (entity_type.to_string(), id.to_string());
        self.headers
            .entry(key.clone())
            .and_modify(|h| h.load_state = EntityLoadState::Loading)
            .or_insert(EntityHeader {
                id: id.to_string(),
                entity_type: entity_type.to_string(),
                hash: 0,
                original_hash: 0,
                load_state: EntityLoadState::Loading,
                display: None,
            });
    }

    pub fn mark_failed(&mut self, entity_type: &str, id: &str) {
        let key = (entity_type.to_string(), id.to_string());
        self.headers
            .entry(key.clone())
            .and_modify(|h| h.load_state = EntityLoadState::Failed);
    }

    /// Aktualisiert den Live-Hash. Erhaeltlich, wenn ein Editor Felder
    /// veraendert hat.
    pub fn touch(&mut self, entity_type: &str, entity: &Entity) {
        let key = (entity_type.to_string(), entity.id.clone());
        if let Some(h) = self.headers.get_mut(&key) {
            h.hash = compute_hash(entity);
        } else {
            self.upsert_loaded(entity_type, entity);
        }
    }

    /// Setzt den aktuellen Stand als neue Baseline (nach erfolgreichem Save).
    pub fn baseline(&mut self, entity_type: &str, id: &str) {
        let key = (entity_type.to_string(), id.to_string());
        if let Some(h) = self.headers.get_mut(&key) {
            h.baseline();
        }
    }

    pub fn get(&self, entity_type: &str, id: &str) -> Option<&EntityHeader> {
        self.headers
            .get(&(entity_type.to_string(), id.to_string()))
    }

    pub fn is_dirty(&self, entity_type: &str, id: &str) -> bool {
        self.get(entity_type, id).is_some_and(|h| h.is_dirty())
    }
}

#[derive(Clone)]
pub struct HeaderRegistryHandle(pub Arc<Mutex<HeaderRegistry>>);

impl HeaderRegistryHandle {
    pub fn with<R>(&self, f: impl FnOnce(&HeaderRegistry) -> R) -> R {
        f(&self.0.lock().expect("HeaderRegistry mutex poisoned"))
    }

    pub fn update(&self, f: impl FnOnce(&mut HeaderRegistry)) {
        f(&mut self.0.lock().expect("HeaderRegistry mutex poisoned"));
    }
}

pub fn provide_header_registry() {
    provide_context(HeaderRegistryHandle(Arc::new(Mutex::new(
        HeaderRegistry::new(),
    ))));
}

pub fn use_header_registry() -> HeaderRegistryHandle {
    use_context::<HeaderRegistryHandle>()
        .expect("Keine HeaderRegistry im Context (provide_header_registry fehlt?)")
}

// =============================================================================
// DebounceQueue
// =============================================================================
//
// Eine pro-Schluessel-Debounce-Queue. Identische Schluessel verlaengern das
// Time-out, der zuletzt eingereihte Callback gewinnt.

/// Pendant zu `HeaderBackgroundDebounceQueue` aus der C#-Vorlage.
///
/// Speichert pro Schluessel ein laufendes `setTimeout`-Handle und den letzten
/// Callback. Beim erneuten `enqueue` mit demselben Schluessel wird das alte
/// Handle abgebrochen und ein neues mit der vollen Verzoegerung gestartet.
#[derive(Default)]
pub struct DebounceQueue {
    pending: HashMap<String, i32>,
    // Halte die aktiven Closures am Leben, damit sie nicht vor dem Trigger
    // freigegeben werden.
    keepalive: HashMap<String, Closure<dyn FnMut()>>,
}

impl DebounceQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Plant `callback` zur Ausfuehrung in `delay_ms`. Cancelt einen
    /// vorherigen Eintrag mit demselben `key`.
    pub fn enqueue(
        registry: Rc<RefCell<Self>>,
        key: impl Into<String>,
        delay_ms: i32,
        callback: impl FnOnce() + 'static,
    ) {
        let key = key.into();

        // Vorherigen Eintrag canceln.
        if let Some(handle) = registry.borrow_mut().pending.remove(&key) {
            if let Some(window) = web_sys::window() {
                window.clear_timeout_with_handle(handle);
            }
        }
        registry.borrow_mut().keepalive.remove(&key);

        let registry_for_cb = registry.clone();
        let key_for_cb = key.clone();
        let mut callback_opt = Some(callback);

        let closure = Closure::wrap(Box::new(move || {
            // Eintrag aus Registry ausbuchen, bevor der Callback laeuft.
            {
                let mut r = registry_for_cb.borrow_mut();
                r.pending.remove(&key_for_cb);
                r.keepalive.remove(&key_for_cb);
            }
            if let Some(cb) = callback_opt.take() {
                cb();
            }
        }) as Box<dyn FnMut()>);

        if let Some(window) = web_sys::window() {
            match window.set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                delay_ms,
            ) {
                Ok(handle) => {
                    let mut r = registry.borrow_mut();
                    r.pending.insert(key.clone(), handle);
                    r.keepalive.insert(key, closure);
                }
                Err(_) => {
                    // setTimeout konnte nicht registriert werden – Closure
                    // verfaellt, kein Callback laeuft. Stille Aufgabe.
                }
            }
        }
    }

    /// Cancelt einen ggf. anstehenden Eintrag.
    pub fn cancel(&mut self, key: &str) {
        if let Some(handle) = self.pending.remove(key) {
            if let Some(window) = web_sys::window() {
                window.clear_timeout_with_handle(handle);
            }
        }
        self.keepalive.remove(key);
    }
}

/// `DebounceQueue` enthaelt `wasm_bindgen::Closure`, das `!Send` ist.
/// Leptos verlangt aber `Send + Sync` fuer den Context. `SendWrapper` ist
/// die Standard-Loesung — im WASM-Single-Thread-Modell ist der Wrap-Check
/// nie verletzt.
#[derive(Clone)]
pub struct DebounceQueueHandle(pub SendWrapper<Rc<RefCell<DebounceQueue>>>);

impl DebounceQueueHandle {
    pub fn enqueue(&self, key: impl Into<String>, delay_ms: i32, cb: impl FnOnce() + 'static) {
        DebounceQueue::enqueue((*self.0).clone(), key, delay_ms, cb);
    }

    pub fn cancel(&self, key: &str) {
        self.0.borrow_mut().cancel(key);
    }
}

pub fn provide_debounce_queue() {
    provide_context(DebounceQueueHandle(SendWrapper::new(Rc::new(RefCell::new(
        DebounceQueue::new(),
    )))));
}

pub fn use_debounce_queue() -> DebounceQueueHandle {
    use_context::<DebounceQueueHandle>()
        .expect("Keine DebounceQueue im Context (provide_debounce_queue fehlt?)")
}
