//! Wire-shaped user record. Shared by `/users/me` and `/admin/users/:id`
//! so the FE can reuse a single decoder for "user as JSON". `password_hash`
//! is deliberately not in the SELECT

use serde::Serialize;
use sqlx::Row;

use crate::{
    AppError,
    user::{UserSex, UserSource},
};

#[derive(Debug, Serialize)]
pub struct UserDto {
    pub id: i32,
    pub name: String,
    pub login: Option<String>,
    pub email: Option<String>,
    pub source: UserSource,
    /// Raw bits. Frontend uses `bit & N` checks, no enum needed.
    pub permissions: i32,
    /// Unix epoch seconds (UTC). The DB stores `timestamptz`; we
    /// project it as `bigint` so the wire stays numeric and the
    /// client can do `new Date(seconds * 1000)` without parsing.
    pub email_verified_at: Option<i64>,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub profile: Option<UserProfileDto>,
}

#[derive(Debug, Serialize)]
pub struct UserProfileDto {
    pub civl_id: Option<i32>,
    pub country: Option<String>,
    pub sex: Option<UserSex>,
}

/// Fetch one user joined with their profile, in the wire shape. `None`
/// if the row is gone. Caller decides whether that's a 404, a 200 with
/// `null`, or a 500 ("vanished mid-request").
pub async fn fetch_user(pool: &sqlx::PgPool, user_id: i32) -> Result<Option<UserDto>, AppError> {
    let row = sqlx::query(
        "SELECT \
            u.id, u.name, u.login, u.email, u.source, u.permissions, \
            EXTRACT(EPOCH FROM u.email_verified_at)::bigint AS email_verified_at, \
            EXTRACT(EPOCH FROM u.last_login_at)::bigint     AS last_login_at, \
            EXTRACT(EPOCH FROM u.created_at)::bigint        AS created_at, \
            p.civl_id, p.country, p.sex \
         FROM users u \
         LEFT JOIN user_profiles p ON p.user_id = u.id \
         WHERE u.id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(into_internal)?;

    let Some(row) = row else { return Ok(None) };

    let civl_id: Option<i32> = row.try_get("civl_id").map_err(sqlx_to_internal)?;
    let country: Option<String> = row.try_get("country").map_err(sqlx_to_internal)?;
    let sex: Option<UserSex> = row.try_get("sex").map_err(sqlx_to_internal)?;

    // `null` for "no profile data" (whether the row was missing
    // or had all-NULL columns); `Some` if any field is populated.
    let profile = if civl_id.is_some() || country.is_some() || sex.is_some() {
        Some(UserProfileDto {
            civl_id,
            country,
            sex,
        })
    } else {
        None
    };

    Ok(Some(UserDto {
        id: row.try_get("id").map_err(sqlx_to_internal)?,
        name: row.try_get("name").map_err(sqlx_to_internal)?,
        login: row.try_get("login").map_err(sqlx_to_internal)?,
        email: row.try_get("email").map_err(sqlx_to_internal)?,
        source: row.try_get("source").map_err(sqlx_to_internal)?,
        permissions: row.try_get("permissions").map_err(sqlx_to_internal)?,
        email_verified_at: row.try_get("email_verified_at").map_err(sqlx_to_internal)?,
        last_login_at: row.try_get("last_login_at").map_err(sqlx_to_internal)?,
        created_at: row.try_get("created_at").map_err(sqlx_to_internal)?,
        profile,
    }))
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}

fn sqlx_to_internal(e: sqlx::Error) -> AppError {
    AppError::Internal(anyhow::Error::new(e))
}
