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
            filterId
            editorId
            formatterId
            actionIds
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
    #[serde(default)]
    filter_id: Option<String>,
    #[serde(default)]
    editor_id: Option<String>,
    #[serde(default)]
    formatter_id: Option<String>,
    #[serde(default)]
    action_ids: Option<Vec<String>>,
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
                filter_id: c.filter_id,
                editor_id: c.editor_id,
                formatter_id: c.formatter_id,
                action_ids: c.action_ids.unwrap_or_default(),
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
            referenceLabels
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
    #[serde(default)]
    reference_labels: serde_json::Value,
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
    /// Aufgeloeste Display-Labels fuer Reference-Felder: `{ col_key → { row_id → label } }`.
    /// Leer, wenn der Server keine Reference-Spalten kennt oder `display_field` nicht
    /// konfiguriert ist.
    pub reference_labels:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}

pub async fn fetch_entities(
    entity_type: &str,
    page: i32,
    page_size: i32,
) -> Result<EntityPageResult, GqlError> {
    let data: EntitiesData = execute(
        ENTITIES_QUERY,
        EntitiesVars {
            entity_type,
            page,
            page_size,
        },
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
    let reference_labels =
        serde_json::from_value(data.entities.reference_labels).unwrap_or_default();
    Ok(EntityPageResult {
        items,
        total_count: data.entities.total_count.max(0) as u64,
        page: data.entities.page.max(0) as u32,
        page_size: data.entities.page_size.max(0) as u32,
        reference_labels,
    })
}

// =============================================================================
// Implementations-Resolution (Phase 1.5)
// =============================================================================

const RESOLVE_IMPLEMENTATION_QUERY: &str = r#"
    query ResolveImplementation($t:String!,$p:String!,$r:String!,$u:String){
        resolveImplementation(entityType:$t, property:$p, registry:$r, userId:$u)
    }
"#;

const ALLOWED_IMPLEMENTATIONS_QUERY: &str = r#"
    query AllowedImplementations($t:String!,$p:String!,$r:String!){
        allowedImplementations(entityType:$t, property:$p, registry:$r)
    }
"#;

const SET_IMPLEMENTATION_CHOICE_MUTATION: &str = r#"
    mutation SetImplementationChoice($t:String!,$p:String!,$r:String!,$c:String!){
        setImplementationChoice(entityType:$t, property:$p, registry:$r, chosenId:$c)
    }
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolveVars<'a> {
    t: &'a str,
    p: &'a str,
    r: &'a str,
    u: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveData {
    resolve_implementation: Option<String>,
}

pub async fn resolve_implementation(
    entity_type: &str,
    property: &str,
    registry: &str,
    user_id: Option<&str>,
) -> Result<Option<String>, GqlError> {
    let data: ResolveData = execute(
        RESOLVE_IMPLEMENTATION_QUERY,
        ResolveVars {
            t: entity_type,
            p: property,
            r: registry,
            u: user_id,
        },
    )
    .await?;
    Ok(data.resolve_implementation)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AllowedVars<'a> {
    t: &'a str,
    p: &'a str,
    r: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AllowedData {
    allowed_implementations: Vec<String>,
}

pub async fn allowed_implementations(
    entity_type: &str,
    property: &str,
    registry: &str,
) -> Result<Vec<String>, GqlError> {
    let data: AllowedData = execute(
        ALLOWED_IMPLEMENTATIONS_QUERY,
        AllowedVars {
            t: entity_type,
            p: property,
            r: registry,
        },
    )
    .await?;
    Ok(data.allowed_implementations)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetChoiceVars<'a> {
    t: &'a str,
    p: &'a str,
    r: &'a str,
    c: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetChoiceData {
    set_implementation_choice: bool,
}

pub async fn set_implementation_choice(
    entity_type: &str,
    property: &str,
    registry: &str,
    chosen_id: &str,
) -> Result<bool, GqlError> {
    let data: SetChoiceData = execute(
        SET_IMPLEMENTATION_CHOICE_MUTATION,
        SetChoiceVars {
            t: entity_type,
            p: property,
            r: registry,
            c: chosen_id,
        },
    )
    .await?;
    Ok(data.set_implementation_choice)
}

// =============================================================================
// Builder-Design-Persistenz (Phase 1.6)
// =============================================================================

const ENTITY_DESIGN_QUERY: &str = r#"
    query EntityDesign($entityType: String!) {
        entityDesign(entityType: $entityType) {
            entityType
            version
            schemaVersion
            state
            createdAt
            createdBy
            locked
        }
    }
"#;

const SAVE_ENTITY_DESIGN_MUTATION: &str = r#"
    mutation SaveEntityDesign(
        $entityType: String!,
        $schemaVersion: Int!,
        $state: JSON!,
        $expectedVersion: Int,
    ) {
        saveEntityDesign(
            entityType: $entityType,
            schemaVersion: $schemaVersion,
            state: $state,
            expectedVersion: $expectedVersion,
        ) {
            ok
            error
            design {
                entityType
                version
                schemaVersion
                state
                createdAt
                createdBy
                locked
            }
            conflictCurrent {
                entityType
                version
                schemaVersion
                state
                createdAt
                createdBy
                locked
            }
        }
    }
"#;

const REVERT_ENTITY_DESIGN_MUTATION: &str = r#"
    mutation RevertEntityDesign($entityType: String!, $targetVersion: Int!) {
        revertEntityDesign(entityType: $entityType, targetVersion: $targetVersion) {
            ok
            error
            design {
                entityType
                version
                schemaVersion
                state
                createdAt
                createdBy
                locked
            }
        }
    }
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityDesign {
    pub entity_type: String,
    pub version: i32,
    pub schema_version: i32,
    /// `state`-Blob (`tree.nodes` + `projection`). Roh; Aufrufer parsen
    /// die `tree`-Sektion separat in `shared::builder` o.ae.
    pub state: serde_json::Value,
    pub created_at: String,
    pub created_by: String,
    pub locked: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityDesignData {
    entity_design: Option<EntityDesign>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveEntityDesignData {
    save_entity_design: SaveEntityDesignServerResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveEntityDesignServerResult {
    ok: bool,
    error: Option<String>,
    design: Option<EntityDesign>,
    conflict_current: Option<EntityDesign>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevertEntityDesignData {
    revert_entity_design: SaveEntityDesignServerResult,
}

#[derive(Debug, Clone)]
pub struct SaveEntityDesignResult {
    pub ok: bool,
    pub error: Option<String>,
    pub design: Option<EntityDesign>,
    pub conflict_current: Option<EntityDesign>,
}

/// Laedt die aktive Builder-Design-Version fuer einen Entity-Typ.
pub async fn fetch_entity_design(entity_type: &str) -> Result<Option<EntityDesign>, GqlError> {
    let data: EntityDesignData =
        execute(ENTITY_DESIGN_QUERY, EntityTypeVars { entity_type }).await?;
    Ok(data.entity_design)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveEntityDesignVars<'a> {
    entity_type: &'a str,
    schema_version: i32,
    state: serde_json::Value,
    expected_version: Option<i32>,
}

pub async fn save_entity_design(
    entity_type: &str,
    schema_version: i32,
    state: serde_json::Value,
    expected_version: Option<i32>,
) -> Result<SaveEntityDesignResult, GqlError> {
    let data: SaveEntityDesignData = execute(
        SAVE_ENTITY_DESIGN_MUTATION,
        SaveEntityDesignVars {
            entity_type,
            schema_version,
            state,
            expected_version,
        },
    )
    .await?;
    Ok(SaveEntityDesignResult {
        ok: data.save_entity_design.ok,
        error: data.save_entity_design.error,
        design: data.save_entity_design.design,
        conflict_current: data.save_entity_design.conflict_current,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RevertEntityDesignVars<'a> {
    entity_type: &'a str,
    target_version: i32,
}

pub async fn revert_entity_design(
    entity_type: &str,
    target_version: i32,
) -> Result<SaveEntityDesignResult, GqlError> {
    let data: RevertEntityDesignData = execute(
        REVERT_ENTITY_DESIGN_MUTATION,
        RevertEntityDesignVars {
            entity_type,
            target_version,
        },
    )
    .await?;
    Ok(SaveEntityDesignResult {
        ok: data.revert_entity_design.ok,
        error: data.revert_entity_design.error,
        design: data.revert_entity_design.design,
        conflict_current: None,
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
    let payload = serde_json::to_value(schema).map_err(|e| GqlError::Decode(e.to_string()))?;
    let data: SaveDbSchemaData = execute(
        SAVE_DB_SCHEMA_MUTATION,
        SaveDbSchemaVars { schema: payload },
    )
    .await?;
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
            displayField
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
    #[serde(default)]
    display_field: Option<String>,
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
        default_sort: raw
            .default_sort
            .and_then(|v| serde_json::from_value(v).ok()),
        default_filter: raw
            .default_filter
            .and_then(|v| serde_json::from_value(v).ok()),
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
                min_width: p
                    .min_width
                    .and_then(|w| if w < 0 { None } else { Some(w as u32) }),
            })
            .collect(),
        // Phase 1.5: field_type_defaults werden ueber das GraphQL-Schema
        // serverseitig erst spaeter exponiert; bis dahin liefert der Client
        // eine leere Map. Der Server-Resolver konsultiert seine eigene
        // Settings-Schicht (`data::settings_for*`), unabhaengig vom Client.
        field_type_defaults: std::collections::BTreeMap::new(),
        // Phase 0.6: binding wird ueber GraphQL noch nicht exponiert.
        binding: None,
        // U1-fix: display_field wird jetzt vom Server geliefert (EntitySettings.displayField).
        display_field: raw.display_field,
        // Phase 1.7.4: append_only wird ueber GraphQL noch nicht exponiert;
        // Client behandelt es konservativ als false. Der Server enforced ihn.
        append_only: false,
        // Phase 1.7.5: state_machine ist heute nicht exponiert; Client
        // braucht es auch nicht (Transitions laufen serverseitig).
        state_machine: None,
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
    EntityChangeResult {
        ok: raw.ok,
        entity,
        validation,
    }
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
        CreateEntitiesVars {
            entity_type,
            items: payload,
        },
    )
    .await?;
    Ok(data
        .create_entities
        .into_iter()
        .map(map_change_result)
        .collect())
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
    Ok(data
        .delete_entities
        .into_iter()
        .map(map_change_result)
        .collect())
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

// Kurzlebiger Return-Wert; Boxen wuerde nur Consumer-Ergonomie kosten.
#[allow(clippy::large_enum_variant)]
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
                // Phase 0.7.5: solange der Server die Projektion nicht ausliefert
                // (geplant fuer Phase 0.7.4), bleibt `effective` hier `None` und
                // `AuthContext` faellt auf das Legacy-`permissions`-Feld zurueck.
                effective: None,
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

pub async fn fetch_current_user() -> Result<(Option<SecurityUser>, Vec<Permission>), GqlError> {
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

pub async fn fetch_entity_by_id(entity_type: &str, id: &str) -> Result<Option<Entity>, GqlError> {
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

// =============================================================================
// Q0005 — Named Views
// =============================================================================

const ENTITY_VIEW_QUERY: &str = r#"
    query EntityView($entityType: String!, $viewName: String!) {
        entityView(entityType: $entityType, viewName: $viewName) {
            id entityType viewName layer ownerId
            properties defaultFilter defaultSort defaultPageSize
            version updatedAt updatedBy
        }
    }
"#;

const ENTITY_VIEWS_QUERY: &str = r#"
    query EntityViews($entityType: String!) {
        entityViews(entityType: $entityType) { viewName layers updatedAt }
    }
"#;

const SAVE_ENTITY_VIEW_MUTATION: &str = r#"
    mutation SaveEntityView($input: SaveEntityViewInput!) {
        saveEntityView(input: $input) {
            kind message
            view { id entityType viewName layer ownerId properties defaultFilter defaultSort defaultPageSize version updatedAt updatedBy }
        }
    }
"#;

const REVERT_ENTITY_VIEW_MUTATION: &str = r#"
    mutation RevertEntityView($entityType: String!, $viewName: String!, $layer: ViewLayer!, $ownerId: String) {
        revertEntityView(entityType: $entityType, viewName: $viewName, layer: $layer, ownerId: $ownerId) { ok message }
    }
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityViewVars<'a> {
    entity_type: &'a str,
    view_name: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityViewsVars<'a> {
    entity_type: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityViewResp {
    entity_view: Option<RawEntityView>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityViewsResp {
    entity_views: Vec<RawEntityViewSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawEntityViewSummary {
    pub view_name: String,
    pub layers: Vec<shared::view::ViewLayer>,
    pub updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEntityView {
    id: String,
    entity_type: String,
    view_name: String,
    layer: shared::view::ViewLayer,
    owner_id: Option<String>,
    properties: serde_json::Value,
    default_filter: Option<serde_json::Value>,
    default_sort: Option<serde_json::Value>,
    default_page_size: Option<u32>,
    version: i32,
    updated_at: String,
    updated_by: Option<String>,
}

impl From<RawEntityView> for shared::view::EntityView {
    fn from(r: RawEntityView) -> Self {
        shared::view::EntityView {
            id: r.id,
            entity_type: r.entity_type,
            view_name: r.view_name,
            layer: r.layer,
            owner_id: r.owner_id,
            properties: serde_json::from_value(r.properties).unwrap_or_default(),
            default_filter: r
                .default_filter
                .and_then(|v| serde_json::from_value(v).ok()),
            default_sort: r.default_sort.and_then(|v| serde_json::from_value(v).ok()),
            default_page_size: r.default_page_size,
            version: r.version,
            updated_at: r.updated_at,
            updated_by: r.updated_by,
        }
    }
}

pub async fn fetch_entity_view(
    entity_type: &str,
    view_name: &str,
) -> Result<Option<shared::view::EntityView>, GqlError> {
    let d: EntityViewResp = execute(
        ENTITY_VIEW_QUERY,
        EntityViewVars {
            entity_type,
            view_name,
        },
    )
    .await?;
    Ok(d.entity_view.map(Into::into))
}

pub async fn fetch_entity_views(entity_type: &str) -> Result<Vec<RawEntityViewSummary>, GqlError> {
    let d: EntityViewsResp = execute(ENTITY_VIEWS_QUERY, EntityViewsVars { entity_type }).await?;
    Ok(d.entity_views)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEntityViewInputClient<'a> {
    pub entity_type: &'a str,
    pub view_name: &'a str,
    pub layer: shared::view::ViewLayer,
    pub owner_id: Option<&'a str>,
    pub payload: serde_json::Value,
    pub expected_version: Option<i32>,
}

#[derive(Serialize)]
struct SaveVars<'a> {
    input: SaveEntityViewInputClient<'a>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveEntityViewOutcomeRaw {
    kind: String,
    message: Option<String>,
    #[serde(default)]
    view: Option<RawEntityView>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveResp {
    save_entity_view: SaveEntityViewOutcomeRaw,
}

pub struct SaveEntityViewOutcomeClient {
    pub kind: String,
    pub message: Option<String>,
    pub view: Option<shared::view::EntityView>,
}

pub async fn save_entity_view(
    input: SaveEntityViewInputClient<'_>,
) -> Result<SaveEntityViewOutcomeClient, GqlError> {
    let r: SaveResp = execute(SAVE_ENTITY_VIEW_MUTATION, SaveVars { input }).await?;
    Ok(SaveEntityViewOutcomeClient {
        kind: r.save_entity_view.kind,
        message: r.save_entity_view.message,
        view: r.save_entity_view.view.map(Into::into),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RevertVars<'a> {
    entity_type: &'a str,
    view_name: &'a str,
    layer: shared::view::ViewLayer,
    owner_id: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevertResp {
    revert_entity_view: RevertEntityViewOutcomeClient,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertEntityViewOutcomeClient {
    pub ok: bool,
    pub message: Option<String>,
}

pub async fn revert_entity_view(
    entity_type: &str,
    view_name: &str,
    layer: shared::view::ViewLayer,
    owner_id: Option<&str>,
) -> Result<RevertEntityViewOutcomeClient, GqlError> {
    let r: RevertResp = execute(
        REVERT_ENTITY_VIEW_MUTATION,
        RevertVars {
            entity_type,
            view_name,
            layer,
            owner_id,
        },
    )
    .await?;
    Ok(r.revert_entity_view)
}

// =============================================================================
// Q0009 — Scripts (Phase 6)
// =============================================================================
//
// Wire-Strategie wie bei `fetch_columns`: dynamische JSON-Felder (manifest,
// kind, lastError) kommen als `serde_json::Value` rein und werden in den
// Typed-Wrapper (`shared::script::*`) deserialisiert. Bei Parse-Fehler des
// Manifests ist die Konvention "Skript als Draft mit Default-Manifest
// behandeln" — Spec §9. Der Aufrufer entscheidet, was er damit anfaengt.

use shared::script::{Script, ScriptError, ScriptId, ScriptKind, ScriptManifest, ScriptState};

const FETCH_SCRIPT_QUERY: &str = r#"
    query Script($id: String!) {
        script(id: $id) {
            id kind source version state manifest lastError
            createdBy createdAt updatedAt
        }
    }
"#;

const FETCH_SCRIPTS_QUERY: &str = r#"
    query Scripts($filter: ScriptFilter) {
        scripts(filter: $filter) {
            id kind source version state manifest lastError
            createdBy createdAt updatedAt
        }
    }
"#;

const SAVE_SCRIPT_MUTATION: &str = r#"
    mutation SaveScript($input: SaveScriptInput!) {
        saveScript(input: $input) {
            id kind source version state manifest lastError
            createdBy createdAt updatedAt
        }
    }
"#;

const PREVIEW_SCRIPT_RUN_MUTATION: &str = r#"
    mutation PreviewScriptRun($input: PreviewScriptRunInput!) {
        previewScriptRun(input: $input) {
            output error tokensUsed runId durationMs
        }
    }
"#;

/// Wire-Repraesentation eines Scripts. Vor der `into_typed`-Konvertierung
/// haengt das Manifest noch als roher JSON-Blob im Feld `manifest`.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawScript {
    pub id: String,
    pub kind: serde_json::Value,
    pub source: String,
    pub version: i32,
    /// `"DRAFT"` / `"ACTIVE"` / `"LOCKED"` — GraphQL-Enum-Wire-Form (UPPER).
    pub state: String,
    pub manifest: serde_json::Value,
    #[serde(default)]
    pub last_error: Option<serde_json::Value>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

impl RawScript {
    /// Konvertiert die Wire-Form in das typed-`Script`-Modell.
    ///
    /// Falls Manifest oder Kind nicht parseable sind, faellt der entsprechende
    /// Pfad auf einen Default zurueck — vgl. `fetch_columns`-Pattern fuer
    /// `FieldType`. Der State wird aus der GraphQL-Enum-Wire-Form rueckgemappt.
    pub fn into_typed(self) -> Script {
        let manifest: ScriptManifest = serde_json::from_value(self.manifest).unwrap_or_default();
        let kind: ScriptKind = serde_json::from_value(self.kind).unwrap_or(ScriptKind::Component {
            entry: "render".into(),
        });
        let state = match self.state.as_str() {
            "ACTIVE" => ScriptState::Active,
            "LOCKED" => ScriptState::Locked,
            _ => ScriptState::Draft,
        };
        let last_error: Option<ScriptError> =
            self.last_error.and_then(|v| serde_json::from_value(v).ok());
        Script {
            id: ScriptId(self.id),
            kind,
            manifest,
            source: self.source,
            version: self.version.max(0) as u32,
            state,
            last_error,
            created_by: self.created_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptResp {
    script: Option<RawScript>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptsResp {
    scripts: Vec<RawScript>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveScriptResp {
    save_script: RawScript,
}

#[derive(Serialize)]
struct FetchScriptVars<'a> {
    id: &'a str,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScriptFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
}

#[derive(Serialize)]
struct FetchScriptsVars {
    filter: Option<ScriptFilter>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveScriptInputClient {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub source: String,
    /// Manifest als `serde_json::Value`. Der Aufrufer baut das aus `ScriptManifest`
    /// via `serde_json::to_value`.
    pub manifest: serde_json::Value,
    pub kind: serde_json::Value,
}

#[derive(Serialize)]
struct SaveScriptVars {
    input: SaveScriptInputClient,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewScriptRunInputClient {
    pub script_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct PreviewVars {
    input: PreviewScriptRunInputClient,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPreviewClient {
    pub output: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub tokens_used: serde_json::Value,
    pub run_id: String,
    pub duration_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewResp {
    preview_script_run: ScriptPreviewClient,
}

pub async fn fetch_script(id: &str) -> Result<Option<Script>, GqlError> {
    let r: ScriptResp = execute(FETCH_SCRIPT_QUERY, FetchScriptVars { id }).await?;
    Ok(r.script.map(RawScript::into_typed))
}

pub async fn fetch_scripts(filter: Option<ScriptFilter>) -> Result<Vec<Script>, GqlError> {
    let r: ScriptsResp = execute(FETCH_SCRIPTS_QUERY, FetchScriptsVars { filter }).await?;
    Ok(r.scripts.into_iter().map(RawScript::into_typed).collect())
}

pub async fn save_script(input: SaveScriptInputClient) -> Result<Script, GqlError> {
    let r: SaveScriptResp = execute(SAVE_SCRIPT_MUTATION, SaveScriptVars { input }).await?;
    Ok(r.save_script.into_typed())
}

pub async fn preview_script_run(
    input: PreviewScriptRunInputClient,
) -> Result<ScriptPreviewClient, GqlError> {
    let r: PreviewResp = execute(PREVIEW_SCRIPT_RUN_MUTATION, PreviewVars { input }).await?;
    Ok(r.preview_script_run)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinnt das `into_typed`-Mapping fuer ein realistisches Wire-Sample:
    /// camelCase-Felder + GraphQL-State-UPPER + Manifest-JSON-Blob.
    #[test]
    fn raw_script_into_typed_roundtrips_active_provider() {
        let raw = RawScript {
            id: "x".into(),
            kind: serde_json::json!({ "kind": "provider", "slot": "formatter" }),
            source: "1+1".into(),
            version: 3,
            state: "ACTIVE".into(),
            manifest: serde_json::json!({
                "manifestVersion": 1,
                "tier": "reader",
                "capabilities": [{ "kind": "computeOnly" }],
                "uiPrimitives": [],
                "timeoutMs": 1000,
                "memoryKb": 1024,
                "liftCapable": false
            }),
            last_error: None,
            created_by: "u".into(),
            created_at: "2026-05-23T00:00:00Z".into(),
            updated_at: "2026-05-23T00:00:00Z".into(),
        };
        let s = raw.into_typed();
        assert_eq!(s.id, ScriptId("x".into()));
        assert!(matches!(s.state, ScriptState::Active));
        assert!(matches!(
            s.kind,
            ScriptKind::Provider {
                slot: shared::script::ProviderSlot::Formatter
            }
        ));
        assert_eq!(s.version, 3);
        assert_eq!(s.manifest.tier, shared::script::ScriptTier::Reader);
    }

    #[test]
    fn raw_script_into_typed_falls_back_on_invalid_manifest() {
        let raw = RawScript {
            id: "y".into(),
            kind: serde_json::Value::Null,
            source: String::new(),
            version: 1,
            state: "DRAFT".into(),
            manifest: serde_json::json!({ "totally": "broken" }),
            last_error: Some(serde_json::json!({
                "kind": "parseFailed",
                "line": 1, "col": 1, "msg": "x"
            })),
            created_by: "u".into(),
            created_at: "".into(),
            updated_at: "".into(),
        };
        let s = raw.into_typed();
        assert!(matches!(s.state, ScriptState::Draft));
        // Manifest faellt auf Default zurueck statt zu panicken.
        assert_eq!(s.manifest.tier, shared::script::ScriptTier::Reader);
        // last_error trifft die ParseFailed-Variante.
        assert!(matches!(
            s.last_error,
            Some(ScriptError::ParseFailed { .. })
        ));
    }

    // ---- U1: reference_labels-Decode ----

    /// Verifiziert, dass ein `referenceLabels`-JSON-Blob korrekt in die
    /// `BTreeMap<String, BTreeMap<String, String>>`-Struktur dekodiert wird.
    /// Diese Logik spiegelt die `serde_json::from_value`-Konvertierung in
    /// `fetch_entities`.
    #[test]
    fn reference_labels_decode_from_json() {
        let raw = serde_json::json!({
            "category_id": {
                "row-1": "Werkzeug",
                "row-2": "Material"
            }
        });
        let decoded: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, String>,
        > = serde_json::from_value(raw).unwrap();
        assert_eq!(decoded["category_id"]["row-1"], "Werkzeug");
        assert_eq!(decoded["category_id"]["row-2"], "Material");
    }

    /// Verifiziert, dass ein fehlendes / null `referenceLabels`-Feld
    /// (default-Attribut) auf eine leere Map faellt — kein Panic.
    #[test]
    fn reference_labels_missing_falls_back_to_empty() {
        let raw = serde_json::Value::Null;
        let decoded: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, String>,
        > = serde_json::from_value(raw).unwrap_or_default();
        assert!(decoded.is_empty());
    }
}
