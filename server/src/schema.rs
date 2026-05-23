//! GraphQL-Schema-Definition.
//!
//! Hinweis: `async-graphql` benoetigt eigene Typen mit `SimpleObject`-Ableitung,
//! deshalb werden die `shared`-Strukturen hier umgewickelt. Alternativ liesse
//! sich `async-graphql` direkt auf die `shared`-Typen anwenden, was aber
//! einen Featurewechsel im Workspace erzwingen wuerde – fuer ein Mock-Backend
//! ist die Umwandlung pragmatischer.

use async_graphql::{Context, Json, Object, SimpleObject};

use crate::{auth, data, AuthContext};

#[derive(Clone, SimpleObject)]
pub struct NavigationNode {
    pub id: String,
    pub label_key: String,
    pub route: Option<String>,
    pub icon: Option<String>,
    pub children: Vec<NavigationNode>,
    /// Generalisierte Aktion (siehe `shared::MenuAction`). Wird als rohes
    /// JSON ausgeliefert, der Client deserialisiert es typensicher.
    pub action: Option<Json<serde_json::Value>>,
}

#[derive(Clone, SimpleObject)]
pub struct ColumnMeta {
    pub key: String,
    pub label_key: String,
    /// Serialisierter `FieldType` (JSON). Wird clientseitig in den Enum
    /// deserialisiert. So muss `async-graphql` keine getaggte Union abbilden.
    pub field_type: Json<serde_json::Value>,
    pub sortable: bool,
    pub filterable: bool,
    pub comparator_id: Option<String>,
    pub filter_id: Option<String>,
    /// Phase 1.5: erzwungene Editor-/Formatter-IDs + Per-Row-Aktionen
    /// (Resolution-Stufe 1).
    pub editor_id: Option<String>,
    pub formatter_id: Option<String>,
    pub action_ids: Vec<String>,
}

#[derive(Clone, SimpleObject)]
pub struct Entity {
    pub id: String,
    pub fields: Json<serde_json::Value>,
}

#[derive(Clone, SimpleObject)]
pub struct EntityPage {
    pub items: Vec<Entity>,
    pub total_count: i64,
    pub page: i32,
    pub page_size: i32,
}

#[derive(Clone, SimpleObject)]
pub struct DbSchemaSaveResult {
    pub ok: bool,
    pub message: String,
    pub table_count: i32,
    pub relation_count: i32,
}

// ---------- Security ----------

#[derive(Clone, SimpleObject)]
pub struct SecurityUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub locale: Option<String>,
    pub group_ids: Vec<String>,
    pub active: bool,
}

#[derive(Clone, SimpleObject)]
pub struct PropertyPermissionView {
    pub property: String,
    /// `shared::PropertyAccessLevel` als camelCase-String.
    pub access: String,
}

#[derive(Clone, SimpleObject)]
pub struct Permission {
    pub entity_type: String,
    pub can_read: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
    /// `shared::Access` als lowercase-String.
    pub min_access: String,
    pub property_overrides: Vec<PropertyPermissionView>,
}

#[derive(Clone, SimpleObject)]
pub struct SecurityGroup {
    pub id: String,
    pub name_key: String,
    pub description_key: Option<String>,
    pub permissions: Vec<Permission>,
}

// ---------- Permissions (Phase 0.7) ----------

/// Projizierte Sicht einer einzelnen Permission-Regel.
///
/// Im Gegensatz zu `shared::auth::Permission` ist das ein flacher, GraphQL-
/// freundlicher Typ — die Resource wird in ihrer kanonischen String-Form
/// transportiert (siehe `shared::auth::Resource::storage_id`). Der Client
/// kann mit `parse_resource(kind, id)` zurueck in die typisierte Form.
#[derive(Clone, SimpleObject)]
pub struct MyPermissionView {
    pub subject_kind: String,
    pub subject_id: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub op: String,
    pub effect: String,
    pub priority: i32,
}

/// Trace-Eintrag fuer den `whyAllowed`-Debug-Endpoint (Phase 0.7.7).
#[derive(Clone, SimpleObject)]
pub struct WhyAllowedRule {
    pub subject_kind: String,
    pub subject_id: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub op: String,
    pub effect: String,
    pub specificity: i32,
    pub priority: i32,
}

/// Projizierter Allow-Eintrag fuer den Client (Phase 0.7.4-Lueckenschluss).
/// Resource in kanonischer String-Form (siehe shared::auth::Resource::storage_id).
#[derive(Clone, SimpleObject)]
pub struct EffectivePermissionView {
    pub resource_kind: String,
    pub resource_id: String,
    pub op: String,
}

fn map_effective(p: shared::auth::EffectivePermission) -> EffectivePermissionView {
    EffectivePermissionView {
        resource_kind: p.resource.kind_str().to_string(),
        resource_id: p.resource.storage_id(),
        op: p.op.as_str().to_string(),
    }
}

// ---------- Plugins (Phase 2) ----------

#[derive(Clone, SimpleObject)]
pub struct PluginView {
    pub id: String,
    pub version: String,
    /// `shared::plugin::PluginManifest` als JSON-Wert.
    pub manifest: Json<serde_json::Value>,
    pub enabled: bool,
    pub installed_at: String,
}

fn map_plugin(m: crate::entity::plugins::Model) -> PluginView {
    let manifest: serde_json::Value =
        serde_json::from_str(&m.manifest_json).unwrap_or(serde_json::Value::Null);
    PluginView {
        id: m.id,
        version: m.version,
        manifest: Json(manifest),
        enabled: m.enabled,
        installed_at: m.installed_at,
    }
}

#[derive(Clone, SimpleObject)]
pub struct WhyAllowedTrace {
    /// Endergebnis (`"allow"` oder `"deny"`).
    pub final_effect: String,
    /// Sortierte Regelliste — erstes Element ist der Gewinner.
    pub rules: Vec<WhyAllowedRule>,
    /// Hinweis, wenn die Auswertung Sonderfall (z.B. row-level deaktiviert).
    pub note: Option<String>,
}

// ---------- Builder-Design-Persistenz (Phase 1.6) ----------

#[derive(Clone, SimpleObject)]
pub struct EntityDesignView {
    pub entity_type: String,
    pub version: i32,
    pub schema_version: i32,
    /// State-Blob (`tree.nodes` + `projection`). Wie bei `Entity.fields` als
    /// opaques JSON ausgeliefert — der Client deserialisiert in das
    /// `shared::builder`-Format.
    pub state: Json<serde_json::Value>,
    pub created_at: String,
    pub created_by: String,
    pub locked: bool,
}

#[derive(Clone, SimpleObject)]
pub struct SaveEntityDesignResult {
    pub ok: bool,
    /// Bei Erfolg: die soeben angelegte neue Version.
    pub design: Option<EntityDesignView>,
    /// Bei `ok=false`: aktueller Server-Stand (oder `None`, wenn die
    /// Tabelle fuer diesen entity_type noch leer ist) — der Client kann
    /// damit eine Konflikt-Resolution anbieten.
    pub conflict_current: Option<EntityDesignView>,
    /// Klassifizierter Fehlercode (`"concurrent_design_modification"`,
    /// `"locked"`, `"forbidden"`, `"internal"`); leer bei Erfolg.
    pub error: Option<String>,
}

fn map_entity_design(m: crate::entity::entity_designs::Model) -> EntityDesignView {
    let state: serde_json::Value =
        serde_json::from_str(&m.state_json).unwrap_or(serde_json::Value::Null);
    EntityDesignView {
        entity_type: m.entity_type,
        version: m.version,
        schema_version: m.schema_version,
        state: Json(state),
        created_at: m.created_at,
        created_by: m.created_by,
        locked: m.locked,
    }
}

// ---------- Translatable ----------

#[derive(Clone, SimpleObject)]
pub struct TranslatableLanguage {
    pub id: String,
    pub code: String,
    pub name_key: String,
    pub fallback_id: Option<String>,
    pub active: bool,
}

#[derive(Clone, SimpleObject)]
pub struct TranslatableEntry {
    pub id: String,
    pub category: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, SimpleObject)]
pub struct TranslatableValue {
    pub entry_id: String,
    pub language_id: String,
    pub ftl_source: String,
    pub updated_at: Option<String>,
}

#[derive(Clone, SimpleObject)]
pub struct TranslatableBundle {
    pub languages: Vec<TranslatableLanguage>,
    pub entries: Vec<TranslatableEntry>,
    pub values: Vec<TranslatableValue>,
}

// ---------- Editor / Settings ----------

#[derive(Clone, SimpleObject)]
pub struct EditorPropertyMeta {
    pub key: String,
    pub label_key: String,
    pub field_type: Json<serde_json::Value>,
    pub required: bool,
    pub readonly: bool,
    /// `shared::Visibility` lowercase.
    pub visibility: String,
    pub order: i32,
    pub help_key: Option<String>,
    pub placeholder_key: Option<String>,
    pub group_key: Option<String>,
    /// `shared::ControlKind` lowercase.
    pub control: String,
    pub min_length: Option<i32>,
    pub max_length: Option<i32>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub pattern: Option<String>,
}

#[derive(Clone, SimpleObject)]
pub struct EditorMeta {
    pub entity_type: String,
    pub properties: Vec<EditorPropertyMeta>,
}

#[derive(Clone, SimpleObject)]
pub struct PropertySettings {
    pub key: String,
    pub visibility: String,
    pub access: String,
    pub load_method: String,
    pub order: i32,
    pub label_override_key: Option<String>,
    pub min_width: Option<i32>,
}

#[derive(Clone, SimpleObject)]
pub struct EntitySettings {
    pub entity_type: String,
    pub access: String,
    pub default_page_size: Option<i32>,
    pub default_sort: Option<Json<serde_json::Value>>,
    pub default_filter: Option<Json<serde_json::Value>>,
    pub properties: Vec<PropertySettings>,
}

// ---------- Mutation-Ergebnisse ----------

#[derive(Clone, SimpleObject)]
pub struct EntityChangeResult {
    pub ok: bool,
    pub entity: Option<Entity>,
    /// Liste von `shared::ValidationMessage` als rohes JSON-Array.
    pub validation: Json<serde_json::Value>,
}

// ---------- Auth ----------

#[derive(Clone, SimpleObject)]
pub struct AuthSessionView {
    pub token: String,
    pub user: SecurityUser,
    pub permissions: Vec<Permission>,
    /// Projizierte Allows aus Phase 0.7.4. `None` = Legacy-Modus (Client
    /// soll `permissions` benutzen); `Some([...])` = strikte Membership.
    pub effective: Option<Vec<EffectivePermissionView>>,
    pub expires_at: Option<String>,
}

#[derive(Clone, SimpleObject)]
pub struct LoginResult {
    pub ok: bool,
    pub session: Option<AuthSessionView>,
    /// `shared::AuthFailure` als camelCase-String (z.B. `"invalidCredentials"`).
    pub error: Option<String>,
}

// =============================================================================
// Konvertierung shared → schema
// =============================================================================

fn map_user(u: shared::SecurityUser) -> SecurityUser {
    SecurityUser {
        id: u.id,
        username: u.username,
        display_name: u.display_name,
        locale: u.locale,
        group_ids: u.group_ids,
        active: u.active,
    }
}

fn map_perm(p: shared::Permission) -> Permission {
    Permission {
        entity_type: p.entity_type,
        can_read: p.can_read,
        can_create: p.can_create,
        can_update: p.can_update,
        can_delete: p.can_delete,
        min_access: match p.min_access {
            shared::Access::Public => "public",
            shared::Access::Internal => "internal",
            shared::Access::Protected => "protected",
            shared::Access::Admin => "admin",
        }
        .into(),
        property_overrides: p
            .property_overrides
            .into_iter()
            .map(|o| PropertyPermissionView {
                property: o.property,
                access: match o.access {
                    shared::PropertyAccessLevel::NoAccess => "noAccess",
                    shared::PropertyAccessLevel::Read => "read",
                    shared::PropertyAccessLevel::WriteBeforePersist => "writeBeforePersist",
                    shared::PropertyAccessLevel::Write => "write",
                }
                .into(),
            })
            .collect(),
    }
}

fn map_group(g: shared::SecurityGroup) -> SecurityGroup {
    SecurityGroup {
        id: g.id,
        name_key: g.name_key,
        description_key: g.description_key,
        permissions: g.permissions.into_iter().map(map_perm).collect(),
    }
}

fn map_tr_bundle(b: shared::TranslatableBundle) -> TranslatableBundle {
    TranslatableBundle {
        languages: b
            .languages
            .into_iter()
            .map(|l| TranslatableLanguage {
                id: l.id,
                code: l.code,
                name_key: l.name_key,
                fallback_id: l.fallback_id,
                active: l.active,
            })
            .collect(),
        entries: b
            .entries
            .into_iter()
            .map(|e| TranslatableEntry {
                id: e.id,
                category: e.category,
                description: e.description,
            })
            .collect(),
        values: b
            .values
            .into_iter()
            .map(|v| TranslatableValue {
                entry_id: v.entry_id,
                language_id: v.language_id,
                ftl_source: v.ftl_source,
                updated_at: v.updated_at,
            })
            .collect(),
    }
}

fn visibility_str(v: shared::Visibility) -> &'static str {
    match v {
        shared::Visibility::Visible => "visible",
        shared::Visibility::Hidden => "hidden",
        shared::Visibility::ReadOnly => "readOnly",
        shared::Visibility::DetailOnly => "detailOnly",
    }
}

fn control_str(c: shared::ControlKind) -> &'static str {
    match c {
        shared::ControlKind::Auto => "auto",
        shared::ControlKind::Input => "input",
        shared::ControlKind::TextArea => "textArea",
        shared::ControlKind::Select => "select",
        shared::ControlKind::DatePicker => "datePicker",
        shared::ControlKind::Lookup => "lookup",
        shared::ControlKind::InlineList => "inlineList",
        shared::ControlKind::Toggle => "toggle",
    }
}

fn access_str(a: shared::Access) -> &'static str {
    match a {
        shared::Access::Public => "public",
        shared::Access::Internal => "internal",
        shared::Access::Protected => "protected",
        shared::Access::Admin => "admin",
    }
}

fn prop_access_str(a: shared::PropertyAccess) -> &'static str {
    match a {
        shared::PropertyAccess::ReadWrite => "readWrite",
        shared::PropertyAccess::ReadOnly => "readOnly",
        shared::PropertyAccess::WriteOnly => "writeOnly",
        shared::PropertyAccess::None => "none",
    }
}

fn load_method_str(m: shared::LoadMethod) -> &'static str {
    match m {
        shared::LoadMethod::Eager => "eager",
        shared::LoadMethod::Lazy => "lazy",
        shared::LoadMethod::Manual => "manual",
    }
}

fn map_editor(e: shared::EditorMeta) -> EditorMeta {
    EditorMeta {
        entity_type: e.entity_type,
        properties: e
            .properties
            .into_iter()
            .map(|p| EditorPropertyMeta {
                key: p.key,
                label_key: p.label_key,
                field_type: Json(serde_json::to_value(&p.field_type).unwrap_or_default()),
                required: p.required,
                readonly: p.readonly,
                visibility: visibility_str(p.visibility).into(),
                order: p.order,
                help_key: p.help_key,
                placeholder_key: p.placeholder_key,
                group_key: p.group_key,
                control: control_str(p.control).into(),
                min_length: p.min_length.map(|v| v as i32),
                max_length: p.max_length.map(|v| v as i32),
                min: p.min,
                max: p.max,
                pattern: p.pattern,
            })
            .collect(),
    }
}

fn map_session(s: shared::AuthSession) -> AuthSessionView {
    AuthSessionView {
        token: s.token,
        user: map_user(s.user),
        permissions: s.permissions.into_iter().map(map_perm).collect(),
        effective: s
            .effective
            .map(|list| list.into_iter().map(map_effective).collect()),
        expires_at: s.expires_at,
    }
}

fn failure_str(f: shared::AuthFailure) -> &'static str {
    match f {
        shared::AuthFailure::InvalidCredentials => "invalidCredentials",
        shared::AuthFailure::Inactive => "inactive",
        shared::AuthFailure::Internal => "internal",
    }
}

/// Pruefe, ob der aktuelle User eine bestimmte Operation auf einem Entity-Typ
/// darf.
///
/// Enforcement-Pfad (Phase 0.7.4):
/// 1. Wenn die neue `permissions`-Tabelle nicht leer ist, ist sie
///    authoritative — der Resolver aus `auth::resolver::effective`
///    entscheidet. Der Wildcard `entity_type = "*"` faellt nicht ins neue
///    Modell und nutzt weiterhin den Legacy-Pfad (admin-Shortcut).
/// 2. Sonst: alte Logik (Groups + can_*-Flags). Solange ein Example wie
///    `examples/shop/` keine `security/permissions.{toml,json}` mitbringt,
///    bleibt der Server damit kompatibel.
async fn require_permission(
    ctx: &Context<'_>,
    entity_type: &str,
    op: shared::PermissionOp,
) -> async_graphql::Result<shared::SecurityUser> {
    let auth_ctx = ctx.data::<AuthContext>()?.clone();
    let user = auth_ctx.user.ok_or_else(|| async_graphql::Error::new("unauthenticated"))?;

    // Helfer: Deny ins Audit-Log schreiben, dann Error-Token bauen.
    async fn forbid(user_id: &str, entity_type: &str, op_str: &str) -> async_graphql::Error {
        crate::audit::record_deny(user_id, entity_type, op_str).await;
        async_graphql::Error::new("forbidden")
    }
    let op_str = map_permission_op(op).as_str();

    // Neue Schicht aktiv, sobald irgendeine permission persistiert ist.
    if entity_type != "*" && data::permissions_count().await > 0 {
        use shared::auth::{Effect, Resource};
        let resource = Resource::entity_type(entity_type);
        let new_op = map_permission_op(op);
        match crate::auth::resolver::effective(&user.id, &resource, new_op).await {
            Ok(Effect::Allow) => return Ok(user),
            Ok(Effect::Deny) => return Err(forbid(&user.id, entity_type, op_str).await),
            Err(crate::auth::resolver::ResolveError::NotImplemented(_)) => {
                // Row-Level kommt erst spaeter — sollte hier nicht auftreten,
                // weil wir mit `Resource::EntityType` arbeiten.
                return Err(forbid(&user.id, entity_type, op_str).await);
            }
            Err(e) => {
                tracing::warn!(target: "server::auth", "resolver error: {e}");
                return Err(forbid(&user.id, entity_type, op_str).await);
            }
        }
    }

    // Legacy-Pfad (Groups + can_*-Flags).
    let groups = data::groups().await;
    if !shared::is_allowed(&user, &groups, entity_type, op) {
        return Err(forbid(&user.id, entity_type, op_str).await);
    }
    Ok(user)
}

/// Mappt die Legacy-CRUD-Op auf die neue, breitere `Op`-Aufzaehlung.
fn map_permission_op(op: shared::PermissionOp) -> shared::auth::Op {
    match op {
        shared::PermissionOp::Read => shared::auth::Op::Read,
        shared::PermissionOp::Create => shared::auth::Op::Create,
        shared::PermissionOp::Update => shared::auth::Op::Update,
        shared::PermissionOp::Delete => shared::auth::Op::Delete,
    }
}

/// Auch der bestehende `audit_log`-Lookup wird vom Server exportiert, damit
/// kuenftige `auditLog`-GraphQL-Queries oder Admin-UIs ohne Re-Implementation
/// auskommen. Heute nur fuer Tests benutzt.
#[doc(hidden)]
pub async fn recent_audit_entries(limit: u64) -> Vec<crate::entity::audit_log::Model> {
    use sea_orm::{EntityTrait, QueryOrder, QuerySelect};
    crate::entity::audit_log::Entity::find()
        .order_by_desc(crate::entity::audit_log::Column::Id)
        .limit(limit)
        .all(&crate::db::conn())
        .await
        .unwrap_or_default()
}

fn map_settings(s: shared::EntitySettings) -> EntitySettings {
    EntitySettings {
        entity_type: s.entity_type,
        access: access_str(s.access).into(),
        default_page_size: s.default_page_size.map(|p| p as i32),
        default_sort: s.default_sort.map(|v| Json(serde_json::to_value(v).unwrap_or_default())),
        default_filter: s.default_filter.map(|v| Json(serde_json::to_value(v).unwrap_or_default())),
        properties: s
            .properties
            .into_iter()
            .map(|p| PropertySettings {
                key: p.key,
                visibility: visibility_str(p.visibility).into(),
                access: prop_access_str(p.access).into(),
                load_method: load_method_str(p.load_method).into(),
                order: p.order,
                label_override_key: p.label_override_key,
                min_width: p.min_width.map(|w| w as i32),
            })
            .collect(),
    }
}

// =============================================================================
// Query / Mutation roots
// =============================================================================

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    // -- Auth: anonym erlaubt --

    async fn current_user(&self, ctx: &Context<'_>) -> Option<SecurityUser> {
        let auth = ctx.data::<AuthContext>().ok()?;
        auth.user.clone().map(auth::strip_secret).map(map_user)
    }

    async fn current_permissions(&self, ctx: &Context<'_>) -> Vec<Permission> {
        let Some(auth) = ctx.data::<AuthContext>().ok() else { return vec![] };
        let Some(user) = &auth.user else { return vec![] };
        let groups = data::groups().await;
        shared::effective_permissions(user, &groups)
            .into_iter()
            .cloned()
            .map(map_perm)
            .collect()
    }

    /// Projizierte Liste aller `permissions`-Eintraege, die fuer den
    /// eingeloggten User wirksam sind (Phase 0.7.4).
    ///
    /// Die Liste enthaelt sowohl `Allow` als auch `Deny`-Regeln — der
    /// Client muss die Spezifitaets-Regel (`auth::resolver::effective`-
    /// Logik) anwenden, wenn er ohne weitere Server-Anfrage UI-Hints
    /// ableiten will. Fuer harte Permission-Checks ist und bleibt der
    /// Server authoritative.
    ///
    /// Debug-Endpoint (Phase 0.7.7): liefert die Aufloesungs-Reihenfolge des
    /// Resolvers fuer ein konkretes `(user, resource, op)`-Tupel.
    ///
    /// Zugriff: aktuell jeder eingeloggte User darf nur fuer sich selbst
    /// abfragen. Cross-User-Anfragen erfordern den Wildcard-Admin-Check
    /// (alte Logik `require_permission("*", Update)`). Damit ist der Endpoint
    /// fuer Endusers fuer Selbstdiagnose nutzbar; Admins koennen Permissions
    /// fremder User analysieren.
    ///
    /// `resourceKind` und `resourceId` haben die kanonische String-Form (siehe
    /// `shared::auth::Resource::storage_id`).
    async fn why_allowed(
        &self,
        ctx: &Context<'_>,
        user_id: String,
        resource_kind: String,
        resource_id: String,
        op: String,
    ) -> async_graphql::Result<WhyAllowedTrace> {
        let auth_ctx = ctx.data::<AuthContext>()?.clone();
        let me = auth_ctx
            .user
            .ok_or_else(|| async_graphql::Error::new("unauthenticated"))?;

        if me.id != user_id {
            // Wildcard-Admin-Check (legacy oder neue Schicht — beide moeglich).
            let _ = require_permission(ctx, "*", shared::PermissionOp::Update).await?;
        }

        let resource = shared::auth::Resource::from_storage(&resource_kind, &resource_id)
            .ok_or_else(|| async_graphql::Error::new("invalid_resource"))?;
        let parsed_op = shared::auth::Op::from_str(&op)
            .ok_or_else(|| async_graphql::Error::new("invalid_op"))?;

        let trace = crate::auth::resolver::trace_effective(&user_id, &resource, parsed_op)
            .await
            .map_err(|e| async_graphql::Error::new(format!("{e}")))?;

        Ok(WhyAllowedTrace {
            final_effect: trace.final_effect.as_str().to_string(),
            rules: trace
                .rules
                .into_iter()
                .map(|r| WhyAllowedRule {
                    subject_kind: r.subject_kind,
                    subject_id: r.subject_id,
                    resource_kind: r.resource_kind,
                    resource_id: r.resource_id,
                    op: r.op,
                    effect: r.effect.as_str().to_string(),
                    specificity: r.specificity as i32,
                    priority: r.priority,
                })
                .collect(),
            note: trace.note,
        })
    }

    /// Projizierte Allow-Liste des eingeloggten Users (Phase 0.7.4-Lueckenschluss).
    ///
    /// Liefert `None`, solange die `permissions`-Tabelle leer ist
    /// (Legacy-Modus aktiv — Client faellt auf `currentPermissions` zurueck).
    /// Liefert `Some([])`, wenn die Tabelle befuellt ist, aber der User
    /// keinen einzigen Allow hat. Liefert `Some([...])` mit allen Allows.
    ///
    /// Verwendung: der Client cached `AuthSession.effective` aus dem Login.
    /// Nach einer Permission-Aenderung (z.B. via Admin-UI) kann er diese
    /// Query rufen und den Cache aktualisieren, ohne sich neu einzuloggen.
    async fn current_effective(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Option<Vec<EffectivePermissionView>>> {
        let Some(auth_ctx) = ctx.data::<AuthContext>().ok() else { return Ok(None) };
        let Some(user) = &auth_ctx.user else { return Ok(None) };
        match crate::auth::resolver::project_effective(&user.id).await {
            Ok(Some(list)) => Ok(Some(list.into_iter().map(map_effective).collect())),
            Ok(None) => Ok(None),
            Err(e) => Err(async_graphql::Error::new(format!("{e}"))),
        }
    }

    /// Anonyme Anfragen liefern eine leere Liste.
    async fn my_permissions(&self, ctx: &Context<'_>) -> Vec<MyPermissionView> {
        let Some(auth_ctx) = ctx.data::<AuthContext>().ok() else { return vec![] };
        let Some(user) = &auth_ctx.user else { return vec![] };

        let db = crate::db::conn();
        let Ok(subjects) = crate::auth::resolver::subjects_for_user(&db, &user.id).await
        else {
            return vec![];
        };
        let Ok(perms) = <crate::entity::permissions::Entity as sea_orm::EntityTrait>::find()
            .all(&db)
            .await
        else {
            return vec![];
        };

        perms
            .into_iter()
            .filter(|p| {
                shared::auth::Subject::from_storage(&p.subject_kind, &p.subject_id)
                    .map(|s| subjects.contains(&s))
                    .unwrap_or(false)
            })
            .map(|p| MyPermissionView {
                subject_kind: p.subject_kind,
                subject_id: p.subject_id,
                resource_kind: p.resource_kind,
                resource_id: p.resource_id,
                op: p.op,
                effect: p.effect,
                priority: p.priority,
            })
            .collect()
    }

    // -- Translatable: anonym erlaubt (Login-Seite braucht Strings) --

    async fn translatable(&self) -> TranslatableBundle {
        map_tr_bundle(data::translatable_bundle().await)
    }

    // -- Authenticated-Bereich --

    async fn navigation(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<NavigationNode>> {
        let _ = ctx
            .data::<AuthContext>()?
            .user
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("unauthenticated"))?;
        Ok(data::navigation_tree())
    }

    async fn entity_columns(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
    ) -> async_graphql::Result<Vec<ColumnMeta>> {
        require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        Ok(data::columns_for(&entity_type))
    }

    async fn entities(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        #[graphql(default = 1)] page: i32,
        #[graphql(default = 20)] page_size: i32,
        sort_by: Option<String>,
        sort_dir: Option<String>,
        filter: Option<Json<serde_json::Value>>,
    ) -> async_graphql::Result<EntityPage> {
        let user = require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;

        // Sort-Args zu typisiertem `shared::Sort` zusammenfuehren. `sort_by`
        // ohne `sort_dir` defaultet auf `asc`; ungueltige Richtung -> ignoriert
        // (Sortierung entfaellt komplett, keine Fehlermeldung).
        let sort = sort_by.and_then(|field| {
            let direction = match sort_dir.as_deref().unwrap_or("asc") {
                "asc" => shared::SortDirection::Asc,
                "desc" => shared::SortDirection::Desc,
                _ => return None,
            };
            Some(shared::Sort { field, direction })
        });

        // Filter-Args als `shared::FilterCriteria` deserialisieren. Ungueltige
        // JSON-Form bedeutet "kein Filter", ebenfalls ohne Fehler.
        let filter_criteria: shared::FilterCriteria = filter
            .and_then(|j| serde_json::from_value(j.0).ok())
            .unwrap_or_default();

        let groups = data::groups().await;
        let mut page =
            data::entities_page(&entity_type, page, page_size, sort, filter_criteria).await;
        for ent in &mut page.items {
            if let serde_json::Value::Object(map) = &mut ent.fields.0 {
                data::filter_properties_for_user(&entity_type, map, &user, &groups).await;
            }
        }
        Ok(page)
    }

    async fn entity_by_id(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        id: String,
    ) -> async_graphql::Result<Option<Entity>> {
        let user = require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        let groups = data::groups().await;
        let mut ent = match data::entity_by_id(&entity_type, &id).await {
            Some(e) => e,
            None => return Ok(None),
        };
        if let serde_json::Value::Object(map) = &mut ent.fields.0 {
            data::filter_properties_for_user(&entity_type, map, &user, &groups).await;
        }
        Ok(Some(ent))
    }

    async fn users(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<SecurityUser>> {
        match require_permission(ctx, "user", shared::PermissionOp::Read).await {
            Ok(_) => (),
            Err(_) => {
                require_permission(ctx, "*", shared::PermissionOp::Read).await?;
            }
        }
        Ok(data::users()
            .await
            .into_iter()
            .map(auth::strip_secret)
            .map(map_user)
            .collect())
    }

    async fn groups(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<SecurityGroup>> {
        match require_permission(ctx, "group", shared::PermissionOp::Read).await {
            Ok(_) => (),
            Err(_) => {
                require_permission(ctx, "*", shared::PermissionOp::Read).await?;
            }
        }
        Ok(data::groups().await.into_iter().map(map_group).collect())
    }

    async fn entity_editor(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
    ) -> async_graphql::Result<Option<EditorMeta>> {
        require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        Ok(data::editor_for_async(&entity_type).await.map(map_editor))
    }

    async fn entity_settings(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
    ) -> async_graphql::Result<Option<EntitySettings>> {
        require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        let auth = ctx.data_opt::<AuthContext>();
        let user_ref = auth.and_then(|a| a.user.as_ref());
        let mut resolved = crate::views::resolve_view(&entity_type, "default", user_ref).await;
        // I1: Unbekannte Override-Keys (Spec E1) vor der Auslieferung entfernen.
        let known_keys: Vec<String> = crate::data::columns_for(&entity_type)
            .into_iter().map(|c| c.key).collect();
        crate::views::strip_unknown_keys(&mut resolved.properties, &known_keys, &entity_type, "default");
        // Wenn keine Layer vorliegen, ist provenance leer — gib None zurueck,
        // damit der Client auf Default-Verhalten faellt.
        // F1 (loader bootstrap) wird entity_views beim Start befuellen, sodass
        // dieser Pfad im normalen Betrieb nicht mehr zurueck-None liefert.
        if resolved.provenance.is_empty() {
            return Ok(None);
        }
        let settings = shared::EntitySettings {
            entity_type:         resolved.entity_type,
            access:              shared::settings::Access::default(),
            default_page_size:   resolved.default_page_size,
            default_sort:        resolved.default_sort,
            default_filter:      resolved.default_filter,
            properties:          resolved.properties,
            field_type_defaults: Default::default(),
            binding:             None,
        };
        Ok(Some(map_settings(settings)))
    }

    /// Aktive Builder-Version (Phase 1.6). `None` wenn fuer den entity_type
    /// noch nichts persistiert ist (sehr selten — Boot-Snapshot legt
    /// version=0 fuer alle Loader-Typen an).
    async fn entity_design(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
    ) -> async_graphql::Result<Option<EntityDesignView>> {
        require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        Ok(data::entity_design_active(&entity_type)
            .await
            .map(map_entity_design))
    }

    /// Liefert die fuer `(entityType, property, registry)` aufgeloeste
    /// Implementations-ID (Phase 1.5.3). `userId` defaultet auf den
    /// eingeloggten User; Admins koennen einen fremden `userId` angeben.
    async fn resolve_implementation(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        property: String,
        registry: String,
        user_id: Option<String>,
    ) -> async_graphql::Result<Option<String>> {
        let auth_ctx = ctx.data::<AuthContext>()?.clone();
        let me = auth_ctx
            .user
            .ok_or_else(|| async_graphql::Error::new("unauthenticated"))?;
        let effective_user_id = match user_id {
            Some(other) if other != me.id => {
                require_permission(ctx, "*", shared::PermissionOp::Update).await?;
                other
            }
            other => other.unwrap_or(me.id.clone()),
        };
        Ok(data::resolve_implementation(
            &entity_type,
            &property,
            &registry,
            Some(&effective_user_id),
        )
        .await)
    }

    /// Liste der IDs, die fuer `(entityType, property, registry)` zur
    /// Verfuegung stehen — vor dem Choose-Permission-Filter.
    async fn allowed_implementations(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        property: String,
        registry: String,
    ) -> async_graphql::Result<Vec<String>> {
        require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        Ok(data::allowed_implementations(&entity_type, &property, &registry).await)
    }

    /// Liste aller installierten Plugins (Phase 2.5).
    async fn plugins(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<PluginView>> {
        require_permission(ctx, "*", shared::PermissionOp::Update).await?;
        let plugins = crate::plugins::list_plugins()
            .await
            .map_err(|e| async_graphql::Error::new(format!("{e}")))?;
        Ok(plugins.into_iter().map(map_plugin).collect())
    }

    async fn plugin(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> async_graphql::Result<Option<PluginView>> {
        require_permission(ctx, "*", shared::PermissionOp::Update).await?;
        Ok(crate::plugins::get_plugin(&id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("{e}")))?
            .map(map_plugin))
    }

    /// Bestimmte historische Version (fuer Revert-UI / Audit).
    async fn entity_design_at(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        version: i32,
    ) -> async_graphql::Result<Option<EntityDesignView>> {
        require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        Ok(data::entity_design_version(&entity_type, version)
            .await
            .map(map_entity_design))
    }

    // -- Q0005: Named Views --

    async fn entity_view(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        #[graphql(default = "default")]
        view_name: String,
    ) -> async_graphql::Result<EntityViewGql> {
        // I4: Read-Gate — wie entity_settings, entity_editor usw.
        require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        use crate::AuthContext;
        let auth = ctx.data_opt::<AuthContext>();
        let user_ref = auth.and_then(|a| a.user.as_ref());
        let mut resolved = crate::views::resolve_view(&entity_type, &view_name, user_ref).await;
        // I1: Unbekannte Override-Keys (Spec E1) vor der Auslieferung entfernen.
        let known_keys: Vec<String> = crate::data::columns_for(&entity_type)
            .into_iter().map(|c| c.key).collect();
        crate::views::strip_unknown_keys(&mut resolved.properties, &known_keys, &entity_type, &view_name);
        Ok(EntityViewGql {
            id:                format!("resolved:{entity_type}:{view_name}"),
            entity_type:       resolved.entity_type,
            view_name:         resolved.view_name,
            layer:             GqlViewLayer::Global,
            owner_id:          None,
            properties:        async_graphql::Json(serde_json::to_value(resolved.properties).unwrap_or(serde_json::Value::Null)),
            default_filter:    resolved.default_filter.map(|f| async_graphql::Json(serde_json::to_value(f).unwrap_or(serde_json::Value::Null))),
            default_sort:      resolved.default_sort.map(|s| async_graphql::Json(serde_json::to_value(s).unwrap_or(serde_json::Value::Null))),
            default_page_size: resolved.default_page_size,
            version:           resolved.provenance.iter()
                .find(|p| p.layer == shared::view::ViewLayer::Global)
                .map(|p| p.version)
                .unwrap_or(0),
            updated_at:        String::new(),
            updated_by:        None,
        })
    }

    async fn entity_views(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
    ) -> async_graphql::Result<Vec<EntityViewSummaryGql>> {
        // I4: Read-Gate — wie entity_settings, entity_editor usw.
        require_permission(ctx, &entity_type, shared::PermissionOp::Read).await?;
        Ok(crate::data::find_entity_views(&entity_type)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| EntityViewSummaryGql {
                view_name:  s.view_name,
                layers:     s.layers.into_iter().map(GqlViewLayer::from).collect(),
                updated_at: s.updated_at,
            })
            .collect())
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // -- Auth --

    async fn login(&self, username: String, password: String) -> LoginResult {
        match auth::login(&username, &password).await {
            Ok(session) => LoginResult {
                ok: true,
                session: Some(map_session(session)),
                error: None,
            },
            Err(e) => LoginResult {
                ok: false,
                session: None,
                error: Some(failure_str(e).into()),
            },
        }
    }

    /// Schliesst die aktuelle Session (nur den vorgelegten Token).
    async fn logout(&self, ctx: &Context<'_>) -> bool {
        let Some(auth) = ctx.data::<AuthContext>().ok() else { return false };
        match &auth.token {
            Some(token) => auth::close_session(token).await,
            None => false,
        }
    }

    // -- Designer --

    async fn save_db_schema(
        &self,
        ctx: &Context<'_>,
        schema: Json<serde_json::Value>,
    ) -> async_graphql::Result<DbSchemaSaveResult> {
        require_permission(ctx, "*", shared::PermissionOp::Update).await?;
        let value = schema.0;
        let parsed: Result<shared::DbSchema, _> = serde_json::from_value(value.clone());
        let table_count = value
            .get("tables")
            .and_then(|t| t.as_array())
            .map(|a| a.len() as i32)
            .unwrap_or(0);
        let relation_count = value
            .get("relations")
            .and_then(|r| r.as_array())
            .map(|a| a.len() as i32)
            .unwrap_or(0);
        let name = value
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("(unbenannt)");

        let mut ddl_applied = 0usize;
        if let Ok(schema) = parsed {
            data::install_db_schema(schema.clone());
            if let Err(e) = data::persist_db_schema(&schema).await {
                tracing::warn!(target: "server::designer", "persist failed: {e}");
            }
            // Phase 0.6 B5: DDL geht ueber die Source-Capability. Default-
            // Source `local` (managed-sqlite) akzeptiert DDL; foreign-sqlite
            // lehnt mit `ReadOnly` ab. Falls keine Source angemeldet ist
            // (z.B. CLI ohne boot), fallen wir auf den direkten ddl-Pfad
            // zurueck. Der RwLockReadGuard ist !Send — Arc rauskopieren,
            // Guard droppen, dann awaiten.
            let src_arc: Option<std::sync::Arc<dyn crate::source::Source>> = {
                let reg = crate::source::registry();
                let binding = shared::source::EntityBinding {
                    source: "local".into(),
                    ..shared::source::default_binding_for("__designer__")
                };
                reg.route(&binding).ok()
            };
            match src_arc {
                Some(src) if src.capabilities().supports_ddl => {
                    match src.apply_schema(&schema).await {
                        Ok(n) => ddl_applied = n,
                        Err(e) => tracing::warn!(
                            target: "server::designer",
                            "Source 'local' lehnt apply_schema ab: {e}"
                        ),
                    }
                }
                Some(_) => tracing::warn!(
                    target: "server::designer",
                    "Source 'local' supports_ddl=false — DDL nicht angewendet"
                ),
                None => {
                    ddl_applied = crate::ddl::try_apply_schema(&schema).await;
                }
            }
        }
        tracing::info!(
            target: "server::designer",
            "saveDbSchema empfangen: name='{name}', tables={table_count}, \
             relations={relation_count}, ddlApplied={ddl_applied}"
        );
        Ok(DbSchemaSaveResult {
            ok: true,
            message: format!(
                "Schema '{name}' empfangen ({table_count} Tabellen, {relation_count} Beziehungen, {ddl_applied} DDL-Statements)."
            ),
            table_count,
            relation_count,
        })
    }

    // -- Plugins (Phase 2) --

    /// Installiert oder ersetzt ein Plugin. Permissions: Admin-Wildcard.
    ///
    /// `wasmBase64` ist das WASM-Modul als Base64. `manifest` ist das
    /// `shared::plugin::PluginManifest` als JSON. Das Manifest wird gegen
    /// die Pflichtfelder validiert; die WASM-Binary wird einmal in Extism
    /// geladen, um sicherzustellen, dass sie valid ist.
    async fn install_plugin(
        &self,
        ctx: &Context<'_>,
        manifest: Json<serde_json::Value>,
        wasm_base64: String,
    ) -> async_graphql::Result<PluginView> {
        require_permission(ctx, "*", shared::PermissionOp::Update).await?;
        let parsed: shared::plugin::PluginManifest = serde_json::from_value(manifest.0)
            .map_err(|e| async_graphql::Error::new(format!("invalid_manifest: {e}")))?;
        use base64::Engine;
        let wasm = base64::engine::general_purpose::STANDARD
            .decode(&wasm_base64)
            .map_err(|e| async_graphql::Error::new(format!("invalid_wasm_base64: {e}")))?;
        let model = crate::plugins::install_plugin(parsed, wasm)
            .await
            .map_err(|e| async_graphql::Error::new(format!("{e}")))?;
        Ok(map_plugin(model))
    }

    async fn set_plugin_enabled(
        &self,
        ctx: &Context<'_>,
        id: String,
        enabled: bool,
    ) -> async_graphql::Result<bool> {
        require_permission(ctx, "*", shared::PermissionOp::Update).await?;
        crate::plugins::set_enabled(&id, enabled)
            .await
            .map_err(|e| async_graphql::Error::new(format!("{e}")))
    }

    async fn delete_plugin(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> async_graphql::Result<bool> {
        require_permission(ctx, "*", shared::PermissionOp::Delete).await?;
        crate::plugins::delete_plugin(&id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("{e}")))
    }

    // -- Implementations-Choice (Phase 1.5.3) --

    /// Setzt die Per-User-Wahl einer Implementations-ID. Permission-Gate:
    /// Op::Choose auf Resource::ImplementationId { registry, chosenId }
    /// (mit Fallback auf wildcard id="*" pro registry).
    async fn set_implementation_choice(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        property: String,
        registry: String,
        chosen_id: String,
    ) -> async_graphql::Result<bool> {
        let auth_ctx = ctx.data::<AuthContext>()?.clone();
        let me = auth_ctx
            .user
            .ok_or_else(|| async_graphql::Error::new("unauthenticated"))?;

        // Choose-Permission-Check: greift nur, wenn die neue Permission-Schicht
        // aktiv ist. In Legacy-Modus (permissions-Tabelle leer) erlauben wir
        // das Choose — der User kann zwischen IDs waehlen, die der Loader
        // ihm sowieso anbietet.
        if data::permissions_count().await > 0 {
            use shared::auth::{Effect, Resource};
            let exact = Resource::ImplementationId {
                registry: registry.clone(),
                id: chosen_id.clone(),
            };
            let wildcard = Resource::ImplementationId {
                registry: registry.clone(),
                id: "*".to_string(),
            };
            let allow_exact = matches!(
                crate::auth::resolver::effective(&me.id, &exact, shared::auth::Op::Choose).await,
                Ok(Effect::Allow)
            );
            let allow_wild = matches!(
                crate::auth::resolver::effective(&me.id, &wildcard, shared::auth::Op::Choose).await,
                Ok(Effect::Allow)
            );
            if !allow_exact && !allow_wild {
                crate::audit::record_deny(&me.id, &registry, "choose").await;
                return Err(async_graphql::Error::new("forbidden"));
            }
        }

        // Validierung: chosen_id muss in der `allowed_implementations`-Liste
        // sein. Sonst koennte ein Aufrufer eine willkuerliche ID persistieren.
        let allowed =
            data::allowed_implementations(&entity_type, &property, &registry).await;
        if !allowed.iter().any(|id| id == &chosen_id) {
            return Err(async_graphql::Error::new("not_allowed_for_property"));
        }

        data::set_user_implementation_choice(
            &me.id,
            &entity_type,
            &property,
            &registry,
            &chosen_id,
        )
        .await
        .map_err(|e| async_graphql::Error::new(format!("{e}")))?;
        Ok(true)
    }

    // -- Builder-Designs (Phase 1.6) --

    /// Append-only Save eines Builder-States.
    ///
    /// `expectedVersion`:
    /// - `None` ⇒ Aufrufer glaubt, die Tabelle ist fuer den entity_type
    ///   leer. Stimmt das nicht, ist es Konflikt.
    /// - `Some(n)` ⇒ Aufrufer glaubt, die aktuelle Version ist `n`. Server
    ///   bumpt dann auf `n + 1`.
    ///
    /// Bei Konflikt liefert das Result `ok=false`, `error="concurrent_design_modification"`
    /// und `conflictCurrent` mit dem aktuellen Server-Stand.
    async fn save_entity_design(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        schema_version: i32,
        state: Json<serde_json::Value>,
        expected_version: Option<i32>,
    ) -> async_graphql::Result<SaveEntityDesignResult> {
        let actor = require_permission(ctx, &entity_type, shared::PermissionOp::Update).await?;
        let state_json = state.0.to_string();
        match data::save_entity_design(
            &entity_type,
            schema_version,
            &state_json,
            expected_version,
            &actor.id,
        )
        .await
        {
            Ok(model) => Ok(SaveEntityDesignResult {
                ok: true,
                design: Some(map_entity_design(model)),
                conflict_current: None,
                error: None,
            }),
            Err(data::SaveDesignError::Conflict { current_version: _ }) => {
                let current = data::entity_design_active(&entity_type)
                    .await
                    .map(map_entity_design);
                Ok(SaveEntityDesignResult {
                    ok: false,
                    design: None,
                    conflict_current: current,
                    error: Some("concurrent_design_modification".to_string()),
                })
            }
            Err(data::SaveDesignError::Locked) => Ok(SaveEntityDesignResult {
                ok: false,
                design: None,
                conflict_current: data::entity_design_active(&entity_type)
                    .await
                    .map(map_entity_design),
                error: Some("locked".to_string()),
            }),
            Err(data::SaveDesignError::Db(e)) => {
                tracing::warn!(target: "server::designs", "save db error: {e}");
                Ok(SaveEntityDesignResult {
                    ok: false,
                    design: None,
                    conflict_current: None,
                    error: Some("internal".to_string()),
                })
            }
        }
    }

    /// Revert: schreibt eine ALTE Version als NEUE Version. Keine
    /// Loeschung — die Versions-Historie bleibt vollstaendig.
    async fn revert_entity_design(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        target_version: i32,
    ) -> async_graphql::Result<SaveEntityDesignResult> {
        let actor = require_permission(ctx, &entity_type, shared::PermissionOp::Update).await?;
        match data::revert_entity_design(&entity_type, target_version, &actor.id).await {
            Ok(model) => Ok(SaveEntityDesignResult {
                ok: true,
                design: Some(map_entity_design(model)),
                conflict_current: None,
                error: None,
            }),
            Err(data::SaveDesignError::Conflict { .. }) => Ok(SaveEntityDesignResult {
                ok: false,
                design: None,
                conflict_current: data::entity_design_active(&entity_type)
                    .await
                    .map(map_entity_design),
                error: Some("concurrent_design_modification".to_string()),
            }),
            Err(data::SaveDesignError::Locked) => Ok(SaveEntityDesignResult {
                ok: false,
                design: None,
                conflict_current: data::entity_design_active(&entity_type)
                    .await
                    .map(map_entity_design),
                error: Some("locked".to_string()),
            }),
            Err(data::SaveDesignError::Db(e)) => {
                tracing::warn!(target: "server::designs", "revert db error: {e}");
                Ok(SaveEntityDesignResult {
                    ok: false,
                    design: None,
                    conflict_current: None,
                    error: Some("internal".to_string()),
                })
            }
        }
    }

    // -- Q0005: Named Views --

    async fn save_entity_view(
        &self,
        ctx: &Context<'_>,
        input: SaveEntityViewInput,
    ) -> SaveEntityViewResult {
        use crate::AuthContext;
        let auth = ctx.data_opt::<AuthContext>();
        let Some(user) = auth.and_then(|a| a.user.as_ref()) else {
            return SaveEntityViewResult {
                kind: SaveEntityViewResultKind::Forbidden, view: None,
                message: Some("nicht authentifiziert".into()),
            };
        };
        if !shared::is_allowed(user, &crate::data::groups().await, &input.entity_type, shared::PermissionOp::Update) {
            return SaveEntityViewResult {
                kind: SaveEntityViewResultKind::Forbidden, view: None,
                message: Some(format!("kein Update-Recht auf '{}'", input.entity_type)),
            };
        }

        let layer: shared::view::ViewLayer = input.layer.into();
        let owner_id = input.owner_id.clone();
        let invariant_ok = (layer == shared::view::ViewLayer::Global && owner_id.is_none())
            || (layer != shared::view::ViewLayer::Global && owner_id.is_some());
        if !invariant_ok {
            return SaveEntityViewResult {
                kind: SaveEntityViewResultKind::Forbidden, view: None,
                message: Some("layer=global requires owner_id=null and vice versa".into()),
            };
        }

        // C3: Besitzer-Pruefung — verhindert fremde Layer-Schreibzugriffe.
        match layer {
            shared::view::ViewLayer::User => {
                let target = owner_id.as_deref();
                if target != Some(user.id.as_str())
                    && !shared::is_allowed(user, &crate::data::groups().await, "*", shared::PermissionOp::Update)
                {
                    return SaveEntityViewResult {
                        kind: SaveEntityViewResultKind::Forbidden, view: None,
                        message: Some("layer ownership mismatch".into()),
                    };
                }
            }
            shared::view::ViewLayer::Group => {
                let target = owner_id.as_deref();
                let is_member = target.map(|g| user.group_ids.iter().any(|x| x == g)).unwrap_or(false);
                if !is_member
                    && !shared::is_allowed(user, &crate::data::groups().await, "*", shared::PermissionOp::Update)
                {
                    return SaveEntityViewResult {
                        kind: SaveEntityViewResultKind::Forbidden, view: None,
                        message: Some("layer ownership mismatch".into()),
                    };
                }
            }
            shared::view::ViewLayer::Global => {} // bereits durch entity Update-Recht abgedeckt
        }

        let existing = crate::data::find_entity_view(
            &input.entity_type, &input.view_name, layer, owner_id.as_deref()
        ).await.unwrap_or(None);
        if let (Some(curr), Some(expected)) = (existing.as_ref(), input.expected_version) {
            if curr.version != expected {
                return SaveEntityViewResult {
                    kind: SaveEntityViewResultKind::Conflict,
                    view: Some(curr.clone().into()),
                    message: Some(format!("expected_version={expected}, current={}", curr.version)),
                };
            }
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PayloadIn {
            properties: Vec<shared::view::ViewPropertyOverride>,
            #[serde(default)] default_filter:    Option<shared::FilterCriteria>,
            #[serde(default)] default_sort:      Option<shared::Sort>,
            #[serde(default)] default_page_size: Option<u32>,
        }
        let p: PayloadIn = match serde_json::from_value(input.payload.0) {
            Ok(p)  => p,
            Err(e) => return SaveEntityViewResult {
                kind: SaveEntityViewResultKind::Forbidden, view: None,
                message: Some(format!("payload-parse: {e}")),
            },
        };

        let new_version = existing.as_ref().map(|v| v.version + 1).unwrap_or(1);
        let id = existing.as_ref().map(|v| v.id.clone())
            .unwrap_or_else(|| format!("v-{}", uuid::Uuid::new_v4()));
        let v = shared::view::EntityView {
            id,
            entity_type: input.entity_type.clone(),
            view_name:   input.view_name.clone(),
            layer,
            owner_id,
            properties:        p.properties,
            default_filter:    p.default_filter,
            default_sort:      p.default_sort,
            default_page_size: p.default_page_size,
            version: new_version,
            updated_at: chrono::Utc::now().to_rfc3339(),
            updated_by: Some(user.id.clone()),
        };
        if let Err(e) = crate::data::upsert_entity_view(&v).await {
            return SaveEntityViewResult {
                kind: SaveEntityViewResultKind::Forbidden, view: None,
                message: Some(format!("save: {e}")),
            };
        }
        SaveEntityViewResult {
            kind: SaveEntityViewResultKind::Ok,
            view: Some(v.into()),
            message: None,
        }
    }

    async fn revert_entity_view(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        view_name:   String,
        layer:       GqlViewLayer,
        owner_id:    Option<String>,
    ) -> RevertEntityViewResult {
        use crate::AuthContext;
        let auth = ctx.data_opt::<AuthContext>();
        let Some(user) = auth.and_then(|a| a.user.as_ref()) else {
            return RevertEntityViewResult { ok: false, message: Some("unauthenticated".into()) };
        };
        if !shared::is_allowed(user, &crate::data::groups().await, &entity_type, shared::PermissionOp::Update) {
            return RevertEntityViewResult { ok: false, message: Some("forbidden".into()) };
        }

        // C3: Besitzer-Pruefung — verhindert fremde Layer-Schreibzugriffe.
        let layer_typed: shared::view::ViewLayer = layer.into();
        match layer_typed {
            shared::view::ViewLayer::User => {
                let target = owner_id.as_deref();
                if target != Some(user.id.as_str())
                    && !shared::is_allowed(user, &crate::data::groups().await, "*", shared::PermissionOp::Update)
                {
                    return RevertEntityViewResult { ok: false, message: Some("layer ownership mismatch".into()) };
                }
            }
            shared::view::ViewLayer::Group => {
                let target = owner_id.as_deref();
                let is_member = target.map(|g| user.group_ids.iter().any(|x| x == g)).unwrap_or(false);
                if !is_member
                    && !shared::is_allowed(user, &crate::data::groups().await, "*", shared::PermissionOp::Update)
                {
                    return RevertEntityViewResult { ok: false, message: Some("layer ownership mismatch".into()) };
                }
            }
            shared::view::ViewLayer::Global => {} // bereits durch entity Update-Recht abgedeckt
        }

        match crate::data::delete_entity_view(&entity_type, &view_name, layer_typed, owner_id.as_deref()).await {
            Ok(_)  => RevertEntityViewResult { ok: true, message: None },
            Err(e) => RevertEntityViewResult { ok: false, message: Some(e) },
        }
    }

    // -- Entity-CRUD --

    async fn create_entity(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        id: Option<String>,
        fields: Json<serde_json::Value>,
    ) -> async_graphql::Result<EntityChangeResult> {
        let actor = require_permission(ctx, &entity_type, shared::PermissionOp::Create).await?;
        let fields_map = fields.0.as_object().cloned().unwrap_or_default();
        let validation = data::validate_against_editor(&entity_type, &fields_map);
        if validation.has_blocking() {
            return Ok(EntityChangeResult {
                ok: false,
                entity: None,
                validation: Json(serde_json::to_value(&validation).unwrap()),
            });
        }
        // Phase 2.3: beforeSave-Trigger — Plugins koennen Felder mutieren oder
        // den Save mit Validation-Fehlern abbrechen.
        let fields_map = match crate::plugins::run_before_save(
            &entity_type,
            None,
            fields_map,
            &actor.id,
        )
        .await
        {
            Ok(updated) => updated,
            Err(errors) => {
                return Ok(EntityChangeResult {
                    ok: false,
                    entity: None,
                    validation: Json(plugin_errors_to_validation(&errors)),
                });
            }
        };
        let entity = data::create_entity(
            &entity_type,
            id,
            serde_json::Value::Object(fields_map.clone()),
            Some(&actor.id),
        )
        .await;
        // afterSave fire-and-forget (Audit-Log haelt die Spur).
        crate::plugins::run_after_save(
            &entity_type,
            &entity.id,
            &fields_map,
            "create",
            &actor.id,
        )
        .await;
        Ok(EntityChangeResult {
            ok: true,
            entity: Some(entity),
            validation: Json(serde_json::to_value(&validation).unwrap()),
        })
    }

    async fn update_entity(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        id: String,
        fields: Json<serde_json::Value>,
        expected_hash: Option<String>,
    ) -> async_graphql::Result<EntityChangeResult> {
        let actor = require_permission(ctx, &entity_type, shared::PermissionOp::Update).await?;
        let expected_hash_u64 = expected_hash.as_deref().and_then(|s| s.parse::<u64>().ok());
        if let Some(expected) = expected_hash_u64 {
            if let Some(current_hash) = data::current_hash(&entity_type, &id).await {
                if current_hash != expected {
                    return Ok(EntityChangeResult {
                        ok: false,
                        entity: data::entity_by_id(&entity_type, &id).await,
                        validation: Json(serde_json::json!({
                            "messages": [{
                                "severity": "error",
                                "messageKey": "error.concurrent_modification",
                                "target": null,
                                "args": { "id": id }
                            }]
                        })),
                    });
                }
            }
        }
        let merged = data::merged_fields(&entity_type, &id, &fields.0).await;
        let validation = data::validate_against_editor(&entity_type, &merged);
        if validation.has_blocking() {
            return Ok(EntityChangeResult {
                ok: false,
                entity: data::entity_by_id(&entity_type, &id).await,
                validation: Json(serde_json::to_value(&validation).unwrap()),
            });
        }
        // Phase 2.3: beforeSave mit fields_before = aktueller DB-Stand, falls
        // vorhanden. Plugins koennen `merged` mutieren oder ablehnen.
        let fields_before = data::entity_by_id(&entity_type, &id)
            .await
            .and_then(|e| e.fields.0.as_object().cloned());
        let merged = match crate::plugins::run_before_save(
            &entity_type,
            fields_before.as_ref(),
            merged,
            &actor.id,
        )
        .await
        {
            Ok(updated) => updated,
            Err(errors) => {
                return Ok(EntityChangeResult {
                    ok: false,
                    entity: data::entity_by_id(&entity_type, &id).await,
                    validation: Json(plugin_errors_to_validation(&errors)),
                });
            }
        };
        match data::update_entity(
            &entity_type,
            &id,
            serde_json::Value::Object(merged.clone()),
            Some(&actor.id),
        )
        .await
        {
            Some(entity) => {
                crate::plugins::run_after_save(
                    &entity_type,
                    &entity.id,
                    &merged,
                    "update",
                    &actor.id,
                )
                .await;
                Ok(EntityChangeResult {
                    ok: true,
                    entity: Some(entity),
                    validation: Json(serde_json::to_value(&validation).unwrap()),
                })
            }
            None => Ok(EntityChangeResult {
                ok: false,
                entity: None,
                validation: Json(serde_json::json!({
                    "messages": [{
                        "severity": "error",
                        "messageKey": "error.invalid_identifier",
                        "target": null,
                        "args": { "id": id }
                    }]
                })),
            }),
        }
    }

    // ---- Bulk-Varianten ----
    //
    // Pragmatisch implementiert als Schleife ueber die Singular-Resolver:
    // SQLite-Transaktionen kommen erst mit DB-Stack (Task #52). Bis dahin
    // ist "single transaction" semantisch identisch mit "alle nacheinander",
    // weil der In-Memory-Store unter dem Mutex-Lock atomar ist.

    async fn create_entities(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        items: Vec<Json<serde_json::Value>>,
    ) -> async_graphql::Result<Vec<EntityChangeResult>> {
        let actor = require_permission(ctx, &entity_type, shared::PermissionOp::Create).await?;
        let mut out = Vec::with_capacity(items.len());
        for fields in items {
            let fields_map = fields.0.as_object().cloned().unwrap_or_default();
            let validation = data::validate_against_editor(&entity_type, &fields_map);
            if validation.has_blocking() {
                out.push(EntityChangeResult {
                    ok: false,
                    entity: None,
                    validation: Json(serde_json::to_value(&validation).unwrap()),
                });
                continue;
            }
            let entity = data::create_entity(&entity_type, None, fields.0, Some(&actor.id)).await;
            out.push(EntityChangeResult {
                ok: true,
                entity: Some(entity),
                validation: Json(serde_json::to_value(&validation).unwrap()),
            });
        }
        Ok(out)
    }

    async fn delete_entities(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        ids: Vec<String>,
    ) -> async_graphql::Result<Vec<EntityChangeResult>> {
        require_permission(ctx, &entity_type, shared::PermissionOp::Delete).await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let ok = data::delete_entity(&entity_type, &id).await;
            out.push(EntityChangeResult {
                ok,
                entity: None,
                validation: Json(if ok {
                    serde_json::json!({"messages": []})
                } else {
                    serde_json::json!({
                        "messages": [{
                            "severity": "error",
                            "messageKey": "error.invalid_identifier",
                            "target": null,
                            "args": { "id": id }
                        }]
                    })
                }),
            });
        }
        Ok(out)
    }

    async fn delete_entity(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        id: String,
        expected_hash: Option<String>,
    ) -> async_graphql::Result<EntityChangeResult> {
        require_permission(ctx, &entity_type, shared::PermissionOp::Delete).await?;
        let expected_hash_u64 = expected_hash.as_deref().and_then(|s| s.parse::<u64>().ok());
        if let Some(expected) = expected_hash_u64 {
            if let Some(current_hash) = data::current_hash(&entity_type, &id).await {
                if current_hash != expected {
                    return Ok(EntityChangeResult {
                        ok: false,
                        entity: data::entity_by_id(&entity_type, &id).await,
                        validation: Json(serde_json::json!({
                            "messages": [{
                                "severity": "error",
                                "messageKey": "error.concurrent_modification",
                                "target": null,
                                "args": { "id": id }
                            }]
                        })),
                    });
                }
            }
        }
        // Phase 2.3: beforeDelete-Trigger — Plugins koennen ablehnen.
        let fields_before = data::entity_by_id(&entity_type, &id)
            .await
            .and_then(|e| e.fields.0.as_object().cloned())
            .unwrap_or_default();
        let actor_id = ctx
            .data::<AuthContext>()?
            .user
            .as_ref()
            .map(|u| u.id.clone())
            .unwrap_or_default();
        if let Err(errors) =
            crate::plugins::run_before_delete(&entity_type, &id, &fields_before, &actor_id).await
        {
            return Ok(EntityChangeResult {
                ok: false,
                entity: data::entity_by_id(&entity_type, &id).await,
                validation: Json(plugin_errors_to_validation(&errors)),
            });
        }

        let ok = data::delete_entity(&entity_type, &id).await;
        Ok(EntityChangeResult {
            ok,
            entity: None,
            validation: Json(if ok {
                serde_json::json!({"messages": []})
            } else {
                serde_json::json!({
                    "messages": [{
                        "severity": "error",
                        "messageKey": "error.invalid_identifier",
                        "target": null,
                        "args": { "id": id }
                    }]
                })
            }),
        })
    }
}

// =============================================================================
// Q0005 — Named Views
// =============================================================================

#[derive(async_graphql::Enum, Copy, Clone, PartialEq, Eq, Debug)]
#[graphql(name = "ViewLayer")]
pub enum GqlViewLayer { Global, Group, User }

impl From<GqlViewLayer> for shared::view::ViewLayer {
    fn from(v: GqlViewLayer) -> Self {
        match v {
            GqlViewLayer::Global => shared::view::ViewLayer::Global,
            GqlViewLayer::Group  => shared::view::ViewLayer::Group,
            GqlViewLayer::User   => shared::view::ViewLayer::User,
        }
    }
}
impl From<shared::view::ViewLayer> for GqlViewLayer {
    fn from(v: shared::view::ViewLayer) -> Self {
        match v {
            shared::view::ViewLayer::Global => GqlViewLayer::Global,
            shared::view::ViewLayer::Group  => GqlViewLayer::Group,
            shared::view::ViewLayer::User   => GqlViewLayer::User,
        }
    }
}

#[derive(async_graphql::SimpleObject, Clone)]
pub struct EntityViewGql {
    pub id:                String,
    pub entity_type:       String,
    pub view_name:         String,
    pub layer:             GqlViewLayer,
    pub owner_id:          Option<String>,
    /// Sparse Property-Overrides als JSON-Blob — analog zu ColumnMeta.fieldType.
    pub properties:        async_graphql::Json<serde_json::Value>,
    pub default_filter:    Option<async_graphql::Json<serde_json::Value>>,
    pub default_sort:      Option<async_graphql::Json<serde_json::Value>>,
    pub default_page_size: Option<u32>,
    pub version:           i32,
    pub updated_at:        String,
    pub updated_by:        Option<String>,
}

impl From<shared::view::EntityView> for EntityViewGql {
    fn from(v: shared::view::EntityView) -> Self {
        EntityViewGql {
            id:                v.id,
            entity_type:       v.entity_type,
            view_name:         v.view_name,
            layer:             v.layer.into(),
            owner_id:          v.owner_id,
            properties:        async_graphql::Json(serde_json::to_value(v.properties).unwrap_or(serde_json::Value::Null)),
            default_filter:    v.default_filter.map(|f| async_graphql::Json(serde_json::to_value(f).unwrap_or(serde_json::Value::Null))),
            default_sort:      v.default_sort.map(|s| async_graphql::Json(serde_json::to_value(s).unwrap_or(serde_json::Value::Null))),
            default_page_size: v.default_page_size,
            version:           v.version,
            updated_at:        v.updated_at,
            updated_by:        v.updated_by,
        }
    }
}

#[derive(async_graphql::SimpleObject)]
pub struct EntityViewSummaryGql {
    pub view_name:  String,
    pub layers:     Vec<GqlViewLayer>,
    pub updated_at: String,
}

#[derive(async_graphql::InputObject)]
pub struct SaveEntityViewInput {
    pub entity_type:      String,
    pub view_name:        String,
    pub layer:            GqlViewLayer,
    pub owner_id:         Option<String>,
    /// JSON-Blob `{ properties, defaultFilter, defaultSort, defaultPageSize }`
    pub payload:          async_graphql::Json<serde_json::Value>,
    pub expected_version: Option<i32>,
}

#[derive(async_graphql::Enum, Copy, Clone, PartialEq, Eq, Debug)]
pub enum SaveEntityViewResultKind { Ok, Conflict, Forbidden }

#[derive(async_graphql::SimpleObject)]
pub struct SaveEntityViewResult {
    pub kind:    SaveEntityViewResultKind,
    pub view:    Option<EntityViewGql>,
    pub message: Option<String>,
}

#[derive(async_graphql::SimpleObject)]
pub struct RevertEntityViewResult {
    pub ok:      bool,
    pub message: Option<String>,
}

/// Hilfsfunktion: Plugin-Validation-Fehler in das ValidationResult-Wire-
/// Format konvertieren, das EntityChangeResult erwartet.
fn plugin_errors_to_validation(
    errors: &[shared::plugin::ValidationErrorFromPlugin],
) -> serde_json::Value {
    let messages: Vec<serde_json::Value> = errors
        .iter()
        .map(|e| {
            serde_json::json!({
                "severity": "error",
                "messageKey": e.message.clone().unwrap_or_else(|| format!("plugin.{}", e.code)),
                "target": e.field,
                "args": { "code": e.code }
            })
        })
        .collect();
    serde_json::json!({ "messages": messages })
}
