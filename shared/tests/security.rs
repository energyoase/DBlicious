//! Tests fuer das `shared::security`-Modul.

use shared::{
    effective_permissions, is_allowed, Permission, PermissionOp, SecurityGroup, SecurityUser,
};

fn user(id: &str, group_ids: Vec<&str>, active: bool) -> SecurityUser {
    SecurityUser {
        id: id.into(),
        username: id.into(),
        display_name: id.into(),
        locale: None,
        group_ids: group_ids.into_iter().map(String::from).collect(),
        active,
        password_hash: None,
    }
}

fn perm(entity_type: &str, read: bool, write: bool, del: bool) -> Permission {
    Permission {
        entity_type: entity_type.into(),
        can_read: read,
        can_create: write,
        can_update: write,
        can_delete: del,
        min_access: shared::Access::Public,
        property_overrides: vec![],
    }
}

fn group(id: &str, permissions: Vec<Permission>) -> SecurityGroup {
    SecurityGroup {
        id: id.into(),
        name_key: format!("security.group.{id}"),
        description_key: None,
        permissions,
    }
}

#[test]
fn effective_permissions_unions_groups() {
    let u = user("u-1", vec!["editor", "viewer"], true);
    let groups = vec![
        group("editor", vec![perm("product", true, true, false)]),
        group("viewer", vec![perm("customer", true, false, false)]),
        group("noise", vec![perm("order", true, true, true)]), // nicht zugewiesen
    ];
    let effective = effective_permissions(&u, &groups);
    assert_eq!(effective.len(), 2, "nur Permissions zugewiesener Gruppen");
    assert!(effective.iter().any(|p| p.entity_type == "product"));
    assert!(effective.iter().any(|p| p.entity_type == "customer"));
}

#[test]
fn is_allowed_respects_active_flag() {
    let u = user("u-1", vec!["editor"], false);
    let groups = vec![group("editor", vec![perm("product", true, true, true)])];
    assert!(!is_allowed(&u, &groups, "product", PermissionOp::Read));
}

#[test]
fn wildcard_permission_matches_all_entity_types() {
    let u = user("u-1", vec!["admin"], true);
    let groups = vec![group("admin", vec![perm("*", true, true, true)])];
    assert!(is_allowed(&u, &groups, "product", PermissionOp::Read));
    assert!(is_allowed(&u, &groups, "anything", PermissionOp::Delete));
}

#[test]
fn permission_op_distinction_is_enforced() {
    let u = user("u-1", vec!["g"], true);
    let groups = vec![group("g", vec![perm("product", true, true, false)])];
    assert!(is_allowed(&u, &groups, "product", PermissionOp::Read));
    assert!(is_allowed(&u, &groups, "product", PermissionOp::Create));
    assert!(is_allowed(&u, &groups, "product", PermissionOp::Update));
    assert!(!is_allowed(&u, &groups, "product", PermissionOp::Delete));
}
