//! `leonardo_pilots` ⨝ `leonardo_users` → `users`. The pilot row
//! holds the human bits (names, nick) and the users row holds the
//! auth bits (login, email, password hash, registration / activity
//! timestamps). Leonardo XC keeps them as a 1:1 pair via
//! `pilotID = user_id`, so we join directly.
//!
//! Column mapping:
//!
//! | Destination          | Source (`leonardo_*`)                                    |
//! |----------------------|----------------------------------------------------------|
//! | `id`                 | `pilots.pilotID`                                         |
//! | `name`               | composed from `FirstName` / `LastName` / `NickName`      |
//! | `login`              | `users.username`                                         |
//! | `email`              | `users.user_email`, lowercased; `NULL` if blank          |
//! | `password_hash`      | `users.user_password` (phpass `$H$...` hash, verbatim)   |
//! | `source`             | constant `'leo'`                                         |
//! | `permissions`        | bit `CAN_AUTHORIZE` set iff `user_active = 1`            |
//! | `email_verified_at`  | `to_timestamp(user_regdate)` if email present and reg>0  |
//! | `created_at`         | `to_timestamp(user_regdate)` if `>0`, else default now() |
//! | `last_seen_at`       | `to_timestamp(user_lastvisit)` if `>0`                   |
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
//!
//! On password hashes: Leonardo stores phpass portable hashes
//! (`$H$9...`, 34 bytes). We carry them through verbatim — the login
//! flow knows how to verify phpass and silently rehash to a modern
//! KDF on first successful login. No salt column is needed; phpass
//! embeds the salt inside the hash string.
//!
//! Email collisions: Leonardo doesn't enforce uniqueness on
//! `user_email`, ours does (`users_email_key`). When two active
//! pilots share an email we keep the one with the lower `pilotID`
//! (deterministic, matches our `ORDER BY pilotID` fetch) and drop
//! the email on the rest, leaving them able to log in via
//! `login` + `password_hash` but unable to recover the password.
//! That's the honest tradeoff: suffixing the email (`+id15@...`)
//! would also break recovery — the user can't guess our suffix —
//! while pretending to be unique. The collision goes into
//! `Report::notes` so an operator can manually merge the accounts.

use std::collections::{HashMap, HashSet};

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, PgPool, Row};
use tengri_server::user::{Permissions, UserSource};

use super::{Failure, Report};

pub async fn run(mysql: &MySqlPool, pg: &PgPool) -> anyhow::Result<Report> {
    let pilots = fetch(mysql).await?;
    let composed = compose(&pilots);
    let renamed_count = composed.rows.iter().filter(|r| r.renamed).count();
    let inactive_count = composed.rows.iter().filter(|r| !r.active).count();
    let no_email_count = composed.rows.iter().filter(|r| r.email.is_none()).count();
    let upsert_outcome = upsert(pg, &composed.rows).await?;

    let mut notes = Vec::new();
    notes.push(format!("source pilots scanned: {}", pilots.len()));
    if renamed_count > 0 {
        notes.push(format!(
            "renamed for uniqueness: {} (suffix \" (#<id>)\")",
            renamed_count
        ));
    }
    if inactive_count > 0 {
        notes.push(format!(
            "inactive accounts (CAN_AUTHORIZE cleared): {inactive_count}"
        ));
    }
    if no_email_count > 0 {
        notes.push(format!("rows with no email: {no_email_count}"));
    }
    for c in &composed.email_conflicts {
        notes.push(format!(
            "email collision: id={} dropped {:?} (kept by id={})",
            c.id, c.email, c.kept_by
        ));
    }

    if !composed.dropped_overflow.is_empty() {
        notes.push(format!(
            "dropped (pilotID outside i32 range): {}",
            composed.dropped_overflow.len()
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
    username: String,
    email: Option<String>,
    password_hash: String,
    user_active: bool,
    user_regdate: i64,
    user_lastvisit: i64,
}

/// Source pilot resolved into the shape the destination expects.
/// `renamed` records whether `compose_name` produced a duplicate
/// against an earlier pilot and we suffixed `" (#<id>)"` to break the
/// tie. The auth columns ride through with the pilot so we don't
/// have to re-fetch when we hand the row to `upsert`.
struct Resolved {
    id: i32,
    name: String,
    renamed: bool,
    login: String,
    email: Option<String>,
    password_hash: String,
    permissions: Permissions,
    active: bool,
    created_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
    email_verified_at: Option<DateTime<Utc>>,
}

#[derive(Default)]
struct UpsertOutcome {
    inserted: usize,
    skipped: usize,
    failures: Vec<Failure>,
}

async fn fetch(mysql: &MySqlPool) -> anyhow::Result<Vec<SourcePilot>> {
    // `EXISTS` rather than a `JOIN ... GROUP BY` on flights: we only
    // care whether the pilot has any flight, not how many. MySQL
    // stops scanning `leonardo_flights` for that pilot at the first
    // hit, which on the indexed `userID` column is effectively free.
    //
    // The pilots ↔ users join is INNER on purpose: every active
    // pilot in this dump has a matching `leonardo_users` row (we
    // sanity-checked: 31/31). If a future import hits a pilot
    // without a user we'd rather know about it via 0 inserted than
    // silently skip them with a LEFT JOIN.
    let rows = sqlx::query(
        "SELECT \
             p.pilotID, p.FirstName, p.LastName, p.NickName, \
             u.username, u.user_email, u.user_password, \
             u.user_active, u.user_regdate, u.user_lastvisit \
         FROM leonardo_pilots p \
         JOIN leonardo_users u ON u.user_id = p.pilotID \
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
    .context("querying leonardo_pilots ⨝ leonardo_users")?;

    rows.into_iter()
        .map(|r| {
            let email_raw = r.try_get::<Option<String>, _>("user_email")?;
            let email = email_raw
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty());
            // Leonardo stores `user_active` as TINYINT(1); `i8`
            // covers 0 / 1 without `bool` decoding ambiguity.
            let active = r.try_get::<Option<i8>, _>("user_active")?.unwrap_or(1) != 0;

            Ok(SourcePilot {
                pilot_id: r.try_get::<i64, _>("pilotID")?,
                first_name: r.try_get::<String, _>("FirstName")?,
                last_name: r.try_get::<String, _>("LastName")?,
                nick_name: r.try_get::<Option<String>, _>("NickName")?,
                username: r.try_get::<String, _>("username")?,
                email,
                password_hash: r.try_get::<String, _>("user_password")?,
                user_active: active,
                user_regdate: r.try_get::<i64, _>("user_regdate")?,
                user_lastvisit: r.try_get::<i64, _>("user_lastvisit")?,
            })
        })
        .collect()
}

/// What `compose` produces: the rows ready for upsert plus
/// per-source diagnostics the orchestrator surfaces in
/// [`Report::notes`]. Boxed up in a struct because the outputs
/// have grown past what's pleasant to return as a tuple.
struct Composed {
    rows: Vec<Resolved>,
    /// Pilot ids whose `pilotID` overflowed `i32` and were skipped.
    /// At present the source has none, but the column is `bigint`
    /// and some federated pilot networks do issue large ids, so
    /// this is belt-and-braces.
    dropped_overflow: Vec<i64>,
    /// Source pilots whose email collided with an earlier-resolved
    /// pilot's. The losing row still gets imported (login + hash
    /// intact), but its `email` is set to NULL.
    email_conflicts: Vec<EmailConflict>,
}

struct EmailConflict {
    id: i32,
    email: String,
    kept_by: i32,
}

/// Project pilots into the destination shape. Within-source
/// uniqueness is enforced here so we never hand the database a
/// guaranteed-to-fail row:
///
/// * `name` collisions get a `" (#<id>)"` suffix on every
///   occurrence past the first.
/// * `email` collisions clear the email on every occurrence past
///   the first (recorded in `email_conflicts`); the loser keeps
///   `login` + `password_hash` and remains a usable account.
///
/// The first occurrence wins by source order (`ORDER BY pilotID`
/// in `fetch`), so the lower-id pilot keeps the email. That's a
/// deterministic but arbitrary tiebreak; an operator can swap
/// them by hand after import if it matters.
fn compose(pilots: &[SourcePilot]) -> Composed {
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_emails: HashMap<String, i32> = HashMap::new();
    let mut rows: Vec<Resolved> = Vec::with_capacity(pilots.len());
    let mut overflow: Vec<i64> = Vec::new();
    let mut email_conflicts: Vec<EmailConflict> = Vec::new();

    for p in pilots {
        let Ok(id) = i32::try_from(p.pilot_id) else {
            overflow.push(p.pilot_id);
            continue;
        };
        let bare = compose_name(p);

        let (name, renamed) = if seen_names.contains(&bare) {
            (format!("{bare} (#{id})"), true)
        } else {
            (bare, false)
        };
        seen_names.insert(name.clone());

        // Email dedup. Insert returns the previous owner if any —
        // we keep that owner and drop our email. The conflict
        // bookkeeping is purely for the operator; the DB never
        // sees the loser's email so the unique index can't fire.
        let email = match p.email.as_deref() {
            Some(e) => match seen_emails.get(e) {
                Some(&kept_by) => {
                    email_conflicts.push(EmailConflict {
                        id,
                        email: e.to_owned(),
                        kept_by,
                    });
                    None
                }
                None => {
                    seen_emails.insert(e.to_owned(), id);
                    Some(e.to_owned())
                }
            },
            None => None,
        };

        // Default = CAN_AUTHORIZE. Inactive users get an empty
        // bitfield: that's the soft-disable contract documented on
        // `Permissions::CAN_AUTHORIZE` (clear bit 0 → cannot log
        // in, but the row is preserved).
        let permissions = if p.user_active {
            Permissions::default()
        } else {
            Permissions::empty()
        };
        let created_at = unix_to_utc(p.user_regdate);
        let last_seen_at = unix_to_utc(p.user_lastvisit);
        // Leonardo doesn't carry an explicit "email verified"
        // timestamp; it just doesn't let `user_active` flip until
        // the activation key in the welcome email is used. So if
        // the row is active *and* has an email *and* has a
        // registration timestamp, treat that as verified-then.
        // If we just dropped the email above, the verified flag
        // would refer to an address we no longer store, so leave
        // it NULL.
        let email_verified_at = if p.user_active && email.is_some() {
            created_at
        } else {
            None
        };

        rows.push(Resolved {
            id,
            name,
            renamed,
            login: p.username.trim().to_owned(),
            email,
            password_hash: p.password_hash.clone(),
            permissions,
            active: p.user_active,
            created_at,
            last_seen_at,
            email_verified_at,
        });
    }
    Composed {
        rows,
        dropped_overflow: overflow,
        email_conflicts,
    }
}

/// Convert a Leonardo Unix-epoch second column to a UTC datetime.
/// The source uses `0` as "unset", so we map that to `None`.
fn unix_to_utc(seconds: i64) -> Option<DateTime<Utc>> {
    if seconds <= 0 {
        return None;
    }
    DateTime::<Utc>::from_timestamp(seconds, 0)
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
        // for `ON CONFLICT DO NOTHING`. We deliberately *don't*
        // upsert auth fields on conflict: this command is for first
        // imports and re-runs to fill gaps, not for picking up
        // password / email changes that happened in Leonardo after
        // the initial import. (That'd be a future
        // `leonardo sync` command with explicit semantics.)
        let result = sqlx::query_scalar::<_, i32>(
            "INSERT INTO users ( \
                 id, name, login, email, password_hash, \
                 source, permissions, \
                 email_verified_at, last_seen_at, created_at \
             ) \
             VALUES ( \
                 $1, $2, $3, $4, $5, \
                 $6::user_source, $7, \
                 $8, $9, COALESCE($10, now()) \
             ) \
             ON CONFLICT (id) DO NOTHING \
             RETURNING id",
        )
        .bind(u.id)
        .bind(&u.name)
        .bind(&u.login)
        .bind(&u.email)
        .bind(&u.password_hash)
        .bind(UserSource::Leo.pg_enum_value())
        .bind(u.permissions.bits())
        .bind(u.email_verified_at)
        .bind(u.last_seen_at)
        .bind(u.created_at)
        .fetch_optional(pg)
        .await;

        match result {
            Ok(Some(_)) => outcome.inserted += 1,
            Ok(None) => outcome.skipped += 1,
            Err(e) => outcome.failures.push(Failure {
                key: format!("id={} login={:?}", u.id, u.login),
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
