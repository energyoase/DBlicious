//! Sandbox-Schicht (Spec §5.2) — Client-Pendant.
//!
//! Identische Struktur und Semantik wie `server::script::sandbox`, mit
//! einem einzigen Unterschied: das Wall-clock-Timeout. Im Server schliesst
//! `tokio::time::timeout` den Run-Pfad ab — hier laeuft kein async-Runtime,
//! daher pruefen wir die Deadline:
//!   - native (`#[cfg(not(target_arch = "wasm32"))]`) via `std::time::Instant`,
//!   - WASM via `web_sys::window().performance()`.
//!
//! Das deterministische `Engine::set_max_operations`-Limit aus
//! `engine::rhai` deckt den groesseren Teil der Run-Time-Begrenzung — die
//! Deadline ist nur die obere Schranke gegen sehr lange Host-Calls (z.B.
//! GraphQL-Round-Trips). Falls keine Time-Source verfuegbar ist (z.B. WASM
//! ohne `Window`), bleibt das Operation-Limit unsere einzige Schranke; die
//! Deadline degradiert sicher (no-op).

use shared::script::capability::CapabilityToken;
use shared::script::error::ScriptError;
use shared::script::manifest::ScriptManifest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenOutcome {
    Ok,
    Denied,
}

#[derive(Debug, Clone)]
pub struct TokenUse {
    pub token: CapabilityToken,
    pub outcome: TokenOutcome,
}

pub struct Sandbox<'m> {
    manifest: &'m ScriptManifest,
    deadline: Option<Deadline>,
    token_uses: Vec<TokenUse>,
}

impl<'m> Sandbox<'m> {
    pub fn new(manifest: &'m ScriptManifest) -> Self {
        let deadline = manifest.timeout_ms.map(Deadline::from_now);
        Self {
            manifest,
            deadline,
            token_uses: Vec::new(),
        }
    }

    /// Einzige Einlass-Tuer fuer Host-Calls (Spiegel zum Server). Reihenfolge:
    /// erst Manifest-Check (CapabilityDenied), dann Deadline-Check, dann
    /// panic-safe Body.
    pub fn gate<T, F>(&mut self, token: &CapabilityToken, body: F) -> Result<T, ScriptError>
    where
        F: FnOnce() -> Result<T, ScriptError>,
    {
        if !self.manifest.capabilities.contains(token) {
            self.token_uses.push(TokenUse {
                token: token.clone(),
                outcome: TokenOutcome::Denied,
            });
            return Err(ScriptError::CapabilityDenied {
                token: token.clone(),
            });
        }
        if let Some(dl) = &self.deadline {
            if dl.expired() {
                return Err(ScriptError::Timeout {
                    limit_ms: self.manifest.timeout_ms.unwrap_or(0),
                });
            }
        }
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
        match res {
            Ok(Ok(v)) => {
                self.token_uses.push(TokenUse {
                    token: token.clone(),
                    outcome: TokenOutcome::Ok,
                });
                Ok(v)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ScriptError::InternalPanic {
                backtrace: "panic in host call".into(),
            }),
        }
    }

    pub fn token_uses(&self) -> &[TokenUse] {
        &self.token_uses
    }

    /// Public-Convenience fuer Engine-Run-Schritte, die zwischen Iterationen
    /// das Timeout pruefen wollen, ohne einen Host-Call zu wickeln.
    pub fn check_deadline(&self) -> Result<(), ScriptError> {
        if let Some(dl) = &self.deadline {
            if dl.expired() {
                return Err(ScriptError::Timeout {
                    limit_ms: self.manifest.timeout_ms.unwrap_or(0),
                });
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Deadline — plattformabhaengige Time-Source. Public-API ist `from_now` +
// `expired`; die Native- und WASM-Pfade sind hinter `cfg` versteckt.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Deadline {
    repr: DeadlineRepr,
}

#[derive(Debug, Clone)]
enum DeadlineRepr {
    /// Native: konkrete `Instant`-Schranke (Tests + ggf. Tauri-Frontend).
    #[cfg(not(target_arch = "wasm32"))]
    Native(std::time::Instant),
    /// WASM: Performance-now()-Anker in Millisekunden + Limit.
    #[cfg(target_arch = "wasm32")]
    Wasm { start_ms: f64, limit_ms: f64 },
    /// Keine Time-Source verfuegbar — Deadline ist effektiv unbegrenzt.
    /// Der Fall ist bewusst nicht "Fehler": das Operation-Limit der Engine
    /// hat schon ein hartes Limit; ohne `performance` (z.B. spezielle
    /// Worker-Kontexte) lassen wir nur diesen einen Schutz uebrig.
    ///
    /// Nur im WASM-Build konstruierbar; in native-Builds gibt es immer
    /// einen `Instant` (`Native`-Variante).
    #[cfg(target_arch = "wasm32")]
    NoTimeSource,
}

impl Deadline {
    fn from_now(limit_ms: u32) -> Self {
        let repr = build_deadline(limit_ms);
        Self { repr }
    }

    fn expired(&self) -> bool {
        match &self.repr {
            #[cfg(not(target_arch = "wasm32"))]
            DeadlineRepr::Native(dl) => std::time::Instant::now() > *dl,
            #[cfg(target_arch = "wasm32")]
            DeadlineRepr::Wasm { start_ms, limit_ms } => match performance_now() {
                Some(now) => now - start_ms > *limit_ms,
                None => false,
            },
            #[cfg(target_arch = "wasm32")]
            DeadlineRepr::NoTimeSource => false,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_deadline(limit_ms: u32) -> DeadlineRepr {
    let dl = std::time::Instant::now() + std::time::Duration::from_millis(u64::from(limit_ms));
    DeadlineRepr::Native(dl)
}

#[cfg(target_arch = "wasm32")]
fn build_deadline(limit_ms: u32) -> DeadlineRepr {
    match performance_now() {
        Some(start_ms) => DeadlineRepr::Wasm {
            start_ms,
            limit_ms: f64::from(limit_ms),
        },
        None => DeadlineRepr::NoTimeSource,
    }
}

#[cfg(target_arch = "wasm32")]
fn performance_now() -> Option<f64> {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
}
