//! Background-Job-Scheduler (Phase 1.7.7).
//!
//! Was drin ist:
//!   - Schema (`jobs`, `job_runs`)
//!   - Handler-Registry: in-process `BoxFuture`-Callbacks
//!   - `trigger_now(job_id)`: ruft den Handler, schreibt Run-Row, retried
//!     bis `max_retries`
//!   - Audit-Tabelle `job_runs` (eigene domain-spezifische Tabelle wegen
//!     duration_ms + attempt; Roadmap-Hybrid-Audit-Entscheidung)
//!   - Cron-Eval (`is_due`) + Scheduler-Tick (`run_scheduler_tick`) +
//!     Hintergrund-Loop (`start_scheduler_loop`). Cron-Expressions im
//!     5-Felder-UNIX-Format (`min hour day month dow`); intern wird ein
//!     Sekunden-Feld `"0 "` vorangestellt fuer das 6-Felder-Cron-Crate.
//!
//! Was NICHT drin ist (Folge-Items):
//!   - Plugin-Handler (Phase 2) / Script-Handler (Q0009)
//!   - Distributed-Locking ueber mehrere Server-Instanzen
//!   - Backfill verpasster Cron-Slots (Loop springt nur den naechsten
//!     Tick an; bei langer Server-Downtime wird der ueberholte Slot
//!     uebersprungen)

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use parking_lot::RwLock;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use thiserror::Error;

use crate::entity::{job_runs, jobs};

pub const STATUS_RUNNING: &str = "running";
pub const STATUS_SUCCESS: &str = "success";
pub const STATUS_FAILED: &str = "failed";

/// Boxed-Async-Handler-Signatur. Output ist `Result<(), String>` —
/// String-Error wandert in `job_runs.error_message`.
pub type JobHandlerFut = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
pub type JobHandler = Arc<dyn Fn() -> JobHandlerFut + Send + Sync>;

#[derive(Debug, Error)]
pub enum JobError {
    #[error("job_not_found: {0}")]
    NotFound(String),
    #[error("job_disabled: {0}")]
    Disabled(String),
    #[error("handler_not_registered: {0}")]
    HandlerNotRegistered(String),
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
}

// =============================================================================
// Registry (prozessweit, OnceLock + RwLock)
// =============================================================================

fn registry() -> &'static RwLock<BTreeMap<String, JobHandler>> {
    static REG: OnceLock<RwLock<BTreeMap<String, JobHandler>>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(BTreeMap::new()))
}

/// Registriert (oder ersetzt) einen Handler unter `key`.
pub fn register_handler(key: &str, handler: JobHandler) {
    registry().write().insert(key.to_string(), handler);
}

/// Entfernt einen Handler (idempotent). Tests.
pub fn unregister_handler(key: &str) {
    registry().write().remove(key);
}

fn lookup_handler(key: &str) -> Option<JobHandler> {
    registry().read().get(key).cloned()
}

// =============================================================================
// CRUD-Helper auf Jobs
// =============================================================================

pub async fn upsert_job(
    conn: &DatabaseConnection,
    id: &str,
    name: &str,
    handler: &str,
    schedule_cron: Option<&str>,
    enabled: bool,
    max_retries: i32,
) -> Result<(), JobError> {
    let existing = jobs::Entity::find_by_id(id.to_string()).one(conn).await?;
    match existing {
        Some(m) => {
            let mut am: jobs::ActiveModel = m.into();
            am.name = ActiveValue::Set(name.to_string());
            am.handler = ActiveValue::Set(handler.to_string());
            am.schedule_cron = ActiveValue::Set(schedule_cron.map(String::from));
            am.enabled = ActiveValue::Set(enabled);
            am.max_retries = ActiveValue::Set(max_retries);
            am.update(conn).await?;
        }
        None => {
            jobs::ActiveModel {
                id: ActiveValue::Set(id.to_string()),
                name: ActiveValue::Set(name.to_string()),
                handler: ActiveValue::Set(handler.to_string()),
                schedule_cron: ActiveValue::Set(schedule_cron.map(String::from)),
                enabled: ActiveValue::Set(enabled),
                max_retries: ActiveValue::Set(max_retries),
                last_run_at: ActiveValue::Set(None),
                tenant_id: ActiveValue::Set(None),
            }
            .insert(conn)
            .await?;
        }
    }
    Ok(())
}

// =============================================================================
// trigger_now mit Retry
// =============================================================================

/// Ergebnis eines kompletten `trigger_now`-Aufrufs (inkl. aller Retries).
#[derive(Debug, Clone)]
pub struct TriggerOutcome {
    pub job_id: String,
    /// Anzahl Versuche, die tatsaechlich gelaufen sind.
    pub attempts: i32,
    /// `true` ⇒ letzter Versuch hat `success` geliefert.
    pub success: bool,
    /// Letzte Fehlermeldung (sofern alle Versuche failed).
    pub last_error: Option<String>,
}

pub async fn trigger_now(
    conn: &DatabaseConnection,
    job_id: &str,
) -> Result<TriggerOutcome, JobError> {
    let job = jobs::Entity::find_by_id(job_id.to_string())
        .one(conn)
        .await?
        .ok_or_else(|| JobError::NotFound(job_id.to_string()))?;
    if !job.enabled {
        return Err(JobError::Disabled(job_id.to_string()));
    }
    let handler = lookup_handler(&job.handler)
        .ok_or_else(|| JobError::HandlerNotRegistered(job.handler.clone()))?;

    let max_attempts = (job.max_retries.max(0) + 1) as u32;
    let mut last_error: Option<String> = None;
    let mut attempts: i32 = 0;
    let mut success = false;

    for attempt_idx in 1..=max_attempts {
        attempts = attempt_idx as i32;
        let run_id = ulid::Ulid::new().to_string();
        let started_at = chrono::Utc::now();
        let started_iso = started_at.to_rfc3339();

        // Run-Row: running.
        job_runs::ActiveModel {
            id: ActiveValue::Set(run_id.clone()),
            job_id: ActiveValue::Set(job_id.to_string()),
            attempt: ActiveValue::Set(attempts),
            started_at: ActiveValue::Set(started_iso.clone()),
            finished_at: ActiveValue::Set(None),
            status: ActiveValue::Set(STATUS_RUNNING.to_string()),
            duration_ms: ActiveValue::Set(None),
            error_message: ActiveValue::Set(None),
            tenant_id: ActiveValue::Set(None),
        }
        .insert(conn)
        .await?;

        // Handler ausfuehren.
        let result = handler().await;
        let finished_at = chrono::Utc::now();
        let duration_ms = (finished_at - started_at).num_milliseconds().max(0);

        // Update Run-Row.
        let run = job_runs::Entity::find_by_id(run_id.clone())
            .one(conn)
            .await?;
        if let Some(r) = run {
            let mut am: job_runs::ActiveModel = r.into();
            am.finished_at = ActiveValue::Set(Some(finished_at.to_rfc3339()));
            am.duration_ms = ActiveValue::Set(Some(duration_ms));
            match &result {
                Ok(()) => {
                    am.status = ActiveValue::Set(STATUS_SUCCESS.to_string());
                }
                Err(e) => {
                    am.status = ActiveValue::Set(STATUS_FAILED.to_string());
                    am.error_message = ActiveValue::Set(Some(e.clone()));
                }
            }
            am.update(conn).await?;
        }

        if result.is_ok() {
            success = true;
            // Update last_run_at auf Job.
            let mut am: jobs::ActiveModel = job.clone().into();
            am.last_run_at = ActiveValue::Set(Some(finished_at.to_rfc3339()));
            am.update(conn).await?;
            break;
        } else {
            last_error = result.err();
            // weiter retryen (oder Schleife endet, wenn alle Versuche verbraucht).
        }
    }

    Ok(TriggerOutcome {
        job_id: job_id.to_string(),
        attempts,
        success,
        last_error,
    })
}

// =============================================================================
// Cron-Loop (Phase 1.7.7 Folge-Item)
// =============================================================================

/// Wandelt eine 5-Felder-Cron-Expression in das 6-Felder-Format des
/// `cron`-Crates um, indem ein Sekunden-Feld `"0"` vorangestellt wird.
/// Wer bereits 6 Felder schickt (z.B. `"0 0 * * * *"`), bekommt den Wert
/// unveraendert zurueck.
fn normalize_cron_expr(expr: &str) -> String {
    let trimmed = expr.trim();
    let field_count = trimmed.split_whitespace().count();
    if field_count == 5 {
        format!("0 {trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// `true`, wenn der Cron-Expression seit `last_run_at` (exklusiv) bis `now`
/// (inklusiv) mindestens einen Auslose-Zeitpunkt liefert.
///
/// Wenn `last_run_at` `None` ist, wird der Boot-Zeitpunkt `boot_at`
/// herangezogen: ein Job, der seit Boot noch nie gelaufen ist, feuert
/// beim ersten Tick mit `now >= boot_at`. Ungueltige Cron-Expressions
/// werden konservativ als `false` interpretiert (mit `tracing::warn`).
pub fn is_due(
    expr: &str,
    last_run_at: Option<&str>,
    boot_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> bool {
    let normalized = normalize_cron_expr(expr);
    let schedule = match Schedule::from_str(&normalized) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(target: "jobs", "invalid cron expr {expr:?}: {e}");
            return false;
        }
    };
    // `Schedule::after(t)` ist exklusiv. Fuer never-run Jobs wollen wir,
    // dass ein Slot, der exakt auf den Boot-Zeitpunkt faellt, beim ersten
    // Tick mit `now >= boot` mitzaehlt — sonst geht der erste Slot
    // verloren, wenn boot und Slot zufaellig zusammenfallen. Loesung:
    // bei never-run-Jobs schieben wir den Anker eine Sekunde nach hinten.
    // Bei `last_run_at = Some(...)` bleibt der Anker exklusiv, weil dort
    // bereits gefeuert wurde.
    let parsed_last = last_run_at.and_then(|iso| {
        DateTime::parse_from_rfc3339(iso)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });
    let anchor = parsed_last.unwrap_or_else(|| boot_at - chrono::Duration::seconds(1));
    schedule
        .after(&anchor)
        .next()
        .map(|next_fire| next_fire <= now)
        .unwrap_or(false)
}

/// Boot-Anker fuer den Scheduler. Wird beim ersten Tick gelesen; ein Job
/// ohne `last_run_at` benutzt diesen Zeitpunkt als Bezug, damit er nicht
/// retroaktiv "verpasste" Zeitpunkte aus der fernen Vergangenheit
/// einfaengt.
fn boot_anchor() -> &'static RwLock<Option<DateTime<Utc>>> {
    static BOOT: OnceLock<RwLock<Option<DateTime<Utc>>>> = OnceLock::new();
    BOOT.get_or_init(|| RwLock::new(None))
}

fn ensure_boot_anchor(now: DateTime<Utc>) -> DateTime<Utc> {
    let mut slot = boot_anchor().write();
    *slot.get_or_insert(now)
}

/// Tests koennen den Boot-Anker zuruecksetzen, damit aufeinanderfolgende
/// Test-Faelle nicht den Anker eines frueheren Tests erben.
pub fn reset_boot_anchor() {
    *boot_anchor().write() = None;
}

/// Ein Scheduler-Tick: liest alle aktivierten, mit Cron versehenen Jobs;
/// triggert die faelligen via `trigger_now`. Jeder Job laeuft seriell —
/// fuer parallele Verarbeitung wuerde man `tokio::spawn` pro Job machen,
/// das ist hier bewusst noch nicht drin (Single-Server-Annahme; ein lang
/// laufender Job verzoegert den naechsten Tick maximal um seine eigene
/// Dauer).
///
/// Liefert die Liste der Job-IDs, die in diesem Tick gefeuert haben (auch
/// wenn der Handler intern failt — `outcome.success=false` ist trotzdem
/// "gefeuert"). Tests nutzen das zur Verifikation.
pub async fn run_scheduler_tick(
    conn: &DatabaseConnection,
    now: DateTime<Utc>,
) -> Result<Vec<String>, JobError> {
    let boot_at = ensure_boot_anchor(now);
    let candidates = jobs::Entity::find()
        .filter(jobs::Column::Enabled.eq(true))
        .filter(jobs::Column::ScheduleCron.is_not_null())
        .all(conn)
        .await?;
    let mut fired = Vec::new();
    for job in candidates {
        let Some(expr) = job.schedule_cron.as_deref() else {
            continue;
        };
        if !is_due(expr, job.last_run_at.as_deref(), boot_at, now) {
            continue;
        }
        // Handler-Fehler fangen wir hier ab — der Loop darf nicht an
        // einem einzelnen kaputten Job sterben.
        match trigger_now(conn, &job.id).await {
            Ok(_) => fired.push(job.id.clone()),
            Err(JobError::HandlerNotRegistered(_)) => {
                tracing::warn!(target: "jobs", "scheduler tick: handler missing for job {}", job.id);
            }
            Err(JobError::Disabled(_)) => {
                // Race: zwischen find() und trigger_now() wurde disabled.
            }
            Err(e) => {
                tracing::error!(target: "jobs", "scheduler tick: job {} failed: {e}", job.id);
            }
        }
    }
    Ok(fired)
}

/// Spawnt den Scheduler-Loop in einem Tokio-Task. Der Task ruft
/// `run_scheduler_tick` mit `interval`-Abstand. Der Handle laeuft, bis er
/// abortiert wird (z.B. via `JoinHandle::abort()`) oder der Prozess endet.
///
/// Aufrufer (i.d.R. `main.rs::main`) sollte das Handle behalten, falls
/// graceful Shutdown gewuenscht ist.
pub fn start_scheduler_loop(
    conn: DatabaseConnection,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let now = Utc::now();
            if let Err(e) = run_scheduler_tick(&conn, now).await {
                tracing::error!(target: "jobs", "scheduler tick error: {e}");
            }
            tokio::time::sleep(interval).await;
        }
    })
}
