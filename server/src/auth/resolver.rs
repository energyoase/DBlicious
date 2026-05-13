//! Permission-Resolver (Phase 0.7.3).
//!
//! Berechnet `effective(user_id, resource, op) -> Effect` gegen die neuen
//! Tabellen `permissions`, `roles`, `role_assignments` + die bestehende
//! `user_groups`-Mitgliedschaft.
//!
//! Algorithmus
//! -----------
//!
//! 1. **Subject-Aufloesung**: aus `user_id` werden alle wirksamen Subjects
//!    bestimmt:
//!    - `Subject::User { id }`
//!    - `Subject::Group { id }` fuer jede Group des Users
//!    - `Subject::Role { id }` fuer jede Role, die dem User direkt oder
//!      einer seiner Groups zugewiesen ist
//!
//! 2. **Permission-Filter**: alle `permissions`-Eintraege, deren `subject`
//!    in der Subject-Liste ist.
//!
//! 3. **Resource-Match mit Vererbung**: pro Kandidat wird die Spezifitaet
//!    gegen die Anfrage-Resource bestimmt — siehe [`resource_specificity`].
//!    Nicht passende Eintraege fallen raus.
//!
//! 4. **Op-Match**: exakter Vergleich.
//!
//! 5. **Aufloesung mehrerer Treffer**: hoechste Spezifitaet gewinnt; bei
//!    gleicher Spezifitaet gewinnt **Deny vor Allow**; bei gleichem Effekt
//!    gewinnt hoehere `priority`. Bei vollstaendig gleichen Tupeln ist die
//!    Reihenfolge undefiniert, aber der Default ist deterministisch
//!    (lexikographisch nach Subject-ID).
//!
//! 6. **Default ohne Treffer**: [`Effect::Deny`] (Allow-list-Modell).
//!
//! Row-Level (Phase 0.7 deferred)
//! ------------------------------
//!
//! Wenn die Anfrage [`shared::auth::Resource::EntityInstance`] ist UND
//! Row-Level-Enforcement nicht ueber [`set_row_level_enabled`] aktiviert
//! wurde, liefert die Funktion [`ResolveError::NotImplemented`]. Spaetere
//! Aktivierung passiert ueber Server-Config; heute ist der Schalter
//! hartkodiert auf `false`.
//!
//! Cache (kein Cache in Phase 0.7.3)
//! ---------------------------------
//!
//! Der Resolver ist heute stateless und pruegt pro Aufruf gegen die DB.
//! Session-Cache (in der ROADMAP genannt) ist eine Optimierung, die in
//! einer eigenen Iteration ohne API-Aenderung nachgezogen werden kann —
//! die `effective`-Funktion bleibt der Vertrag.

use std::sync::atomic::{AtomicBool, Ordering};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use shared::auth::{Effect, Op, Resource, Subject};

use crate::db::conn;
use crate::entity;

// =============================================================================
// Fehler
// =============================================================================

#[derive(Debug)]
pub enum ResolveError {
    /// Row-Level (`EntityInstance`) ist heute nicht enforced.
    NotImplemented(&'static str),
    /// DB-Fehler beim Lookup.
    Db(sea_orm::DbErr),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::NotImplemented(what) => write!(f, "not_implemented: {what}"),
            ResolveError::Db(e) => write!(f, "db: {e}"),
        }
    }
}

impl std::error::Error for ResolveError {}

impl From<sea_orm::DbErr> for ResolveError {
    fn from(e: sea_orm::DbErr) -> Self {
        ResolveError::Db(e)
    }
}

// =============================================================================
// Row-Level-Switch (Konfig-Stub)
// =============================================================================

static ROW_LEVEL_ENABLED: AtomicBool = AtomicBool::new(false);

/// Schaltet Row-Level-Enforcement an oder aus.
///
/// Heute der einzige Konfig-Eintritt; spaeter aus Server-Config gelesen.
/// In Tests bequem zum Umschalten zwischen "deferred" und "enforce".
pub fn set_row_level_enabled(enabled: bool) {
    ROW_LEVEL_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn row_level_enabled() -> bool {
    ROW_LEVEL_ENABLED.load(Ordering::SeqCst)
}

// =============================================================================
// Subject-Aufloesung
// =============================================================================

/// Liefert die Menge aller Subjects, die fuer `user_id` wirksam sind.
async fn subjects_for_user(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<Vec<Subject>, sea_orm::DbErr> {
    let mut subjects: Vec<Subject> = Vec::new();
    subjects.push(Subject::User { id: user_id.to_string() });

    // Groups via user_groups.
    let group_rows = entity::user_groups::Entity::find()
        .filter(entity::user_groups::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let group_ids: Vec<String> = group_rows.into_iter().map(|m| m.group_id).collect();
    for g in &group_ids {
        subjects.push(Subject::Group { id: g.clone() });
    }

    // Roles: direkt via role_assignments(subject=user) ODER ueber Groups
    // (subject=group, group_id ∈ group_ids).
    let mut role_query = entity::role_assignments::Entity::find().filter(
        entity::role_assignments::Column::SubjectKind
            .eq("user")
            .and(entity::role_assignments::Column::SubjectId.eq(user_id)),
    );
    if !group_ids.is_empty() {
        role_query = entity::role_assignments::Entity::find().filter(
            entity::role_assignments::Column::SubjectKind
                .eq("user")
                .and(entity::role_assignments::Column::SubjectId.eq(user_id))
                .or(entity::role_assignments::Column::SubjectKind
                    .eq("group")
                    .and(entity::role_assignments::Column::SubjectId.is_in(group_ids.clone()))),
        );
    }
    let role_rows = role_query.all(db).await?;
    let mut role_ids: Vec<String> = role_rows.into_iter().map(|m| m.role_id).collect();
    role_ids.sort();
    role_ids.dedup();
    for r in role_ids {
        subjects.push(Subject::Role { id: r });
    }

    Ok(subjects)
}

// =============================================================================
// Resource-Vererbung / Spezifitaet
// =============================================================================

/// Spezifitaets-Score: groesser = spezifischer. Eine Permission auf der
/// Anfrage-Resource passt nur, wenn der Score `Some(...)` ist. `None` ⇒ Permission
/// trifft die Anfrage nicht.
///
/// Vererbungs-Regeln:
/// - `EntityType { e }` matcht Anfragen auf `EntityType { e }`,
///   `EntityProperty { e, * }`, `EntityInstance { e, * }`. Score = 1.
/// - `EntityProperty { e, p }` matcht nur exakt dieselbe Property. Score = 2.
///   (Bewusst keine Vererbung auf `EntityInstance` — das waere doppelt
///   spezifisch und schwer zu erklaeren.)
/// - `EntityInstance { e, id }` matcht nur exakt dieselbe Zeile. Score = 3.
/// - Alle anderen Varianten: exakter Match, Score = 1.
fn resource_specificity(perm: &Resource, query: &Resource) -> Option<u8> {
    match (perm, query) {
        // EntityType-Permission ist Wurzel — vererbt auf Property und Instance.
        (Resource::EntityType { name: a }, Resource::EntityType { name: b }) if a == b => Some(1),
        (Resource::EntityType { name: a }, Resource::EntityProperty { entity_type: b, .. })
            if a == b =>
        {
            Some(1)
        }
        (Resource::EntityType { name: a }, Resource::EntityInstance { entity_type: b, .. })
            if a == b =>
        {
            Some(1)
        }
        // EntityProperty exakt.
        (
            Resource::EntityProperty { entity_type: a, property: p1 },
            Resource::EntityProperty { entity_type: b, property: p2 },
        ) if a == b && p1 == p2 => Some(2),
        // EntityInstance exakt.
        (
            Resource::EntityInstance { entity_type: a, id: i1 },
            Resource::EntityInstance { entity_type: b, id: i2 },
        ) if a == b && i1 == i2 => Some(3),
        // Action / Migration / ImplementationId: exakter Match.
        (Resource::Action { name: a }, Resource::Action { name: b }) if a == b => Some(1),
        (Resource::Migration { id: a }, Resource::Migration { id: b }) if a == b => Some(1),
        (
            Resource::ImplementationId { registry: r1, id: i1 },
            Resource::ImplementationId { registry: r2, id: i2 },
        ) if r1 == r2 && (i1 == i2 || i1 == "*") => Some(1),
        _ => None,
    }
}

// =============================================================================
// Hauptfunktion: effective(user, resource, op)
// =============================================================================

/// Wertet aus, ob `user_id` `op` auf `resource` ausfuehren darf.
///
/// Default-Verhalten ohne passende Regel: [`Effect::Deny`] (Allow-list).
///
/// Bei [`Resource::EntityInstance`] und deaktiviertem Row-Level
/// (`row_level_enabled() == false`) liefert die Funktion
/// [`ResolveError::NotImplemented`].
pub async fn effective(
    user_id: &str,
    resource: &Resource,
    op: Op,
) -> Result<Effect, ResolveError> {
    if matches!(resource, Resource::EntityInstance { .. }) && !row_level_enabled() {
        return Err(ResolveError::NotImplemented("row-level"));
    }

    let db = conn();
    let subjects = subjects_for_user(&db, user_id).await?;

    // Permissions aller Subjects laden. Wir koennten in der DB filtern; fuer
    // den ersten Wurf reicht "alle Permissions + Subject-Match in Rust".
    let perms = entity::permissions::Entity::find().all(&db).await?;

    let mut best: Option<(u8, Effect, i32, String)> = None; // (specificity, effect, priority, subject_id_for_tiebreak)
    for p in perms {
        let Some(subject) = Subject::from_storage(&p.subject_kind, &p.subject_id) else {
            continue;
        };
        if !subjects.contains(&subject) {
            continue;
        }
        let Some(perm_resource) = Resource::from_storage(&p.resource_kind, &p.resource_id) else {
            continue;
        };
        let Some(spec) = resource_specificity(&perm_resource, resource) else {
            continue;
        };
        let Some(perm_op) = Op::from_str(&p.op) else { continue };
        if perm_op != op {
            continue;
        }
        let Some(perm_effect) = Effect::from_str(&p.effect) else { continue };

        let candidate = (spec, perm_effect, p.priority, p.subject_id.clone());
        best = Some(match best {
            None => candidate,
            Some(current) => choose_winner(current, candidate),
        });
    }

    Ok(best.map(|(_, e, _, _)| e).unwrap_or(Effect::Deny))
}

/// Tiebreak-Logik: spezifischere gewinnt → Deny vor Allow → hoehere Priority
/// → lexikographisch nach subject_id (deterministisch).
fn choose_winner(
    current: (u8, Effect, i32, String),
    candidate: (u8, Effect, i32, String),
) -> (u8, Effect, i32, String) {
    let (cs, ce, cp, cid) = &current;
    let (ns, ne, np, nid) = &candidate;
    if ns > cs {
        return candidate;
    }
    if ns < cs {
        return current;
    }
    // gleiche Spezifitaet
    if ne != ce {
        // Deny gewinnt vor Allow
        return if *ne == Effect::Deny { candidate } else { current };
    }
    if np > cp {
        return candidate;
    }
    if np < cp {
        return current;
    }
    if nid < cid {
        candidate
    } else {
        current
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, ActiveValue};
    use shared::auth::{Op, Resource, Subject};

    async fn setup() {
        // Frischer In-Memory-Pool; Beispiel wird beim Setup geladen, damit
        // db::init() seinen Boot durchziehen kann (Seed-User fuer admin/...).
        if crate::example::current().is_none() {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("examples")
                .join("shop");
            let set = crate::example::load(&dir).expect("shop laden");
            crate::example::install(set);
        }
        crate::db::reset();
        crate::db::init().await.expect("db init");
        // Row-Level-Switch zwischen Tests reset.
        set_row_level_enabled(false);
    }

    async fn insert_permission(
        subject: Subject,
        resource: Resource,
        op: Op,
        effect: Effect,
        priority: i32,
    ) {
        let db = crate::db::conn();
        entity::permissions::ActiveModel {
            id: ActiveValue::NotSet,
            subject_kind: ActiveValue::Set(subject.kind_str().to_string()),
            subject_id: ActiveValue::Set(subject.id().to_string()),
            resource_kind: ActiveValue::Set(resource.kind_str().to_string()),
            resource_id: ActiveValue::Set(resource.storage_id()),
            op: ActiveValue::Set(op.as_str().to_string()),
            effect: ActiveValue::Set(effect.as_str().to_string()),
            priority: ActiveValue::Set(priority),
            tenant_id: ActiveValue::Set(None),
        }
        .insert(&db)
        .await
        .expect("insert permission");
    }

    // FK-Helfer: die `user_groups`- und `role_assignments`-Tabellen haben
    // Foreign Keys auf `users` / `groups` / `roles`. SQLite enforced sie per
    // Default, also legen wir die referenzierten Rows zuerst an, bevor wir
    // die Mitgliedschaft/Zuweisung einfuegen.

    async fn ensure_user(user_id: &str) {
        let db = crate::db::conn();
        if entity::users::Entity::find_by_id(user_id.to_string())
            .one(&db)
            .await
            .unwrap()
            .is_some()
        {
            return;
        }
        entity::users::ActiveModel {
            id: ActiveValue::Set(user_id.to_string()),
            username: ActiveValue::Set(user_id.to_string()),
            display_name: ActiveValue::Set(user_id.to_string()),
            locale: ActiveValue::Set(None),
            active: ActiveValue::Set(true),
            password_hash: ActiveValue::Set(None),
        }
        .insert(&db)
        .await
        .expect("ensure_user insert");
    }

    async fn ensure_group(group_id: &str) {
        let db = crate::db::conn();
        if entity::groups::Entity::find_by_id(group_id.to_string())
            .one(&db)
            .await
            .unwrap()
            .is_some()
        {
            return;
        }
        entity::groups::ActiveModel {
            id: ActiveValue::Set(group_id.to_string()),
            name_key: ActiveValue::Set(group_id.to_string()),
            description_key: ActiveValue::Set(None),
            permissions_json: ActiveValue::Set("[]".to_string()),
        }
        .insert(&db)
        .await
        .expect("ensure_group insert");
    }

    async fn ensure_role(role_id: &str) {
        let db = crate::db::conn();
        if entity::roles::Entity::find_by_id(role_id.to_string())
            .one(&db)
            .await
            .unwrap()
            .is_some()
        {
            return;
        }
        entity::roles::ActiveModel {
            id: ActiveValue::Set(role_id.to_string()),
            name_key: ActiveValue::Set(role_id.to_string()),
            description_key: ActiveValue::Set(None),
        }
        .insert(&db)
        .await
        .expect("ensure_role insert");
    }

    async fn assign_user_to_group(user_id: &str, group_id: &str) {
        ensure_user(user_id).await;
        ensure_group(group_id).await;
        let db = crate::db::conn();
        entity::user_groups::ActiveModel {
            user_id: ActiveValue::Set(user_id.to_string()),
            group_id: ActiveValue::Set(group_id.to_string()),
        }
        .insert(&db)
        .await
        .expect("user_groups insert");
    }

    async fn assign_role(subject: Subject, role_id: &str) {
        match &subject {
            Subject::User { id } => ensure_user(id).await,
            Subject::Group { id } => ensure_group(id).await,
            Subject::Role { .. } => panic!("Role-as-subject ist im RoleAssignment nicht erlaubt"),
        }
        ensure_role(role_id).await;
        let db = crate::db::conn();
        entity::role_assignments::ActiveModel {
            id: ActiveValue::NotSet,
            subject_kind: ActiveValue::Set(subject.kind_str().to_string()),
            subject_id: ActiveValue::Set(subject.id().to_string()),
            role_id: ActiveValue::Set(role_id.to_string()),
        }
        .insert(&db)
        .await
        .expect("insert role_assignment");
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn no_permissions_default_deny() {
        setup().await;
        let e = effective("u-x", &Resource::entity_type("product"), Op::Read)
            .await
            .expect("ok");
        assert_eq!(e, Effect::Deny);
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn direct_user_permission_allows() {
        setup().await;
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Allow,
            0,
        )
        .await;
        let e = effective("u-1", &Resource::entity_type("product"), Op::Read)
            .await
            .expect("ok");
        assert_eq!(e, Effect::Allow);
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn group_membership_inherits_permission() {
        setup().await;
        assign_user_to_group("u-1", "g-readers").await;
        insert_permission(
            Subject::Group { id: "g-readers".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Allow,
            0,
        )
        .await;
        let e = effective("u-1", &Resource::entity_type("product"), Op::Read)
            .await
            .expect("ok");
        assert_eq!(e, Effect::Allow, "User soll via Group lesen duerfen");
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn direct_role_assignment_inherits_permission() {
        setup().await;
        assign_role(Subject::User { id: "u-1".into() }, "r-editor").await;
        insert_permission(
            Subject::Role { id: "r-editor".into() },
            Resource::entity_type("product"),
            Op::Update,
            Effect::Allow,
            0,
        )
        .await;
        let e = effective("u-1", &Resource::entity_type("product"), Op::Update)
            .await
            .expect("ok");
        assert_eq!(e, Effect::Allow, "User mit direkter Role soll updaten duerfen");
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn user_via_group_via_role_inherits_permission() {
        // Akzeptanz: User-via-Group-via-Role.
        setup().await;
        assign_user_to_group("u-1", "g-marketing").await;
        assign_role(Subject::Group { id: "g-marketing".into() }, "r-content").await;
        insert_permission(
            Subject::Role { id: "r-content".into() },
            Resource::entity_type("product"),
            Op::Update,
            Effect::Allow,
            0,
        )
        .await;
        let e = effective("u-1", &Resource::entity_type("product"), Op::Update)
            .await
            .expect("ok");
        assert_eq!(e, Effect::Allow, "User soll via Group via Role updaten duerfen");
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn deny_wins_over_allow_at_same_specificity() {
        setup().await;
        assign_user_to_group("u-1", "g-readers").await;
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Allow,
            100,
        )
        .await;
        insert_permission(
            Subject::Group { id: "g-readers".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Deny,
            0,
        )
        .await;
        let e = effective("u-1", &Resource::entity_type("product"), Op::Read)
            .await
            .expect("ok");
        assert_eq!(e, Effect::Deny, "Deny soll bei gleicher Spezifitaet gewinnen");
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn priority_breaks_tie_within_same_effect() {
        setup().await;
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Allow,
            0,
        )
        .await;
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Allow,
            10,
        )
        .await;
        // Beide allow; priority entscheidet — Outcome ist trotzdem Allow.
        // Test verifiziert "kein Crash bei mehrfachen Treffern".
        let e = effective("u-1", &Resource::entity_type("product"), Op::Read)
            .await
            .expect("ok");
        assert_eq!(e, Effect::Allow);
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn entity_type_permission_inherits_to_property() {
        setup().await;
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Allow,
            0,
        )
        .await;
        let e = effective(
            "u-1",
            &Resource::entity_property("product", "price"),
            Op::Read,
        )
        .await
        .expect("ok");
        assert_eq!(e, Effect::Allow, "EntityType-Allow soll auf Property vererben");
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn property_specific_deny_overrides_entity_type_allow() {
        setup().await;
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Allow,
            0,
        )
        .await;
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::entity_property("product", "price"),
            Op::Read,
            Effect::Deny,
            0,
        )
        .await;
        let e = effective(
            "u-1",
            &Resource::entity_property("product", "price"),
            Op::Read,
        )
        .await
        .expect("ok");
        assert_eq!(e, Effect::Deny, "Spezifischere Property-Deny soll EntityType-Allow schlagen");
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn entity_instance_default_is_not_implemented() {
        setup().await;
        // Row-Level deaktiviert (Default).
        let res = effective(
            "u-1",
            &Resource::EntityInstance {
                entity_type: "product".into(),
                id: "p-42".into(),
            },
            Op::Read,
        )
        .await;
        assert!(matches!(res, Err(ResolveError::NotImplemented(_))));
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn entity_instance_works_when_row_level_enabled() {
        setup().await;
        set_row_level_enabled(true);
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::EntityInstance {
                entity_type: "product".into(),
                id: "p-42".into(),
            },
            Op::Read,
            Effect::Allow,
            0,
        )
        .await;
        let e = effective(
            "u-1",
            &Resource::EntityInstance {
                entity_type: "product".into(),
                id: "p-42".into(),
            },
            Op::Read,
        )
        .await
        .expect("ok");
        assert_eq!(e, Effect::Allow);
        set_row_level_enabled(false);
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn unmatched_op_falls_through_to_default_deny() {
        setup().await;
        insert_permission(
            Subject::User { id: "u-1".into() },
            Resource::entity_type("product"),
            Op::Read,
            Effect::Allow,
            0,
        )
        .await;
        // Update wurde nicht erlaubt -> default Deny.
        let e = effective("u-1", &Resource::entity_type("product"), Op::Update)
            .await
            .expect("ok");
        assert_eq!(e, Effect::Deny);
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn migration_approve_resource_works() {
        setup().await;
        insert_permission(
            Subject::User { id: "u-rel".into() },
            Resource::Migration { id: "mig-42".into() },
            Op::Approve,
            Effect::Allow,
            0,
        )
        .await;
        let e = effective(
            "u-rel",
            &Resource::Migration { id: "mig-42".into() },
            Op::Approve,
        )
        .await
        .expect("ok");
        assert_eq!(e, Effect::Allow);
        // Cutover ist eine andere Op -> Deny.
        let e = effective(
            "u-rel",
            &Resource::Migration { id: "mig-42".into() },
            Op::Cutover,
        )
        .await
        .expect("ok");
        assert_eq!(e, Effect::Deny);
    }
}
