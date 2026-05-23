//! Client-Registry fuer geladene Skripte (Q0009 Phase 5.1).
//!
//! Haelt die zuletzt vom Server gelieferten `Script`-Records pro `ScriptId`
//! im Speicher. Heutiger Scope (Phase 5):
//!   - In-Memory-Cache fuer Lookup (Formatter/Filter/Provider, Renderer).
//!   - Loader-Hook: einmal pro App-Start einen Snapshot vom Server ziehen
//!     (Phase 6 verdrahtet das gegen GraphQL; Phase 5-Tests stuetzen sich
//!     auf direktes `insert()` ab).
//!
//! Bewusst KEIN AST-Cache: das `RhaiAst` ist `Send+Sync`-grenzwertig (es
//! enthaelt `Arc<rhai::AST>`) und der Renderer kompiliert beim ersten Run
//! pro `Script`-Version neu. Der Performance-Gewinn eines AST-Caches
//! lohnt sich erst, wenn Skripte pro Frame mehrfach evaluiert werden —
//! das tut der Phase-5-Renderer nicht (Re-Run nur bei `RwSignal`-Update).
//!
//! Threading: `Mutex<HashMap>` reicht — der Client laeuft single-threaded
//! im Browser; die Mutex ist nur fuer den Native-Test-Pfad noetig.

use std::collections::HashMap;
use std::sync::Mutex;

use shared::script::{Script, ScriptId};

/// In-Memory-Snapshot der vom Server geladenen Skripte.
#[derive(Debug, Default)]
pub struct ScriptRegistry {
    cache: Mutex<HashMap<ScriptId, Script>>,
}

impl ScriptRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Liefert eine Kopie des Skripts, falls bekannt.
    pub fn get(&self, id: &ScriptId) -> Option<Script> {
        self.cache.lock().ok()?.get(id).cloned()
    }

    /// Setzt/ueberschreibt einen Eintrag. Aufrufer: GraphQL-Loader, Tests.
    pub fn insert(&self, script: Script) {
        if let Ok(mut g) = self.cache.lock() {
            g.insert(script.id.clone(), script);
        }
    }

    /// Loescht einen Eintrag (z.B. nach `state=Locked`-Wechsel).
    pub fn remove(&self, id: &ScriptId) -> Option<Script> {
        self.cache.lock().ok()?.remove(id)
    }

    pub fn len(&self) -> usize {
        self.cache.lock().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iteriert alle bekannten Skript-IDs (Snapshot).
    pub fn ids(&self) -> Vec<ScriptId> {
        self.cache
            .lock()
            .map(|g| g.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Laedt alle Skripte vom Server und befuellt den Cache (Q0009 Phase 6).
    ///
    /// Existierende Eintraege werden ueberschrieben — die letzte Server-
    /// Version ist authoritative. Eintraege, die der Server NICHT mehr
    /// kennt, bleiben heute im Cache: ein expliziter `remove` muss vom
    /// Aufrufer kommen (z.B. nach `state=Locked`-Switch). Phase 7 kann das
    /// auf "Server-Snapshot ist authoritative" verschaerfen.
    ///
    /// `wasm32`-only: das hier ruft ueber `gloo_net::http::Request` — auf
    /// nativen Targets ist `fetch_scripts` nicht verfuegbar.
    #[cfg(target_arch = "wasm32")]
    pub async fn refresh_from_server(
        &self,
        filter: Option<crate::graphql::queries::ScriptFilter>,
    ) -> Result<usize, crate::graphql::GqlError> {
        let scripts = crate::graphql::queries::fetch_scripts(filter).await?;
        let count = scripts.len();
        for s in scripts {
            self.insert(s);
        }
        Ok(count)
    }

    /// Laedt ein einzelnes Skript per ID vom Server und legt es in den
    /// Cache. Liefert `Ok(None)`, wenn der Server kein Skript dieser ID
    /// kennt — der Cache bleibt dann unveraendert.
    #[cfg(target_arch = "wasm32")]
    pub async fn refresh_one(
        &self,
        id: &ScriptId,
    ) -> Result<Option<Script>, crate::graphql::GqlError> {
        let maybe = crate::graphql::queries::fetch_script(&id.0).await?;
        if let Some(s) = maybe.clone() {
            self.insert(s);
        }
        Ok(maybe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::script::model::ProviderSlot;
    use shared::script::{ScriptKind, ScriptManifest, ScriptState};

    fn dummy_script(id: &str) -> Script {
        Script {
            id: ScriptId(id.into()),
            kind: ScriptKind::Provider {
                slot: ProviderSlot::Formatter,
            },
            manifest: ScriptManifest::default(),
            source: "v".into(),
            version: 1,
            state: ScriptState::Active,
            last_error: None,
            created_by: "u-1".into(),
            created_at: "2026-05-23T00:00:00Z".into(),
            updated_at: "2026-05-23T00:00:00Z".into(),
        }
    }

    #[test]
    fn empty_registry_returns_none() {
        let reg = ScriptRegistry::new();
        assert!(reg.get(&ScriptId("nope".into())).is_none());
        assert!(reg.is_empty());
    }

    #[test]
    fn insert_then_get_roundtrips_script() {
        let reg = ScriptRegistry::new();
        reg.insert(dummy_script("formatter-1"));
        let got = reg
            .get(&ScriptId("formatter-1".into()))
            .expect("script back");
        assert_eq!(got.id, ScriptId("formatter-1".into()));
        assert_eq!(got.state, ScriptState::Active);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn insert_overwrites_existing_entry() {
        let reg = ScriptRegistry::new();
        reg.insert(dummy_script("a"));
        let mut updated = dummy_script("a");
        updated.version = 2;
        reg.insert(updated);
        assert_eq!(reg.get(&ScriptId("a".into())).unwrap().version, 2);
        assert_eq!(reg.len(), 1, "kein Duplikat");
    }

    #[test]
    fn remove_clears_entry() {
        let reg = ScriptRegistry::new();
        reg.insert(dummy_script("x"));
        let removed = reg.remove(&ScriptId("x".into())).unwrap();
        assert_eq!(removed.id, ScriptId("x".into()));
        assert!(reg.get(&ScriptId("x".into())).is_none());
    }

    #[test]
    fn ids_snapshot_lists_all_keys() {
        let reg = ScriptRegistry::new();
        reg.insert(dummy_script("a"));
        reg.insert(dummy_script("b"));
        let mut ids: Vec<String> = reg.ids().into_iter().map(|i| i.0).collect();
        ids.sort();
        assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
    }
}
