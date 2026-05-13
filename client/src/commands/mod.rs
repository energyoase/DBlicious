//! Command-Registry fuer [`shared::MenuAction::Code`].
//!
//! Pendant zu `MenuCodeAction` aus der C#-Vorlage. Anders als dort
//! (Reflection: ein Type-Name pro Aktion) wird hier eine Map
//! `command_id → Handler` als Leptos-Context bereitgestellt. Jeder Handler
//! ist `Fn(&Map<String, Value>)` — keine Asynchronitaet, keine
//! Rueckgabewerte: Aktionen, die GraphQL-Aufrufe brauchen, spawnen
//! intern eine `LocalResource`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use leptos::prelude::*;

pub type CommandFn = Arc<dyn Fn(&serde_json::Map<String, serde_json::Value>) + Send + Sync>;

#[derive(Default)]
pub struct CommandRegistry {
    commands: HashMap<String, CommandFn>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, id: impl Into<String>, handler: F)
    where
        F: Fn(&serde_json::Map<String, serde_json::Value>) + Send + Sync + 'static,
    {
        self.commands.insert(id.into(), Arc::new(handler));
    }

    pub fn dispatch(
        &self,
        id: &str,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> bool {
        if let Some(handler) = self.commands.get(id) {
            handler(args);
            true
        } else {
            log::warn!("Kein Handler fuer Command '{id}' registriert");
            false
        }
    }
}

#[derive(Clone)]
pub struct CommandRegistryHandle(pub Arc<Mutex<CommandRegistry>>);

impl CommandRegistryHandle {
    pub fn update(&self, f: impl FnOnce(&mut CommandRegistry)) {
        f(&mut self.0.lock().expect("CommandRegistry mutex poisoned"));
    }

    pub fn dispatch(
        &self,
        id: &str,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> bool {
        self.0
            .lock()
            .expect("CommandRegistry mutex poisoned")
            .dispatch(id, args)
    }
}

pub fn provide_command_registry() {
    provide_context(CommandRegistryHandle(Arc::new(Mutex::new(
        CommandRegistry::new(),
    ))));
}

pub fn use_command_registry() -> CommandRegistryHandle {
    use_context::<CommandRegistryHandle>()
        .expect("Keine CommandRegistry im Context (provide_command_registry fehlt?)")
}
