//! Konkrete GraphQL-Queries.
//!
//! Die Antwortstrukturen verwenden, wo immer moeglich, die Typen aus
//! `shared`. So bleibt der Vertrag mit dem Server an einer Stelle definiert.

use serde::{Deserialize, Serialize};
use shared::{
    AuthSession, ColumnMeta, DbSchema, DbSchemaSaveResult, EditorMeta, Entity, EntityChangeResult,
    EntitySettings, FieldType, NavigationNode, Permission, SecurityUser, TranslatableBundle,
};

use super::{execute, GqlError};

const NAVIGATION_QUERY: &str = r#"
    query Navigation {
        navigation {
            id
            labelKey
            route
            icon
            children {
                id
                labelKey
                route
                icon
                children {
                    id
                    labelKey
                    route
                    icon
                    children {
                        id
                        labelKey
                        route
                        icon
                        children { id labelKey route icon }
                    }
                }
            }
        }
    }
"#;

#[derive(Deserialize)]
struct NavigationData {
    navigation: Vec<NavigationNode>,
}

#[derive(Serialize)]
struct EmptyVars {}

pub async fn fetch_navigation() -> Result<Vec<NavigationNode>, GqlError> {
    let data: NavigationData = execute(NAVIGATION_QUERY, EmptyVars {}).await?;
    Ok(data.navigation)
}

const COLUMNS_QUERY: &str = r#"
    query Columns($entityType: String!) {
        entityColumns(entityType: $entityType) {
            key
            labelKey
            fieldType
            sortable
            filterable
        }
    }
"#;

/// Server-Variante der Spalten-Metadaten – `field_type` kommt als rohes JSON
/// und wird hier in den `FieldType`-Enum aus `shared` ueberfuehrt.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawColumnMeta {
    key: String,
    label_key: String,
    field_type: serde_json::Value,
    sortable: bool,
    filterable: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ColumnsData {
    entity_columns: Vec<RawColumnMeta>,
}

#[derive(Serialize)]
struct EntityTypeVars<'a> {
    #[serde(rename = "entityType")]
    entity_type: &'a str,
}

pub async fn fetch_columns(entity_type: &str) -> Result<Vec<ColumnMeta>, GqlError> {
    let data: ColumnsData = execute(COLUMNS_QUERY, EntityTypeVars { entity_type }).await?;
    let columns = data
        .entity_columns
        .into_iter()
        .map(|c| {
            let field_type: FieldType =
                serde_json::from_value(c.field_type).unwrap_or(FieldType::Text);
            ColumnMeta {
                key: c.key,
                label_key: c.label_key,
                field_type,
                sortable: c.sortable,
                filterable: c.filterable,
                comparator_id: None,
                filter_id: None,
            }
        })
        .collect();
    Ok(columns)
}

const ENTITIES_QUERY: &str = r#"
    query Entities($entityType: String!, $page: Int!, $pageSize: Int!) {
        entities(entityType: $entityType, page: $page, pageSize: $pageSize) {
            items { id fields }
            totalCount
            page
            pageSize
        }
    }
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntitiesVars<'a> {
    entity_type: &'a str,
    page: i32,
    page_size: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntitiesData {
    entities: ServerEntityPage,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerEntityPage {
    items: Vec<ServerEntity>,
    total_count: i64,
    page: i32,
    page_size: i32,
}

#[derive(Deserialize)]
struct ServerEntity {
    id: String,
    fields: serde_json::Value,
}

pub struct EntityPageResult {
    pub items: Vec<Entity>,
    pub total_count: u64,
    pub page: u32,
    pub page_size: u32,
}

pub async fn fetch_entities(
    entity_type: &str,
    page: i32,
    page_size: i32,
) -> Result<EntityPageResult, GqlError> {
    let data: EntitiesData = execute(
        ENTITIES_QUERY,
        EntitiesVars { entity_type, page, page_size },
    )
    .await?;
    let items = data
        .entities
        .items
        .into_iter()
        .map(|e| {
            let fields = match e.fields {
                serde_json::Value::Object(map) => map,
                _ => serde_json::Map::new(),
            };
            Entity { id: e.id, fields }
        })
        .collect();
    Ok(EntityPageResult {
        items,
        total_count: data.entities.total_count.max(0) as u64,
        page: data.entities.page.max(0) as u32,
        page_size: data.entities.page_size.max(0) as u32,
    })
}

// =============================================================================
// Designer-Mutation
// =============================================================================

const SAVE_DB_SCHEMA_MUTATION: &str = r#"
    mutation SaveDbSchema($schema: JSON!) {
        saveDbSchema(schema: $schema) {
            ok
            message
            tableCount
            relationCount
        }
    }
"#;

#[derive(Serialize)]
struct SaveDbSchemaVars {
    schema: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveDbSchemaData {
    save_db_schema: ServerSaveResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerSaveResult {
    ok: bool,
    message: String,
    table_count: i64,
    relation_count: i64,
}

/// Schickt den aktuellen Designer-Stand an den Server.
///
/// Der Server validiert/speichert in dieser Ausbaustufe nichts – er
/// quittiert lediglich den Empfang. Trotzdem wird der Vertrag bereits
/// ueber `shared::DbSchema` typisiert, damit zukuenftige Iterationen
/// ohne API-Aenderung scharfgeschaltet werden koennen.
pub async fn save_db_schema(schema: &DbSchema) -> Result<DbSchemaSaveResult, GqlError> {
    let payload = serde_json::to_value(schema)
        .map_err(|e| GqlError::Decode(e.to_string()))?;
    let data: SaveDbSchemaData =
        execute(SAVE_DB_SCHEMA_MUTATION, SaveDbSchemaVars { schema: payload }).await?;
    Ok(DbSchemaSaveResult {
        ok: data.save_db_schema.ok,
        message: data.save_db_schema.message,
        table_count: data.save_db_schema.table_count.max(0) as u32,
        relation_count: data.save_db_schema.relation_count.max(0) as u32,
    })
}

// =============================================================================
// Translatable / Editor / Settings
// =============================================================================

const TRANSLATABLE_QUERY: &str = r#"
    query Translatable {
        translatable {
            languages { id code nameKey fallbackId active }
            entries   { id category description }
            values    { entryId languageId ftlSource updatedAt }
        }
    }
"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranslatableData {
    translatable: TranslatableBundle,
}

pub async fn fetch_translatable() -> Result<TranslatableBundle, GqlError> {
    let data: TranslatableData = execute(TRANSLATABLE_QUERY, EmptyVars {}).await?;
    Ok(data.translatable)
}

const EDITOR_QUERY: &str = r#"
    query Editor($entityType: String!) {
        entityEditor(entityType: $entityType) {
            entityType
            properties {
                key labelKey fieldType required readonly visibility
                order helpKey placeholderKey groupKey control
                minLength maxLength min max pattern
            }
        }
    }
"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEditorProperty {
    key: String,
    label_key: String,
    field_type: serde_json::Value,
    required: bool,
    readonly: bool,
    visibility: String,
    order: i32,
    help_key: Option<String>,
    placeholder_key: Option<String>,
    group_key: Option<String>,
    control: String,
    #[serde(default)]
    min_length: Option<u32>,
    #[serde(default)]
    max_length: Option<u32>,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    pattern: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEditor {
    entity_type: String,
    properties: Vec<RawEditorProperty>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditorData {
    entity_editor: Option<RawEditor>,
}

pub async fn fetch_editor(entity_type: &str) -> Result<Option<EditorMeta>, GqlError> {
    let data: EditorData = execute(EDITOR_QUERY, EntityTypeVars { entity_type }).await?;
    Ok(data.entity_editor.map(|raw| EditorMeta {
        entity_type: raw.entity_type,
        properties: raw
            .properties
            .into_iter()
            .map(|p| shared::EditorPropertyMeta {
                key: p.key,
                label_key: p.label_key,
                field_type: serde_json::from_value(p.field_type).unwrap_or(FieldType::Text),
                required: p.required,
                readonly: p.readonly,
                visibility: parse_visibility(&p.visibility),
                order: p.order,
                help_key: p.help_key,
                placeholder_key: p.placeholder_key,
                group_key: p.group_key,
                control: parse_control(&p.control),
                min_length: p.min_length,
                max_length: p.max_length,
                min: p.min,
                max: p.max,
                pattern: p.pattern,
            })
            .collect(),
    }))
}

fn parse_visibility(s: &str) -> shared::Visibility {
    match s {
        "hidden" => shared::Visibility::Hidden,
        "readOnly" => shared::Visibility::ReadOnly,
        "detailOnly" => shared::Visibility::DetailOnly,
        _ => shared::Visibility::Visible,
    }
}

fn parse_control(s: &str) -> shared::ControlKind {
    match s {
        "input" => shared::ControlKind::Input,
        "textArea" => shared::ControlKind::TextArea,
        "select" => shared::ControlKind::Select,
        "datePicker" => shared::ControlKind::DatePicker,
        "lookup" => shared::ControlKind::Lookup,
        "inlineList" => shared::ControlKind::InlineList,
        "toggle" => shared::ControlKind::Toggle,
        _ => shared::ControlKind::Auto,
    }
}

const SETTINGS_QUERY: &str = r#"
    query Settings($entityType: String!) {
        entitySettings(entityType: $entityType) {
            entityType
            access
            defaultPageSize
            defaultSort
            defaultFilter
            properties {
                key visibility access loadMethod order labelOverrideKey minWidth
            }
        }
    }
"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPropertySettings {
    key: String,
    visibility: String,
    access: String,
    load_method: String,
    order: i32,
    label_override_key: Option<String>,
    min_width: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSettings {
    entity_type: String,
    access: String,
    default_page_size: Option<i32>,
    default_sort: Option<serde_json::Value>,
    default_filter: Option<serde_json::Value>,
    properties: Vec<RawPropertySettings>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsData {
    entity_settings: Option<RawSettings>,
}

pub async fn fetch_settings(entity_type: &str) -> Result<Option<EntitySettings>, GqlError> {
    let data: SettingsData = execute(SETTINGS_QUERY, EntityTypeVars { entity_type }).await?;
    Ok(data.entity_settings.map(|raw| EntitySettings {
        entity_type: raw.entity_type,
        access: parse_access(&raw.access),
        default_page_size: raw.default_page_size.map(|p| p.max(0) as u32),
        default_sort: raw.default_sort.and_then(|v| serde_json::from_value(v).ok()),
        default_filter: raw.default_filter.and_then(|v| serde_json::from_value(v).ok()),
        properties: raw
            .properties
            .into_iter()
            .map(|p| shared::PropertySettings {
                key: p.key,
                visibility: parse_visibility(&p.visibility),
                access: parse_property_access(&p.access),
                load_method: parse_load_method(&p.load_method),
                order: p.order,
                label_override_key: p.label_override_key,
                min_width: p.min_width.and_then(|w| if w < 0 { None } else { Some(w as u32) }),
            })
            .collect(),
    }))
}

fn parse_access(s: &str) -> shared::Access {
    match s {
        "internal" => shared::Access::Internal,
        "protected" => shared::Access::Protected,
        "admin" => shared::Access::Admin,
        _ => shared::Access::Public,
    }
}

fn parse_property_access(s: &str) -> shared::PropertyAccess {
    match s {
        "readOnly" => shared::PropertyAccess::ReadOnly,
        "writeOnly" => shared::PropertyAccess::WriteOnly,
        "none" => shared::PropertyAccess::None,
        _ => shared::PropertyAccess::ReadWrite,
    }
}

fn parse_load_method(s: &str) -> shared::LoadMethod {
    match s {
        "lazy" => shared::LoadMethod::Lazy,
        "manual" => shared::LoadMethod::Manual,
        _ => shared::LoadMethod::Eager,
    }
}

// =============================================================================
// Entity-Mutationen
// =============================================================================

const CREATE_ENTITY_MUTATION: &str = r#"
    mutation CreateEntity($entityType: String!, $id: String, $fields: JSON!) {
        createEntity(entityType: $entityType, id: $id, fields: $fields) {
            ok
            entity { id fields }
            validation
        }
    }
"#;

const UPDATE_ENTITY_MUTATION: &str = r#"
    mutation UpdateEntity($entityType: String!, $id: String!, $fields: JSON!, $expectedHash: String) {
        updateEntity(entityType: $entityType, id: $id, fields: $fields, expectedHash: $expectedHash) {
            ok
            entity { id fields }
            validation
        }
    }
"#;

const DELETE_ENTITY_MUTATION: &str = r#"
    mutation DeleteEntity($entityType: String!, $id: String!, $expectedHash: String) {
        deleteEntity(entityType: $entityType, id: $id, expectedHash: $expectedHash) {
            ok
            validation
        }
    }
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateEntityVars<'a> {
    entity_type: &'a str,
    id: Option<&'a str>,
    fields: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateEntityVars<'a> {
    entity_type: &'a str,
    id: &'a str,
    fields: serde_json::Value,
    expected_hash: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEntityVars<'a> {
    entity_type: &'a str,
    id: &'a str,
    expected_hash: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawChangeResult {
    ok: bool,
    #[serde(default)]
    entity: Option<ServerEntity>,
    validation: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateEntityData {
    create_entity: RawChangeResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateEntityData {
    update_entity: RawChangeResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEntityData {
    delete_entity: RawChangeResult,
}

fn map_change_result(raw: RawChangeResult) -> EntityChangeResult {
    let entity = raw.entity.map(|e| {
        let fields = match e.fields {
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        Entity { id: e.id, fields }
    });
    let validation = serde_json::from_value(raw.validation).unwrap_or_default();
    EntityChangeResult { ok: raw.ok, entity, validation }
}

pub async fn create_entity(
    entity_type: &str,
    id: Option<&str>,
    fields: serde_json::Map<String, serde_json::Value>,
) -> Result<EntityChangeResult, GqlError> {
    let data: CreateEntityData = execute(
        CREATE_ENTITY_MUTATION,
        CreateEntityVars {
            entity_type,
            id,
            fields: serde_json::Value::Object(fields),
        },
    )
    .await?;
    Ok(map_change_result(data.create_entity))
}

pub async fn update_entity(
    entity_type: &str,
    id: &str,
    fields: serde_json::Map<String, serde_json::Value>,
    expected_hash: Option<u64>,
) -> Result<EntityChangeResult, GqlError> {
    let data: UpdateEntityData = execute(
        UPDATE_ENTITY_MUTATION,
        UpdateEntityVars {
            entity_type,
            id,
            fields: serde_json::Value::Object(fields),
            expected_hash: expected_hash.map(|h| h.to_string()),
        },
    )
    .await?;
    Ok(map_change_result(data.update_entity))
}

pub async fn delete_entity(
    entity_type: &str,
    id: &str,
    expected_hash: Option<u64>,
) -> Result<EntityChangeResult, GqlError> {
    let data: DeleteEntityData = execute(
        DELETE_ENTITY_MUTATION,
        DeleteEntityVars {
            entity_type,
            id,
            expected_hash: expected_hash.map(|h| h.to_string()),
        },
    )
    .await?;
    Ok(map_change_result(data.delete_entity))
}

// ---- Bulk-Varianten ----

const CREATE_ENTITIES_MUTATION: &str = r#"
    mutation CreateEntities($entityType: String!, $items: [JSON!]!) {
        createEntities(entityType: $entityType, items: $items) {
            ok
            entity { id fields }
            validation
        }
    }
"#;

const DELETE_ENTITIES_MUTATION: &str = r#"
    mutation DeleteEntities($entityType: String!, $ids: [String!]!) {
        deleteEntities(entityType: $entityType, ids: $ids) {
            ok
            validation
        }
    }
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateEntitiesVars<'a> {
    entity_type: &'a str,
    items: Vec<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEntitiesVars<'a> {
    entity_type: &'a str,
    ids: &'a [String],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateEntitiesData {
    create_entities: Vec<RawChangeResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteEntitiesData {
    delete_entities: Vec<RawChangeResult>,
}

pub async fn create_entities(
    entity_type: &str,
    items: Vec<serde_json::Map<String, serde_json::Value>>,
) -> Result<Vec<EntityChangeResult>, GqlError> {
    let payload: Vec<serde_json::Value> =
        items.into_iter().map(serde_json::Value::Object).collect();
    let data: CreateEntitiesData = execute(
        CREATE_ENTITIES_MUTATION,
        CreateEntitiesVars { entity_type, items: payload },
    )
    .await?;
    Ok(data.create_entities.into_iter().map(map_change_result).collect())
}

pub async fn delete_entities(
    entity_type: &str,
    ids: &[String],
) -> Result<Vec<EntityChangeResult>, GqlError> {
    let data: DeleteEntitiesData = execute(
        DELETE_ENTITIES_MUTATION,
        DeleteEntitiesVars { entity_type, ids },
    )
    .await?;
    Ok(data.delete_entities.into_iter().map(map_change_result).collect())
}

// =============================================================================
// Auth
// =============================================================================

const LOGIN_MUTATION: &str = r#"
    mutation Login($username: String!, $password: String!) {
        login(username: $username, password: $password) {
            ok
            error
            session {
                token
                user { id username displayName locale groupIds active }
                permissions {
                    entityType canRead canCreate canUpdate canDelete minAccess
                    propertyOverrides { property access }
                }
                expiresAt
            }
        }
    }
"#;

const LOGOUT_MUTATION: &str = r#"
    mutation Logout { logout }
"#;

const CURRENT_USER_QUERY: &str = r#"
    query CurrentUser {
        currentUser { id username displayName locale groupIds active }
        currentPermissions {
            entityType canRead canCreate canUpdate canDelete minAccess
            propertyOverrides { property access }
        }
    }
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginVars<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPropertyPermission {
    property: String,
    access: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPermission {
    entity_type: String,
    can_read: bool,
    can_create: bool,
    can_update: bool,
    can_delete: bool,
    min_access: String,
    #[serde(default)]
    property_overrides: Vec<RawPropertyPermission>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAuthSession {
    token: String,
    user: SecurityUser,
    permissions: Vec<RawPermission>,
    expires_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginResultRaw {
    ok: bool,
    error: Option<String>,
    session: Option<RawAuthSession>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginData {
    login: LoginResultRaw,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogoutData {
    logout: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurrentUserData {
    current_user: Option<SecurityUser>,
    current_permissions: Vec<RawPermission>,
}

pub enum LoginOutcome {
    Success(AuthSession),
    Failed(String),
}

fn map_permission(r: RawPermission) -> Permission {
    Permission {
        entity_type: r.entity_type,
        can_read: r.can_read,
        can_create: r.can_create,
        can_update: r.can_update,
        can_delete: r.can_delete,
        min_access: match r.min_access.as_str() {
            "internal" => shared::Access::Internal,
            "protected" => shared::Access::Protected,
            "admin" => shared::Access::Admin,
            _ => shared::Access::Public,
        },
        property_overrides: r
            .property_overrides
            .into_iter()
            .map(|o| shared::PropertyPermission {
                property: o.property,
                access: match o.access.as_str() {
                    "noAccess" => shared::PropertyAccessLevel::NoAccess,
                    "read" => shared::PropertyAccessLevel::Read,
                    "writeBeforePersist" => shared::PropertyAccessLevel::WriteBeforePersist,
                    _ => shared::PropertyAccessLevel::Write,
                },
            })
            .collect(),
    }
}

pub async fn login(username: &str, password: &str) -> Result<LoginOutcome, GqlError> {
    let data: LoginData = execute(LOGIN_MUTATION, LoginVars { username, password }).await?;
    if data.login.ok {
        if let Some(raw) = data.login.session {
            return Ok(LoginOutcome::Success(AuthSession {
                token: raw.token,
                user: raw.user,
                permissions: raw.permissions.into_iter().map(map_permission).collect(),
                expires_at: raw.expires_at,
            }));
        }
    }
    Ok(LoginOutcome::Failed(
        data.login.error.unwrap_or_else(|| "internal".into()),
    ))
}

pub async fn logout() -> Result<bool, GqlError> {
    let data: LogoutData = execute(LOGOUT_MUTATION, EmptyVars {}).await?;
    Ok(data.logout)
}

pub async fn fetch_current_user(
) -> Result<(Option<SecurityUser>, Vec<Permission>), GqlError> {
    let data: CurrentUserData = execute(CURRENT_USER_QUERY, EmptyVars {}).await?;
    Ok((
        data.current_user,
        data.current_permissions
            .into_iter()
            .map(map_permission)
            .collect(),
    ))
}

// =============================================================================
// EntityById
// =============================================================================

const ENTITY_BY_ID_QUERY: &str = r#"
    query EntityById($entityType: String!, $id: String!) {
        entityById(entityType: $entityType, id: $id) { id fields }
    }
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityByIdVars<'a> {
    entity_type: &'a str,
    id: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityByIdData {
    entity_by_id: Option<ServerEntity>,
}

pub async fn fetch_entity_by_id(
    entity_type: &str,
    id: &str,
) -> Result<Option<Entity>, GqlError> {
    let data: EntityByIdData =
        execute(ENTITY_BY_ID_QUERY, EntityByIdVars { entity_type, id }).await?;
    Ok(data.entity_by_id.map(|e| {
        let fields = match e.fields {
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        Entity { id: e.id, fields }
    }))
}
