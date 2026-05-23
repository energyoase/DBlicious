//! `ctx`-Host-Modul (Spec §7.4).
//!
//! Liefert die kontextuellen Read-Only-Werte (`user_id`, `tenant_id`,
//! `locale`, `now()`). `state` (persistierter Skript-Slot) kommt in
//! Phase 3, `invoke()` (Cross-Skript-Aufruf) in Phase 5.

use shared::script::engine::ScriptCtx;

pub struct CtxHost {
    pub ctx: ScriptCtx,
}

impl CtxHost {
    pub fn new(ctx: ScriptCtx) -> Self {
        Self { ctx }
    }

    pub fn user_id(&self) -> Option<&str> {
        self.ctx.user_id.as_deref()
    }

    pub fn tenant_id(&self) -> Option<&str> {
        self.ctx.tenant_id.as_deref()
    }

    pub fn locale(&self) -> &str {
        &self.ctx.locale
    }

    /// UTC-Now als RFC-3339-String — der Server-Pfad ist deterministisch
    /// nur ueber den `ScriptCtx` (kein impliziter Test-Mock noetig), siehe
    /// Spec §7.4 fuer den Determinism-Plan in Phase 5.
    pub fn now(&self) -> String {
        chrono::Utc::now().to_rfc3339()
    }
}
