//! Persistenz-Tests fuer entity_views (Q0005).

use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, PaginatorTrait};
use server::{entity, fresh_test_setup};

#[tokio::test]
#[serial_test::serial]
async fn entity_views_table_exists_after_init() {
    let _ = fresh_test_setup().await;
    let db = server::db::conn();
    let count = entity::entity_views::Entity::find()
        .count(&db)
        .await
        .expect("query");
    // Nach Phase F (Q0005) legt seed_entity_views_from_example beim Init
    // eine Row pro Entity-Typ mit Settings an — Tabelle ist nicht mehr leer.
    assert!(
        count >= 3,
        "Tabelle existiert und hat mindestens 3 Loader-Rows (product/order/customer)"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn insert_and_read_back_a_view_row() {
    let _ = fresh_test_setup().await;
    let db = server::db::conn();
    // Verwende "manual-test" als view_name, damit kein Konflikt mit der
    // durch F1 (seed_entity_views_from_example) angelegten "default"-Row entsteht.
    entity::entity_views::ActiveModel {
        id: ActiveValue::Set("v-1".into()),
        entity_type: ActiveValue::Set("order".into()),
        view_name: ActiveValue::Set("manual-test".into()),
        layer: ActiveValue::Set("global".into()),
        owner_id: ActiveValue::Set(None),
        payload: ActiveValue::Set("{\"properties\":[]}".into()),
        version: ActiveValue::Set(1),
        updated_by: ActiveValue::Set(Some("system".into())),
        updated_at: ActiveValue::Set("2026-05-21T00:00:00Z".into()),
    }
    .insert(&db)
    .await
    .expect("insert");

    let row = entity::entity_views::Entity::find_by_id("v-1".to_string())
        .one(&db)
        .await
        .expect("query")
        .expect("found");
    assert_eq!(row.entity_type, "order");
    assert_eq!(row.layer, "global");
    assert_eq!(row.version, 1);
}

// =============================================================================
// C1 — CRUD-Helpers
// =============================================================================

use server::data;
use shared::view::{EntityView, ViewLayer, ViewPropertyOverride};
use shared::Visibility;

fn sample_view(name: &str, layer: ViewLayer, owner_id: Option<&str>) -> EntityView {
    EntityView {
        id: format!("v-{name}-{layer:?}"),
        entity_type: "order".into(),
        view_name: name.into(),
        layer,
        owner_id: owner_id.map(String::from),
        properties: vec![ViewPropertyOverride {
            key: "amount".into(),
            visibility: Some(Visibility::Visible),
            ..Default::default()
        }],
        default_filter: None,
        default_sort: None,
        default_page_size: None,
        version: 1,
        updated_at: "2026-05-21T00:00:00Z".into(),
        updated_by: Some("system".into()),
    }
}

#[tokio::test]
#[serial_test::serial]
async fn upsert_view_inserts_then_updates() {
    let _ = server::fresh_test_setup().await;
    let v = sample_view("default", ViewLayer::Global, None);
    data::upsert_entity_view(&v).await.expect("insert");
    let read = data::find_entity_view("order", "default", ViewLayer::Global, None)
        .await
        .expect("read");
    assert_eq!(read.unwrap().version, 1);

    let mut v2 = v.clone();
    v2.version = 2;
    data::upsert_entity_view(&v2).await.expect("update");
    let read2 = data::find_entity_view("order", "default", ViewLayer::Global, None)
        .await
        .expect("read");
    assert_eq!(read2.unwrap().version, 2);
}

#[tokio::test]
#[serial_test::serial]
async fn find_views_for_entity_lists_per_layer() {
    let _ = server::fresh_test_setup().await;
    data::upsert_entity_view(&sample_view("default", ViewLayer::Global, None))
        .await
        .unwrap();
    data::upsert_entity_view(&sample_view("default", ViewLayer::Group, Some("g-1")))
        .await
        .unwrap();
    data::upsert_entity_view(&sample_view("default", ViewLayer::User, Some("u-1")))
        .await
        .unwrap();
    let summary = data::find_entity_views("order").await.unwrap();
    assert_eq!(summary.len(), 1, "Eine View namens 'default'");
    assert!(summary[0].layers.contains(&ViewLayer::Global));
    assert!(summary[0].layers.contains(&ViewLayer::Group));
    assert!(summary[0].layers.contains(&ViewLayer::User));
}

#[tokio::test]
#[serial_test::serial]
async fn delete_view_removes_only_target_layer() {
    let _ = server::fresh_test_setup().await;
    data::upsert_entity_view(&sample_view("default", ViewLayer::Global, None))
        .await
        .unwrap();
    data::upsert_entity_view(&sample_view("default", ViewLayer::User, Some("u-1")))
        .await
        .unwrap();
    data::delete_entity_view("order", "default", ViewLayer::User, Some("u-1"))
        .await
        .unwrap();
    assert!(
        data::find_entity_view("order", "default", ViewLayer::Global, None)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        data::find_entity_view("order", "default", ViewLayer::User, Some("u-1"))
            .await
            .unwrap()
            .is_none()
    );
}

// =============================================================================
// F1 — Loader-Bootstrap
// =============================================================================

#[tokio::test]
#[serial_test::serial]
async fn loader_bootstrap_creates_default_global_view_per_entity_type_with_settings() {
    let _ = server::fresh_test_setup().await;
    // setup_for_tests laedt 'shop'-Beispiel; pruefen wir auf 'product' (hat settings).
    let row = data::find_entity_view("product", "default", ViewLayer::Global, None)
        .await
        .expect("find");
    assert!(
        row.is_some(),
        "Loader-Settings fuer 'product' werden zur Default-View gemacht"
    );
    let v = row.unwrap();
    assert_eq!(v.version, 0);
    assert_eq!(v.updated_by.as_deref(), Some("system"));
}

#[tokio::test]
#[serial_test::serial]
async fn loader_bootstrap_is_idempotent() {
    let _ = server::fresh_test_setup().await;
    // Manuell nochmal aufrufen; bestehende Row bleibt unangetastet (version=0).
    server::data::seed_entity_views_from_example(&server::db::conn())
        .await
        .unwrap();
    let row = data::find_entity_view("product", "default", ViewLayer::Global, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.version, 0, "idempotent");
}
