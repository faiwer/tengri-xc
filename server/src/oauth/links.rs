//! Reads/writes for `user_oauth_links` (`0024`) — a user's connected provider
//! accounts. Identity is `(provider, provider_user_id)`; `email`/`display_name`
//! are display-only snapshots, refreshed each time the same account is
//! (re)linked.

use serde::Serialize;

use crate::AppError;

use super::provider::{OAuthIdentity, OAuthProvider};

/// One connected account, as listed in the user's settings. The provider
/// subject id is the identity key and stays server-side — never serialized.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkSnapshot {
    pub provider: OAuthProvider,
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
        "SELECT provider, email, display_name \
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
/// lookup. Identity is `(provider, subject)`, never email.
pub async fn find_user_by_link(
    pool: &sqlx::PgPool,
    provider: OAuthProvider,
    provider_user_id: &str,
) -> Result<Option<i32>, AppError> {
    sqlx::query_scalar::<_, i32>(
        "SELECT user_id \
         FROM user_oauth_links \
         WHERE provider = $1::oauth_provider AND provider_user_id = $2",
    )
    .bind(provider.pg_enum_value())
    .bind(provider_user_id)
    .fetch_optional(pool)
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
    pool: &sqlx::PgPool,
    user_id: i32,
    provider: OAuthProvider,
    identity: &OAuthIdentity,
) -> Result<LinkOutcome, AppError> {
    let existing_owner: Option<i32> = find_user_by_link(pool, provider, &identity.subject).await?;

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
            .execute(pool)
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
            .execute(pool)
            .await
            .map_err(into_internal)?;
            Ok(LinkOutcome::Created)
        }
    }
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
