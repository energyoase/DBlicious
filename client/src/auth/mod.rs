//! Client-seitiger Auth-State.
//!
//! Pendant zum `SecurityUser`-Anwendungsfall auf Server-Seite. Eine Instanz
//! lebt als Leptos-Context und enthaelt:
//!   - aktuelles Bearer-Token (oder `None`),
//!   - aktuellen `SecurityUser` (oder `None`),
//!   - effektive Permissions,
//!   - LocalStorage-Persistenz (Token + minimaler User-Snapshot).
//!
//! Re-Render-Verhalten: alle Felder sind `RwSignal`, also reagieren
//! Komponenten automatisch auf Login/Logout.

use leptos::prelude::*;
use shared::{AuthSession, Permission, PermissionOp, SecurityUser};

use crate::graphql::set_auth_token;

const LS_TOKEN_KEY: &str = "dblicious.auth.token";
const LS_USER_KEY: &str = "dblicious.auth.user";
const LS_PERMS_KEY: &str = "dblicious.auth.perms";

#[derive(Clone, Copy)]
pub struct AuthContext {
    pub token: RwSignal<Option<String>>,
    pub user: RwSignal<Option<SecurityUser>>,
    pub permissions: RwSignal<Vec<Permission>>,
}

impl AuthContext {
    pub fn provide() -> Self {
        let ctx = Self {
            token: RwSignal::new(None),
            user: RwSignal::new(None),
            permissions: RwSignal::new(Vec::new()),
        };
        provide_context(ctx);
        ctx.hydrate_from_storage();
        ctx
    }

    pub fn use_context() -> Self {
        use_context::<Self>().expect("AuthContext nicht im Context (provide fehlt?)")
    }

    /// Schreibt den Auth-State aus LocalStorage zurueck (falls vorhanden).
    fn hydrate_from_storage(&self) {
        let Some(storage) = local_storage() else { return };
        if let Ok(Some(token)) = storage.get_item(LS_TOKEN_KEY) {
            set_auth_token(Some(token.clone()));
            self.token.set(Some(token));
        }
        if let Ok(Some(user_json)) = storage.get_item(LS_USER_KEY) {
            if let Ok(user) = serde_json::from_str::<SecurityUser>(&user_json) {
                self.user.set(Some(user));
            }
        }
        if let Ok(Some(perms_json)) = storage.get_item(LS_PERMS_KEY) {
            if let Ok(perms) = serde_json::from_str::<Vec<Permission>>(&perms_json) {
                self.permissions.set(perms);
            }
        }
    }

    /// Wendet ein erfolgreiches Login-Ergebnis an: setzt In-Memory- und
    /// LocalStorage-Werte, aktualisiert den GraphQL-Client-Header.
    pub fn apply_session(&self, session: AuthSession) {
        set_auth_token(Some(session.token.clone()));
        self.token.set(Some(session.token.clone()));
        self.user.set(Some(session.user.clone()));
        self.permissions.set(session.permissions.clone());

        if let Some(storage) = local_storage() {
            let _ = storage.set_item(LS_TOKEN_KEY, &session.token);
            if let Ok(json) = serde_json::to_string(&session.user) {
                let _ = storage.set_item(LS_USER_KEY, &json);
            }
            if let Ok(json) = serde_json::to_string(&session.permissions) {
                let _ = storage.set_item(LS_PERMS_KEY, &json);
            }
        }
    }

    pub fn clear(&self) {
        set_auth_token(None);
        self.token.set(None);
        self.user.set(None);
        self.permissions.set(Vec::new());
        if let Some(storage) = local_storage() {
            let _ = storage.remove_item(LS_TOKEN_KEY);
            let _ = storage.remove_item(LS_USER_KEY);
            let _ = storage.remove_item(LS_PERMS_KEY);
        }
    }

    /// Pruefe, ob der aktuelle User eine Operation auf einem Entity-Typ darf.
    /// Liest reaktiv aus `permissions`.
    pub fn is_allowed(&self, entity_type: &str, op: PermissionOp) -> bool {
        self.permissions.with(|perms| {
            perms.iter().filter(|p| p.matches(entity_type)).any(|p| match op {
                PermissionOp::Read => p.can_read,
                PermissionOp::Create => p.can_create,
                PermissionOp::Update => p.can_update,
                PermissionOp::Delete => p.can_delete,
            })
        })
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.with(|t| t.is_some())
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
