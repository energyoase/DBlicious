//! Server-seitige Authentifizierung (SeaORM-Sessions) + Phase-0.7-Resolver.
//!
//! Bausteine in diesem Modul:
//!   - Sessions liegen in der `sessions`-Tabelle.
//!   - `login` / `user_for_bearer` / `close_session` / `invalidate_sessions_for`
//!     sind `async fn`.
//!   - Password-Hashing (Argon2id) bleibt sync — der CPU-Cost ist niedrig
//!     genug und blockt den Tokio-Reaktor nicht.
//!   - Sub-Modul [`resolver`]: das neue Permission-Modell aus Phase 0.7,
//!     mit Group- und Role-Vererbung, Deny-vor-Allow und Spezifitaets-Regel.

pub mod resolver;

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::Engine;
use chrono::Utc;
use rand::RngCore;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use shared::{AuthFailure, AuthSession, Permission, SecurityUser};

use crate::db::conn;
use crate::entity;

// =============================================================================
// Password-Hashing
// =============================================================================

pub fn hash_password(plain: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    argon
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("argon2 hash error: {e}"))
}

pub fn verify_password(plain: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else { return false };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

// =============================================================================
// Session-Operationen (DB-backed)
// =============================================================================

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub async fn open_session(user_id: &str) -> String {
    let token = generate_token();
    let am = entity::sessions::ActiveModel {
        token: ActiveValue::Set(token.clone()),
        user_id: ActiveValue::Set(user_id.to_string()),
        issued_at: ActiveValue::Set(Utc::now()),
    };
    let _ = am.insert(&conn()).await;
    token
}

pub async fn close_session(token: &str) -> bool {
    let res = entity::sessions::Entity::delete_by_id(token.to_string())
        .exec(&conn())
        .await;
    matches!(res, Ok(r) if r.rows_affected > 0)
}

pub async fn invalidate_sessions_for(user_id: &str) {
    let _ = entity::sessions::Entity::delete_many()
        .filter(entity::sessions::Column::UserId.eq(user_id))
        .exec(&conn())
        .await;
}

pub async fn user_id_for(token: &str) -> Option<String> {
    let row = entity::sessions::Entity::find_by_id(token.to_string())
        .one(&conn())
        .await
        .ok()
        .flatten()?;
    Some(row.user_id)
}

// =============================================================================
// Login + Bearer-Aufloesung
// =============================================================================

pub async fn login(username: &str, password: &str) -> Result<AuthSession, AuthFailure> {
    let user = crate::data::user_by_username(username)
        .await
        .ok_or(AuthFailure::InvalidCredentials)?;
    if !user.active {
        return Err(AuthFailure::Inactive);
    }
    let hash = user.password_hash.as_deref().ok_or(AuthFailure::Internal)?;
    if !verify_password(password, hash) {
        return Err(AuthFailure::InvalidCredentials);
    }
    let token = open_session(&user.id).await;
    let groups = crate::data::groups().await;
    let permissions: Vec<Permission> = shared::effective_permissions(&user, &groups)
        .into_iter()
        .cloned()
        .collect();
    // Phase 0.7.4-Lueckenschluss: projizierte Permission-Liste aus der neuen
    // `permissions`-Tabelle, sobald sie befuellt ist. Leer/`None` solange
    // Legacy-Modus aktiv ist — der Client faellt dann auf `permissions`
    // zurueck (siehe shared::auth::EffectivePermission).
    let effective = match resolver::project_effective(&user.id).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(target: "server::auth", "project_effective failed: {e}");
            None
        }
    };
    Ok(AuthSession {
        token,
        user: strip_secret(user),
        permissions,
        effective,
        expires_at: None,
    })
}

pub async fn user_for_bearer(header: Option<&str>) -> Option<SecurityUser> {
    let token = header?.strip_prefix("Bearer ").map(str::trim)?;
    let user_id = user_id_for(token).await?;
    let user = crate::data::user_by_id(&user_id).await?;
    if !user.active {
        return None;
    }
    Some(strip_secret(user))
}

pub fn strip_secret(mut user: SecurityUser) -> SecurityUser {
    user.password_hash = None;
    user
}

// =============================================================================
// Tests
// =============================================================================
//
// Unit-Tests koennen sich nicht auf einen externen DB-Pool verlassen — sie
// initialisieren ihn explizit per `crate::db::init().await`. Da `init` per
// `OnceCell` idempotent ist, teilen sich alle Tests denselben Pool (mit
// SQLite-im-RAM als Default).

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() {
        // Beispiel-Set muss geladen sein, damit `db::init` Seed-User
        // (admin/editor/...) einspielt — Login-Tests bauen darauf.
        if crate::example::current().is_none() {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("examples")
                .join("shop");
            let set = crate::example::load(&dir).expect("examples/shop fuer Tests");
            crate::example::install(set);
        }
        crate::db::reset();
        crate::db::init().await.expect("db::init() failed");
    }

    #[tokio::test]
    async fn hash_then_verify_roundtrip() {
        let h = hash_password("hunter2").expect("hash");
        assert!(verify_password("hunter2", &h));
        assert!(!verify_password("wrong", &h));
    }

    #[tokio::test]
    async fn verify_with_bad_hash_returns_false() {
        assert!(!verify_password("anything", "not-a-phc-string"));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn session_open_and_resolve() {
        setup().await;
        let token = open_session("u-test").await;
        assert_eq!(user_id_for(&token).await, Some("u-test".into()));
        assert!(close_session(&token).await);
        assert_eq!(user_id_for(&token).await, None);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn invalidate_sessions_clears_only_target_user() {
        setup().await;
        let t1 = open_session("u-a").await;
        let t2 = open_session("u-b").await;
        invalidate_sessions_for("u-a").await;
        assert_eq!(user_id_for(&t1).await, None);
        assert_eq!(user_id_for(&t2).await, Some("u-b".into()));
        close_session(&t2).await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn user_for_bearer_strips_prefix_and_returns_user() {
        setup().await;
        let session = login("admin", "admin").await.expect("login admin");
        let header = format!("Bearer {}", session.token);
        let user = user_for_bearer(Some(&header)).await.expect("resolved user");
        assert_eq!(user.username, "admin");
        assert!(user.password_hash.is_none(), "Hash muss gestrippt sein");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn login_with_wrong_password_fails() {
        setup().await;
        let res = login("admin", "definitely-wrong").await;
        assert!(matches!(res, Err(shared::AuthFailure::InvalidCredentials)));
    }
}
