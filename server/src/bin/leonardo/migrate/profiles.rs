//! `leonardo_pilots` → `user_profiles`. The pilot row carries the
//! profile-shaped fields (CIVL ID, country, sex) that we keep
//! separate from `users` because they live on a different lifecycle
//! and are read by different paths. See `0003_user_profiles.sql`
//! for the rationale on splitting.
//!
//! Column mapping:
//!
//! | Destination | Source                                    |
//! |-------------|-------------------------------------------|
//! | `user_id`   | `pilots.pilotID`                          |
//! | `civl_id`   | `pilots.CIVL_ID` if `> 0`                 |
//! | `country`   | `pilots.countryCode` (uppercased)         |
//! | `sex`       | `pilots.Sex` via `UserSex::from_leonardo` |
//!
//! Idempotency: `ON CONFLICT (user_id) DO NOTHING`. Like the
//! `users` step, this is *first-import*, not *sync*: edits made
//! in Leonardo after the initial migration won't propagate. A
//! future `leonardo sync` command would handle that with explicit
//! `DO UPDATE` semantics.
//!
//! Filter: only pilots that survived the `users` step. We use the
//! same `EXISTS(flights)` clause to skip dead-soul rows; if a row
//! ever sneaked into `users` without a flight (it can't today,
//! but defensively) the FK on `user_id` would still keep this
//! step honest.

use anyhow::Context;
use sqlx::{MySqlPool, PgPool, Row};
use tengri_server::user::UserSex;

use super::{Failure, Report};

pub async fn run(mysql: &MySqlPool, pg: &PgPool) -> anyhow::Result<Report> {
    let pilots = fetch(mysql).await?;
    let composed: Vec<Resolved> = pilots.iter().filter_map(compose).collect();
    let dropped_overflow = pilots.len() - composed.len();

    let with_civl = composed.iter().filter(|r| r.civl_id.is_some()).count();
    let with_country = composed.iter().filter(|r| r.country.is_some()).count();
    let with_sex = composed.iter().filter(|r| r.sex.is_some()).count();

    let outcome = upsert(pg, &composed).await?;

    let mut notes = Vec::new();
    notes.push(format!("source pilots scanned: {}", pilots.len()));
    notes.push(format!("with CIVL ID: {with_civl}"));
    notes.push(format!("with country: {with_country}"));
    notes.push(format!("with sex: {with_sex}"));
    if dropped_overflow > 0 {
        notes.push(format!(
            "dropped (pilotID outside i32 range): {dropped_overflow}"
        ));
    }

    Ok(Report {
        table: "user_profiles",
        inserted: outcome.inserted,
        skipped: outcome.skipped,
        failures: outcome.failures,
        notes,
    })
}

struct SourcePilot {
    pilot_id: i64,
    /// `mediumint unsigned` in source: max 2^24 - 1 ≈ 16.7M, so
    /// `u32` covers it without surprises. Narrowed to `i32` (with
    /// 0 → `None`) in [`compose`].
    civl_id: u32,
    country_code: String,
    sex: String,
}

struct Resolved {
    user_id: i32,
    civl_id: Option<i32>,
    country: Option<String>,
    sex: Option<UserSex>,
}

#[derive(Default)]
struct UpsertOutcome {
    inserted: usize,
    skipped: usize,
    failures: Vec<Failure>,
}

async fn fetch(mysql: &MySqlPool) -> anyhow::Result<Vec<SourcePilot>> {
    let rows = sqlx::query(
        "SELECT p.pilotID, p.CIVL_ID, p.countryCode, p.Sex \
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
    .context("querying leonardo_pilots for profiles")?;

    rows.into_iter()
        .map(|r| {
            Ok(SourcePilot {
                pilot_id: r.try_get::<i64, _>("pilotID")?,
                // sqlx-mysql decodes MEDIUMINT UNSIGNED as u32;
                // any other Rust type triggers a runtime decode
                // error rather than silently widening, so match
                // it exactly here.
                civl_id: r.try_get::<u32, _>("CIVL_ID")?,
                country_code: r.try_get::<String, _>("countryCode")?,
                sex: r.try_get::<String, _>("Sex")?,
            })
        })
        .collect()
}

/// Project a source pilot into a destination profile row. Returns
/// `None` if the pilot id can't fit in `i32` (matches the `users`
/// step's defence).
fn compose(p: &SourcePilot) -> Option<Resolved> {
    let user_id = i32::try_from(p.pilot_id).ok()?;

    // 0 is Leonardo's sentinel for "unset"; CIVL never issues a
    // 0 anyway. We don't enforce uniqueness on the destination —
    // see the column comment in 0003_user_profiles.sql for why.
    // The `try_from` covers the (impossible-on-mediumint) case
    // where a u32 wouldn't fit i32; in practice mediumint caps
    // at ~16M, well inside i32.
    let civl_id = if p.civl_id > 0 {
        i32::try_from(p.civl_id).ok()
    } else {
        None
    };

    // ISO-3166 alpha-2: trim, uppercase, then sanity-check that
    // we got two ASCII letters. Anything else (empty string,
    // "Kazakhstan" full-name, garbage) becomes NULL — char(2)
    // would either truncate or refuse it, and silent truncation
    // is worse than NULL.
    let country = {
        let candidate = p.country_code.trim().to_ascii_uppercase();
        if candidate.len() == 2 && candidate.chars().all(|c| c.is_ascii_alphabetic()) {
            Some(candidate)
        } else {
            None
        }
    };

    let sex = UserSex::from_leonardo(&p.sex);

    Some(Resolved {
        user_id,
        civl_id,
        country,
        sex,
    })
}

/// Upsert one row per pilot. Same shape as the `users` step:
/// autocommit per row, per-row failures captured rather than
/// aborting the run. Re-runs are no-ops thanks to
/// `ON CONFLICT (user_id) DO NOTHING`.
async fn upsert(pg: &PgPool, profiles: &[Resolved]) -> anyhow::Result<UpsertOutcome> {
    let mut outcome = UpsertOutcome::default();

    for p in profiles {
        let result = sqlx::query_scalar::<_, i32>(
            "INSERT INTO user_profiles (user_id, civl_id, country, sex) \
             VALUES ($1, $2, $3, $4::user_sex) \
             ON CONFLICT (user_id) DO NOTHING \
             RETURNING user_id",
        )
        .bind(p.user_id)
        .bind(p.civl_id)
        .bind(&p.country)
        .bind(p.sex.map(|s| s.pg_enum_value()))
        .fetch_optional(pg)
        .await;

        match result {
            Ok(Some(_)) => outcome.inserted += 1,
            Ok(None) => outcome.skipped += 1,
            Err(e) => outcome.failures.push(Failure {
                key: format!("user_id={}", p.user_id),
                reason: format!("{e:#}"),
            }),
        }
    }

    Ok(outcome)
}
