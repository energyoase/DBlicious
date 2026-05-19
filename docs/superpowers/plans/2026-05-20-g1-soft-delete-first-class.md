# G1 — Soft-Delete as First-Class Concept Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `deleted_at`-by-convention workaround with native soft-delete support: `AuditRole::DeletedAt`/`DeletedBy`, an `EntitySettings.soft_delete` opt-in, automatic server-side filtering, plus `restoreEntity` / `purgeDeletedEntity` mutations and a `Restore` permission op.

**Architecture:**
- Add two `AuditRole` variants. Server auto-fills `DeletedAt` (timestamp) and `DeletedBy` (user id) on `delete_entity` when the entity type has `soft_delete = true`; otherwise hard-delete preserves today's behavior.
- Default queries get a `WHERE deleted_at IS NULL` predicate when the entity has a `DeletedAt` audit role; new mutations `restoreEntity` and `purgeDeletedEntity` invert / finalize the delete.
- `Permission.can_restore` + `PermissionOp::Restore` extend the existing permission matrix.

**Tech Stack:** Rust (workspace crates `shared`, `server`), serde for wire format, SeaORM for persistence, async-graphql for the resolver.

**Source spec:** `docs/superpowers/specs/2026-05-20-dblicious-schema-language-gaps.md` §G1.

---

## File Structure

- Modify: `shared/src/lib.rs` (lines 374–395) — add `AuditRole::DeletedAt` / `DeletedBy`, plus a `fills_on_delete` helper.
- Modify: `shared/src/settings.rs` (`EntitySettings` struct around line 121) — add `soft_delete: bool` with `#[serde(default)]`.
- Modify: `shared/src/security.rs` — add `PermissionOp::Restore` + `Permission.can_restore`.
- Modify: `shared/tests/field_type_wire_format.rs` (or new sibling test) — wire-format pin for the new variants.
- Modify: `shared/tests/db_schema_roundtrip.rs` — extend roundtrip with `DeletedAt` column.
- Modify: `shared/tests/security.rs` — extend permission matrix tests for `Restore`.
- Modify: `server/src/data.rs` — `delete_entity` branches on `soft_delete`; new `restore_entity` and `purge_deleted_entity`; default filter adds `deleted_at IS NULL` when applicable.
- Modify: `server/src/schema.rs` — expose `restoreEntity` and `purgeDeletedEntity` GraphQL mutations; check `PermissionOp::Restore`.
- Create: `server/tests/soft_delete.rs` — end-to-end behavior for soft-delete, restore, purge, and default-filter exclusion.

---

## Task 1: Wire format for `AuditRole::DeletedAt` / `DeletedBy`

**Files:**
- Modify: `shared/src/lib.rs:374-395`
- Test: `shared/tests/audit_role_wire_format.rs` (new)

- [ ] **Step 1: Write the failing test**

Create new file `shared/tests/audit_role_wire_format.rs`:

```rust
//! Pins the wire form of `AuditRole`, especially the new soft-delete variants.

use serde_json::json;
use shared::AuditRole;

#[test]
fn deleted_at_serializes_as_camel_case() {
    let v = serde_json::to_value(AuditRole::DeletedAt).unwrap();
    assert_eq!(v, json!("deletedAt"));
}

#[test]
fn deleted_by_serializes_as_camel_case() {
    let v = serde_json::to_value(AuditRole::DeletedBy).unwrap();
    assert_eq!(v, json!("deletedBy"));
}

#[test]
fn fills_on_delete_is_true_only_for_delete_roles() {
    assert!(AuditRole::DeletedAt.fills_on_delete());
    assert!(AuditRole::DeletedBy.fills_on_delete());
    assert!(!AuditRole::CreatedAt.fills_on_delete());
    assert!(!AuditRole::UpdatedAt.fills_on_delete());
    assert!(!AuditRole::None.fills_on_delete());
}

#[test]
fn delete_roles_do_not_fill_on_create_or_update() {
    assert!(!AuditRole::DeletedAt.fills_on_create());
    assert!(!AuditRole::DeletedAt.fills_on_update());
    assert!(!AuditRole::DeletedBy.fills_on_create());
    assert!(!AuditRole::DeletedBy.fills_on_update());
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p shared --test audit_role_wire_format`
Expected: compile error — `DeletedAt`/`DeletedBy` variants don't exist.

- [ ] **Step 3: Add the variants and helper**

Edit `shared/src/lib.rs` around the `AuditRole` enum (lines 374–395):

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AuditRole {
    #[default]
    None,
    CreatedAt,
    UpdatedAt,
    CreatedBy,
    UpdatedBy,
    DeletedAt,
    DeletedBy,
}

impl AuditRole {
    pub fn fills_on_create(self) -> bool {
        matches!(
            self,
            AuditRole::CreatedAt | AuditRole::UpdatedAt | AuditRole::CreatedBy | AuditRole::UpdatedBy
        )
    }
    pub fn fills_on_update(self) -> bool {
        matches!(self, AuditRole::UpdatedAt | AuditRole::UpdatedBy)
    }
    pub fn fills_on_delete(self) -> bool {
        matches!(self, AuditRole::DeletedAt | AuditRole::DeletedBy)
    }
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cargo test -p shared --test audit_role_wire_format`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add shared/src/lib.rs shared/tests/audit_role_wire_format.rs
git commit -m "feat(shared): AuditRole::DeletedAt + DeletedBy + fills_on_delete"
```

---

## Task 2: `EntitySettings.soft_delete` opt-in flag

**Files:**
- Modify: `shared/src/settings.rs:119-141`
- Test: `shared/tests/settings_menu_error.rs` (extend) **or** new `shared/tests/settings_soft_delete.rs`

- [ ] **Step 1: Write the failing test**

Create new file `shared/tests/settings_soft_delete.rs`:

```rust
use serde_json::json;
use shared::EntitySettings;

#[test]
fn soft_delete_defaults_to_false_when_missing() {
    let s: EntitySettings = serde_json::from_value(json!({
        "entityType": "product"
    }))
    .unwrap();
    assert!(!s.soft_delete);
}

#[test]
fn soft_delete_roundtrips() {
    let s = EntitySettings {
        entity_type: "product".into(),
        soft_delete: true,
        ..Default::default()
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: EntitySettings = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
    assert!(json.contains("\"softDelete\":true"));
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p shared --test settings_soft_delete`
Expected: compile error — `soft_delete` field doesn't exist.

- [ ] **Step 3: Add the field**

Edit `shared/src/settings.rs` inside `EntitySettings` (around line 121):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntitySettings {
    pub entity_type: String,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub default_page_size: Option<u32>,
    #[serde(default)]
    pub default_sort: Option<Sort>,
    #[serde(default)]
    pub default_filter: Option<FilterCriteria>,
    #[serde(default)]
    pub properties: Vec<PropertySettings>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_type_defaults: BTreeMap<String, FieldTypeDefaults>,
    /// Phase 0.7-G1: Opt-in. When true, `delete` mutations set the
    /// `DeletedAt`/`DeletedBy` audit columns instead of removing the row,
    /// and the default list query filters `WHERE deleted_at IS NULL`.
    #[serde(default)]
    pub soft_delete: bool,
}
```

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cargo test -p shared --test settings_soft_delete`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add shared/src/settings.rs shared/tests/settings_soft_delete.rs
git commit -m "feat(shared): EntitySettings.soft_delete opt-in flag"
```

---

## Task 3: `PermissionOp::Restore` + `Permission.can_restore`

**Files:**
- Modify: `shared/src/security.rs` — extend `PermissionOp` and `Permission` (around line 140–157).
- Test: `shared/tests/security.rs` (extend) **or** new `shared/tests/permission_restore.rs`.

- [ ] **Step 1: Write the failing test**

Create new file `shared/tests/permission_restore.rs`:

```rust
use shared::{is_allowed, AuthSession, Permission, PermissionOp};

fn perm(can_restore: bool) -> Permission {
    Permission {
        entity_type: "product".into(),
        can_read: true,
        can_create: false,
        can_update: false,
        can_delete: false,
        can_restore,
        ..Default::default()
    }
}

#[test]
fn restore_permission_is_independent_of_delete() {
    let session = AuthSession {
        user_id: "u1".into(),
        groups: vec![],
        permissions: vec![perm(true)],
        ..Default::default()
    };
    assert!(is_allowed(&session, "product", PermissionOp::Restore));

    let session_no_restore = AuthSession {
        user_id: "u1".into(),
        groups: vec![],
        permissions: vec![perm(false)],
        ..Default::default()
    };
    assert!(!is_allowed(
        &session_no_restore,
        "product",
        PermissionOp::Restore
    ));
}
```

- [ ] **Step 2: Run the test and confirm it fails**

Run: `cargo test -p shared --test permission_restore`
Expected: compile error — `PermissionOp::Restore` and `Permission.can_restore` don't exist. Some `Default` calls may need fields added.

- [ ] **Step 3: Extend the enum and struct**

Edit `shared/src/security.rs`. Add `Restore` to `PermissionOp` and `can_restore: bool` to `Permission` (mirror existing `can_delete`). Update the `is_allowed` match:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionOp {
    Read,
    Create,
    Update,
    Delete,
    Restore,
}

// In Permission struct, after can_delete:
#[serde(default)]
pub can_restore: bool,

// In is_allowed:
PermissionOp::Restore => p.can_restore,
```

If you find unrelated callsites that construct `Permission { ... }` without `..Default::default()`, add the new field with `false`.

- [ ] **Step 4: Run the test and confirm it passes**

Run: `cargo test -p shared --test permission_restore`
Expected: PASS.

- [ ] **Step 5: Run the full shared suite to catch fallout**

Run: `cargo test -p shared`
Expected: PASS. Fix any callsites that destructure `Permission`.

- [ ] **Step 6: Commit**

```bash
git add shared/src/security.rs shared/tests/permission_restore.rs
git commit -m "feat(shared): PermissionOp::Restore + Permission.can_restore"
```

---

## Task 4: Server `delete_entity` honors `soft_delete`

**Files:**
- Modify: `server/src/data.rs` — `delete_entity` function.
- Test: `server/tests/soft_delete.rs` (new).

- [ ] **Step 1: Read the current `delete_entity` to understand inputs**

Run: `cargo run -p server -- --help` is not relevant; instead grep the function.

Read the function via the Read tool: look at `server/src/data.rs` for `pub async fn delete_entity`.

- [ ] **Step 2: Write the failing test**

Create new file `server/tests/soft_delete.rs`:

```rust
//! End-to-end soft-delete behavior. Uses the in-memory test setup from
//! `server::lib::fresh_test_setup` so each test starts with a clean DB.

use serial_test::serial;
use server::{data, example, fresh_test_setup};
use shared::EntitySettings;

const ENTITY: &str = "product";

async fn enable_soft_delete() {
    // Install a settings bundle that opts `product` into soft-delete.
    let mut bundle = shared::SettingsBundle::default();
    let s = bundle.ensure(ENTITY);
    s.soft_delete = true;
    example::install_settings_for_tests(bundle);
}

#[tokio::test]
#[serial]
async fn soft_delete_sets_deleted_at_and_hides_from_list() {
    fresh_test_setup().await;
    enable_soft_delete().await;

    // Create one product
    let created = data::create_entity(
        ENTITY,
        serde_json::json!({"name": "T-Shirt"}).as_object().unwrap().clone(),
    )
    .await
    .expect("create");

    // Soft-delete
    let ok = data::delete_entity(ENTITY, &created.id).await;
    assert!(ok, "delete_entity should return true");

    // Default list excludes the soft-deleted row
    let page = data::list_entities(ENTITY, None, None, None, None, None).await;
    assert!(
        page.entities.iter().all(|e| e.id != created.id),
        "soft-deleted entity must not appear in default listing"
    );

    // Including-deleted listing returns it with deleted_at populated
    let page_all = data::list_entities_including_deleted(ENTITY).await;
    let row = page_all
        .entities
        .iter()
        .find(|e| e.id == created.id)
        .expect("soft-deleted row should be retrievable with include_deleted");
    assert!(
        row.fields.get("deleted_at").is_some(),
        "deleted_at must be set on soft-deleted row"
    );
}

#[tokio::test]
#[serial]
async fn hard_delete_when_soft_delete_disabled() {
    fresh_test_setup().await;
    // No `enable_soft_delete()` call: default is hard-delete.
    let created = data::create_entity(
        ENTITY,
        serde_json::json!({"name": "T-Shirt"}).as_object().unwrap().clone(),
    )
    .await
    .expect("create");

    let ok = data::delete_entity(ENTITY, &created.id).await;
    assert!(ok);

    let page_all = data::list_entities_including_deleted(ENTITY).await;
    assert!(
        page_all.entities.iter().all(|e| e.id != created.id),
        "hard-delete must remove the row entirely"
    );
}
```

- [ ] **Step 3: Run the test and confirm it fails**

Run: `cargo test -p server --test soft_delete --target-dir target-test`
Expected: compile error — `data::list_entities_including_deleted`, `example::install_settings_for_tests` don't exist; `delete_entity` returns success but doesn't soft-delete.

- [ ] **Step 4: Implement the soft-delete branch in `data.rs`**

In `server/src/data.rs`:

```rust
pub async fn delete_entity(entity_type: &str, id: &str) -> bool {
    let opts_in = example::current_settings()
        .as_ref()
        .and_then(|b| b.get(entity_type))
        .map(|s| s.soft_delete)
        .unwrap_or(false);

    if opts_in {
        let now = chrono::Utc::now().to_rfc3339();
        match update_entity_internal(entity_type, id, |fields| {
            fields.insert("deleted_at".into(), serde_json::Value::String(now.clone()));
            // current_user_id() is the same helper that fills CreatedBy/UpdatedBy.
            if let Some(user) = current_user_id() {
                fields.insert("deleted_by".into(), serde_json::Value::String(user));
            }
        })
        .await
        {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(target: "server::data", "soft delete failed: {e}");
                false
            }
        }
    } else {
        hard_delete_entity(entity_type, id).await
    }
}

pub async fn list_entities_including_deleted(entity_type: &str) -> shared::EntityPage {
    // Bypasses the default `deleted_at IS NULL` filter.
    list_entities_internal(entity_type, /* include_deleted = */ true, None, None, None, None, None)
        .await
}
```

Then in the default `list_entities` path, when the entity type opts into soft-delete, append `WHERE json_extract(fields, '$.deleted_at') IS NULL` to the query (or the equivalent for the storage layout actually used in `data.rs`). Mirror today's filter assembly.

- [ ] **Step 5: Add the test helper `install_settings_for_tests`**

In `server/src/example/mod.rs` (next to `setup_for_tests`), add:

```rust
#[cfg(any(test, feature = "test-helpers"))]
pub fn install_settings_for_tests(bundle: shared::SettingsBundle) {
    INSTALLED.write().expect("settings lock").as_mut().map(|set| {
        set.settings = bundle;
    });
}

pub fn current_settings() -> Option<std::sync::Arc<shared::SettingsBundle>> {
    // Replace the body with whatever read-accessor the example crate already
    // exposes; this is a thin wrapper around the existing `INSTALLED` slot.
    INSTALLED
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|set| set.settings.clone().into()))
}
```

If the existing `example::install` already stores settings under the per-set slot, expose a getter rather than mutate the slot directly.

- [ ] **Step 6: Run the test and confirm it passes**

Run: `cargo test -p server --test soft_delete --target-dir target-test`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add server/src/data.rs server/src/example/mod.rs server/tests/soft_delete.rs
git commit -m "feat(server): soft delete via EntitySettings.soft_delete + DeletedAt audit role"
```

---

## Task 5: GraphQL `restoreEntity` + `purgeDeletedEntity`

**Files:**
- Modify: `server/src/data.rs` — add `restore_entity` and `purge_deleted_entity`.
- Modify: `server/src/schema.rs` — wire the two mutations into `MutationRoot`.
- Test: `server/tests/soft_delete.rs` (extend).

- [ ] **Step 1: Add restore/purge tests**

Append to `server/tests/soft_delete.rs`:

```rust
#[tokio::test]
#[serial]
async fn restore_clears_deleted_at() {
    fresh_test_setup().await;
    enable_soft_delete().await;

    let created = data::create_entity(
        ENTITY,
        serde_json::json!({"name": "T"}).as_object().unwrap().clone(),
    )
    .await
    .expect("create");
    assert!(data::delete_entity(ENTITY, &created.id).await);

    let ok = data::restore_entity(ENTITY, &created.id).await;
    assert!(ok, "restore_entity should return true");

    let page = data::list_entities(ENTITY, None, None, None, None, None).await;
    assert!(
        page.entities.iter().any(|e| e.id == created.id),
        "restored row must appear in default listing"
    );
}

#[tokio::test]
#[serial]
async fn purge_removes_row_permanently() {
    fresh_test_setup().await;
    enable_soft_delete().await;

    let created = data::create_entity(
        ENTITY,
        serde_json::json!({"name": "T"}).as_object().unwrap().clone(),
    )
    .await
    .expect("create");
    assert!(data::delete_entity(ENTITY, &created.id).await);

    let ok = data::purge_deleted_entity(ENTITY, &created.id).await;
    assert!(ok);

    let page_all = data::list_entities_including_deleted(ENTITY).await;
    assert!(
        page_all.entities.iter().all(|e| e.id != created.id),
        "purged row must be gone"
    );
}

#[tokio::test]
#[serial]
async fn purge_refuses_when_not_soft_deleted() {
    fresh_test_setup().await;
    enable_soft_delete().await;

    let created = data::create_entity(
        ENTITY,
        serde_json::json!({"name": "T"}).as_object().unwrap().clone(),
    )
    .await
    .expect("create");

    let ok = data::purge_deleted_entity(ENTITY, &created.id).await;
    assert!(!ok, "purge must not touch live (un-deleted) rows");
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p server --test soft_delete --target-dir target-test`
Expected: compile error on `data::restore_entity` and `data::purge_deleted_entity`.

- [ ] **Step 3: Implement the two functions in `data.rs`**

```rust
pub async fn restore_entity(entity_type: &str, id: &str) -> bool {
    match update_entity_internal(entity_type, id, |fields| {
        fields.insert("deleted_at".into(), serde_json::Value::Null);
        fields.insert("deleted_by".into(), serde_json::Value::Null);
    })
    .await
    {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!(target: "server::data", "restore failed: {e}");
            false
        }
    }
}

pub async fn purge_deleted_entity(entity_type: &str, id: &str) -> bool {
    // Refuse to purge a row that hasn't been soft-deleted first.
    let row = match get_entity(entity_type, id).await {
        Some(r) => r,
        None => return false,
    };
    let is_deleted = row
        .fields
        .get("deleted_at")
        .map(|v| !v.is_null())
        .unwrap_or(false);
    if !is_deleted {
        return false;
    }
    hard_delete_entity(entity_type, id).await
}
```

- [ ] **Step 4: Wire mutations into GraphQL**

In `server/src/schema.rs` `MutationRoot`:

```rust
async fn restore_entity(
    &self,
    ctx: &Context<'_>,
    entity_type: String,
    id: String,
) -> async_graphql::Result<EntityChangeResult> {
    require_permission(ctx, &entity_type, shared::PermissionOp::Restore).await?;
    let ok = data::restore_entity(&entity_type, &id).await;
    Ok(EntityChangeResult {
        ok,
        entity: None,
        validation: Json(serde_json::Value::Null),
    })
}

async fn purge_deleted_entity(
    &self,
    ctx: &Context<'_>,
    entity_type: String,
    id: String,
) -> async_graphql::Result<EntityChangeResult> {
    // Purge is a hard-delete; treat it as the Delete op.
    require_permission(ctx, &entity_type, shared::PermissionOp::Delete).await?;
    let ok = data::purge_deleted_entity(&entity_type, &id).await;
    Ok(EntityChangeResult {
        ok,
        entity: None,
        validation: Json(serde_json::Value::Null),
    })
}
```

- [ ] **Step 5: Run the tests and confirm they pass**

Run: `cargo test -p server --test soft_delete --target-dir target-test`
Expected: PASS (5 tests total).

- [ ] **Step 6: Run the workspace suite**

Run: `cargo test --workspace --target-dir target-test`
Expected: PASS. Fix any compilation breaks in callers of the changed signatures.

- [ ] **Step 7: Commit**

```bash
git add server/src/data.rs server/src/schema.rs server/tests/soft_delete.rs
git commit -m "feat(server): restoreEntity + purgeDeletedEntity GraphQL mutations"
```

---

## Notes for the implementer

- The exact storage layout in `server/src/data.rs` is the generic `entities` table (see CLAUDE.md "Persistenz via SeaORM"). The `deleted_at` filter therefore reads through `json_extract`-like access on the `fields` JSON column. If the implementer-side data layer evolved away from JSON storage during Phase 0.6 / Source-Architecture work, mirror today's filter assembly idiom rather than this plan's example SQL.
- `current_user_id()` is the same helper the existing audit-fill code uses; if it lives under another name (e.g. `audit::actor()`), follow the existing call sites.
- The mutation `delete_entities` (bulk, see `server/src/schema.rs:1506`) should call the same `data::delete_entity` and inherits soft-delete automatically.
