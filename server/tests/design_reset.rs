//! Phase 1.6: `data::{count_entity_designs, delete_all_entity_designs}` —
//! die Server-Bausteine fuer `dblicious design reset <entity_type>`.

use serial_test::serial;
use server::data;

#[tokio::test]
#[serial]
async fn delete_all_entity_designs_is_idempotent_on_empty() {
    server::fresh_test_setup().await;
    // Frisch — kein Eintrag fuer diesen Typ.
    let count_before = data::count_entity_designs("__nope__").await.unwrap();
    assert_eq!(count_before, 0);
    let deleted = data::delete_all_entity_designs("__nope__").await.unwrap();
    assert_eq!(deleted, 0, "Idempotent: 0 vorhanden ⇒ 0 geloescht");
}

#[tokio::test]
#[serial]
async fn delete_all_entity_designs_removes_seeded_versions() {
    // fresh_test_setup laedt `examples/shop`, das per
    // `seed_entity_designs_from_example` Version 0 fuer jeden Entity-Typ
    // anlegt. Wir loeschen sie wieder.
    server::fresh_test_setup().await;
    let _ = data::seed_entity_designs_from_example(&server::db::conn()).await;
    let count_before = data::count_entity_designs("product").await.unwrap();
    assert!(
        count_before >= 1,
        "Erwarte mindestens Seed-Version 0 fuer 'product', got {count_before}"
    );
    let deleted = data::delete_all_entity_designs("product").await.unwrap();
    assert_eq!(deleted, count_before);
    let count_after = data::count_entity_designs("product").await.unwrap();
    assert_eq!(count_after, 0);
}

#[tokio::test]
#[serial]
async fn delete_all_entity_designs_only_affects_target_type() {
    server::fresh_test_setup().await;
    let _ = data::seed_entity_designs_from_example(&server::db::conn()).await;
    let before_product = data::count_entity_designs("product").await.unwrap();
    let before_customer = data::count_entity_designs("customer").await.unwrap();
    assert!(before_product >= 1 && before_customer >= 1);
    let deleted = data::delete_all_entity_designs("product").await.unwrap();
    assert_eq!(deleted, before_product);
    // andere Typen sind unangetastet
    assert_eq!(
        data::count_entity_designs("customer").await.unwrap(),
        before_customer
    );
}
