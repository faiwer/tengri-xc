//! User creation shared by the CLI and future HTTP registration.

use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    auth::password,
    db::{Insert, advance_identity_sequence},
    user::UserSource,
};

pub struct CreateUser {
    pub id: Option<i32>,
    pub name: String,
    pub login: Option<String>,
    pub email: Option<String>,
    pub password: Option<CreateUserPassword>,
    pub permissions: i32,
    pub source: UserSource,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

pub enum CreateUserPassword {
    Plaintext(String),
    Hash(String),
}

impl CreateUser {
    pub fn internal(name: String) -> Self {
        Self {
            id: None,
            name,
            login: None,
            email: None,
            password: None,
            permissions: 1,
            source: UserSource::Internal,
            email_verified_at: None,
            last_login_at: None,
            created_at: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CreatedUser {
    pub id: i32,
    pub name: String,
    pub login: Option<String>,
    pub email: Option<String>,
    pub permissions: i32,
}

pub async fn create_user(pool: &sqlx::PgPool, input: CreateUser) -> anyhow::Result<CreatedUser> {
    create_user_inner(pool, input, Conflict::Error)
        .await?
        .ok_or_else(|| anyhow!("user was not inserted"))
}

pub async fn create_user_if_absent(
    pool: &sqlx::PgPool,
    input: CreateUser,
) -> anyhow::Result<Option<CreatedUser>> {
    create_user_inner(pool, input, Conflict::DoNothing).await
}

enum Conflict {
    Error,
    DoNothing,
}

async fn create_user_inner(
    pool: &sqlx::PgPool,
    input: CreateUser,
    conflict: Conflict,
) -> anyhow::Result<Option<CreatedUser>> {
    let name = normalize_required("name", input.name)?;
    let login = normalize_optional("login", input.login)?;
    let email = normalize_optional("email", input.email)?.map(|email| email.to_ascii_lowercase());
    if input.permissions < 0 {
        return Err(anyhow!("permissions must be non-negative"));
    }

    let password_hash = password_hash(input.password, login.as_deref(), email.as_deref())?;

    let id = insert_user(
        pool,
        InsertUser {
            id: input.id,
            name: &name,
            login: login.as_deref(),
            email: email.as_deref(),
            password_hash: password_hash.as_deref(),
            permissions: input.permissions,
            source: input.source,
            email_verified_at: input.email_verified_at,
            last_login_at: input.last_login_at,
            created_at: input.created_at,
            conflict,
        },
    )
    .await?;
    let Some(id) = id else {
        return Ok(None);
    };
    if input.id.is_some() {
        advance_identity_sequence(pool, "users", "id").await?;
    }

    Ok(Some(CreatedUser {
        id,
        name,
        login,
        email,
        permissions: input.permissions,
    }))
}

struct InsertUser<'a> {
    id: Option<i32>,
    name: &'a str,
    login: Option<&'a str>,
    email: Option<&'a str>,
    password_hash: Option<&'a str>,
    permissions: i32,
    source: UserSource,
    email_verified_at: Option<DateTime<Utc>>,
    last_login_at: Option<DateTime<Utc>>,
    created_at: Option<DateTime<Utc>>,
    conflict: Conflict,
}

async fn insert_user(pool: &sqlx::PgPool, user: InsertUser<'_>) -> anyhow::Result<Option<i32>> {
    let mut q = Insert::into("users");
    if let Some(id) = user.id {
        q.value("id", id);
    }
    q.value("name", user.name);
    q.value("login", user.login);
    q.value("email", user.email);
    q.value("password_hash", user.password_hash);
    q.value("permissions", user.permissions);
    q.value("source", user.source);
    if let Some(email_verified_at) = user.email_verified_at {
        q.value("email_verified_at", email_verified_at);
    }
    if let Some(last_login_at) = user.last_login_at {
        q.value("last_login_at", last_login_at);
    }
    if let Some(created_at) = user.created_at {
        q.value("created_at", created_at);
    }
    if let Conflict::DoNothing = user.conflict {
        q.on_conflict_do_nothing("id");
    }
    q.returning("id");

    match user.conflict {
        Conflict::Error => q.fetch_one_scalar(pool).await.map(Some),
        Conflict::DoNothing => q.fetch_optional_scalar(pool).await,
    }
    .context("inserting users row")
}

fn password_hash(
    password: Option<CreateUserPassword>,
    login: Option<&str>,
    email: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let Some(password) = password else {
        return Ok(None);
    };
    if login.is_none() && email.is_none() {
        return Err(anyhow!(
            "password requires login or email so the user can authenticate"
        ));
    }
    match password {
        CreateUserPassword::Plaintext(password) => {
            if password.is_empty() {
                return Err(anyhow!("password is empty"));
            }
            password::hash_argon2(&password)
                .context("hashing password")
                .map(Some)
        }
        CreateUserPassword::Hash(hash) => {
            if hash.trim().is_empty() {
                return Err(anyhow!("password hash is empty"));
            }
            Ok(Some(hash))
        }
    }
}

fn normalize_required(field: &str, value: String) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("{field} must not be empty"));
    }
    Ok(trimmed.to_owned())
}

fn normalize_optional(field: &str, value: Option<String>) -> anyhow::Result<Option<String>> {
    value
        .map(|value| normalize_required(field, value))
        .transpose()
}
