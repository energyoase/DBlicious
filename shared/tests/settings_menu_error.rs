//! Tests fuer Settings, Menu, Error und Mutation-DTOs.

use shared::{
    AppError, EntityChangeResult, EntityCreate, EntityDelete, EntitySettings, EntityUpdate,
    MenuAction, NavigationNode, PropertySettings, SettingsBundle, Visibility,
};

#[test]
fn settings_ensure_property_creates_default_entry() {
    let mut s = EntitySettings::default();
    let p = s.ensure_property("name");
    assert_eq!(p.key, "name");
    assert_eq!(p.visibility, Visibility::Visible);
    assert_eq!(s.properties.len(), 1);

    // Idempotent: ein zweiter Aufruf legt nichts neues an.
    let p = s.ensure_property("name");
    p.visibility = Visibility::Hidden;
    assert_eq!(s.properties.len(), 1);
    assert_eq!(s.property("name").unwrap().visibility, Visibility::Hidden);
}

#[test]
fn settings_bundle_ensure_returns_existing_entity() {
    let mut b = SettingsBundle::default();
    {
        let es = b.ensure("product");
        es.default_page_size = Some(25);
    }
    let again = b.ensure("product");
    assert_eq!(again.default_page_size, Some(25));
}

#[test]
fn menu_action_route_round_trip() {
    let a = MenuAction::from_route(Some("/foo".into()));
    assert_eq!(a.route(), Some("/foo"));
    let none = MenuAction::from_route(None);
    assert!(matches!(none, MenuAction::None));
}

#[test]
fn navigation_node_resolves_to_link_when_route_set() {
    let n = NavigationNode {
        id: "x".into(),
        label_key: "x".into(),
        route: Some("/x".into()),
        icon: None,
        children: vec![],
        action: None,
    };
    match n.resolved_action() {
        MenuAction::Link { route } => assert_eq!(route, "/x"),
        _ => panic!("erwartete Link-Action"),
    }
}

#[test]
fn navigation_node_explicit_action_overrides_route() {
    let n = NavigationNode {
        id: "x".into(),
        label_key: "x".into(),
        route: Some("/old".into()),
        icon: None,
        children: vec![],
        action: Some(MenuAction::Tab {
            tab_id: "t-1".into(),
            focus_existing: true,
        }),
    };
    match n.resolved_action() {
        MenuAction::Tab { tab_id, .. } => assert_eq!(tab_id, "t-1"),
        _ => panic!("erwartete Tab-Action"),
    }
}

#[test]
fn app_error_message_keys_are_stable() {
    assert_eq!(
        AppError::Decode { detail: "x".into() }.message_key(),
        "error.decode"
    );
    assert_eq!(
        AppError::Network { detail: "x".into() }.message_key(),
        "error.network"
    );
    assert_eq!(
        AppError::Validation { messages: 3 }.message_key(),
        "error.validation"
    );
}

#[test]
fn mutation_dtos_round_trip_through_json() {
    let create = EntityCreate {
        entity_type: "product".into(),
        id: None,
        fields: serde_json::Map::new(),
    };
    let json = serde_json::to_value(&create).unwrap();
    let back: EntityCreate = serde_json::from_value(json).unwrap();
    assert_eq!(back, create);

    let update = EntityUpdate {
        entity_type: "product".into(),
        id: "p-1".into(),
        fields: serde_json::Map::new(),
        expected_hash: Some(42),
    };
    let json = serde_json::to_value(&update).unwrap();
    let back: EntityUpdate = serde_json::from_value(json).unwrap();
    assert_eq!(back, update);

    let del = EntityDelete {
        entity_type: "product".into(),
        id: "p-1".into(),
        expected_hash: None,
    };
    let json = serde_json::to_value(&del).unwrap();
    let back: EntityDelete = serde_json::from_value(json).unwrap();
    assert_eq!(back, del);
}

#[test]
fn entity_change_result_success_constructor_sets_flags() {
    use shared::Entity;
    let e = Entity {
        id: "p-1".into(),
        fields: serde_json::Map::new(),
    };
    let r = EntityChangeResult::success(e.clone());
    assert!(r.ok);
    assert_eq!(r.entity.as_ref().unwrap().id, "p-1");
    assert!(r.validation.is_empty());
}

#[test]
fn property_settings_default_is_safe() {
    let p = PropertySettings::default();
    assert_eq!(p.visibility, Visibility::Visible);
    assert_eq!(p.access, shared::PropertyAccess::ReadWrite);
}
