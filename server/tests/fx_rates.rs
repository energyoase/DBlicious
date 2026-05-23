//! Phase 1.7.2: FX-Rate-Store + Conversion.
//!
//! Akzeptanz aus Roadmap: historische Buchung mit damaligem Kurs
//! reproduzierbar; Banker-Rounding korrekt.

use serial_test::serial;
use server::fx;

#[tokio::test]
#[serial]
async fn identity_conversion_just_rounds() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    let v = fx::convert(&conn, 99.997, "EUR", "EUR", "2026-01-01", 2)
        .await
        .unwrap();
    // Banker-Rounding auf 2 Stellen
    assert_eq!(v, 100.00);
}

#[tokio::test]
#[serial]
async fn forward_lookup_uses_exact_date_rate() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    fx::upsert_rate(&conn, "2024-06-15", "EUR", "USD", 1.078, "manual")
        .await
        .unwrap();
    let v = fx::convert(&conn, 100.0, "EUR", "USD", "2024-06-15", 2)
        .await
        .unwrap();
    assert_eq!(v, 107.80);
}

#[tokio::test]
#[serial]
async fn inverted_lookup_when_only_reverse_pair_exists() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    // Nur USD→EUR vorhanden; EUR→USD muss invertiert berechnet werden.
    fx::upsert_rate(&conn, "2024-06-15", "USD", "EUR", 0.9277, "ecb")
        .await
        .unwrap();
    let v = fx::convert(&conn, 100.0, "EUR", "USD", "2024-06-15", 4)
        .await
        .unwrap();
    // 100 / 0.9277 ≈ 107.7935...
    assert!((v - 107.7935).abs() < 0.0005, "got {v}");
}

#[tokio::test]
#[serial]
async fn historical_lookup_falls_back_to_most_recent_le_date() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    fx::upsert_rate(&conn, "2024-01-01", "EUR", "CHF", 0.95, "manual")
        .await
        .unwrap();
    fx::upsert_rate(&conn, "2024-06-15", "EUR", "CHF", 0.97, "manual")
        .await
        .unwrap();
    // Buchung am 2024-08-01 — kein exakter Treffer; nimmt 2024-06-15.
    let v = fx::convert(&conn, 100.0, "EUR", "CHF", "2024-08-01", 2)
        .await
        .unwrap();
    assert_eq!(v, 97.00);
    // Buchung am 2024-03-15 — nimmt 2024-01-01.
    let v = fx::convert(&conn, 100.0, "EUR", "CHF", "2024-03-15", 2)
        .await
        .unwrap();
    assert_eq!(v, 95.00);
}

#[tokio::test]
#[serial]
async fn missing_rate_returns_error() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    let err = fx::convert(&conn, 100.0, "EUR", "JPY", "2024-06-15", 0)
        .await
        .unwrap_err();
    assert!(matches!(err, fx::FxError::NoRate { .. }));
}

#[tokio::test]
#[serial]
async fn upsert_overwrites_existing_rate() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    fx::upsert_rate(&conn, "2024-06-15", "EUR", "USD", 1.05, "stale")
        .await
        .unwrap();
    fx::upsert_rate(&conn, "2024-06-15", "EUR", "USD", 1.10, "fresh")
        .await
        .unwrap();
    let v = fx::convert(&conn, 100.0, "EUR", "USD", "2024-06-15", 2)
        .await
        .unwrap();
    assert_eq!(v, 110.00);
}

#[test]
fn banker_round_table() {
    // Doku-Test fuer die Rundung.
    assert_eq!(fx::banker_round(0.5, 0), 0.0); // even
    assert_eq!(fx::banker_round(1.5, 0), 2.0); // even
    assert_eq!(fx::banker_round(2.5, 0), 2.0); // even
    assert_eq!(fx::banker_round(-0.5, 0), 0.0); // even
    assert_eq!(fx::banker_round(0.125, 2), 0.12); // 0.13 waere odd→0.12
}
