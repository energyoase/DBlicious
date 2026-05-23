//! Phase 1.6 Subscription: `entityDesignUpdated` empfaengt Events, sobald
//! `save_entity_design` erfolgreich war.
//!
//! Wir testen den Broadcast-Bus direkt (Receiver subscribed vor save,
//! save publisht, recv liefert). Der WebSocket-/GraphQL-Subscription-
//! Wrapper darueber ist in async-graphql gepinnt; hier reicht der
//! Server-Bus-Pfad.

use serial_test::serial;
use server::{data, events};
use std::time::Duration;
use tokio::time::timeout;

const SHORT_TIMEOUT: Duration = Duration::from_millis(500);

#[tokio::test]
#[serial]
async fn save_publishes_design_update_to_subscribers() {
    server::fresh_test_setup().await;

    let mut rx = events::subscribe_design_updates();

    // Erste Version fuer einen Entity-Typ schreiben.
    let model = data::save_entity_design(
        "phase16_widget",
        data::DESIGN_SCHEMA_VERSION,
        r#"{"schemaVersion":1,"tree":{"nodes":[]},"projection":{}}"#,
        None,
        data::SYSTEM_USER_ID,
    )
    .await
    .expect("save_entity_design ok");
    assert_eq!(model.version, 0);

    let ev = timeout(SHORT_TIMEOUT, rx.recv())
        .await
        .expect("Subscription muss innerhalb 500ms ein Event liefern")
        .expect("recv ok");
    assert_eq!(ev.entity_type, "phase16_widget");
    assert_eq!(ev.version, 0);
}

#[tokio::test]
#[serial]
async fn second_save_publishes_incrementing_version() {
    server::fresh_test_setup().await;
    let mut rx = events::subscribe_design_updates();

    let _ = data::save_entity_design(
        "phase16_inc",
        data::DESIGN_SCHEMA_VERSION,
        "{}",
        None,
        data::SYSTEM_USER_ID,
    )
    .await
    .unwrap();
    let _ = data::save_entity_design(
        "phase16_inc",
        data::DESIGN_SCHEMA_VERSION,
        "{}",
        Some(0),
        data::SYSTEM_USER_ID,
    )
    .await
    .unwrap();

    let ev1 = timeout(SHORT_TIMEOUT, rx.recv()).await.unwrap().unwrap();
    let ev2 = timeout(SHORT_TIMEOUT, rx.recv()).await.unwrap().unwrap();
    assert_eq!(ev1.version, 0);
    assert_eq!(ev2.version, 1);
}

#[tokio::test]
#[serial]
async fn conflict_does_not_publish_event() {
    server::fresh_test_setup().await;
    // Erst v0 anlegen, damit ein Konflikt simuliert werden kann.
    let _ = data::save_entity_design(
        "phase16_conf",
        data::DESIGN_SCHEMA_VERSION,
        "{}",
        None,
        data::SYSTEM_USER_ID,
    )
    .await
    .unwrap();

    // Jetzt Subscriber abonnieren, dann mit falscher expected_version
    // saven → Conflict.
    let mut rx = events::subscribe_design_updates();
    let res = data::save_entity_design(
        "phase16_conf",
        data::DESIGN_SCHEMA_VERSION,
        "{}",
        None, // erwartet "nichts vorhanden" → Konflikt (v0 existiert)
        data::SYSTEM_USER_ID,
    )
    .await;
    assert!(matches!(res, Err(data::SaveDesignError::Conflict { .. })));

    // Kein Event innerhalb der kurzen Timeout-Spanne.
    let recv = timeout(Duration::from_millis(100), rx.recv()).await;
    assert!(
        recv.is_err(),
        "Bei Konflikt darf kein Event publiziert werden"
    );
}
