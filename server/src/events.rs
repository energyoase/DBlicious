//! Prozess-weiter Event-Bus fuer Builder-Design-Updates (Phase 1.6).
//!
//! `save_entity_design` veroeffentlicht hier nach erfolgreichem Insert;
//! der `entityDesignUpdated`-Subscription-Resolver konsumiert.
//!
//! Implementation: `tokio::sync::broadcast` — ein Producer, viele Consumer,
//! kein Backpressure (Subscriber lagged ⇒ verpasst Events, kein Crash).

use std::sync::OnceLock;

use tokio::sync::broadcast::{self, Receiver, Sender};

/// Wire-Form eines Design-Update-Events.
#[derive(Debug, Clone, async_graphql::SimpleObject)]
pub struct DesignUpdate {
    pub entity_type: String,
    pub version:     i32,
}

/// Channel-Kapazitaet: solange < N Subscriber langsamer als der Producer
/// sind, geht nichts verloren. 64 reicht fuer Designer-Use-Cases dicke.
const CHANNEL_CAPACITY: usize = 64;

fn slot() -> &'static Sender<DesignUpdate> {
    static TX: OnceLock<Sender<DesignUpdate>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        tx
    })
}

/// Veroeffentlicht ein Event. Fehlt's an Subscribern, wird das Event
/// stillschweigend verworfen — das ist gewollt (Fire-and-Forget).
pub fn publish_design_update(entity_type: &str, version: i32) {
    let _ = slot().send(DesignUpdate {
        entity_type: entity_type.to_string(),
        version,
    });
}

/// Abonniere den Event-Stream. Subscription-Resolver liest hieraus.
pub fn subscribe_design_updates() -> Receiver<DesignUpdate> {
    slot().subscribe()
}
