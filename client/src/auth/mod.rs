//! Client-seitiger Auth-State.
//!
//! Pendant zum `SecurityUser`-Anwendungsfall auf Server-Seite. Eine Instanz
//! lebt als Leptos-Context und enthaelt:
//!   - aktuelles Bearer-Token (oder `None`),
//!   - aktuellen `SecurityUser` (oder `None`),
//!   - Legacy-Permissions (`Vec<shared::security::Permission>`),
//!   - projizierte Effective-Permissions (`Vec<shared::auth::EffectivePermission>`),
//!   - LocalStorage-Persistenz fuer alle drei.
//!
//! ## Resolution-Reihenfolge (Phase 0.7.5)
//!
//! Wenn der Server eine Effective-Liste mitschickt (`effective = Some(…)`),
//! gilt sie als **alleinige Wahrheit**: alles, was nicht in der Liste
//! steht, gilt als verweigert. Wenn die Liste `None` ist (Phase 0.7.4 noch
//! nicht ausgeliefert), faellt der Client auf das Legacy-`Permission`-Feld
//! zurueck.
//!
//! Aufrufer sollten neue Stellen bevorzugt mit [`AuthContext::can`] /
//! [`AuthContext::can_entity_type`] gaten — diese APIs respektieren beide
//! Modi. Die alte [`AuthContext::is_allowed`]-API bleibt fuer
//! Bestandscode unveraendert.
//!
//! Re-Render-Verhalten: alle Felder sind `RwSignal`, also reagieren
//! Komponenten automatisch auf Login/Logout.

use leptos::prelude::*;
use shared::auth::{EffectivePermission, Op, Resource};
use shared::{AuthSession, Permission, PermissionOp, SecurityUser};

use crate::graphql::set_auth_token;

const LS_TOKEN_KEY: &str = "dblicious.auth.token";
const LS_USER_KEY: &str = "dblicious.auth.user";
const LS_PERMS_KEY: &str = "dblicious.auth.perms";
const LS_EFFECTIVE_KEY: &str = "dblicious.auth.effective";

#[derive(Clone, Copy)]
pub struct AuthContext {
    pub token: RwSignal<Option<String>>,
    pub user: RwSignal<Option<SecurityUser>>,
    /// Legacy-Permission-Modell. Fallback, wenn der Server keine
    /// projizierte Effective-Liste liefert.
    pub permissions: RwSignal<Vec<Permission>>,
    /// Projizierte Effective-Liste aus dem neuen Auth-Modell. `None`
    /// bedeutet "Server hat noch keine Projektion implementiert" — der
    /// Client greift dann auf `permissions` zurueck.
    pub effective: RwSignal<Option<Vec<EffectivePermission>>>,
}

impl AuthContext {
    pub fn provide() -> Self {
        let ctx = Self {
            token: RwSignal::new(None),
            user: RwSignal::new(None),
            permissions: RwSignal::new(Vec::new()),
            effective: RwSignal::new(None),
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
        let Some(storage) = local_storage() else {
            return;
        };
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
        if let Ok(Some(eff_json)) = storage.get_item(LS_EFFECTIVE_KEY) {
            if let Ok(eff) = serde_json::from_str::<Vec<EffectivePermission>>(&eff_json) {
                self.effective.set(Some(eff));
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
        self.effective.set(session.effective.clone());

        if let Some(storage) = local_storage() {
            let _ = storage.set_item(LS_TOKEN_KEY, &session.token);
            if let Ok(json) = serde_json::to_string(&session.user) {
                let _ = storage.set_item(LS_USER_KEY, &json);
            }
            if let Ok(json) = serde_json::to_string(&session.permissions) {
                let _ = storage.set_item(LS_PERMS_KEY, &json);
            }
            match &session.effective {
                Some(list) => {
                    if let Ok(json) = serde_json::to_string(list) {
                        let _ = storage.set_item(LS_EFFECTIVE_KEY, &json);
                    }
                }
                None => {
                    let _ = storage.remove_item(LS_EFFECTIVE_KEY);
                }
            }
        }
    }

    pub fn clear(&self) {
        set_auth_token(None);
        self.token.set(None);
        self.user.set(None);
        self.permissions.set(Vec::new());
        self.effective.set(None);
        if let Some(storage) = local_storage() {
            let _ = storage.remove_item(LS_TOKEN_KEY);
            let _ = storage.remove_item(LS_USER_KEY);
            let _ = storage.remove_item(LS_PERMS_KEY);
            let _ = storage.remove_item(LS_EFFECTIVE_KEY);
        }
    }

    /// Pruefe, ob der aktuelle User eine Operation auf einem Entity-Typ darf.
    /// Liest reaktiv aus `permissions`.
    ///
    /// **Legacy-API**: respektiert nur das alte CRUD-Flag-Modell. Neue
    /// Aufrufer sollten [`AuthContext::can_entity_type`] benutzen — diese
    /// API respektiert auch die projizierte Effective-Liste, sobald der
    /// Server sie liefert.
    pub fn is_allowed(&self, entity_type: &str, op: PermissionOp) -> bool {
        self.permissions.with(|perms| {
            perms
                .iter()
                .filter(|p| p.matches(entity_type))
                .any(|p| match op {
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

    // =========================================================================
    // Phase 0.7.5: Lookup gegen die projizierte Effective-Liste
    // =========================================================================

    /// Prueft, ob der aktuelle User (`Resource`, `Op`) ausfuehren darf.
    ///
    /// Resolution:
    ///   1. Wenn der Server eine Effective-Liste geliefert hat
    ///      (`effective = Some(_)`), gilt strikte Membership: `true` nur
    ///      wenn ein Eintrag exakt passt. Wildcards werden vom Server
    ///      aufgeloest, der Client macht **keine** Expansion.
    ///   2. Wenn `effective = None`, faellt der Client auf das Legacy-
    ///      Permission-Modell zurueck — soweit die Resource dort
    ///      ueberhaupt darstellbar ist (`Resource::EntityType` ⇒
    ///      [`is_allowed`](Self::is_allowed)); alle anderen Resource-Arten
    ///      gelten im Legacy-Modus als **permissiv** (`true`), weil die
    ///      UI ohne Projektion nichts verlaesslich ausblenden kann.
    pub fn can(&self, resource: &Resource, op: Op) -> bool {
        if let Some(list) = self.effective.with(Option::clone) {
            return list.iter().any(|e| &e.resource == resource && e.op == op);
        }
        legacy_fallback(self, resource, op)
    }

    /// Bequemer Shortcut fuer den haeufigsten Fall.
    pub fn can_entity_type(&self, name: &str, op: Op) -> bool {
        self.can(&Resource::entity_type(name), op)
    }

    /// Pruefe Property-Permission (`product.price` etc.).
    pub fn can_property(&self, entity_type: &str, property: &str, op: Op) -> bool {
        self.can(&Resource::entity_property(entity_type, property), op)
    }

    /// Pruefe, ob eine benannte Aktion (z.B. `"exportCsv"`) ausfuehrbar
    /// ist.
    pub fn can_execute(&self, action_name: &str) -> bool {
        self.can(
            &Resource::Action {
                name: action_name.into(),
            },
            Op::Execute,
        )
    }

    /// Pruefe, ob der User eine konkrete Implementations-ID waehlen darf
    /// (Phase 1.5).
    pub fn can_choose(&self, registry: &str, id: &str) -> bool {
        self.can(
            &Resource::ImplementationId {
                registry: registry.into(),
                id: id.into(),
            },
            Op::Choose,
        )
    }

    /// Pruefe, ob eine Migration-Lifecycle-Op (Phase 3) erlaubt ist.
    pub fn can_migration(&self, migration_id: &str, op: Op) -> bool {
        self.can(
            &Resource::Migration {
                id: migration_id.into(),
            },
            op,
        )
    }

    /// `true`, wenn der Server eine Effective-Liste projiziert hat.
    /// Aufrufer koennen damit zwischen "noch nicht projiziert
    /// (permissiv)" und "projiziert (strikt)" unterscheiden.
    pub fn has_projection(&self) -> bool {
        self.effective.with(Option::is_some)
    }
}

/// Bruecke ins Legacy-Modell, wenn die Effective-Liste fehlt.
///
/// `EntityType`+CRUD-Op laesst sich auf `Permission::can_*` abbilden;
/// alle anderen Resource-Arten gibt es im Legacy-Modell nicht, daher
/// gelten sie als permissiv (`true`). Sobald der Server eine Projektion
/// liefert (Phase 0.7.4), faellt dieser Pfad weg.
fn legacy_fallback(ctx: &AuthContext, resource: &Resource, op: Op) -> bool {
    match resource {
        Resource::EntityType { name } => {
            let Some(legacy_op) = legacy_op(op) else {
                return true;
            };
            ctx.is_allowed(name, legacy_op)
        }
        // EntityProperty erbt im neuen Modell vom EntityType — solange das
        // Legacy-Modell keine Property-Granularitaet kennt, fallen wir auf
        // den Entity-Typ-Check zurueck.
        Resource::EntityProperty { entity_type, .. } => {
            let Some(legacy_op) = legacy_op(op) else {
                return true;
            };
            ctx.is_allowed(entity_type, legacy_op)
        }
        // Action/Migration/Implementation/EntityInstance: ohne Projektion
        // nicht im Legacy-Modell abbildbar — permissiv.
        _ => true,
    }
}

fn legacy_op(op: Op) -> Option<PermissionOp> {
    match op {
        Op::Read => Some(PermissionOp::Read),
        Op::Create => Some(PermissionOp::Create),
        Op::Update => Some(PermissionOp::Update),
        Op::Delete => Some(PermissionOp::Delete),
        _ => None,
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::auth::{Op as NewOp, Resource};

    fn with_owner<R>(f: impl FnOnce() -> R) -> R {
        let owner = Owner::new();
        owner.with(f)
    }

    fn legacy_perm(entity: &str, read: bool, create: bool) -> Permission {
        Permission {
            entity_type: entity.into(),
            can_read: read,
            can_create: create,
            can_update: false,
            can_delete: false,
            min_access: shared::Access::Public,
            property_overrides: Vec::new(),
        }
    }

    fn empty_ctx() -> AuthContext {
        AuthContext {
            token: RwSignal::new(Some("t".into())),
            user: RwSignal::new(None),
            permissions: RwSignal::new(Vec::new()),
            effective: RwSignal::new(None),
        }
    }

    #[test]
    fn legacy_mode_uses_crud_flags_for_entity_type() {
        with_owner(|| {
            let ctx = empty_ctx();
            ctx.permissions
                .set(vec![legacy_perm("product", true, false)]);
            assert!(ctx.can_entity_type("product", NewOp::Read));
            assert!(!ctx.can_entity_type("product", NewOp::Create));
            assert!(!ctx.can_entity_type("order", NewOp::Read));
        });
    }

    #[test]
    fn legacy_mode_is_permissive_for_non_entity_resources() {
        with_owner(|| {
            let ctx = empty_ctx();
            // Keine Projektion ⇒ Actions/Migrations sind "erlaubt", weil
            // wir sie ohne Server-Wissen nicht verlaesslich blocken
            // koennen.
            assert!(ctx.can_execute("exportCsv"));
            assert!(ctx.can_migration("m-1", NewOp::Approve));
            assert!(ctx.can_choose("filter", "text-contains"));
        });
    }

    #[test]
    fn legacy_property_inherits_from_entity_type() {
        with_owner(|| {
            let ctx = empty_ctx();
            ctx.permissions
                .set(vec![legacy_perm("product", true, false)]);
            assert!(ctx.can_property("product", "price", NewOp::Read));
            assert!(!ctx.can_property("product", "price", NewOp::Create));
        });
    }

    #[test]
    fn projection_mode_is_strict_membership() {
        with_owner(|| {
            let ctx = empty_ctx();
            ctx.effective.set(Some(vec![
                EffectivePermission::entity_type("product", NewOp::Read),
                EffectivePermission::new(
                    Resource::Action {
                        name: "exportCsv".into(),
                    },
                    NewOp::Execute,
                ),
            ]));
            // In Projektion erlaubt
            assert!(ctx.can_entity_type("product", NewOp::Read));
            assert!(ctx.can_execute("exportCsv"));
            // In Projektion nicht enthalten ⇒ verweigert
            assert!(!ctx.can_entity_type("product", NewOp::Update));
            assert!(!ctx.can_execute("rebuildIndex"));
            // Legacy-Permissions werden in Projektions-Modus IGNORIERT.
            ctx.permissions.set(vec![legacy_perm("order", true, true)]);
            assert!(!ctx.can_entity_type("order", NewOp::Read));
        });
    }

    #[test]
    fn has_projection_reflects_effective_state() {
        with_owner(|| {
            let ctx = empty_ctx();
            assert!(!ctx.has_projection());
            ctx.effective.set(Some(Vec::new()));
            assert!(ctx.has_projection());
        });
    }

    // Hinweis: `AuthContext::clear()` greift via `web_sys::window()` auf den
    // Browser zu — auf non-wasm-Test-Targets panikt das. Der Clear-Pfad ist
    // deshalb hier *nicht* nativ getestet; wasm-bindgen-test (`tests/`) deckt
    // ihn ab. Die in-memory-Reset-Semantik ist trivial (drei `signal.set(...)`)
    // und bricht nicht still.

    #[test]
    fn empty_projection_blocks_everything_for_entity_resources() {
        with_owner(|| {
            let ctx = empty_ctx();
            ctx.effective.set(Some(Vec::new()));
            assert!(!ctx.can_entity_type("product", NewOp::Read));
            assert!(!ctx.can_execute("exportCsv"));
        });
    }
}
