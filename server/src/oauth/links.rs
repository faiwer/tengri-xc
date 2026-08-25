//! Reads/writes for `user_oauth_links` (`0024`) — a user's connected provider
//! accounts. Identity is `(provider, provider_user_id)`; `email`/`display_name`
//! are display-only snapshots, refreshed each time the same account is
//! (re)linked.

use chrono::Utc;
use rand::Rng;
use serde::Serialize;
use sqlx::{PgConnection, PgExecutor};

use crate::{
    AppError,
    user::{CreateUser, Permissions, UserSource, create_user},
};

use super::provider::{OAuthIdentity, OAuthProvider};

/// One connected account, as listed in the user's settings.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkSnapshot {
    pub provider: OAuthProvider,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
}

/// Result of an [`upsert_link`] attempt. `TakenByOther` is the one case the
/// caller must surface to the user ("this account is already linked to a
/// different Tengri account"); the rest are silent successes.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkOutcome {
    /// New link created.
    Created,
    /// The caller already owned this identity; snapshot refreshed.
    Refreshed,
    /// The identity belongs to a *different* user — refused.
    TakenByOther,
}

/// A user's links, ordered by provider then link time for a stable list.
pub async fn list_links_for_user(
    pool: &sqlx::PgPool,
    user_id: i32,
) -> Result<Vec<LinkSnapshot>, AppError> {
    sqlx::query_as::<_, LinkSnapshot>(
        "SELECT provider, provider_user_id, email, display_name \
         FROM user_oauth_links \
         WHERE user_id = $1 \
         ORDER BY provider, created_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(into_internal)
}

/// Which user (if any) a provider identity resolves to — the login flow's
/// lookup. Identity is `(provider, subject)`, never email. Generic over the
/// executor so it runs on a pool or inside [`upsert_link`]'s transaction.
pub async fn find_user_by_link<'e, E>(
    executor: E,
    provider: OAuthProvider,
    provider_user_id: &str,
) -> Result<Option<i32>, AppError>
where
    E: PgExecutor<'e>,
{
    sqlx::query_scalar::<_, i32>(
        "SELECT user_id \
         FROM user_oauth_links \
         WHERE provider = $1::oauth_provider AND provider_user_id = $2",
    )
    .bind(provider.pg_enum_value())
    .bind(provider_user_id)
    .fetch_optional(executor)
    .await
    .map_err(into_internal)
}

/// Create the link for `user_id`, or refresh its snapshot if the same user
/// already holds it. If the identity is already linked to a *different* user,
/// nothing is written and [`LinkOutcome::TakenByOther`] is returned — the PK is
/// `(provider, provider_user_id)`, so one provider account maps to one user.
///
/// Not a single UPSERT: `ON CONFLICT DO UPDATE` can't express "only if the
/// existing row is mine", and moving the row to a new `user_id` on conflict
/// would silently steal another account's link. A read-then-write is correct
/// here; the window is a user racing themselves, harmless either way.
pub async fn upsert_link(
    conn: &mut PgConnection,
    user_id: i32,
    provider: OAuthProvider,
    identity: &OAuthIdentity,
) -> Result<LinkOutcome, AppError> {
    let existing_owner: Option<i32> =
        find_user_by_link(&mut *conn, provider, &identity.subject).await?;

    match existing_owner {
        Some(owner) if owner != user_id => Ok(LinkOutcome::TakenByOther),
        Some(_) => {
            sqlx::query(
                "UPDATE user_oauth_links \
                 SET email = $1, display_name = $2 \
                 WHERE provider = $3::oauth_provider AND provider_user_id = $4",
            )
            .bind(identity.email.as_deref())
            .bind(identity.display_name.as_deref())
            .bind(provider.pg_enum_value())
            .bind(&identity.subject)
            .execute(&mut *conn)
            .await
            .map_err(into_internal)?;
            Ok(LinkOutcome::Refreshed)
        }
        None => {
            sqlx::query(
                "INSERT INTO user_oauth_links \
                 (user_id, provider, provider_user_id, email, display_name) \
                 VALUES ($1, $2::oauth_provider, $3, $4, $5)",
            )
            .bind(user_id)
            .bind(provider.pg_enum_value())
            .bind(&identity.subject)
            .bind(identity.email.as_deref())
            .bind(identity.display_name.as_deref())
            .execute(&mut *conn)
            .await
            .map_err(into_internal)?;
            Ok(LinkOutcome::Created)
        }
    }
}

/// How many display-name candidates to try before giving up. `users.name` is
/// case-insensitively unique, so a common OAuth display name ("John Smith")
/// can collide; after the bare name we suffix a random number until one is
/// free.
const MAX_NAME_ATTEMPTS: usize = 10;

/// Range for the random display-name suffix. Wide enough that even a very
/// common base name almost never exhausts [`MAX_NAME_ATTEMPTS`].
const NAME_SUFFIX_RANGE: std::ops::RangeInclusive<u32> = 2..=10_000;

/// Create a fresh account for a provider identity that isn't linked to (or
/// email-matched against) any existing user, then link it. The new user gets
/// only [`Permissions::CAN_AUTHORIZE`] — no manage powers. The email (already
/// verified by [`OAuthProvider::resolve_email`]) is stored and stamped
/// confirmed; a missing email leaves the column `NULL`.
///
/// `users.name` is unique, so the display name is resolved against
/// `users_name_lower_key` by suffixing a random number, retrying on the
/// insert's unique-violation as the race backstop. After [`MAX_NAME_ATTEMPTS`]
/// the registration fails rather than looping.
///
/// Each attempt runs in its own transaction: the `users` insert and the link
/// insert commit together (no orphan user if the link fails), and a name
/// collision aborts that transaction, so the next candidate starts clean.
pub async fn register_oauth_user(
    pool: &sqlx::PgPool,
    provider: OAuthProvider,
    identity: &OAuthIdentity,
) -> Result<i32, AppError> {
    let base = base_name(identity);
    let email_verified_at = identity.email.as_ref().map(|_| Utc::now());

    let mut last_conflict = None;
    for attempt in 1..=MAX_NAME_ATTEMPTS {
        let name = if attempt == 1 {
            base.clone()
        } else {
            format!("{base} {}", rand::rng().random_range(NAME_SUFFIX_RANGE))
        };
        let input = CreateUser {
            id: None,
            name,
            login: None,
            email: identity.email.clone(),
            password: None,
            permissions: Permissions::default().bits(),
            source: UserSource::Internal,
            email_verified_at,
            last_login_at: None,
            created_at: None,
        };

        let mut tx = pool.begin().await.map_err(into_internal)?;
        match create_user(&mut tx, input).await {
            Ok(created) => {
                upsert_link(&mut tx, created.id, provider, identity).await?;
                tx.commit().await.map_err(into_internal)?;
                return Ok(created.id);
            }
            // Unique-violation on `name` — the insert aborted `tx`; drop it and
            // try the next candidate on a fresh transaction.
            Err(err) if is_name_conflict(&err) => {
                last_conflict = Some(err);
            }
            Err(err) => return Err(AppError::Internal(err)),
        }
    }

    Err(AppError::Internal(last_conflict.unwrap_or_else(|| {
        anyhow::anyhow!("no free display name after {MAX_NAME_ATTEMPTS} attempts")
    })))
}

/// Display name to seed the uniqueness search: the provider's name, else the
/// email's local part, else a generic fallback.
fn base_name(identity: &OAuthIdentity) -> String {
    identity
        .display_name
        .clone()
        .or_else(|| {
            identity
                .email
                .as_deref()
                .and_then(|email| email.split('@').next())
                .map(str::to_owned)
        })
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Pilot".to_owned())
}

/// Whether `err` is a `users_name_lower_key` unique violation — the signal to
/// try the next name candidate. Any other error propagates.
fn is_name_conflict(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<sqlx::Error>()
            .and_then(|e| e.as_database_error())
            .and_then(|db| db.constraint())
            == Some("users_name_lower_key")
    })
}

/// Result of a [`delete_link`] attempt.
pub enum UnlinkOutcome {
    /// Row removed.
    Deleted,
    /// Removing it would leave a password-less account with no way to sign
    /// in — refused, nothing written.
    WouldBrick,
    /// No such link for this user (already gone) — a no-op.
    NotFound,
}

/// Remove one of `user_id`'s links, scoped by `user_id` so a caller can only
/// unlink their own. Refuses the delete if it would strand a password-less
/// account with zero links ([`UnlinkOutcome::WouldBrick`]). The check and the
/// delete share a transaction so a user racing themselves can't slip past it.
pub async fn delete_link(
    pool: &sqlx::PgPool,
    user_id: i32,
    provider: OAuthProvider,
    provider_user_id: &str,
) -> Result<UnlinkOutcome, AppError> {
    let mut tx = pool.begin().await.map_err(into_internal)?;

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_oauth_links \
         WHERE user_id = $1 AND provider = $2::oauth_provider AND provider_user_id = $3)",
    )
    .bind(user_id)
    .bind(provider.pg_enum_value())
    .bind(provider_user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(into_internal)?;
    if !exists {
        return Ok(UnlinkOutcome::NotFound);
    }

    // Password-less account whose last link this is → bricking.
    let would_brick: bool = sqlx::query_scalar(
        "SELECT (SELECT password_hash FROM users WHERE id = $1) IS NULL \
         AND (SELECT count(*) FROM user_oauth_links WHERE user_id = $1) = 1",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(into_internal)?;
    if would_brick {
        return Ok(UnlinkOutcome::WouldBrick);
    }

    sqlx::query(
        "DELETE FROM user_oauth_links \
         WHERE user_id = $1 AND provider = $2::oauth_provider AND provider_user_id = $3",
    )
    .bind(user_id)
    .bind(provider.pg_enum_value())
    .bind(provider_user_id)
    .execute(&mut *tx)
    .await
    .map_err(into_internal)?;

    tx.commit().await.map_err(into_internal)?;
    Ok(UnlinkOutcome::Deleted)
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
