//! Phase 1.7.7: Background-Job-Scheduler.
//!
//! Akzeptanz aus Roadmap: Retry bei Fehler; Audit zeigt Erfolg/Fail/
//! Duration. Cron-Loop als Folge-Item: `is_due` plus
//! `run_scheduler_tick`. Der echte Tokio-Loop (`start_scheduler_loop`)
//! wird nicht in Tests gestartet — wir rufen einen Tick deterministisch
//! mit fixierter `now` auf.

use chrono::{TimeZone, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use server::entity::job_runs;
use server::jobs::{self, TriggerOutcome};

#[tokio::test]
#[serial]
async fn trigger_not_found_for_unknown_job() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    let err = jobs::trigger_now(&conn, "nope").await.unwrap_err();
    assert!(matches!(err, jobs::JobError::NotFound(_)));
}

#[tokio::test]
#[serial]
async fn trigger_disabled_returns_disabled_error() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    jobs::register_handler("noop", Arc::new(|| Box::pin(async { Ok(()) })));
    jobs::upsert_job(&conn, "j1", "Job 1", "noop", None, false, 0)
        .await
        .unwrap();
    let err = jobs::trigger_now(&conn, "j1").await.unwrap_err();
    assert!(matches!(err, jobs::JobError::Disabled(_)));
}

#[tokio::test]
#[serial]
async fn handler_not_registered_returns_specific_error() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    jobs::upsert_job(&conn, "j2", "Job 2", "missing_handler", None, true, 0)
        .await
        .unwrap();
    let err = jobs::trigger_now(&conn, "j2").await.unwrap_err();
    assert!(matches!(err, jobs::JobError::HandlerNotRegistered(_)));
}

#[tokio::test]
#[serial]
async fn successful_handler_writes_run_with_status_success() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    jobs::register_handler("ok", Arc::new(|| Box::pin(async { Ok(()) })));
    jobs::upsert_job(&conn, "j-ok", "OK", "ok", None, true, 0)
        .await
        .unwrap();

    let outcome = jobs::trigger_now(&conn, "j-ok").await.unwrap();
    assert!(outcome.success);
    assert_eq!(outcome.attempts, 1);

    // Run-Row verifizieren.
    let runs = job_runs::Entity::find()
        .filter(job_runs::Column::JobId.eq("j-ok"))
        .all(&conn)
        .await
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, "success");
    assert!(
        runs[0].duration_ms.is_some(),
        "duration_ms muss gesetzt sein"
    );
    assert!(runs[0].finished_at.is_some());
}

#[tokio::test]
#[serial]
async fn failing_handler_retries_up_to_max_retries() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_handler = Arc::clone(&counter);
    jobs::register_handler(
        "always_fail",
        Arc::new(move || {
            let c = Arc::clone(&counter_for_handler);
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err::<(), String>("boom".into())
            })
        }),
    );
    jobs::upsert_job(&conn, "j-fail", "Fail", "always_fail", None, true, 2)
        .await
        .unwrap();

    let outcome: TriggerOutcome = jobs::trigger_now(&conn, "j-fail").await.unwrap();
    assert!(!outcome.success);
    assert_eq!(outcome.attempts, 3, "1 initial + 2 retries");
    assert_eq!(counter.load(Ordering::SeqCst), 3);

    // Drei Run-Rows, alle "failed", attempts 1..3.
    let mut runs = job_runs::Entity::find()
        .filter(job_runs::Column::JobId.eq("j-fail"))
        .order_by_asc(job_runs::Column::Attempt)
        .all(&conn)
        .await
        .unwrap();
    runs.sort_by_key(|r| r.attempt);
    assert_eq!(runs.len(), 3);
    for (i, r) in runs.iter().enumerate() {
        assert_eq!(r.attempt, (i + 1) as i32);
        assert_eq!(r.status, "failed");
        assert_eq!(r.error_message.as_deref(), Some("boom"));
    }

    // last_error im Outcome
    assert_eq!(outcome.last_error.as_deref(), Some("boom"));
}

#[tokio::test]
#[serial]
async fn retry_succeeds_on_second_attempt() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    let attempt = Arc::new(AtomicUsize::new(0));
    let a = Arc::clone(&attempt);
    jobs::register_handler(
        "flaky",
        Arc::new(move || {
            let a = Arc::clone(&a);
            Box::pin(async move {
                let n = a.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err::<(), String>("temporary".into())
                } else {
                    Ok(())
                }
            })
        }),
    );
    jobs::upsert_job(&conn, "j-flaky", "Flaky", "flaky", None, true, 3)
        .await
        .unwrap();
    let outcome = jobs::trigger_now(&conn, "j-flaky").await.unwrap();
    assert!(outcome.success);
    assert_eq!(outcome.attempts, 2);

    // Run-Rows: erste failed, zweite success.
    let mut runs = job_runs::Entity::find()
        .filter(job_runs::Column::JobId.eq("j-flaky"))
        .order_by_asc(job_runs::Column::Attempt)
        .all(&conn)
        .await
        .unwrap();
    runs.sort_by_key(|r| r.attempt);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].status, "failed");
    assert_eq!(runs[1].status, "success");
}

// =============================================================================
// Cron-Loop (Folge-Item zu 1.7.7)
// =============================================================================

#[test]
fn is_due_first_tick_after_boot_for_never_run_job() {
    // Cron "alle 5 Minuten". Job noch nie gelaufen. Boot war 10:00,
    // now ist 10:05 — der erste Cron-Fire nach Boot ist 10:05, der Tick
    // bei 10:05 muss feuern.
    let boot = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 10, 5, 0).unwrap();
    assert!(jobs::is_due("*/5 * * * *", None, boot, now));
}

#[test]
fn is_due_does_not_fire_when_no_cron_slot_passed_since_last_run() {
    // Cron "alle 5 Minuten". Letzter Lauf 10:05. Now 10:06 — naechster
    // Slot waere 10:10.
    let boot = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
    let last = "2026-01-01T10:05:00+00:00";
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 10, 6, 0).unwrap();
    assert!(!jobs::is_due("*/5 * * * *", Some(last), boot, now));
}

#[test]
fn is_due_fires_when_next_slot_reached() {
    // Letzter Lauf 10:05, now 10:10 — Slot ist erreicht.
    let boot = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
    let last = "2026-01-01T10:05:00+00:00";
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 10, 10, 0).unwrap();
    assert!(jobs::is_due("*/5 * * * *", Some(last), boot, now));
}

#[test]
fn is_due_accepts_six_field_cron_with_seconds() {
    // 6-Felder-Form (Sekunden vorne) muss unveraendert akzeptiert werden,
    // damit Power-User die Sekunden explizit setzen koennen.
    let boot = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 30).unwrap();
    assert!(jobs::is_due("*/30 * * * * *", None, boot, now));
}

#[test]
fn is_due_returns_false_for_invalid_expression() {
    let boot = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 11, 0, 0).unwrap();
    assert!(!jobs::is_due("not a cron", None, boot, now));
}

#[tokio::test]
#[serial]
async fn scheduler_tick_fires_due_job_and_updates_last_run_at() {
    server::fresh_test_setup().await;
    jobs::reset_boot_anchor();
    let conn = server::db::conn();

    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);
    jobs::register_handler(
        "cron_ok",
        Arc::new(move || {
            let c = Arc::clone(&c);
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }),
    );
    jobs::upsert_job(
        &conn,
        "j-cron",
        "Cron OK",
        "cron_ok",
        Some("* * * * *"),
        true,
        0,
    )
    .await
    .unwrap();

    let now = Utc.with_ymd_and_hms(2026, 1, 1, 10, 1, 0).unwrap();
    let fired = jobs::run_scheduler_tick(&conn, now).await.unwrap();
    assert_eq!(fired, vec!["j-cron".to_string()]);
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Zweiter Tick zur selben Sekunde feuert nicht erneut, weil das
    // letzte last_run_at vor `now` liegt und der naechste Cron-Slot
    // (10:02:00) noch nicht erreicht ist.
    let fired2 = jobs::run_scheduler_tick(&conn, now).await.unwrap();
    assert!(fired2.is_empty());
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[serial]
async fn scheduler_tick_skips_jobs_without_cron() {
    server::fresh_test_setup().await;
    jobs::reset_boot_anchor();
    let conn = server::db::conn();

    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);
    jobs::register_handler(
        "no_cron",
        Arc::new(move || {
            let c = Arc::clone(&c);
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }),
    );
    jobs::upsert_job(&conn, "j-no-cron", "No Cron", "no_cron", None, true, 0)
        .await
        .unwrap();

    let now = Utc::now();
    let fired = jobs::run_scheduler_tick(&conn, now).await.unwrap();
    assert!(fired.is_empty());
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
#[serial]
async fn scheduler_tick_skips_disabled_jobs() {
    server::fresh_test_setup().await;
    jobs::reset_boot_anchor();
    let conn = server::db::conn();

    let counter = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&counter);
    jobs::register_handler(
        "disabled_cron",
        Arc::new(move || {
            let c = Arc::clone(&c);
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }),
    );
    jobs::upsert_job(
        &conn,
        "j-disabled",
        "Disabled Cron",
        "disabled_cron",
        Some("* * * * *"),
        false, // disabled
        0,
    )
    .await
    .unwrap();

    let now = Utc.with_ymd_and_hms(2026, 1, 1, 10, 1, 0).unwrap();
    let fired = jobs::run_scheduler_tick(&conn, now).await.unwrap();
    assert!(fired.is_empty());
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
#[serial]
async fn scheduler_tick_survives_handler_missing() {
    server::fresh_test_setup().await;
    jobs::reset_boot_anchor();
    let conn = server::db::conn();

    // Bewusst keinen Handler registriert.
    jobs::upsert_job(
        &conn,
        "j-orphan",
        "Orphan",
        "no_such_handler",
        Some("* * * * *"),
        true,
        0,
    )
    .await
    .unwrap();

    let now = Utc.with_ymd_and_hms(2026, 1, 1, 10, 1, 0).unwrap();
    // Tick darf nicht panic'en oder den ganzen Loop killen.
    let fired = jobs::run_scheduler_tick(&conn, now).await.unwrap();
    assert!(
        fired.is_empty(),
        "orphan job darf nicht als gefeuert zaehlen"
    );
}
