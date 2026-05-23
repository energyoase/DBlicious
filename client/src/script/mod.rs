//! Client-seitige Skript-Sprachen-Integration (Q0009 Phase 4).
//!
//! Spiegelt den Server-Pfad (`server::script`) mit identischen Trait-Impls,
//! anderem Backing-Stack:
//!   - `engine::RhaiEngine` — derselbe `Engine::new_raw()` + `configure_strict`-
//!     Aufbau wie auf dem Server (Spec §11: Rhai-Symbole leben einzig in
//!     `engine::rhai`).
//!   - `sandbox` — gleiche `gate(token, body)`-Logik. Deadline-Pruefung
//!     wechselt zwischen `Instant` (native) und `web_sys::Performance::now()`
//!     (WASM).
//!
//! Submodule:
//!   - `audit_queue` — prozesslokaler Buffer fuer Skript-Audit-Events.
//!   - `data_source` — `DataSource`-Adapter, der einen Provider-Skript-Run
//!     in die generische Tabellen-Datenquelle einspeist.

pub mod audit_queue;
pub mod data_source;
pub mod engine;
pub mod host;
pub mod registry;
pub mod sandbox;

pub use registry::ScriptRegistry;

/// Client-seitige Auflistung aller Host-Funktionen. Pendant ist
/// `server::script::ServerHostApiRegistry`. Der `HostApiRegistry`-
/// Default-`symmetry_check` vergleicht beide Listen in Tests
/// (`server/tests/script_symmetry.rs`).
///
/// Eintraege mit `server_only=true` sind hier deklariert, damit die Symmetry-
/// Pruefung den vollen Namensraum auf beiden Seiten sieht. Die Run-Time-
/// Implementierung im Client lehnt diese Calls mit
/// `ScriptError::ServerOnlyFunction` ab (siehe `host::db::DbHost::patch_entity`
/// und `host::audit::AuditHost::log`).
pub struct ClientHostApiRegistry;

impl shared::script::HostApiRegistry for ClientHostApiRegistry {
    fn functions() -> Vec<shared::script::HostFunctionDescriptor> {
        use shared::script::capability::CapabilityToken::*;
        use shared::script::capability::UiScope;
        use shared::script::HostFunctionDescriptor as F;
        vec![
            F {
                name: "db.entities",
                token: ReadOwnEntities,
                server_only: false,
            },
            F {
                name: "db.entity",
                token: ReadOwnEntities,
                server_only: false,
            },
            F {
                name: "db.patch",
                token: WriteEntity { validated: true },
                server_only: true,
            },
            F {
                name: "ui.vstack",
                token: EmitUiNode {
                    scope: UiScope::Composite,
                },
                server_only: false,
            },
            F {
                name: "ui.hstack",
                token: EmitUiNode {
                    scope: UiScope::Composite,
                },
                server_only: false,
            },
            F {
                name: "ui.text",
                token: EmitUiNode {
                    scope: UiScope::Leaf,
                },
                server_only: false,
            },
            F {
                name: "ui.table",
                token: EmitUiNode {
                    scope: UiScope::Composite,
                },
                server_only: false,
            },
            F {
                name: "ui.chart",
                token: EmitUiNode {
                    scope: UiScope::Composite,
                },
                server_only: false,
            },
            F {
                name: "ctx.t",
                token: ReadI18n,
                server_only: false,
            },
            F {
                name: "audit.log",
                token: WriteAuditLog,
                server_only: true,
            },
        ]
    }
}
