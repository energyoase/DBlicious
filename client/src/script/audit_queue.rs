//! Prozesslokale Queue fuer Skript-Audit-Events, die der Renderer-Host
//! buffert. Der Heartbeat in Phase 6 entlaedt die Queue ueber eine
//! GraphQL-Mutation.
//!
//! Bewusst minimalistisch: ein `Mutex<Vec<...>>` reicht — der Browser ist
//! single-thread, der Mutex deckt nur den Korrektheitsfall im native-Test-
//! Build ab (wo der Test mehrere Threads parallel laufen lassen koennte).
//!
//! Die Queue ist nicht persistiert: bei Page-Reload sind nicht-geflushten
//! Events weg. Das ist akzeptabel — der Server fuehrt sein eigenes Audit-
//! Log (Phase 3) und ist immer noch die Wahrheits-Quelle.

use std::sync::{Mutex, OnceLock};

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub event: String,
    pub payload: Value,
}

fn queue() -> &'static Mutex<Vec<AuditEvent>> {
    static Q: OnceLock<Mutex<Vec<AuditEvent>>> = OnceLock::new();
    Q.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn push(event: impl Into<String>, payload: Value) {
    let mut q = queue().lock().unwrap();
    q.push(AuditEvent {
        event: event.into(),
        payload,
    });
}

/// Drain — wird vom Phase-6-Heartbeat genutzt; bis dahin nur Tests.
pub fn drain() -> Vec<AuditEvent> {
    let mut q = queue().lock().unwrap();
    std::mem::take(&mut *q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_then_drain_returns_in_order_and_empties_queue() {
        // Isolation: ein vorher angesammelter Stand kann von anderen Tests
        // stammen — wir drainen vorher, damit der Test deterministisch ist.
        let _ = drain();
        push("a", serde_json::json!({"n": 1}));
        push("b", serde_json::json!({"n": 2}));
        let v = drain();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].event, "a");
        assert_eq!(v[1].event, "b");
        // Nach drain leer:
        assert!(drain().is_empty());
    }
}
