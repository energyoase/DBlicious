//! Q0004 Option C: jeder Entity-Link bekommt einen Designer-Sub-Link.

use shared::{augment_with_designer_links, MenuAction, NavigationNode};

fn link(id: &str, route: &str) -> NavigationNode {
    NavigationNode {
        id: id.into(),
        label_key: format!("nav.{id}"),
        route: Some(route.into()),
        icon: None,
        children: Vec::new(),
        action: None,
    }
}

fn group(id: &str, children: Vec<NavigationNode>) -> NavigationNode {
    NavigationNode {
        id: id.into(),
        label_key: format!("nav.{id}"),
        route: None,
        icon: None,
        children,
        action: None,
    }
}

#[test]
fn entity_link_gets_designer_child() {
    let tree = vec![link("products", "/entities/product")];
    let out = augment_with_designer_links(tree);
    assert_eq!(out.len(), 1);
    let p = &out[0];
    assert_eq!(p.children.len(), 1);
    let designer = &p.children[0];
    assert_eq!(designer.id, "products__designer");
    assert_eq!(designer.label_key, "nav.builder");
    assert_eq!(
        designer.resolved_action(),
        MenuAction::Link {
            route: "/builder/product".into()
        }
    );
}

#[test]
fn non_entity_link_is_untouched() {
    let tree = vec![link("dashboard", "/")];
    let out = augment_with_designer_links(tree);
    assert!(out[0].children.is_empty());
}

#[test]
fn group_recurses_into_children() {
    let tree = vec![group(
        "catalog",
        vec![
            link("products", "/entities/product"),
            link("categories", "/entities/category"),
        ],
    )];
    let out = augment_with_designer_links(tree);
    let catalog = &out[0];
    assert!(catalog.children.iter().any(|c| c.id == "products"));
    let products = catalog
        .children
        .iter()
        .find(|c| c.id == "products")
        .unwrap();
    assert_eq!(products.children.len(), 1);
    assert_eq!(products.children[0].id, "products__designer");
    let categories = catalog
        .children
        .iter()
        .find(|c| c.id == "categories")
        .unwrap();
    assert_eq!(categories.children[0].id, "categories__designer");
}

#[test]
fn idempotent_when_designer_already_present() {
    let mut existing_designer = link("products", "/entities/product");
    existing_designer.children.push(NavigationNode {
        id: "products__designer".into(),
        label_key: "custom-label".into(),
        route: Some("/builder/product".into()),
        icon: None,
        children: Vec::new(),
        action: None,
    });
    let out = augment_with_designer_links(vec![existing_designer]);
    let products = &out[0];
    // Existing children-Wert wird nicht dupliziert.
    assert_eq!(products.children.len(), 1);
    assert_eq!(products.children[0].label_key, "custom-label");
}
