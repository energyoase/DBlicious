//! Server-seitige Skript-Sprachen-Integration (Q0009).
//!
//! Architektur-Garantie (Spec §11): Rhai-Symbole tauchen ausschliesslich im
//! `engine::rhai`-Submodul auf. Andere Module sprechen ueber die Traits in
//! `shared::script::*` (`ScriptEngine`, `HostApi`, `HostApiRegistry`).

pub mod engine;
pub mod host;
pub mod provider_lookup;
pub mod run;
pub mod sandbox;
pub mod save;

/// Server-seitige Auflistung aller Host-Funktionen. Pendant ist
/// `client::script::ClientHostApiRegistry` (Phase 4). Der `symmetry_check`-
/// Default auf `HostApiRegistry` (in `shared`) vergleicht beide Listen
/// laufzeitig — Eintraege mit `server_only=true` werden dabei nicht auf der
/// Client-Seite erwartet (`audit.log`, `db.patch`).
pub struct ServerHostApiRegistry;

impl shared::script::HostApiRegistry for ServerHostApiRegistry {
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
