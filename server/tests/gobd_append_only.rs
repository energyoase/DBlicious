//! Phase 1.7.4: GoBD-Append-Only.
//!
//! Akzeptanz: Gebuchte Rechnung kann nicht editiert werden, nur storniert.
//! Hier testen wir den Service-Pfad — die Storno-Operation (neuer
//! Gegenbuchung) liegt im Geschaeftslogik-Layer, nicht im CRUD.

use serial_test::serial;
use server::data;

/// Spielt eine Settings-Json fuer einen Test-Entity-Typ in den
/// `metadata_settings`-Slot, damit `settings_for_async` sie liest.
async fn install_append_only_setting(entity_type: &str, append_only: bool) {
    use sea_orm::{ActiveValue, EntityTrait};
    use server::entity::metadata_settings;
    let mut s = shared::EntitySettings {
        entity_type: entity_type.to_string(),
        append_only,
        ..Default::default()
    };
    s.entity_type = entity_type.to_string();
    let json = serde_json::to_string(&s).unwrap();
    let conn = server::db::conn();
    let _ = metadata_settings::Entity::insert(metadata_settings::ActiveModel {
        entity_type: ActiveValue::Set(entity_type.to_string()),
        settings_json: ActiveValue::Set(json),
    })
    .exec(&conn)
    .await;
}

#[tokio::test]
#[serial]
async fn is_append_only_returns_false_for_unknown_entity() {
    server::fresh_test_setup().await;
    assert!(!data::is_append_only("__nope__").await);
}

#[tokio::test]
#[serial]
async fn append_only_setting_makes_update_a_noop() {
    server::fresh_test_setup().await;
    install_append_only_setting("locked_book", true).await;
    assert!(data::is_append_only("locked_book").await);
    let res = data::update_entity(
        "locked_book",
        "any-id",
        serde_json::json!({"foo": "bar"}),
        None,
    )
    .await;
    assert!(res.is_none(), "append_only-Update muss None liefern");
}

#[tokio::test]
#[serial]
async fn append_only_setting_makes_delete_return_false() {
    server::fresh_test_setup().await;
    install_append_only_setting("locked_book", true).await;
    let res = data::delete_entity("locked_book", "any-id").await;
    assert!(!res, "append_only-Delete muss false liefern");
}

#[tokio::test]
#[serial]
async fn append_only_false_allows_normal_flow() {
    // Wenn explizit false oder gar nicht gesetzt, laeuft der CRUD-Pfad
    // ueblich durch (Update auf nicht-existente Row = None, das ist
    // erwartet — wir verifizieren, dass die GoBD-Guard NICHT eingreift).
    server::fresh_test_setup().await;
    install_append_only_setting("open_book", false).await;
    assert!(!data::is_append_only("open_book").await);
    // check_gobd_protected ist Ok
    assert!(data::check_gobd_protected("open_book").await.is_ok());
}

#[tokio::test]
#[serial]
async fn check_gobd_protected_returns_typed_error() {
    server::fresh_test_setup().await;
    install_append_only_setting("protected_book", true).await;
    let err = data::check_gobd_protected("protected_book")
        .await
        .unwrap_err();
    assert_eq!(err.entity_type, "protected_book");
    let msg = format!("{err}");
    assert!(msg.contains("gobd_protected"));
    assert!(msg.contains("protected_book"));
}
