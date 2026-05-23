//! Daten-Schicht (SeaORM-backed).
//!
//! Alle Zugriffe sind `async fn` und sprechen `DatabaseConnection` aus
//! [`crate::db`] an. *Demo-Inhalte* (Navigation, Spalten-/Editor-/Settings-
//! Metadaten, Seed-User/-Gruppen/-Translatables/-Entities) sind im Server-Code
//! **nicht** mehr hartkodiert — sie kommen aus dem installierten
//! [`crate::example::ExampleSet`] (siehe `--data-dir`). Ist kein Beispiel
//! installiert (z.B. CLI-Lauf gegen Bestands-DB), sind die Lese-Pfade leer
//! und die Seed-Pfade no-op.
//!
//! `editor_for`/`settings_for` bleiben **synchron**, damit die Validation
//! ohne `.await` auskommt. `editor_for_async`/`settings_for_async` fragt
//! zuerst die `metadata_*`-Tabellen ab (Designer-Override) und faellt sonst
//! auf das Beispiel zurueck.

use std::sync::{Mutex, OnceLock};

use async_graphql::Json;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use shared::{
    EditorMeta, EntitySettings, SecurityGroup, SecurityUser, TranslatableBundle,
    TranslatableEntry, TranslatableLanguage, TranslatableValue,
};

use crate::db::conn;
use crate::entity;
use crate::schema::{ColumnMeta, Entity, EntityPage, NavigationNode};

// =============================================================================
// Navigation (static)
// =============================================================================

/// Navigation aus dem installierten Beispiel. Ohne Beispiel: leere Liste.
pub fn navigation_tree() -> Vec<NavigationNode> {
    let Some(set) = crate::example::current() else {
        return Vec::new();
    };
    // Q0004 Option C: jeder Entity-Link bekommt einen Designer-Sub-Link.
    // Permission-Gating macht der Client.
    let augmented = shared::augment_with_designer_links(set.navigation);
    augmented.into_iter().map(convert_nav_node).collect()
}

fn convert_nav_node(n: shared::NavigationNode) -> NavigationNode {
    NavigationNode {
        id: n.id,
        label_key: n.label_key,
        route: n.route,
        icon: n.icon,
        action: n
            .action
            .map(|a| Json(serde_json::to_value(a).unwrap_or_default())),
        children: n.children.into_iter().map(convert_nav_node).collect(),
    }
}

// =============================================================================
// Spalten-Metadaten (aus Beispiel-Set)
// =============================================================================

pub fn columns_for(entity_type: &str) -> Vec<ColumnMeta> {
    let Some(set) = crate::example::current() else {
        return Vec::new();
    };
    let Some(et) = set.entities.get(entity_type) else {
        return Vec::new();
    };
    et.columns.iter().cloned().map(convert_column).collect()
}

fn convert_column(c: shared::ColumnMeta) -> ColumnMeta {
    ColumnMeta {
        key: c.key,
        label_key: c.label_key,
        field_type: Json(serde_json::to_value(c.field_type).unwrap_or_default()),
        sortable: c.sortable,
        filterable: c.filterable,
        comparator_id: c.comparator_id,
        filter_id: c.filter_id,
        editor_id: c.editor_id,
        formatter_id: c.formatter_id,
        action_ids: c.action_ids,
    }
}

// =============================================================================
// Entity-CRUD (DB-backed)
// =============================================================================

fn next_id_prefix(entity_type: &str) -> String {
    use rand::Rng;
    let n: u64 = rand::thread_rng().gen_range(1_000..1_000_000);
    format!("{}-{:04}", entity_type.chars().next().unwrap_or('x'), n)
}

#[allow(dead_code)]
fn fields_from_model(model: entity::entities::Model) -> Entity {
    let parsed: serde_json::Value =
        serde_json::from_str(&model.fields_json).unwrap_or(serde_json::Value::Null);
    Entity { id: model.id, fields: Json(parsed) }
}

fn fields_obj_from_value(v: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    match v {
        serde_json::Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    }
}

fn hash_for_entity(id: &str, fields: &serde_json::Map<String, serde_json::Value>) -> String {
    let s = shared::Entity { id: id.to_string(), fields: fields.clone() };
    shared::compute_hash(&s).to_string()
}

/// Holt die `shared::ColumnMeta`-Liste fuer einen Entity-Typ. Pendant zu
/// [`columns_for`], aber ohne die Konvertierung in den GraphQL-Wrapper —
/// fuer Sort/Filter-Auswertung brauchen wir die typisierten `FieldType`s.
fn shared_columns_for(entity_type: &str) -> Vec<shared::ColumnMeta> {
    crate::example::current()
        .and_then(|set| set.entities.get(entity_type).cloned())
        .map(|et| et.columns)
        .unwrap_or_default()
}

/// Wertet eine [`shared::FilterCriteria`] gegen eine Entitaet aus. Logik
/// gespiegelt zu `client/src/components/table/data_source.rs::LocalSource`,
/// damit Server- und Clientseite identisches Verhalten zeigen. Wenn die
/// `LocalSource`-Logik kuenftig nach `shared` gezogen wird, faellt diese
/// Funktion ersatzlos weg.
fn passes_filter(
    entity: &shared::Entity,
    filter: &shared::FilterCriteria,
    columns: &std::collections::HashMap<String, &shared::ColumnMeta>,
) -> bool {
    for cf in &filter.predicates {
        let Some(col) = columns.get(&cf.key) else { return false };
        let value = entity
            .fields
            .get(&cf.key)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let ops = shared::ops_for_named(
            &col.field_type,
            col.comparator_id.as_deref(),
            col.filter_id.as_deref(),
        );
        if !ops.matches(&value, &cf.predicate) {
            return false;
        }
    }
    if let Some(needle) = filter.global_search.as_deref().filter(|s| !s.is_empty()) {
        let hit = columns.iter().any(|(key, col)| {
            let value = entity
                .fields
                .get(key)
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let ops = shared::ops_for_named(
                &col.field_type,
                col.comparator_id.as_deref(),
                col.filter_id.as_deref(),
            );
            ops.matches_search(&value, needle)
        });
        if !hit {
            return false;
        }
    }
    true
}

fn sort_entities(
    items: &mut [shared::Entity],
    sort: &shared::Sort,
    columns: &std::collections::HashMap<String, &shared::ColumnMeta>,
) {
    use std::cmp::Ordering;
    let Some(col) = columns.get(&sort.field) else { return };
    let ops = shared::ops_for_named(
        &col.field_type,
        col.comparator_id.as_deref(),
        col.filter_id.as_deref(),
    );
    let direction = sort.direction;
    let key = sort.field.clone();
    items.sort_by(|a, b| {
        let va = a.fields.get(&key).cloned().unwrap_or(serde_json::Value::Null);
        let vb = b.fields.get(&key).cloned().unwrap_or(serde_json::Value::Null);
        let ord = ops.compare(&va, &vb);
        match direction {
            shared::SortDirection::Asc => ord,
            shared::SortDirection::Desc => match ord {
                Ordering::Less => Ordering::Greater,
                Ordering::Greater => Ordering::Less,
                Ordering::Equal => Ordering::Equal,
            },
        }
    });
}

fn shared_entity_from_model(model: entity::entities::Model) -> shared::Entity {
    let parsed: serde_json::Value =
        serde_json::from_str(&model.fields_json).unwrap_or(serde_json::Value::Null);
    let fields = match parsed {
        serde_json::Value::Object(o) => o,
        _ => serde_json::Map::new(),
    };
    shared::Entity { id: model.id, fields }
}

/// Interne Variante, die das `shared::EntityPage` liefert.
/// Wird von der Source-Schicht aufgerufen; die alte `entities_page`
/// (Async-GraphQL-Json-Wrap) ruft sie ihrerseits auf.
pub async fn entities_page_raw(
    entity_type: &str,
    page: i32,
    page_size: i32,
    sort: Option<shared::Sort>,
    filter: shared::FilterCriteria,
) -> shared::EntityPage {
    let db = &conn();

    let rows = entity::entities::Entity::find()
        .filter(entity::entities::Column::EntityType.eq(entity_type))
        .all(db)
        .await
        .unwrap_or_default();

    let mut entities: Vec<shared::Entity> =
        rows.into_iter().map(shared_entity_from_model).collect();

    let columns = shared_columns_for(entity_type);
    let columns_map: std::collections::HashMap<String, &shared::ColumnMeta> =
        columns.iter().map(|c| (c.key.clone(), c)).collect();

    if !filter.is_empty() {
        entities.retain(|e| passes_filter(e, &filter, &columns_map));
    }
    if let Some(s) = sort.as_ref() {
        sort_entities(&mut entities, s, &columns_map);
    }

    let total = entities.len() as u64;
    let page_idx = (page.max(1) - 1) as usize;
    let take = page_size.max(1) as usize;
    let start = page_idx.saturating_mul(take);
    let end = start.saturating_add(take).min(entities.len());
    let slice = if start < entities.len() {
        entities[start..end].to_vec()
    } else {
        Vec::new()
    };

    shared::EntityPage {
        items: slice,
        total_count: total,
        page: page.max(1) as u32,
        page_size: page_size.max(1) as u32,
    }
}

pub(crate) async fn entity_by_id_raw(entity_type: &str, id: &str) -> Option<shared::Entity> {
    let db = &conn();
    entity::entities::Entity::find_by_id((entity_type.to_string(), id.to_string()))
        .one(db).await.ok().flatten().map(shared_entity_from_model)
}

pub(crate) async fn create_entity_raw(
    entity_type: &str,
    id: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
    actor_user_id: Option<&str>,
) -> shared::Entity {
    let db = &conn();
    // If no explicit id is supplied, check whether the caller pre-injected one
    // into the fields map (convention used by data::create_entity when routing
    // through SourceRegistry without a trait-level id parameter).
    let id = id
        .or_else(|| {
            fields
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| next_id_prefix(entity_type));
    let mut fields_map = fields;
    fields_map.insert("id".into(), serde_json::Value::String(id.clone()));
    apply_audit_columns(entity_type, &mut fields_map, actor_user_id, AuditPhase::Create);
    let value = serde_json::Value::Object(fields_map.clone());
    let hash = hash_for_entity(&id, &fields_map);

    let model = entity::entities::ActiveModel {
        entity_type: ActiveValue::Set(entity_type.to_string()),
        id: ActiveValue::Set(id.clone()),
        fields_json: ActiveValue::Set(value.to_string()),
        hash: ActiveValue::Set(hash),
    };
    let _ = model.insert(db).await;
    shared::Entity { id, fields: fields_map }
}

pub(crate) async fn update_entity_raw(
    entity_type: &str,
    id: &str,
    field_patch: serde_json::Map<String, serde_json::Value>,
    actor_user_id: Option<&str>,
) -> Option<shared::Entity> {
    let db = &conn();
    let existing = entity::entities::Entity::find_by_id((entity_type.to_string(), id.to_string()))
        .one(db).await.ok().flatten()?;
    let mut current: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&existing.fields_json).unwrap_or_default();
    for (k, v) in field_patch {
        current.insert(k, v);
    }
    apply_audit_columns(entity_type, &mut current, actor_user_id, AuditPhase::Update);
    let value = serde_json::Value::Object(current.clone());
    let hash = hash_for_entity(id, &current);
    let am = entity::entities::ActiveModel {
        entity_type: ActiveValue::Set(entity_type.to_string()),
        id: ActiveValue::Set(id.to_string()),
        fields_json: ActiveValue::Set(value.to_string()),
        hash: ActiveValue::Set(hash),
    };
    let _ = am.update(db).await;
    Some(shared::Entity { id: id.to_string(), fields: current })
}

pub(crate) async fn delete_entity_raw(entity_type: &str, id: &str) -> bool {
    let db = &conn();
    let res = entity::entities::Entity::delete_by_id((entity_type.to_string(), id.to_string()))
        .exec(db)
        .await;
    matches!(res, Ok(r) if r.rows_affected > 0)
}

fn binding_for(entity_type: &str) -> shared::source::EntityBinding {
    crate::example::current()
        .and_then(|set| set.entities.get(entity_type).cloned())
        .and_then(|ty| ty.settings)
        .and_then(|s| s.binding)
        .unwrap_or_else(|| shared::source::default_binding_for(entity_type))
}

pub async fn entities_page(
    entity_type: &str,
    page: i32,
    page_size: i32,
    sort: Option<shared::Sort>,
    filter: shared::FilterCriteria,
) -> EntityPage {
    let binding = binding_for(entity_type);
    // Acquire Arc before dropping the read-lock so we don't hold it across await.
    let source = {
        let reg = crate::source::registry();
        match reg.route(&binding) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(target: "server::data", "route error: {e}");
                return EntityPage {
                    items: Vec::new(),
                    total_count: 0,
                    page,
                    page_size,
                };
            }
        }
    };
    let q = crate::source::PageQuery { page, page_size, sort, filter };
    match source.list_page(&binding, &q).await {
        Ok(p) => EntityPage {
            items: p.items.into_iter().map(|e| Entity {
                id: e.id,
                fields: Json(serde_json::Value::Object(e.fields)),
            }).collect(),
            total_count: p.total_count as i64,
            page: p.page as i32,
            page_size: p.page_size as i32,
        },
        Err(e) => {
            tracing::error!(target: "server::data", "list_page error: {e}");
            EntityPage {
                items: Vec::new(),
                total_count: 0,
                page,
                page_size,
            }
        }
    }
}

pub async fn entity_by_id(entity_type: &str, id: &str) -> Option<Entity> {
    let binding = binding_for(entity_type);
    let source = { crate::source::registry().route(&binding).ok()? };
    let entity_id = shared::source::EntityId::decode(id);
    match source.get(&binding, &entity_id).await {
        Ok(Some(e)) => Some(Entity {
            id: e.id,
            fields: Json(serde_json::Value::Object(e.fields)),
        }),
        _ => None,
    }
}

pub async fn current_hash(entity_type: &str, id: &str) -> Option<u64> {
    let db = &conn();
    let row = entity::entities::Entity::find_by_id((entity_type.to_string(), id.to_string()))
        .one(db)
        .await
        .ok()
        .flatten()?;
    row.hash.parse().ok()
}

pub async fn merged_fields(
    entity_type: &str,
    id: &str,
    patch: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    let mut base = entity_by_id(entity_type, id)
        .await
        .map(|e| fields_obj_from_value(&e.fields.0))
        .unwrap_or_default();
    if let serde_json::Value::Object(p) = patch {
        for (k, v) in p {
            base.insert(k.clone(), v.clone());
        }
    }
    base
}

pub async fn create_entity(
    entity_type: &str,
    id: Option<String>,
    fields: serde_json::Value,
    actor_user_id: Option<&str>,
) -> Entity {
    let binding = binding_for(entity_type);
    let fields_map = match fields {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    let source = {
        let reg = crate::source::registry();
        match reg.route(&binding) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(target: "server::data", "route error: {e}");
                return Entity { id: id.unwrap_or_default(), fields: Json(serde_json::Value::Null) };
            }
        }
    };
    match source.create(&binding, id.clone(), fields_map, actor_user_id).await {
        Ok(e) => Entity {
            id: e.id,
            fields: Json(serde_json::Value::Object(e.fields)),
        },
        Err(e) => {
            tracing::error!(target: "server::data", "create error: {e}");
            Entity { id: id.unwrap_or_default(), fields: Json(serde_json::Value::Null) }
        }
    }
}

pub async fn update_entity(
    entity_type: &str,
    id: &str,
    field_patch: serde_json::Value,
    actor_user_id: Option<&str>,
) -> Option<Entity> {
    let binding = binding_for(entity_type);
    let source = { crate::source::registry().route(&binding).ok()? };
    let entity_id = shared::source::EntityId::decode(id);
    let patch = match field_patch {
        serde_json::Value::Object(m) => m,
        _ => return None,
    };
    match source.update(&binding, &entity_id, patch, actor_user_id).await {
        Ok(Some(e)) => Some(Entity {
            id: e.id,
            fields: Json(serde_json::Value::Object(e.fields)),
        }),
        _ => None,
    }
}

pub async fn delete_entity(entity_type: &str, id: &str) -> bool {
    let binding = binding_for(entity_type);
    let source = match { crate::source::registry().route(&binding) } {
        Ok(s) => s,
        Err(_) => return false,
    };
    let entity_id = shared::source::EntityId::decode(id);
    source.delete(&binding, &entity_id).await.unwrap_or(false)
}

// =============================================================================
// Audit
// =============================================================================

#[derive(Clone, Copy)]
enum AuditPhase {
    Create,
    Update,
}

fn apply_audit_columns(
    entity_type: &str,
    fields: &mut serde_json::Map<String, serde_json::Value>,
    actor_user_id: Option<&str>,
    phase: AuditPhase,
) {
    let Some(schema) = current_db_schema() else { return };
    let Some(table) = schema.tables.iter().find(|t| t.name == entity_type) else { return };
    let now = Utc::now().to_rfc3339();
    for col in &table.columns {
        let role = col.audit_role;
        let fill = match phase {
            AuditPhase::Create => role.fills_on_create(),
            AuditPhase::Update => role.fills_on_update(),
        };
        if !fill {
            continue;
        }
        let value = match role {
            shared::AuditRole::CreatedAt | shared::AuditRole::UpdatedAt => {
                serde_json::Value::String(now.clone())
            }
            shared::AuditRole::CreatedBy | shared::AuditRole::UpdatedBy => actor_user_id
                .map(|s| serde_json::Value::String(s.into()))
                .unwrap_or(serde_json::Value::Null),
            shared::AuditRole::None => continue,
        };
        fields.insert(col.name.clone(), value);
    }
}

// =============================================================================
// Designer-Schema (in-memory Cache + DB persistiert)
// =============================================================================

fn db_schema_cell() -> &'static Mutex<Option<shared::DbSchema>> {
    static C: OnceLock<Mutex<Option<shared::DbSchema>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

pub fn current_db_schema() -> Option<shared::DbSchema> {
    db_schema_cell().lock().unwrap().clone()
}

pub fn install_db_schema(schema: shared::DbSchema) {
    *db_schema_cell().lock().unwrap() = Some(schema);
}

pub async fn persist_db_schema(schema: &shared::DbSchema) -> Result<(), sea_orm::DbErr> {
    let db = &conn();
    let name = if schema.name.is_empty() {
        "default".to_string()
    } else {
        schema.name.clone()
    };
    let json = serde_json::to_string(schema).map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
    let am = entity::db_schemas::ActiveModel {
        name: ActiveValue::Set(name.clone()),
        schema_json: ActiveValue::Set(json),
        updated_at: ActiveValue::Set(Utc::now()),
    };
    if entity::db_schemas::Entity::find_by_id(name.clone())
        .one(db)
        .await?
        .is_some()
    {
        am.update(db).await?;
    } else {
        am.insert(db).await?;
    }
    Ok(())
}

/// Laed beim Server-Start (oder Test-Setup) den zuletzt persistierten
/// Designer-Stand in den In-Memory-Cache.
pub async fn rehydrate_db_schema() -> Result<(), sea_orm::DbErr> {
    use sea_orm::QueryOrder;
    let db = &conn();
    if let Some(row) = entity::db_schemas::Entity::find()
        .order_by_desc(entity::db_schemas::Column::UpdatedAt)
        .one(db)
        .await?
    {
        if let Ok(schema) = serde_json::from_str::<shared::DbSchema>(&row.schema_json) {
            install_db_schema(schema);
        }
    }
    Ok(())
}

// =============================================================================
// Users / Groups (DB-backed)
// =============================================================================

pub async fn users() -> Vec<SecurityUser> {
    let db = &conn();
    let mut out = Vec::new();
    let user_rows = entity::users::Entity::find()
        .all(db)
        .await
        .unwrap_or_default();
    for u in user_rows {
        let groups = entity::user_groups::Entity::find()
            .filter(entity::user_groups::Column::UserId.eq(u.id.clone()))
            .all(db)
            .await
            .unwrap_or_default();
        out.push(SecurityUser {
            id: u.id,
            username: u.username,
            display_name: u.display_name,
            locale: u.locale,
            group_ids: groups.into_iter().map(|g| g.group_id).collect(),
            active: u.active,
            password_hash: u.password_hash,
        });
    }
    out
}

pub async fn user_by_username(username: &str) -> Option<SecurityUser> {
    let db = &conn();
    let row = entity::users::Entity::find()
        .filter(entity::users::Column::Username.eq(username))
        .one(db)
        .await
        .ok()
        .flatten()?;
    let groups = entity::user_groups::Entity::find()
        .filter(entity::user_groups::Column::UserId.eq(row.id.clone()))
        .all(db)
        .await
        .unwrap_or_default();
    Some(SecurityUser {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
        locale: row.locale,
        group_ids: groups.into_iter().map(|g| g.group_id).collect(),
        active: row.active,
        password_hash: row.password_hash,
    })
}

pub async fn user_by_id(id: &str) -> Option<SecurityUser> {
    let db = &conn();
    let row = entity::users::Entity::find_by_id(id.to_string())
        .one(db)
        .await
        .ok()
        .flatten()?;
    let groups = entity::user_groups::Entity::find()
        .filter(entity::user_groups::Column::UserId.eq(row.id.clone()))
        .all(db)
        .await
        .unwrap_or_default();
    Some(SecurityUser {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
        locale: row.locale,
        group_ids: groups.into_iter().map(|g| g.group_id).collect(),
        active: row.active,
        password_hash: row.password_hash,
    })
}

pub async fn groups() -> Vec<SecurityGroup> {
    let db = &conn();
    entity::groups::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|g| SecurityGroup {
            id: g.id,
            name_key: g.name_key,
            description_key: g.description_key,
            permissions: serde_json::from_str(&g.permissions_json).unwrap_or_default(),
        })
        .collect()
}

// =============================================================================
// Users / Groups – Verwaltung (CLI/Admin)
// =============================================================================

fn random_id_suffix() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut s = String::with_capacity(8);
    for _ in 0..8 {
        let c = rng.gen_range(0u8..36);
        s.push(if c < 10 {
            (b'0' + c) as char
        } else {
            (b'a' + c - 10) as char
        });
    }
    s
}

pub async fn create_user(
    username: &str,
    display_name: Option<&str>,
    locale: Option<&str>,
    password: Option<&str>,
) -> Result<SecurityUser, String> {
    let db = &conn();
    let exists = entity::users::Entity::find()
        .filter(entity::users::Column::Username.eq(username))
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    if exists.is_some() {
        return Err(format!("Nutzer '{username}' existiert bereits"));
    }
    let id = format!("u-{}", random_id_suffix());
    let display = display_name.unwrap_or(username).to_string();
    let password_hash = match password {
        Some(p) => Some(crate::auth::hash_password(p)?),
        None => None,
    };
    entity::users::ActiveModel {
        id: ActiveValue::Set(id.clone()),
        username: ActiveValue::Set(username.to_string()),
        display_name: ActiveValue::Set(display.clone()),
        locale: ActiveValue::Set(locale.map(|s| s.to_string())),
        active: ActiveValue::Set(true),
        password_hash: ActiveValue::Set(password_hash.clone()),
    }
    .insert(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(SecurityUser {
        id,
        username: username.to_string(),
        display_name: display,
        locale: locale.map(|s| s.to_string()),
        group_ids: vec![],
        active: true,
        password_hash,
    })
}

pub async fn delete_user_by_username(username: &str) -> Result<bool, String> {
    let db = &conn();
    let row = entity::users::Entity::find()
        .filter(entity::users::Column::Username.eq(username))
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    let Some(user) = row else {
        return Ok(false);
    };
    entity::user_groups::Entity::delete_many()
        .filter(entity::user_groups::Column::UserId.eq(user.id.clone()))
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    entity::sessions::Entity::delete_many()
        .filter(entity::sessions::Column::UserId.eq(user.id.clone()))
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    let res = entity::users::Entity::delete_by_id(user.id)
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(res.rows_affected > 0)
}

pub async fn set_user_password(username: &str, password: &str) -> Result<(), String> {
    let db = &conn();
    let row = entity::users::Entity::find()
        .filter(entity::users::Column::Username.eq(username))
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Nutzer '{username}' nicht gefunden"))?;
    let hash = crate::auth::hash_password(password)?;
    let user_id = row.id.clone();
    let mut am: entity::users::ActiveModel = row.into();
    am.password_hash = ActiveValue::Set(Some(hash));
    am.update(db).await.map_err(|e| e.to_string())?;
    // Bestehende Sessions des Nutzers entwerten — Passwort wurde rotiert.
    entity::sessions::Entity::delete_many()
        .filter(entity::sessions::Column::UserId.eq(user_id))
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn add_user_to_group(username: &str, group_id: &str) -> Result<bool, String> {
    let db = &conn();
    let user = entity::users::Entity::find()
        .filter(entity::users::Column::Username.eq(username))
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Nutzer '{username}' nicht gefunden"))?;
    let group = entity::groups::Entity::find_by_id(group_id.to_string())
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Gruppe '{group_id}' nicht gefunden"))?;
    let already = entity::user_groups::Entity::find_by_id((user.id.clone(), group.id.clone()))
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    if already.is_some() {
        return Ok(false);
    }
    entity::user_groups::ActiveModel {
        user_id: ActiveValue::Set(user.id),
        group_id: ActiveValue::Set(group.id),
    }
    .insert(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(true)
}

pub async fn remove_user_from_group(username: &str, group_id: &str) -> Result<bool, String> {
    let db = &conn();
    let user = entity::users::Entity::find()
        .filter(entity::users::Column::Username.eq(username))
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Nutzer '{username}' nicht gefunden"))?;
    let res = entity::user_groups::Entity::delete_by_id((user.id, group_id.to_string()))
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(res.rows_affected > 0)
}

pub async fn create_group(
    id: &str,
    name_key: &str,
    description_key: Option<&str>,
) -> Result<SecurityGroup, String> {
    let db = &conn();
    if entity::groups::Entity::find_by_id(id.to_string())
        .one(db)
        .await
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err(format!("Gruppe '{id}' existiert bereits"));
    }
    let perms: Vec<shared::Permission> = vec![];
    let perms_json = serde_json::to_string(&perms).map_err(|e| e.to_string())?;
    entity::groups::ActiveModel {
        id: ActiveValue::Set(id.to_string()),
        name_key: ActiveValue::Set(name_key.to_string()),
        description_key: ActiveValue::Set(description_key.map(|s| s.to_string())),
        permissions_json: ActiveValue::Set(perms_json),
    }
    .insert(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(SecurityGroup {
        id: id.to_string(),
        name_key: name_key.to_string(),
        description_key: description_key.map(|s| s.to_string()),
        permissions: perms,
    })
}

pub async fn delete_group(id: &str) -> Result<bool, String> {
    let db = &conn();
    entity::user_groups::Entity::delete_many()
        .filter(entity::user_groups::Column::GroupId.eq(id))
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    let res = entity::groups::Entity::delete_by_id(id.to_string())
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(res.rows_affected > 0)
}

/// Idempotent: legt fehlende Standard-Gruppen (`g-admin`, `g-users`) an.
/// Wird vom CLI bei jedem Start aufgerufen, damit der Admin auf jedem
/// Bestands-DB die erwarteten Gruppen vorfindet.
pub async fn ensure_default_groups() -> Result<(), String> {
    let db = &conn();
    let admin_perms = serde_json::to_string(&vec![shared::Permission {
        entity_type: "*".into(),
        can_read: true,
        can_create: true,
        can_update: true,
        can_delete: true,
        min_access: shared::Access::Admin,
        property_overrides: vec![],
    }])
    .map_err(|e| e.to_string())?;
    let defaults: [(&str, &str, Option<&str>, &str); 2] = [
        (
            "g-admin",
            "security.group.admin",
            Some("security.group.admin.desc"),
            admin_perms.as_str(),
        ),
        (
            "g-users",
            "security.group.users",
            Some("security.group.users.desc"),
            "[]",
        ),
    ];
    for (id, name_key, desc_key, perms_json) in defaults {
        if entity::groups::Entity::find_by_id(id.to_string())
            .one(db)
            .await
            .map_err(|e| e.to_string())?
            .is_some()
        {
            continue;
        }
        entity::groups::ActiveModel {
            id: ActiveValue::Set(id.to_string()),
            name_key: ActiveValue::Set(name_key.to_string()),
            description_key: ActiveValue::Set(desc_key.map(|s| s.to_string())),
            permissions_json: ActiveValue::Set(perms_json.to_string()),
        }
        .insert(db)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// =============================================================================
// Translatables
// =============================================================================

pub async fn translatable_bundle() -> TranslatableBundle {
    let db = &conn();
    let langs: Vec<TranslatableLanguage> = entity::translatable_languages::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|l| TranslatableLanguage {
            id: l.id,
            code: l.code,
            name_key: l.name_key,
            fallback_id: l.fallback_id,
            active: l.active,
        })
        .collect();
    let entries: Vec<TranslatableEntry> = entity::translatable_entries::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|e| TranslatableEntry {
            id: e.id,
            category: e.category,
            description: e.description,
        })
        .collect();
    let values: Vec<TranslatableValue> = entity::translatable_values::Entity::find()
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|v| TranslatableValue {
            entry_id: v.entry_id,
            language_id: v.language_id,
            ftl_source: v.ftl_source,
            updated_at: v.updated_at,
        })
        .collect();
    TranslatableBundle { languages: langs, entries, values }
}

// =============================================================================
// Editor / Settings (in-Memory-Defaults, optional via DB)
// =============================================================================

pub async fn editor_for_async(entity_type: &str) -> Option<EditorMeta> {
    let db = &conn();
    if let Ok(Some(row)) =
        entity::metadata_editor::Entity::find_by_id(entity_type.to_string())
            .one(db)
            .await
    {
        if let Ok(meta) = serde_json::from_str::<EditorMeta>(&row.meta_json) {
            return Some(meta);
        }
    }
    editor_for(entity_type)
}

pub async fn settings_for_async(entity_type: &str) -> Option<EntitySettings> {
    let db = &conn();
    if let Ok(Some(row)) =
        entity::metadata_settings::Entity::find_by_id(entity_type.to_string())
            .one(db)
            .await
    {
        if let Ok(settings) = serde_json::from_str::<EntitySettings>(&row.settings_json) {
            return Some(settings);
        }
    }
    settings_for(entity_type)
}

/// Editor-Metadaten aus dem installierten Beispiel.
pub fn editor_for(entity_type: &str) -> Option<EditorMeta> {
    let set = crate::example::current()?;
    set.entities.get(entity_type)?.editor.clone()
}

/// Entity-Settings aus dem installierten Beispiel.
pub fn settings_for(entity_type: &str) -> Option<EntitySettings> {
    let set = crate::example::current()?;
    set.entities.get(entity_type)?.settings.clone()
}

// =============================================================================
// Validation gegen EditorMeta
// =============================================================================

pub fn validate_against_editor(
    entity_type: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> shared::ValidationResult {
    use shared::ValidationMessage;

    let mut result = shared::ValidationResult::default();
    let Some(meta) = editor_for(entity_type) else { return result };
    for prop in &meta.properties {
        let value = fields.get(&prop.key);

        let is_empty = match value {
            None | Some(serde_json::Value::Null) => true,
            Some(serde_json::Value::String(s)) => s.is_empty(),
            _ => false,
        };

        if prop.readonly {
            continue;
        }

        if prop.required && is_empty {
            result.push(ValidationMessage::error(
                prop.key.clone(),
                "validation.required",
            ));
            continue;
        }

        if is_empty {
            continue;
        }

        if let Some(serde_json::Value::String(s)) = value {
            let len = s.chars().count() as u32;
            if let Some(min) = prop.min_length {
                if len < min {
                    result.push(
                        ValidationMessage::error(prop.key.clone(), "validation.min_length")
                            .with_arg("min", min as i64),
                    );
                }
            }
            if let Some(max) = prop.max_length {
                if len > max {
                    result.push(
                        ValidationMessage::error(prop.key.clone(), "validation.max_length")
                            .with_arg("max", max as i64),
                    );
                }
            }
            if let Some(pat) = &prop.pattern {
                match regex::Regex::new(pat) {
                    Ok(re) => {
                        if !re.is_match(s) {
                            result.push(ValidationMessage::error(
                                prop.key.clone(),
                                "validation.pattern",
                            ));
                        }
                    }
                    Err(_) => {
                        result.push(ValidationMessage {
                            severity: shared::Severity::Error,
                            message_key: "validation.pattern.invalid_definition".into(),
                            target: Some(prop.key.clone()),
                            args: serde_json::Map::new(),
                        });
                    }
                }
            }
        }

        if let Some(n) = value.and_then(json_to_f64) {
            if let Some(min) = prop.min {
                if n < min {
                    result.push(
                        ValidationMessage::error(prop.key.clone(), "validation.number_range")
                            .with_arg("min", min)
                            .with_arg("max", prop.max.unwrap_or(f64::INFINITY)),
                    );
                }
            }
            if let Some(max) = prop.max {
                if n > max {
                    result.push(
                        ValidationMessage::error(prop.key.clone(), "validation.number_range")
                            .with_arg("min", prop.min.unwrap_or(f64::NEG_INFINITY))
                            .with_arg("max", max),
                    );
                }
            }
        }
    }
    result
}

fn json_to_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

// =============================================================================
// Property-Filter
// =============================================================================

pub async fn filter_properties_for_user(
    entity_type: &str,
    fields: &mut serde_json::Map<String, serde_json::Value>,
    user: &shared::SecurityUser,
    groups: &[shared::SecurityGroup],
) {
    let Some(meta) = editor_for_async(entity_type).await else { return };
    let drop_keys: Vec<String> = meta
        .properties
        .iter()
        .filter(|p| {
            matches!(
                shared::property_access_for(user, groups, entity_type, &p.key),
                shared::PropertyAccessLevel::NoAccess
            )
        })
        .map(|p| p.key.clone())
        .collect();
    for k in drop_keys {
        fields.remove(&k);
    }
}

// =============================================================================
// DB-Seed aus installiertem Beispiel
// =============================================================================
//
// Wird ausschliesslich aus `db::seed_if_empty` aufgerufen — d.h. nur, wenn
// die jeweilige Tabelle leer ist. Wenn kein Beispiel installiert ist (z.B.
// CLI-Lauf gegen Bestands-DB), tut die jeweilige Funktion nichts.

pub async fn seed_users(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let Some(set) = crate::example::current() else {
        return Ok(());
    };
    for u in set.users {
        let hash = u
            .password_plain
            .as_deref()
            .map(|p| crate::auth::hash_password(p).expect("argon2"));
        entity::users::ActiveModel {
            id: ActiveValue::Set(u.id.clone()),
            username: ActiveValue::Set(u.username),
            display_name: ActiveValue::Set(u.display_name),
            locale: ActiveValue::Set(u.locale),
            active: ActiveValue::Set(u.active),
            password_hash: ActiveValue::Set(hash),
        }
        .insert(db)
        .await?;
        for g in u.group_ids {
            entity::user_groups::ActiveModel {
                user_id: ActiveValue::Set(u.id.clone()),
                group_id: ActiveValue::Set(g),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

pub async fn seed_groups(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let Some(set) = crate::example::current() else {
        return Ok(());
    };
    for g in set.groups {
        let perms_json = serde_json::to_string(&g.permissions)
            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;
        entity::groups::ActiveModel {
            id: ActiveValue::Set(g.id),
            name_key: ActiveValue::Set(g.name_key),
            description_key: ActiveValue::Set(g.description_key),
            permissions_json: ActiveValue::Set(perms_json),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

// =============================================================================
// Phase 0.7 — Permissions / Roles / Role-Assignments
// =============================================================================

/// Anzahl der Eintraege in der `permissions`-Tabelle. Wird vom
/// Enforcement-Pfad in `schema.rs::require_permission` als Schalter benutzt:
/// > 0 ⇒ neuer Resolver authoritative; = 0 ⇒ alte Logik (Groups +
/// can_*-Flags) bleibt aktiv. So bleiben Bestands-Examples ohne explizite
/// Phase-0.7-Konfiguration weiterhin nutzbar.
pub async fn permissions_count() -> u64 {
    use sea_orm::PaginatorTrait;
    entity::permissions::Entity::find()
        .count(&conn())
        .await
        .unwrap_or(0)
}

// =============================================================================
// Phase 1.5.3 — Implementations-Resolution
// =============================================================================
//
// Resolution-Kette (siehe ROADMAP Phase 1.5):
//   1. Per-User-Choice (user_implementation_choices)
//   2. ColumnMeta.X_id (erzwungene Spalten-Override)
//   3. EntitySettings.field_type_defaults[<kind_str>].X_id
//   4. None — Client benutzt seinen hartcodierten Fallback pro FieldType.

/// Registry-Art als String (`"filter"`, `"editor"`, `"formatter"`, `"action"`).
/// Wird genauso in `shared::auth::Resource::ImplementationId.registry` benutzt.
pub type RegistryKind<'a> = &'a str;

/// Pro Spalte+Registry die wirksame Implementations-ID. `None` bedeutet
/// "kein Override" — der Client soll seinen eigenen Default pro FieldType
/// anwenden.
pub async fn resolve_implementation(
    entity_type: &str,
    property: &str,
    registry: RegistryKind<'_>,
    user_id: Option<&str>,
) -> Option<String> {
    // 1) Per-User-Choice
    if let Some(uid) = user_id {
        if let Some(choice) = user_implementation_choice(uid, entity_type, property, registry).await {
            return Some(choice);
        }
    }

    // 2) ColumnMeta-Override + 3) field_type_defaults
    let columns = shared_columns_for(entity_type);
    let Some(col) = columns.iter().find(|c| c.key == property) else {
        return None;
    };
    let column_override = match registry {
        "filter" => col.filter_id.clone(),
        "editor" => col.editor_id.clone(),
        "formatter" => col.formatter_id.clone(),
        _ => None,
    };
    if column_override.is_some() {
        return column_override;
    }

    // 3) FieldType-Defaults aus den Settings
    let Some(settings) = settings_for_async(entity_type).await else {
        return None;
    };
    let kind = col.field_type.kind_str();
    let defaults = settings.field_type_defaults.get(kind)?;
    match registry {
        "filter" => defaults.filter_id.clone(),
        "editor" => defaults.editor_id.clone(),
        "formatter" => defaults.formatter_id.clone(),
        _ => None,
    }
}

/// Liste aller IDs, die ein User fuer eine Spalte+Registry waehlen darf.
///
/// Quelle: `field_type_defaults.allowed_X_ids` plus die Default-ID selbst.
/// Filtert nicht nach `Choose`-Permission — der Aufrufer (z.B.
/// `setImplementationChoice`) prueft das separat.
pub async fn allowed_implementations(
    entity_type: &str,
    property: &str,
    registry: RegistryKind<'_>,
) -> Vec<String> {
    let columns = shared_columns_for(entity_type);
    let Some(col) = columns.iter().find(|c| c.key == property) else {
        return Vec::new();
    };
    let Some(settings) = settings_for_async(entity_type).await else {
        return Vec::new();
    };
    let kind = col.field_type.kind_str();
    let Some(defaults) = settings.field_type_defaults.get(kind) else {
        return Vec::new();
    };
    let (default, allowed) = match registry {
        "filter" => (defaults.filter_id.clone(), defaults.allowed_filter_ids.clone()),
        "editor" => (defaults.editor_id.clone(), defaults.allowed_editor_ids.clone()),
        "formatter" => (
            defaults.formatter_id.clone(),
            defaults.allowed_formatter_ids.clone(),
        ),
        _ => (None, Vec::new()),
    };
    let mut out: Vec<String> = allowed;
    if let Some(d) = default {
        if !out.iter().any(|x| x == &d) {
            out.insert(0, d);
        }
    }
    out
}

async fn user_implementation_choice(
    user_id: &str,
    entity_type: &str,
    property: &str,
    registry: &str,
) -> Option<String> {
    entity::user_implementation_choices::Entity::find()
        .filter(entity::user_implementation_choices::Column::UserId.eq(user_id))
        .filter(entity::user_implementation_choices::Column::EntityType.eq(entity_type))
        .filter(entity::user_implementation_choices::Column::Property.eq(property))
        .filter(entity::user_implementation_choices::Column::Registry.eq(registry))
        .one(&conn())
        .await
        .ok()
        .flatten()
        .map(|m| m.chosen_id)
}

/// Test-Helper: mutiert die EntitySettings des installierten Beispiels
/// fuer einen entity_type in-place.
pub fn with_settings_mut<F>(entity_type: &str, f: F)
where
    F: FnOnce(&mut shared::EntitySettings),
{
    crate::example::mutate(|set| {
        if let Some(ets) = set.entities.get_mut(entity_type) {
            let s = ets.settings.get_or_insert_with(|| shared::EntitySettings {
                entity_type: entity_type.to_string(),
                ..Default::default()
            });
            f(s);
        }
    });
}

/// Test-Helper: mutiert die ColumnMeta-Liste eines entity_type.
pub fn with_columns_mut<F>(entity_type: &str, f: F)
where
    F: FnOnce(&mut Vec<shared::ColumnMeta>),
{
    crate::example::mutate(|set| {
        if let Some(ets) = set.entities.get_mut(entity_type) {
            f(&mut ets.columns);
        }
    });
}

/// Setzt eine Per-User-Wahl. Falls bereits ein Eintrag fuer
/// `(user_id, entity_type, property, registry)` existiert, wird er
/// ueberschrieben.
pub async fn set_user_implementation_choice(
    user_id: &str,
    entity_type: &str,
    property: &str,
    registry: &str,
    chosen_id: &str,
) -> Result<(), sea_orm::DbErr> {
    let existing = entity::user_implementation_choices::Entity::find()
        .filter(entity::user_implementation_choices::Column::UserId.eq(user_id))
        .filter(entity::user_implementation_choices::Column::EntityType.eq(entity_type))
        .filter(entity::user_implementation_choices::Column::Property.eq(property))
        .filter(entity::user_implementation_choices::Column::Registry.eq(registry))
        .one(&conn())
        .await?;
    if let Some(model) = existing {
        let mut am: entity::user_implementation_choices::ActiveModel = model.into();
        am.chosen_id = ActiveValue::Set(chosen_id.to_string());
        am.update(&conn()).await?;
    } else {
        entity::user_implementation_choices::ActiveModel {
            id: ActiveValue::NotSet,
            user_id: ActiveValue::Set(user_id.to_string()),
            entity_type: ActiveValue::Set(entity_type.to_string()),
            property: ActiveValue::Set(property.to_string()),
            registry: ActiveValue::Set(registry.to_string()),
            chosen_id: ActiveValue::Set(chosen_id.to_string()),
        }
        .insert(&conn())
        .await?;
    }
    Ok(())
}

// =============================================================================
// Phase 1.6 — entity_designs (Builder-Persistenz)
// =============================================================================
//
// Wire-Format der `state`-Spalte (vom Client geliefert):
// ```json
// { "schemaVersion": 1, "tree": { "nodes": [...] }, "projection": {...} }
// ```
//
// Der Server kennt nur den `projection`-Teil typisiert — `tree` ist eine
// opaque Vorlage fuer Phase 4 (Codegen).

/// Aktuelle Builder-State-`schemaVersion` (Phase-1.6-Format). Spaetere
/// Migrationen erhoehen das.
pub const DESIGN_SCHEMA_VERSION: i32 = 1;

/// Reservierte User-ID fuer System-Schreiber (Boot-Snapshot, Migrations-Tools).
pub const SYSTEM_USER_ID: &str = "system";

/// Legt einen System-User an, falls noch nicht vorhanden. Audit-Anker fuer
/// alle automatischen Schreiber (DB-Init, Migrationen).
pub async fn ensure_system_user(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    if entity::users::Entity::find_by_id(SYSTEM_USER_ID.to_string())
        .one(db)
        .await?
        .is_some()
    {
        return Ok(());
    }
    entity::users::ActiveModel {
        id: ActiveValue::Set(SYSTEM_USER_ID.to_string()),
        username: ActiveValue::Set(SYSTEM_USER_ID.to_string()),
        display_name: ActiveValue::Set("system".to_string()),
        locale: ActiveValue::Set(None),
        // Login blockiert.
        active: ActiveValue::Set(false),
        password_hash: ActiveValue::Set(None),
    }
    .insert(db)
    .await?;
    Ok(())
}

/// Liefert die hoechste vorhandene Version fuer einen entity_type, oder `None`
/// wenn die Tabelle leer ist.
pub async fn entity_design_latest_version(entity_type: &str) -> Option<i32> {
    use sea_orm::QueryOrder;
    entity::entity_designs::Entity::find()
        .filter(entity::entity_designs::Column::EntityType.eq(entity_type))
        .order_by_desc(entity::entity_designs::Column::Version)
        .one(&conn())
        .await
        .ok()
        .flatten()
        .map(|m| m.version)
}

/// Liefert die aktive Version (`MAX(version)`) als komplettes Model.
pub async fn entity_design_active(
    entity_type: &str,
) -> Option<entity::entity_designs::Model> {
    use sea_orm::QueryOrder;
    entity::entity_designs::Entity::find()
        .filter(entity::entity_designs::Column::EntityType.eq(entity_type))
        .order_by_desc(entity::entity_designs::Column::Version)
        .one(&conn())
        .await
        .ok()
        .flatten()
}

/// Spezifische Version laden.
pub async fn entity_design_version(
    entity_type: &str,
    version: i32,
) -> Option<entity::entity_designs::Model> {
    entity::entity_designs::Entity::find_by_id((entity_type.to_string(), version))
        .one(&conn())
        .await
        .ok()
        .flatten()
}

/// Append eine neue Version. `expected_version` ist die zuletzt vom Client
/// gesehene Version — `None` bedeutet "ich gehe davon aus, dass noch keine
/// Version existiert".
///
/// Liefert `Err(SaveDesignError::Conflict { current })` wenn die Server-Sicht
/// abweicht (klassisches optimistic locking).
#[derive(Debug)]
pub enum SaveDesignError {
    Conflict { current_version: Option<i32> },
    Locked,
    Db(sea_orm::DbErr),
}

impl From<sea_orm::DbErr> for SaveDesignError {
    fn from(e: sea_orm::DbErr) -> Self {
        SaveDesignError::Db(e)
    }
}

pub async fn save_entity_design(
    entity_type: &str,
    schema_version: i32,
    state_json: &str,
    expected_version: Option<i32>,
    created_by: &str,
) -> Result<entity::entity_designs::Model, SaveDesignError> {
    let current = entity_design_latest_version(entity_type).await;
    if current != expected_version {
        return Err(SaveDesignError::Conflict { current_version: current });
    }
    // Locked-Check: wenn die aktive Version locked ist, keine neue erlaubt.
    if let Some(active) = entity_design_active(entity_type).await {
        if active.locked {
            return Err(SaveDesignError::Locked);
        }
    }
    let next_version = current.unwrap_or(-1) + 1;
    let model = entity::entity_designs::ActiveModel {
        entity_type: ActiveValue::Set(entity_type.to_string()),
        version: ActiveValue::Set(next_version),
        state_json: ActiveValue::Set(state_json.to_string()),
        schema_version: ActiveValue::Set(schema_version),
        created_at: ActiveValue::Set(Utc::now().to_rfc3339()),
        created_by: ActiveValue::Set(created_by.to_string()),
        locked: ActiveValue::Set(false),
    }
    .insert(&conn())
    .await?;
    // Phase 1.6: Live-Reload-Subscribers benachrichtigen.
    crate::events::publish_design_update(entity_type, next_version);
    Ok(model)
}

/// Loescht alle `entity_designs`-Eintraege fuer einen `entity_type`.
/// Idempotent — Rueckgabe ist die Anzahl geloeschter Zeilen (0 wenn keine
/// existierten). Wird von `dblicious design reset` benutzt.
pub async fn delete_all_entity_designs(
    entity_type: &str,
) -> Result<u64, sea_orm::DbErr> {
    let res = entity::entity_designs::Entity::delete_many()
        .filter(entity::entity_designs::Column::EntityType.eq(entity_type))
        .exec(&conn())
        .await?;
    Ok(res.rows_affected)
}

/// Zaehlt vorhandene `entity_designs`-Eintraege fuer einen `entity_type`.
/// Dient dry-run-Anzeigen (CLI), um zu zeigen, wie viele Zeilen ein Reset
/// loeschen *wuerde*.
pub async fn count_entity_designs(
    entity_type: &str,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::PaginatorTrait;
    entity::entity_designs::Entity::find()
        .filter(entity::entity_designs::Column::EntityType.eq(entity_type))
        .count(&conn())
        .await
}

/// Schreibt den State einer alten Version als *neue* Version.
pub async fn revert_entity_design(
    entity_type: &str,
    target_version: i32,
    created_by: &str,
) -> Result<entity::entity_designs::Model, SaveDesignError> {
    let Some(target) = entity_design_version(entity_type, target_version).await else {
        return Err(SaveDesignError::Db(sea_orm::DbErr::Custom(format!(
            "version {target_version} fuer entity_type '{entity_type}' nicht gefunden"
        ))));
    };
    let current = entity_design_latest_version(entity_type).await;
    save_entity_design(
        entity_type,
        target.schema_version,
        &target.state_json,
        current,
        created_by,
    )
    .await
}

/// Boot-Snapshot: pro entity_type im Loader-Set, fuer den noch keine
/// `version=0` existiert, wird ein Snapshot aus `projection` (Loader-
/// Spalten/Editor/Settings) und einem leeren `tree.nodes`-Array geschrieben.
/// Idempotent.
pub async fn seed_entity_designs_from_example(
    db: &DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    use sea_orm::PaginatorTrait;
    let Some(set) = crate::example::current() else {
        return Ok(());
    };
    for (entity_type, ets) in &set.entities {
        let count = entity::entity_designs::Entity::find()
            .filter(entity::entity_designs::Column::EntityType.eq(entity_type.clone()))
            .count(db)
            .await
            .unwrap_or(0);
        if count > 0 {
            continue;
        }
        let projection = serde_json::json!({
            "columns":  serde_json::to_value(&ets.columns).unwrap_or(serde_json::Value::Null),
            "settings": ets.settings.as_ref().and_then(|s| serde_json::to_value(s).ok()).unwrap_or(serde_json::Value::Null),
            "editor":   ets.editor.as_ref().and_then(|e| serde_json::to_value(e).ok()).unwrap_or(serde_json::Value::Null),
        });
        let state = serde_json::json!({
            "schemaVersion": DESIGN_SCHEMA_VERSION,
            "tree": { "nodes": [] },
            "projection": projection,
        });
        entity::entity_designs::ActiveModel {
            entity_type: ActiveValue::Set(entity_type.clone()),
            version: ActiveValue::Set(0),
            state_json: ActiveValue::Set(state.to_string()),
            schema_version: ActiveValue::Set(DESIGN_SCHEMA_VERSION),
            created_at: ActiveValue::Set(Utc::now().to_rfc3339()),
            created_by: ActiveValue::Set(SYSTEM_USER_ID.to_string()),
            locked: ActiveValue::Set(false),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub async fn seed_permissions(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let Some(set) = crate::example::current() else {
        return Ok(());
    };
    for p in set.permissions {
        entity::permissions::ActiveModel {
            id: ActiveValue::NotSet,
            subject_kind: ActiveValue::Set(p.subject.kind_str().to_string()),
            subject_id: ActiveValue::Set(p.subject.id().to_string()),
            resource_kind: ActiveValue::Set(p.resource.kind_str().to_string()),
            resource_id: ActiveValue::Set(p.resource.storage_id()),
            op: ActiveValue::Set(p.op.as_str().to_string()),
            effect: ActiveValue::Set(p.effect.as_str().to_string()),
            priority: ActiveValue::Set(p.priority),
            tenant_id: ActiveValue::Set(p.tenant_id),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub async fn seed_roles(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let Some(set) = crate::example::current() else {
        return Ok(());
    };
    for r in set.roles {
        entity::roles::ActiveModel {
            id: ActiveValue::Set(r.id),
            name_key: ActiveValue::Set(r.name_key),
            description_key: ActiveValue::Set(r.description_key),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub async fn seed_role_assignments(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let Some(set) = crate::example::current() else {
        return Ok(());
    };
    for ra in set.role_assignments {
        entity::role_assignments::ActiveModel {
            id: ActiveValue::NotSet,
            subject_kind: ActiveValue::Set(ra.subject.kind_str().to_string()),
            subject_id: ActiveValue::Set(ra.subject.id().to_string()),
            role_id: ActiveValue::Set(ra.role_id),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub async fn seed_translatables(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let Some(set) = crate::example::current() else {
        return Ok(());
    };
    let bundle = set.translatables;
    for l in bundle.languages {
        entity::translatable_languages::ActiveModel {
            id: ActiveValue::Set(l.id),
            code: ActiveValue::Set(l.code),
            name_key: ActiveValue::Set(l.name_key),
            fallback_id: ActiveValue::Set(l.fallback_id),
            active: ActiveValue::Set(l.active),
        }
        .insert(db)
        .await?;
    }
    for e in bundle.entries {
        entity::translatable_entries::ActiveModel {
            id: ActiveValue::Set(e.id),
            category: ActiveValue::Set(e.category),
            description: ActiveValue::Set(e.description),
        }
        .insert(db)
        .await?;
    }
    for v in bundle.values {
        entity::translatable_values::ActiveModel {
            entry_id: ActiveValue::Set(v.entry_id),
            language_id: ActiveValue::Set(v.language_id),
            ftl_source: ActiveValue::Set(v.ftl_source),
            updated_at: ActiveValue::Set(v.updated_at),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// Spiegelt alle Entity-Seeds aus dem Beispiel in die `entities`-Tabelle.
/// Ersetzt das fruehere hartkodierte `mock_*`-Set.
pub async fn seed_entities_from_example(
    db: &DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    let Some(set) = crate::example::current() else {
        return Ok(());
    };
    for (entity_type, type_set) in &set.entities {
        for e in &type_set.seeds {
            insert_seed_entity(db, entity_type, e).await?;
        }
    }
    Ok(())
}

async fn insert_seed_entity(
    db: &DatabaseConnection,
    entity_type: &str,
    e: &shared::Entity,
) -> Result<(), sea_orm::DbErr> {
    let fields_value = serde_json::Value::Object(e.fields.clone());
    let hash = hash_for_entity(&e.id, &e.fields);
    entity::entities::ActiveModel {
        entity_type: ActiveValue::Set(entity_type.to_string()),
        id: ActiveValue::Set(e.id.clone()),
        fields_json: ActiveValue::Set(fields_value.to_string()),
        hash: ActiveValue::Set(hash),
    }
    .insert(db)
    .await?;
    Ok(())
}

// =============================================================================
// Q0005: Named Views — CRUD-Helpers
// =============================================================================

use shared::view::{EntityView, ViewLayer, ViewPropertyOverride};

/// JSON-Form, die im `payload`-Feld der `entity_views`-Tabelle liegt.
/// Nur das, was *nicht* in den eigenen Spalten redundant gehalten wird.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewPayload {
    properties: Vec<ViewPropertyOverride>,
    #[serde(default)]
    default_filter: Option<shared::FilterCriteria>,
    #[serde(default)]
    default_sort: Option<shared::Sort>,
    #[serde(default)]
    default_page_size: Option<u32>,
}

pub struct EntityViewSummary {
    pub view_name: String,
    pub layers: Vec<ViewLayer>,
    pub updated_at: String,
}

fn layer_str(l: ViewLayer) -> &'static str {
    match l {
        ViewLayer::Global => "global",
        ViewLayer::Group => "group",
        ViewLayer::User => "user",
    }
}

fn layer_from_str(s: &str) -> Option<ViewLayer> {
    match s {
        "global" => Some(ViewLayer::Global),
        "group" => Some(ViewLayer::Group),
        "user" => Some(ViewLayer::User),
        _ => None,
    }
}

fn assert_layer_invariant(layer: ViewLayer, owner_id: Option<&str>) -> Result<(), String> {
    let global_no_owner = layer == ViewLayer::Global && owner_id.is_none();
    let scoped_has_owner =
        matches!(layer, ViewLayer::Group | ViewLayer::User) && owner_id.is_some();
    if global_no_owner || scoped_has_owner {
        Ok(())
    } else {
        Err(format!(
            "Invalides Layer/Owner-Paar: layer={layer:?}, owner_id={owner_id:?}"
        ))
    }
}

pub async fn upsert_entity_view(v: &EntityView) -> Result<(), String> {
    assert_layer_invariant(v.layer, v.owner_id.as_deref())?;
    let payload = ViewPayload {
        properties: v.properties.clone(),
        default_filter: v.default_filter.clone(),
        default_sort: v.default_sort.clone(),
        default_page_size: v.default_page_size,
    };
    let payload_json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let db = &conn();
    let existing = entity::entity_views::Entity::find()
        .filter(entity::entity_views::Column::EntityType.eq(v.entity_type.clone()))
        .filter(entity::entity_views::Column::ViewName.eq(v.view_name.clone()))
        .filter(entity::entity_views::Column::Layer.eq(layer_str(v.layer)))
        .filter(match &v.owner_id {
            Some(o) => entity::entity_views::Column::OwnerId.eq(o.clone()),
            None => entity::entity_views::Column::OwnerId.is_null(),
        })
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    match existing {
        Some(row) => {
            let mut am: entity::entity_views::ActiveModel = row.into();
            am.payload = ActiveValue::Set(payload_json);
            am.version = ActiveValue::Set(v.version);
            am.updated_by = ActiveValue::Set(v.updated_by.clone());
            am.updated_at = ActiveValue::Set(v.updated_at.clone());
            am.update(db).await.map_err(|e| e.to_string())?;
        }
        None => {
            entity::entity_views::ActiveModel {
                id: ActiveValue::Set(v.id.clone()),
                entity_type: ActiveValue::Set(v.entity_type.clone()),
                view_name: ActiveValue::Set(v.view_name.clone()),
                layer: ActiveValue::Set(layer_str(v.layer).into()),
                owner_id: ActiveValue::Set(v.owner_id.clone()),
                payload: ActiveValue::Set(payload_json),
                version: ActiveValue::Set(v.version),
                updated_by: ActiveValue::Set(v.updated_by.clone()),
                updated_at: ActiveValue::Set(v.updated_at.clone()),
            }
            .insert(db)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub async fn find_entity_view(
    entity_type: &str,
    view_name: &str,
    layer: ViewLayer,
    owner_id: Option<&str>,
) -> Result<Option<EntityView>, String> {
    let db = &conn();
    let row = entity::entity_views::Entity::find()
        .filter(entity::entity_views::Column::EntityType.eq(entity_type))
        .filter(entity::entity_views::Column::ViewName.eq(view_name))
        .filter(entity::entity_views::Column::Layer.eq(layer_str(layer)))
        .filter(match owner_id {
            Some(o) => entity::entity_views::Column::OwnerId.eq(o),
            None => entity::entity_views::Column::OwnerId.is_null(),
        })
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.map(row_to_view))
}

pub async fn find_entity_views(entity_type: &str) -> Result<Vec<EntityViewSummary>, String> {
    use std::collections::BTreeMap;
    let db = &conn();
    let rows = entity::entity_views::Entity::find()
        .filter(entity::entity_views::Column::EntityType.eq(entity_type))
        .all(db)
        .await
        .map_err(|e| e.to_string())?;
    let mut by_name: BTreeMap<String, EntityViewSummary> = BTreeMap::new();
    for r in rows {
        let layer = layer_from_str(&r.layer)
            .ok_or_else(|| format!("Unbekannter Layer: '{}'", r.layer))?;
        let entry = by_name.entry(r.view_name.clone()).or_insert_with(|| EntityViewSummary {
            view_name: r.view_name.clone(),
            layers: Vec::new(),
            updated_at: r.updated_at.clone(),
        });
        if !entry.layers.contains(&layer) {
            entry.layers.push(layer);
        }
        if r.updated_at > entry.updated_at {
            entry.updated_at = r.updated_at;
        }
    }
    Ok(by_name.into_values().collect())
}

pub async fn delete_entity_view(
    entity_type: &str,
    view_name: &str,
    layer: ViewLayer,
    owner_id: Option<&str>,
) -> Result<(), String> {
    let db = &conn();
    entity::entity_views::Entity::delete_many()
        .filter(entity::entity_views::Column::EntityType.eq(entity_type))
        .filter(entity::entity_views::Column::ViewName.eq(view_name))
        .filter(entity::entity_views::Column::Layer.eq(layer_str(layer)))
        .filter(match owner_id {
            Some(o) => entity::entity_views::Column::OwnerId.eq(o),
            None => entity::entity_views::Column::OwnerId.is_null(),
        })
        .exec(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Phase F (Q0005): Loader-Bootstrap. Pro Entity-Typ aus dem aktuellen
/// `ExampleSet` mit gesetztem `EntitySettings` legen wir eine
/// `(view_name="default", layer="global", owner_id=NULL)`-Row an.
/// Idempotent — bestehende Rows bleiben unangetastet.
///
/// Achtung: Diese Funktion wird aus `db::seed_if_empty` aufgerufen, bevor
/// der Pool-Slot gesetzt ist. Sie darf deshalb `conn()` NICHT aufrufen,
/// sondern muss ausschliesslich das uebergebene `db` verwenden.
pub async fn seed_entity_views_from_example(db: &sea_orm::DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let Some(set) = crate::example::current() else { return Ok(()); };
    for (entity_type, ty_set) in &set.entities {
        let Some(settings) = ty_set.settings.as_ref() else { continue; };

        // Existiert schon? -> skip (direkt ueber db, nicht ueber conn())
        let existing = entity::entity_views::Entity::find()
            .filter(entity::entity_views::Column::EntityType.eq(entity_type.clone()))
            .filter(entity::entity_views::Column::ViewName.eq("default"))
            .filter(entity::entity_views::Column::Layer.eq("global"))
            .filter(entity::entity_views::Column::OwnerId.is_null())
            .one(db)
            .await?;
        if existing.is_some() { continue; }

        // Aus EntitySettings → ViewPropertyOverride mappen.
        let properties: Vec<shared::view::ViewPropertyOverride> = settings.properties.iter().map(|p| {
            shared::view::ViewPropertyOverride {
                key: p.key.clone(),
                visibility: Some(p.visibility),
                order: Some(p.order),
                min_width: p.min_width,
                label_override_key: p.label_override_key.clone(),
                sortable: None,
                filter_id_override: None,
                formatter_id_override: None,
            }
        }).collect();

        let payload = ViewPayload {
            properties,
            default_filter: settings.default_filter.clone(),
            default_sort: settings.default_sort.clone(),
            default_page_size: settings.default_page_size,
        };
        let payload_json = serde_json::to_string(&payload)
            .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

        entity::entity_views::ActiveModel {
            id: ActiveValue::Set(format!("v-system-{entity_type}-default-global")),
            entity_type: ActiveValue::Set(entity_type.clone()),
            view_name: ActiveValue::Set("default".into()),
            layer: ActiveValue::Set("global".into()),
            owner_id: ActiveValue::Set(None),
            payload: ActiveValue::Set(payload_json),
            version: ActiveValue::Set(0),
            updated_at: ActiveValue::Set(chrono::Utc::now().to_rfc3339()),
            updated_by: ActiveValue::Set(Some("system".into())),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

fn row_to_view(r: entity::entity_views::Model) -> EntityView {
    let payload: ViewPayload = serde_json::from_str(&r.payload).unwrap_or(ViewPayload {
        properties: Vec::new(),
        default_filter: None,
        default_sort: None,
        default_page_size: None,
    });
    EntityView {
        id: r.id,
        entity_type: r.entity_type,
        view_name: r.view_name,
        layer: layer_from_str(&r.layer).unwrap_or(ViewLayer::Global),
        owner_id: r.owner_id,
        properties: payload.properties,
        default_filter: payload.default_filter,
        default_sort: payload.default_sort,
        default_page_size: payload.default_page_size,
        version: r.version,
        updated_by: r.updated_by,
        updated_at: r.updated_at,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ensure_example() {
        if crate::example::current().is_some() {
            return;
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("examples")
            .join("shop");
        let set = crate::example::load(&dir).expect("examples/shop fuer Tests laden");
        crate::example::install(set);
    }

    async fn setup() {
        ensure_example();
        crate::db::reset();
        crate::source::reset();
        crate::db::init().await.expect("db::init() failed");
        // Phase 0.6: Routing via SourceRegistry — also tests must boot it.
        let set = crate::example::current().expect("example installed");
        crate::source::boot_registry(&set.sources)
            .await
            .expect("boot_registry");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn create_then_read_back_persists_id_and_fields() {
        setup().await;
        let entity = create_entity(
            "product",
            Some("p-test-1".into()),
            json!({"name": "Probe"}),
            None,
        )
        .await;
        assert_eq!(entity.id, "p-test-1");
        let back = entity_by_id("product", "p-test-1").await.expect("created");
        let fields = back.fields.0.as_object().unwrap().clone();
        assert_eq!(fields.get("name").unwrap().as_str(), Some("Probe"));
        assert!(delete_entity("product", "p-test-1").await);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn update_merges_patch_into_existing_fields() {
        setup().await;
        create_entity(
            "product",
            Some("p-test-2".into()),
            json!({"name": "A", "price": 10}),
            None,
        )
        .await;
        let updated = update_entity("product", "p-test-2", json!({"price": 20}), None)
            .await
            .expect("updated");
        let fields = updated.fields.0.as_object().unwrap().clone();
        assert_eq!(fields.get("name").unwrap().as_str(), Some("A"));
        assert_eq!(fields.get("price").unwrap().as_i64(), Some(20));
        delete_entity("product", "p-test-2").await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn current_hash_changes_when_fields_change() {
        setup().await;
        create_entity(
            "product",
            Some("p-test-3".into()),
            json!({"name": "A"}),
            None,
        )
        .await;
        let h1 = current_hash("product", "p-test-3").await.expect("hash");
        update_entity("product", "p-test-3", json!({"name": "B"}), None)
            .await
            .unwrap();
        let h2 = current_hash("product", "p-test-3").await.expect("hash");
        assert_ne!(h1, h2);
        delete_entity("product", "p-test-3").await;
    }

    #[tokio::test]
    async fn validate_against_editor_flags_missing_required_field() {
        // Synchron — keine DB noetig, aber das Beispiel muss geladen sein,
        // damit `editor_for("product")` Meta liefert.
        ensure_example();
        let fields = serde_json::Map::new();
        let r = validate_against_editor("product", &fields);
        assert!(r.has_blocking());
        assert!(r
            .messages
            .iter()
            .any(|m| m.target.as_deref() == Some("name")));
    }

    #[tokio::test]
    async fn validate_against_editor_passes_when_required_set() {
        ensure_example();
        let mut fields = serde_json::Map::new();
        fields.insert("name".into(), serde_json::Value::String("Foo".into()));
        let r = validate_against_editor("product", &fields);
        assert!(!r
            .for_target("name")
            .any(|m| m.severity == shared::Severity::Error));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn user_by_username_finds_admin_with_hashed_password() {
        setup().await;
        let u = user_by_username("admin").await.expect("admin existiert");
        let hash = u.password_hash.expect("Hash muss vorhanden sein");
        assert!(crate::auth::verify_password("admin", &hash));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn db_schema_round_trip_via_db() {
        setup().await;
        let schema = shared::DbSchema {
            id: "s-1".into(),
            name: "demo".into(),
            tables: vec![],
            relations: vec![],
            keys: vec![],
            indices: vec![],
        };
        persist_db_schema(&schema).await.expect("persist");
        rehydrate_db_schema().await.expect("rehydrate");
        assert_eq!(current_db_schema().unwrap().name, "demo");
    }
}
