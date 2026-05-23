//! `HostApiRegistry` — Compile-Time-Sicherung der Server/Client-Symmetrie.
//!
//! Jeder Konsument (server, client) implementiert den Trait und listet seine
//! Funktionen. Der `symmetry_check()`-Default vergleicht beide Listen
//! laufzeitig in Test-Runs (siehe `server/tests/script_engine.rs` und
//! `client/tests/script_engine.rs`).

use crate::script::capability::CapabilityToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFunctionDescriptor {
    pub name: &'static str,
    pub token: CapabilityToken,
    pub server_only: bool,
}

/// Implementiert auf Server- *und* Client-Seite mit derselben Funktionsliste.
pub trait HostApiRegistry {
    fn functions() -> Vec<HostFunctionDescriptor>;

    /// Nur in Tests aufrufen: vergleicht zwei Listen.
    fn symmetry_check(
        server: &[HostFunctionDescriptor],
        client: &[HostFunctionDescriptor],
    ) -> Vec<String> {
        let mut errors = Vec::new();
        for s in server {
            if s.server_only {
                continue;
            }
            let matched = client.iter().find(|c| c.name == s.name);
            match matched {
                None => errors.push(format!("client missing function: {}", s.name)),
                Some(c) if c.token != s.token => {
                    errors.push(format!(
                        "token mismatch on '{}': server={:?}, client={:?}",
                        s.name, s.token, c.token
                    ));
                }
                _ => {}
            }
        }
        for c in client {
            if !server.iter().any(|s| s.name == c.name) {
                errors.push(format!(
                    "server missing function declared on client: {}",
                    c.name
                ));
            }
        }
        errors
    }
}
