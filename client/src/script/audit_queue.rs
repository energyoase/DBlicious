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

// ---------------------------------------------------------------------------
// Fallback-Reasons (Phase 5.2)
// ---------------------------------------------------------------------------

/// Warum musste die Provider-Lookup-Schicht auf den statischen Default
/// zurueckfallen? Camel-Case-Wire-Form spiegelt die Server-Variante in
/// `script_audit_log.outcome` (Spec §6.1). Die Event-Form auf der Queue
/// ist `script.provider.fallback` mit dem `FallbackReason` als JSON-Payload.
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackReason {
    /// Skript-ID nicht in der Registry — Cache-Miss, Skript existiert noch
    /// nicht oder wurde geloescht.
    Missing { script_id: String },
    /// Skript existiert, ist aber nicht im State `Active` (Draft, Locked).
    /// `state` ist der lowercase Wire-Name (`"draft"`, `"locked"`).
    NotActive { script_id: String, state: String },
    /// Slot-Mismatch — Skript ist z.B. ein Validator, wurde aber als
    /// Formatter referenziert. `expected` ist der camelCase-Slot-Name.
    SlotMismatch { script_id: String, expected: String },
    /// Compile-Fehler beim Aufsetzen der AST.
    CompileFailed { script_id: String, error: String },
    /// Run-Fehler (Capability-Denied, Timeout, Host-Error usw.).
    RuntimeError { script_id: String, error: String },
}

impl FallbackReason {
    pub fn event_name(&self) -> &'static str {
        match self {
            FallbackReason::Missing { .. } => "script.provider.fallback.missing",
            FallbackReason::NotActive { .. } => "script.provider.fallback.notActive",
            FallbackReason::SlotMismatch { .. } => "script.provider.fallback.slotMismatch",
            FallbackReason::CompileFailed { .. } => "script.provider.fallback.compileFailed",
            FallbackReason::RuntimeError { .. } => "script.provider.fallback.runtimeError",
        }
    }

    pub fn to_payload(&self) -> Value {
        match self {
            FallbackReason::Missing { script_id } => serde_json::json!({"scriptId": script_id}),
            FallbackReason::NotActive { script_id, state } => {
                serde_json::json!({"scriptId": script_id, "state": state})
            }
            FallbackReason::SlotMismatch { script_id, expected } => {
                serde_json::json!({"scriptId": script_id, "expected": expected})
            }
            FallbackReason::CompileFailed { script_id, error } => {
                serde_json::json!({"scriptId": script_id, "error": error})
            }
            FallbackReason::RuntimeError { script_id, error } => {
                serde_json::json!({"scriptId": script_id, "error": error})
            }
        }
    }
}

/// Bequemer Wrapper, der einen Fallback in die Queue legt.
pub fn push_fallback(reason: FallbackReason) {
    let event = reason.event_name();
    let payload = reason.to_payload();
    push(event, payload);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialisiert audit_queue-Tests gegen die prozessweite Queue. Andere
    /// Module (`provider_lookup`) pushen ebenfalls Events, wenn ihre Tests
    /// nebenher laufen — die Serialisierung verhindert Drain-Race-Conditions.
    fn test_lock() -> &'static Mutex<()> {
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        L.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn push_then_drain_returns_in_order_and_empties_queue() {
        let _g = test_lock().lock().unwrap();
        // Vor dem Test alles, was andere Tests reingeschoben haben, abraeumen.
        let _ = drain();
        let prefix = "test.aq.push_drain.";
        push(format!("{prefix}a"), serde_json::json!({"n": 1}));
        push(format!("{prefix}b"), serde_json::json!({"n": 2}));
        let drained = drain();
        let mine: Vec<&AuditEvent> = drained
            .iter()
            .filter(|e| e.event.starts_with(prefix))
            .collect();
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].event, format!("{prefix}a"));
        assert_eq!(mine[1].event, format!("{prefix}b"));
    }

    #[test]
    fn push_fallback_appends_event_with_camelcase_payload() {
        let _g = test_lock().lock().unwrap();
        let _ = drain();
        let marker = "test.aq.fallback.xyz";
        push_fallback(FallbackReason::Missing {
            script_id: marker.into(),
        });
        let drained = drain();
        let mine: Vec<&AuditEvent> = drained
            .iter()
            .filter(|e| {
                e.payload
                    .get("scriptId")
                    .and_then(|v| v.as_str())
                    .map(|s| s == marker)
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].event, "script.provider.fallback.missing");
    }
}
