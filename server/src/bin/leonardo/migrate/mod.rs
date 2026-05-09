//! `leonardo migrate` — copy data from a Leonardo XC MySQL database
//! into our Postgres. Built as a sequence of self-contained steps,
//! one per source table, all sharing a single contract:
//!
//! - **Idempotent.** Re-running is safe: existing rows are not
//!   touched, only missing ones are inserted. Each step owns its own
//!   conflict handling (typically `ON CONFLICT DO NOTHING` on a
//!   natural key).
//! - **Id-preserving.** Source primary keys map straight onto ours
//!   (`leonardo_pilots.pilotID → users.id` today; flights, comments,
//!   photos to follow). That keeps cross-table references trivially
//!   translatable — a child step can reference parent ids without
//!   consulting a mapping table — at the cost of having to bump each
//!   destination IDENTITY sequence past `MAX(id)` after the bulk
//!   insert. That bump lives with the step, not here.
//! - **Best-effort.** A single bad source row doesn't abort the run;
//!   it lands in the step's [`Report::failures`] and the orchestrator
//!   prints a per-row reason at the end. That's a deliberate
//!   trade-off vs. wrapping each step in one big transaction:
//!   per-row reporting is the whole point of this command, and a
//!   partial run can be finished by re-running thanks to the
//!   idempotent upsert. Steps still wrap *infrastructure* work
//!   (sequence bumps, multi-row writes that must move together) in
//!   their own narrow transactions where it actually matters.
//!
//! Adding a new step:
//! 1. Drop a sibling module with `pub async fn run(&MySqlPool,
//!    &PgPool) -> anyhow::Result<Report>`. Internal step-specific
//!    detail (e.g. how many rows got renamed for uniqueness) goes
//!    into [`Report::notes`] rather than the struct itself, so this
//!    file doesn't accumulate every step's quirks.
//! 2. Call it from [`run`] below in dependency order (children after
//!    their parents) and push its report onto `reports`.

mod flights;
mod profiles;
mod progress;
mod users;

use super::shared::{connect_mysql_pool, connect_pg_pool};

/// What a single migration step reports back to the orchestrator.
/// Plain data, no behaviour — formatting lives in [`print_summary`].
pub(super) struct Report {
    /// Human-readable name of the destination table this step writes
    /// into (`"users"`, `"flights"`, …). Used as the row label in the
    /// summary and as the prefix on per-failure lines.
    pub table: &'static str,
    pub inserted: usize,
    pub skipped: usize,
    pub failures: Vec<Failure>,
    /// Step-specific stats that don't fit the inserted/skipped/failed
    /// rubric. Examples: `"38 renamed for uniqueness"`,
    /// `"12 dropped: pilotID outside i32 range"`. Free-form so adding
    /// a step doesn't force a schema change here.
    pub notes: Vec<String>,
}

pub(super) struct Failure {
    /// Identifier of the offending row, formatted by the step
    /// (typically `"id=<n> name=<…>"`). Free-form so each step picks
    /// whatever uniquely identifies its rows in the source.
    pub key: String,
    /// One-line error message, already flattened with `{:#}`.
    pub reason: String,
}

pub async fn run() -> anyhow::Result<()> {
    let mysql = connect_mysql_pool().await?;
    let pg = connect_pg_pool().await?;

    let mut reports: Vec<Report> = Vec::new();
    reports.push(users::run(&mysql, &pg).await?);
    reports.push(profiles::run(&mysql, &pg).await?);
    reports.push(flights::run(&mysql, &pg).await?);

    print_summary(&reports);
    Ok(())
}

fn print_summary(reports: &[Report]) {
    println!("migrate complete");
    for r in reports {
        println!(
            "  {:<14} created {}, skipped {}, failed {}",
            r.table,
            r.inserted,
            r.skipped,
            r.failures.len()
        );
        for note in &r.notes {
            println!("    {note}");
        }
    }

    let total_failed: usize = reports.iter().map(|r| r.failures.len()).sum();
    if total_failed > 0 {
        println!();
        println!("failures:");
        for r in reports {
            for f in &r.failures {
                println!("  [{}] {}: {}", r.table, f.key, f.reason);
            }
        }
    }
}
