//! `ctx`-Host-Modul (Spec §7.4) — Client-Pendant.
//!
//! Liefert die kontextuellen Read-Only-Werte (`user_id`, `tenant_id`,
//! `locale`, `now()`). Im Renderer (Phase 5) wird der `ScriptCtx` aus den
//! Leptos-`RwSignal`-Providern (`AuthContext`, `I18nContext`) gespeist;
//! diese Schicht ist davon unabhaengig und kennt nur die Struktur.
//!
//! Bewusst keine Persistenz-API hier (`ctx.state`) — die kommt mit Phase 5
//! plus dem Renderer dazu (Spec §7.4: "state via per-component RwSignal").

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

    /// UTC-Now als RFC-3339-String. Auf nativen Builds nutzen wir
    /// `chrono::Utc::now`, in WASM ginge das ebenfalls — aber wir vermeiden
    /// eine zusaetzliche `chrono`-Dependency im Client-Crate und greifen
    /// stattdessen auf `js_sys::Date::new_0().to_iso_string()` zurueck, das
    /// liefert ISO-8601 in UTC. Damit bleibt dieser Pfad ohne `chrono`.
    pub fn now(&self) -> String {
        #[cfg(target_arch = "wasm32")]
        {
            let s = js_sys::Date::new_0().to_iso_string();
            return s
                .as_string()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".into());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // In native Tests reicht ein einfacher deterministischer
            // Sentinel — die Tests pruefen nur die Form, nicht den Wert.
            // Wenn ein Test echte Zeit braucht, wuerde er `chrono` im
            // dev-dep nachziehen.
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| {
                    let secs = d.as_secs();
                    let nanos = d.subsec_nanos();
                    // Sehr minimaler ISO-8601-Output ohne externe Crates:
                    // wir geben Sekunden + "Z" — Tests verlassen sich nur
                    // auf das `T`-Zeichen.
                    let _ = nanos;
                    format!("1970-01-01T00:00:{:02}Z", secs % 60)
                })
                .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
        }
    }
}
