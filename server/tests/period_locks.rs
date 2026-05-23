//! Phase 1.7.3: Period-Locks.
//!
//! Akzeptanz aus Roadmap: Monats-/Quartals-/Jahresabschluss moeglich;
//! Backdating geblockt.

use serial_test::serial;
use server::period_locks::{self, PeriodLockedError};

#[tokio::test]
#[serial]
async fn no_lock_means_not_locked() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    assert!(!period_locks::is_locked(&conn, "datev_entry", "2024-01-01")
        .await
        .unwrap());
    assert!(
        period_locks::guard_or_period_locked(&conn, "datev_entry", "2024-01-01")
            .await
            .is_ok()
    );
}

#[tokio::test]
#[serial]
async fn set_lock_blocks_dates_through_inclusive() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    period_locks::set_lock(
        &conn,
        "invoice",
        "2024-12-31",
        Some("admin"),
        Some("Year-End"),
    )
    .await
    .unwrap();

    // gesperrt: alles bis inkl. 2024-12-31
    assert!(period_locks::is_locked(&conn, "invoice", "2024-12-31")
        .await
        .unwrap());
    assert!(period_locks::is_locked(&conn, "invoice", "2024-06-15")
        .await
        .unwrap());
    assert!(period_locks::is_locked(&conn, "invoice", "2020-01-01")
        .await
        .unwrap());
    // frei: ab 2025-01-01
    assert!(!period_locks::is_locked(&conn, "invoice", "2025-01-01")
        .await
        .unwrap());

    // Guard liefert PeriodLockedError mit details
    let err = period_locks::guard_or_period_locked(&conn, "invoice", "2024-06-15")
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("period_locked"), "got: {msg}");
    assert!(msg.contains("invoice"));
    assert!(msg.contains("2024-12-31"));
}

#[tokio::test]
#[serial]
async fn other_scope_is_unaffected() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    period_locks::set_lock(&conn, "invoice", "2024-12-31", None, None)
        .await
        .unwrap();
    // 'order'-scope hat keinen Lock
    assert!(!period_locks::is_locked(&conn, "order", "2024-06-15")
        .await
        .unwrap());
    assert!(
        period_locks::guard_or_period_locked(&conn, "order", "2024-06-15")
            .await
            .is_ok()
    );
}

#[tokio::test]
#[serial]
async fn set_lock_is_upsert_and_can_extend() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    period_locks::set_lock(&conn, "x", "2024-06-30", None, None)
        .await
        .unwrap();
    assert!(!period_locks::is_locked(&conn, "x", "2024-09-30")
        .await
        .unwrap());
    // Erweitern auf Quartalsende
    period_locks::set_lock(&conn, "x", "2024-09-30", None, None)
        .await
        .unwrap();
    assert!(period_locks::is_locked(&conn, "x", "2024-09-30")
        .await
        .unwrap());
}

#[tokio::test]
#[serial]
async fn clear_lock_removes_the_block() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    period_locks::set_lock(&conn, "x", "2024-12-31", None, None)
        .await
        .unwrap();
    assert!(period_locks::is_locked(&conn, "x", "2024-06-15")
        .await
        .unwrap());
    let removed = period_locks::clear_lock(&conn, "x").await.unwrap();
    assert!(removed);
    assert!(!period_locks::is_locked(&conn, "x", "2024-06-15")
        .await
        .unwrap());
    // Idempotent: zweiter clear macht keine Zeile dirty
    let removed2 = period_locks::clear_lock(&conn, "x").await.unwrap();
    assert!(!removed2);
}

#[tokio::test]
#[serial]
async fn guard_returns_locked_variant_with_fields() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    period_locks::set_lock(&conn, "datev_entry", "2024-12-31", Some("system"), None)
        .await
        .unwrap();
    let err = period_locks::guard_or_period_locked(&conn, "datev_entry", "2024-12-31")
        .await
        .unwrap_err();
    match err {
        PeriodLockedError::Locked {
            scope,
            attempted_date,
            locked_through_date,
        } => {
            assert_eq!(scope, "datev_entry");
            assert_eq!(attempted_date, "2024-12-31");
            assert_eq!(locked_through_date, "2024-12-31");
        }
        other => panic!("erwartet PeriodLockedError::Locked, got {other:?}"),
    }
}
