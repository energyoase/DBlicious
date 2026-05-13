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
/// darf. `async`, weil `data::groups()` jetzt die DB anfragt.
async fn require_permission(
    ctx: &Context<'_>,
    entity_type: &str,
    op: shared::PermissionOp,
) -> async_graphql::Result<shared::SecurityUser> {
    let auth = ctx.data::<AuthContext>()?.clone();
    let user = auth.user.ok_or_else(|| async_graphql::Error::new("unauthenticated"))?;
    let groups = data::groups().await;
    if !shared::is_allowed(&user, &groups, entity_type, op) {
        return Err(async_graphql::Error::new("forbidden"));
    }
    Ok(user)
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
        let _ = (sort_by, sort_dir, filter);
        let groups = data::groups().await;
        let mut page = data::entities_page(&entity_type, page, page_size).await;
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
        Ok(data::settings_for_async(&entity_type).await.map(map_settings))
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
            ddl_applied = crate::ddl::try_apply_schema(&schema).await;
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
        let entity = data::create_entity(&entity_type, id, fields.0, Some(&actor.id)).await;
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
        match data::update_entity(&entity_type, &id, fields.0, Some(&actor.id)).await {
            Some(entity) => Ok(EntityChangeResult {
                ok: true,
                entity: Some(entity),
                validation: Json(serde_json::to_value(&validation).unwrap()),
            }),
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
