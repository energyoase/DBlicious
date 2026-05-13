//! Tests fuer [`shared::EntityHeader`] und [`shared::compute_hash`].

use serde_json::json;
use shared::{compute_hash, Entity, EntityHeader, EntityLoadState};

fn entity(id: &str, name: &str) -> Entity {
    Entity {
        id: id.into(),
        fields: match json!({"id": id, "name": name}) {
            serde_json::Value::Object(m) => m,
            _ => unreachable!(),
        },
    }
}

#[test]
fn compute_hash_is_stable_for_identical_input() {
    let a = entity("p-1", "Foo");
    let b = entity("p-1", "Foo");
    assert_eq!(compute_hash(&a), compute_hash(&b));
}

#[test]
fn compute_hash_differs_for_different_id() {
    let a = entity("p-1", "Foo");
    let b = entity("p-2", "Foo");
    assert_ne!(compute_hash(&a), compute_hash(&b));
}

#[test]
fn compute_hash_differs_for_different_fields() {
    let a = entity("p-1", "Foo");
    let b = entity("p-1", "Bar");
    assert_ne!(compute_hash(&a), compute_hash(&b));
}

#[test]
fn header_is_clean_after_load() {
    let e = entity("p-1", "Foo");
    let h = EntityHeader::new_loaded("product", &e);
    assert!(!h.is_dirty());
    assert_eq!(h.load_state, EntityLoadState::Loaded);
}

#[test]
fn header_dirty_after_touch_then_clean_after_baseline() {
    let e1 = entity("p-1", "Foo");
    let mut h = EntityHeader::new_loaded("product", &e1);

    let e2 = entity("p-1", "Bar");
    h.touch(&e2);
    assert!(h.is_dirty(), "Aenderung muss als dirty erkannt werden");

    h.baseline();
    assert!(!h.is_dirty(), "Baseline muss den Live-Hash uebernehmen");
}
