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
    assert_eq!(count, 0, "Tabelle existiert und ist leer nach init");
}

#[tokio::test]
#[serial_test::serial]
async fn insert_and_read_back_a_view_row() {
    let _ = fresh_test_setup().await;
    let db = server::db::conn();
    entity::entity_views::ActiveModel {
        id:          ActiveValue::Set("v-1".into()),
        entity_type: ActiveValue::Set("order".into()),
        view_name:   ActiveValue::Set("default".into()),
        layer:       ActiveValue::Set("global".into()),
        owner_id:    ActiveValue::Set(None),
        payload:     ActiveValue::Set("{\"properties\":[]}".into()),
        version:     ActiveValue::Set(1),
        updated_by:  ActiveValue::Set(Some("system".into())),
        updated_at:  ActiveValue::Set("2026-05-21T00:00:00Z".into()),
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
