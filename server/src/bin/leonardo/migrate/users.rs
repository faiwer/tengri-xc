//! `leonardo_pilots` → `users`. We carry over `pilotID` as `users.id`
//! and a single composed `name`; nothing else (yet).
//!
//! Why we ignore `serverID > 0`: the Leonardo schema is built for a
//! federated network where pilots from peer servers are mirrored
//! locally with a non-zero `serverID`. We only want our own pilots.
//!
//! Why we ignore `pilotID = 0`: it's a placeholder Leonardo uses for
//! "unassigned" / system rows, not a real pilot.
//!
//! Why we require at least one flight: we don't want dead souls.
//! Leonardo's `leonardo_pilots` is the user table for the entire
//! site (forum accounts, abandoned signups, admins) and most rows
//! have never logged a flight. The flights step is the only thing
//! that references `users.id`, so anyone without flights is, on our
//! platform, by definition a ghost.
//!
//! Name composition: `"First Last"`, falling back to `NickName`,
//! finally `"pilot_<id>"`. The destination column is `UNIQUE`, so we
//! dedup against earlier source rows in memory (`" (#<id>)"` suffix
//! on every occurrence past the first). That handles
//! within-source collisions; collisions against rows that already
//! exist in the destination (e.g. a dev-seeded user) bubble up as
//! per-row failures in the [`Report`].

use std::collections::HashSet;

use anyhow::Context;
use sqlx::{MySqlPool, PgPool, Row};

use super::{Failure, Report};

pub async fn run(mysql: &MySqlPool, pg: &PgPool) -> anyhow::Result<Report> {
    let pilots = fetch(mysql).await?;
    let (resolved, dropped_overflow) = compose(&pilots);
    let renamed_count = resolved.iter().filter(|r| r.renamed).count();
    let upsert_outcome = upsert(pg, &resolved).await?;

    let mut notes = Vec::new();
    notes.push(format!("source pilots scanned: {}", pilots.len()));
    if renamed_count > 0 {
        notes.push(format!(
            "renamed for uniqueness: {} (suffix \" (#<id>)\")",
            renamed_count
        ));
    }

    if !dropped_overflow.is_empty() {
        notes.push(format!(
            "dropped (pilotID outside i32 range): {}",
            dropped_overflow.len()
        ));
    }

    Ok(Report {
        table: "users",
        inserted: upsert_outcome.inserted,
        skipped: upsert_outcome.skipped,
        failures: upsert_outcome.failures,
        notes,
    })
}

struct SourcePilot {
    pilot_id: i64,
    first_name: String,
    last_name: String,
    nick_name: Option<String>,
}

/// Source pilot resolved into the shape the destination expects.
/// `renamed` records whether `compose_name` produced a duplicate
/// against an earlier pilot and we suffixed `" (#<id>)"` to break the
/// tie. Carried separately from the name so we can count renames
/// without re-parsing the strings.
struct Resolved {
    id: i32,
    name: String,
    renamed: bool,
}

#[derive(Default)]
struct UpsertOutcome {
    inserted: usize,
    skipped: usize,
    failures: Vec<Failure>,
}

async fn fetch(mysql: &MySqlPool) -> anyhow::Result<Vec<SourcePilot>> {
    // `EXISTS` rather than a `JOIN ... GROUP BY`: we only care
    // whether the pilot has any flight, not how many. MySQL stops
    // scanning `leonardo_flights` for that pilot at the first hit,
    // which on the indexed `userID` column is effectively free.
    let rows = sqlx::query(
        "SELECT p.pilotID, p.FirstName, p.LastName, p.NickName \
         FROM leonardo_pilots p \
         WHERE p.serverID = 0 \
           AND p.pilotID > 0 \
           AND EXISTS ( \
             SELECT 1 FROM leonardo_flights f \
             WHERE f.userID = p.pilotID \
               AND f.serverID = 0 \
           ) \
         ORDER BY p.pilotID",
    )
    .fetch_all(mysql)
    .await
    .context("querying leonardo_pilots")?;

    rows.into_iter()
        .map(|r| {
            Ok(SourcePilot {
                pilot_id: r.try_get::<i64, _>("pilotID")?,
                first_name: r.try_get::<String, _>("FirstName")?,
                last_name: r.try_get::<String, _>("LastName")?,
                nick_name: r.try_get::<Option<String>, _>("NickName")?,
            })
        })
        .collect()
}

/// Project pilots into the destination shape. Rows whose `pilotID`
/// doesn't fit in `i32` (the destination column type) are dropped
/// here rather than failing the entire run; their ids land in the
/// returned overflow list so the orchestrator can report the loss.
/// At present the source has none, but the column is `bigint` and
/// some federated pilot networks do issue large ids, so this is
/// belt-and-braces.
fn compose(pilots: &[SourcePilot]) -> (Vec<Resolved>, Vec<i64>) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<Resolved> = Vec::with_capacity(pilots.len());
    let mut overflow: Vec<i64> = Vec::new();

    for p in pilots {
        let Ok(id) = i32::try_from(p.pilot_id) else {
            overflow.push(p.pilot_id);
            continue;
        };
        let bare = compose_name(p);

        let (name, renamed) = if seen.contains(&bare) {
            (format!("{bare} (#{id})"), true)
        } else {
            (bare, false)
        };
        seen.insert(name.clone());
        out.push(Resolved { id, name, renamed });
    }
    (out, overflow)
}

fn compose_name(p: &SourcePilot) -> String {
    let first = p.first_name.trim();
    let last = p.last_name.trim();
    let full = match (first.is_empty(), last.is_empty()) {
        (false, false) => format!("{first} {last}"),
        (false, true) => first.to_owned(),
        (true, false) => last.to_owned(),
        (true, true) => String::new(),
    };
    if !full.is_empty() {
        return full;
    }
    if let Some(nick) = p
        .nick_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return nick.to_owned();
    }
    format!("pilot_{}", p.pilot_id)
}

/// Upsert rows one at a time in autocommit mode (no enclosing
/// transaction). Per-row failures land in [`UpsertOutcome::failures`]
/// and the loop keeps going — the whole point of this command is to
/// tell the operator which rows didn't make it; a poisoned
/// transaction can't keep inserting after the first error. Idempotency
/// is preserved by `ON CONFLICT (id) DO NOTHING`, so a half-finished
/// run can be completed by re-running.
///
/// The IDENTITY sequence bump *does* run as part of its own implicit
/// transaction, after all inserts; if every row failed it'll just
/// reset to 1, which is harmless.
async fn upsert(pg: &PgPool, users: &[Resolved]) -> anyhow::Result<UpsertOutcome> {
    let mut outcome = UpsertOutcome::default();

    for u in users {
        // `RETURNING id` only emits a row when the insert actually
        // happened, which is the cleanest "did we insert it?" signal
        // for `ON CONFLICT DO NOTHING`.
        let result = sqlx::query_scalar::<_, i32>(
            "INSERT INTO users (id, name) VALUES ($1, $2) \
             ON CONFLICT (id) DO NOTHING \
             RETURNING id",
        )
        .bind(u.id)
        .bind(&u.name)
        .fetch_optional(pg)
        .await;

        match result {
            Ok(Some(_)) => outcome.inserted += 1,
            Ok(None) => outcome.skipped += 1,
            Err(e) => outcome.failures.push(Failure {
                key: format!("id={} name={:?}", u.id, u.name),
                reason: format!("{e:#}"),
            }),
        }
    }

    sqlx::query(
        "SELECT setval( \
             pg_get_serial_sequence('users', 'id'), \
             GREATEST((SELECT COALESCE(MAX(id), 0) FROM users), 1) \
         )",
    )
    .execute(pg)
    .await
    .context("advancing users.id sequence")?;

    Ok(outcome)
}
