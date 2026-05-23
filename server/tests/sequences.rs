//! Phase 1.7.1: Number-Sequence-Service.
//!
//! Akzeptanzkriterien aus Roadmap:
//! - Property-Test: 1000 parallele Aufrufe liefern 1..1000 lueckenlos.
//! - FY-Reset zum 01.01. konfigurierbar (per `reset_sequence`).

use std::collections::HashSet;
use std::sync::Arc;

use serial_test::serial;
use server::sequences::{self, NO_FISCAL_YEAR};

#[tokio::test]
#[serial]
async fn next_number_first_call_returns_one_in_default_format() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    let s = sequences::next_number(&conn, "invoice", NO_FISCAL_YEAR)
        .await
        .unwrap();
    // DEFAULT_FORMAT_TEMPLATE ist "{seq:06}" → 6 Stellen.
    assert_eq!(s, "000001");
}

#[tokio::test]
#[serial]
async fn next_number_increments_sequentially() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    let one = sequences::next_number(&conn, "order", NO_FISCAL_YEAR)
        .await
        .unwrap();
    let two = sequences::next_number(&conn, "order", NO_FISCAL_YEAR)
        .await
        .unwrap();
    let three = sequences::next_number(&conn, "order", NO_FISCAL_YEAR)
        .await
        .unwrap();
    assert_eq!(one, "000001");
    assert_eq!(two, "000002");
    assert_eq!(three, "000003");
}

#[tokio::test]
#[serial]
async fn ensure_sequence_pins_custom_template_before_first_next() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    sequences::ensure_sequence(&conn, "invoice", 2026, "INV-{year}-{seq:04}")
        .await
        .unwrap();
    let s = sequences::next_number(&conn, "invoice", 2026)
        .await
        .unwrap();
    assert_eq!(s, "INV-2026-0001");
}

#[tokio::test]
#[serial]
async fn reset_sequence_starts_over_at_one() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    for _ in 0..5 {
        sequences::next_number(&conn, "x", NO_FISCAL_YEAR)
            .await
            .unwrap();
    }
    assert_eq!(
        sequences::current_number(&conn, "x", NO_FISCAL_YEAR)
            .await
            .unwrap(),
        5
    );
    sequences::reset_sequence(&conn, "x", NO_FISCAL_YEAR)
        .await
        .unwrap();
    let again = sequences::next_number(&conn, "x", NO_FISCAL_YEAR)
        .await
        .unwrap();
    assert_eq!(again, "000001");
}

#[tokio::test]
#[serial]
async fn fiscal_year_creates_independent_counter() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    sequences::next_number(&conn, "rg", 2025).await.unwrap();
    sequences::next_number(&conn, "rg", 2025).await.unwrap();
    let twentysix = sequences::next_number(&conn, "rg", 2026).await.unwrap();
    // 2026 startet eigene Sequenz bei 1.
    assert_eq!(twentysix, "000001");
    assert_eq!(
        sequences::current_number(&conn, "rg", 2025).await.unwrap(),
        2
    );
    assert_eq!(
        sequences::current_number(&conn, "rg", 2026).await.unwrap(),
        1
    );
}

#[tokio::test]
#[serial]
async fn one_thousand_concurrent_calls_yield_gapless_one_to_n() {
    // Roadmap-Property-Test. SQLite serialisiert Writes ueber globalen
    // BEGIN-IMMEDIATE-Lock — selbst bei 1000 parallelen Tasks ist
    // current monoton.
    server::fresh_test_setup().await;
    let conn = Arc::new(server::db::conn());
    const N: usize = 1000;
    let mut handles = Vec::with_capacity(N);
    for _ in 0..N {
        let c = Arc::clone(&conn);
        handles.push(tokio::spawn(async move {
            sequences::next_number(&c, "race", NO_FISCAL_YEAR).await
        }));
    }
    let mut seen: HashSet<String> = HashSet::with_capacity(N);
    for h in handles {
        let s = h.await.unwrap().unwrap();
        assert!(seen.insert(s.clone()), "Duplikat erhalten: {s}");
    }
    assert_eq!(seen.len(), N);
    // Erwartete Menge: "000001".."001000" exakt.
    let expected: HashSet<String> = (1..=N as i64).map(|i| format!("{i:06}")).collect();
    assert_eq!(seen, expected, "gaps oder fremde Werte in der Sequenz");
}
