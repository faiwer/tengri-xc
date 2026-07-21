//! Global route-scoring queue. An upload persists the flight synchronously and
//! defers the CPU-heavy scoring here: a bounded worker pool drains the
//! `scoring_jobs` table, and the upload handler subscribes to its job so it
//! returns the instant scoring finishes (or after a caller-chosen ceiling).
//!
//! The `scoring_jobs` table is the durable record; the in-memory channel is
//! just the dispatch fast-path. On boot the dispatcher re-enqueues everything
//! left `queued`/`running` from a previous process, so nothing is stranded.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use sqlx::PgPool;
use tokio::sync::{Semaphore, mpsc, oneshot};

use crate::flight::assess;

type Waiters = Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>;

/// Handle to the scoring queue. Cheap to clone (sender + `Arc`s). Lives on
/// `AppState`.
#[derive(Clone)]
pub struct ScoringQueue {
    pool: PgPool,
    tx: mpsc::UnboundedSender<String>,
    waiters: Waiters,
}

/// `cpu/2`, floored at 1 — half the cores drain the queue, the rest stay free
/// for request handling.
pub fn default_worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get() / 2)
        .unwrap_or(1)
        .max(1)
}

impl ScoringQueue {
    /// Build the queue and spawn its dispatcher. Synchronous — the dispatcher
    /// runs boot recovery then drains work on its own task. Must be called
    /// inside a Tokio runtime.
    pub fn spawn(pool: PgPool, worker_count: usize) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        let waiters: Waiters = Arc::new(Mutex::new(HashMap::new()));
        let queue = Self {
            pool: pool.clone(),
            tx: tx.clone(),
            waiters: waiters.clone(),
        };
        tokio::spawn(dispatch(pool, tx, rx, waiters, worker_count.max(1)));
        queue
    }

    /// Register interest in a job's completion *before* enqueuing it, so a fast
    /// worker can't fire the notifier before the caller is listening. The
    /// receiver resolves when the job reaches a terminal state (done or failed).
    pub fn register(&self, flight_id: &str) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.waiters
            .lock()
            .expect("waiters mutex poisoned")
            .insert(flight_id.to_owned(), tx);
        rx
    }

    /// Insert the job row (`queued`) and hand the id to the workers.
    pub async fn enqueue(&self, flight_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO scoring_jobs (flight_id) VALUES ($1) ON CONFLICT (flight_id) DO NOTHING",
        )
        .bind(flight_id)
        .execute(&self.pool)
        .await?;
        // A send error means the dispatcher is gone (shutdown); the row stays
        // `queued` and boot recovery re-enqueues it next start.
        let _ = self.tx.send(flight_id.to_owned());
        Ok(())
    }

    /// Number of `queued` jobs ahead of this one — the caller's place in line.
    /// 0 once the job has started or finished.
    pub async fn position(&self, flight_id: &str) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT count(*) FROM scoring_jobs \
             WHERE state = 'queued' \
               AND created_at < (SELECT created_at FROM scoring_jobs WHERE flight_id = $1)",
        )
        .bind(flight_id)
        .fetch_one(&self.pool)
        .await
    }
}

async fn dispatch(
    pool: PgPool,
    tx: mpsc::UnboundedSender<String>,
    mut rx: mpsc::UnboundedReceiver<String>,
    waiters: Waiters,
    worker_count: usize,
) {
    if let Err(e) = recover(&pool, &tx).await {
        tracing::error!(error = %e, "scoring queue boot recovery failed");
    }

    let permits = Arc::new(Semaphore::new(worker_count));
    while let Some(flight_id) = rx.recv().await {
        // Acquire before spawning so at most `worker_count` jobs run at once.
        let permit = permits
            .clone()
            .acquire_owned()
            .await
            .expect("scoring semaphore closed");
        let pool = pool.clone();
        let waiters = waiters.clone();
        tokio::spawn(async move {
            run_job(&pool, &flight_id, &waiters).await;
            drop(permit);
        });
    }
}

/// Re-enqueue jobs left behind by a previous process: `running` rows are
/// orphaned (reset to `queued`), then every `queued` row is fed to the workers
/// in FIFO order.
async fn recover(pool: &PgPool, tx: &mpsc::UnboundedSender<String>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE scoring_jobs SET state = 'queued', started_at = NULL WHERE state = 'running'",
    )
    .execute(pool)
    .await?;
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT flight_id FROM scoring_jobs WHERE state = 'queued' ORDER BY created_at",
    )
    .fetch_all(pool)
    .await?;
    let n = ids.len();
    for id in ids {
        let _ = tx.send(id);
    }
    if n > 0 {
        tracing::info!(n, "scoring queue: re-enqueued pending jobs on boot");
    }
    Ok(())
}

async fn run_job(pool: &PgPool, flight_id: &str, waiters: &Waiters) {
    if let Err(e) = mark_running(pool, flight_id).await {
        tracing::error!(flight_id, error = %e, "failed to mark scoring job running");
        notify(waiters, flight_id);
        return;
    }

    match assess::score_and_store(pool, flight_id).await {
        Ok(saved) => {
            if let Err(e) = mark_done(pool, flight_id).await {
                tracing::error!(flight_id, error = %e, "failed to mark scoring job done");
            } else {
                tracing::info!(flight_id, saved, "scored flight");
            }
        }
        Err(e) => {
            let message = format!("{e:#}");
            tracing::error!(flight_id, error = %message, "scoring flight failed");
            if let Err(e) = mark_failed(pool, flight_id, &message).await {
                tracing::error!(flight_id, error = %e, "failed to mark scoring job failed");
            }
        }
    }

    notify(waiters, flight_id);
}

fn notify(waiters: &Waiters, flight_id: &str) {
    if let Some(tx) = waiters
        .lock()
        .expect("waiters mutex poisoned")
        .remove(flight_id)
    {
        let _ = tx.send(());
    }
}

async fn mark_running(pool: &PgPool, flight_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE scoring_jobs SET state = 'running', started_at = now(), attempts = attempts + 1 \
         WHERE flight_id = $1",
    )
    .bind(flight_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_done(pool: &PgPool, flight_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE scoring_jobs SET state = 'done', finished_at = now(), error = NULL \
         WHERE flight_id = $1",
    )
    .bind(flight_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_failed(pool: &PgPool, flight_id: &str, message: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE scoring_jobs SET state = 'failed', finished_at = now(), error = $2 \
         WHERE flight_id = $1",
    )
    .bind(flight_id)
    .bind(message)
    .execute(pool)
    .await?;
    Ok(())
}
