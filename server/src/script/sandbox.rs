//! Sandbox-Schicht (Spec §5.2).
//!
//! Pro Skript-Run instantiiert: haellt Token-Audit-Buffer, Timeout-Deadline,
//! PanicCatch-Flag. Engine-agnostisch — referenziert nur
//! `shared::script::*`-Typen, niemand importiert hier `rhai::*`.
//!
//! Das `gate(token, body)`-API ist die zentrale Einlass-Kontrolle: jede
//! Host-Funktion ruft `gate(<eigenes-token>, || tatsaechliche_aktion)`. Der
//! Sandbox-Zustand merkt sich, welche Tokens benutzt wurden (`token_uses`),
//! damit der spaeter geschriebene Audit-Log-Eintrag das volle Bild hat.

use std::time::{Duration, Instant};

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
    deadline: Option<Instant>,
    token_uses: Vec<TokenUse>,
}

impl<'m> Sandbox<'m> {
    pub fn new(manifest: &'m ScriptManifest) -> Self {
        let deadline = manifest
            .timeout_ms
            .map(|ms| Instant::now() + Duration::from_millis(ms as u64));
        Self {
            manifest,
            deadline,
            token_uses: Vec::new(),
        }
    }

    /// Einzige Einlass-Tuer fuer Host-Calls. Reihenfolge: erst Manifest-Check
    /// (CapabilityDenied), dann Timeout-Check, dann panic-safe Body.
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
        if let Some(dl) = self.deadline {
            if Instant::now() > dl {
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
        if let Some(dl) = self.deadline {
            if Instant::now() > dl {
                return Err(ScriptError::Timeout {
                    limit_ms: self.manifest.timeout_ms.unwrap_or(0),
                });
            }
        }
        Ok(())
    }
}
