//! Phase 1.7.7: Background-Job-Scheduler (MVP).
//!
//! Akzeptanz aus Roadmap: Retry bei Fehler; Audit zeigt Erfolg/Fail/
//! Duration. Cron-Scheduling ist out-of-scope (separates Folge-Item).

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serial_test::serial;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    jobs::upsert_job(&conn, "j1", "Job 1", "noop", None, false, 0).await.unwrap();
    let err = jobs::trigger_now(&conn, "j1").await.unwrap_err();
    assert!(matches!(err, jobs::JobError::Disabled(_)));
}

#[tokio::test]
#[serial]
async fn handler_not_registered_returns_specific_error() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    jobs::upsert_job(&conn, "j2", "Job 2", "missing_handler", None, true, 0).await.unwrap();
    let err = jobs::trigger_now(&conn, "j2").await.unwrap_err();
    assert!(matches!(err, jobs::JobError::HandlerNotRegistered(_)));
}

#[tokio::test]
#[serial]
async fn successful_handler_writes_run_with_status_success() {
    server::fresh_test_setup().await;
    let conn = server::db::conn();
    jobs::register_handler("ok", Arc::new(|| Box::pin(async { Ok(()) })));
    jobs::upsert_job(&conn, "j-ok", "OK", "ok", None, true, 0).await.unwrap();

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
    assert!(runs[0].duration_ms.is_some(), "duration_ms muss gesetzt sein");
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
    jobs::upsert_job(&conn, "j-fail", "Fail", "always_fail", None, true, 2).await.unwrap();

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
                if n == 0 { Err::<(), String>("temporary".into()) } else { Ok(()) }
            })
        }),
    );
    jobs::upsert_job(&conn, "j-flaky", "Flaky", "flaky", None, true, 3).await.unwrap();
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
