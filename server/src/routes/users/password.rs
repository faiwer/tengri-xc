use axum::{Json, extract::State};
use serde::Deserialize;
use sqlx::PgPool;

use super::into_internal;
use crate::{
    AppError, AppState,
    auth::{Identity, password},
    db::Update,
    user::{MeDto, fetch_me, weak_password},
    validation::FieldErrors,
};

/// Change (or, for a password-less account, set) the caller's password. `login`
/// is honored only when the account has none yet — a set-once initial login;
/// once a login exists the client sends nothing and any value here is ignored.
#[derive(Debug, Deserialize)]
pub(super) struct ChangePasswordRequest {
    #[serde(default)]
    login: Option<String>,
    #[serde(default)]
    current_password: String,
    new_password: String,
}

pub(super) async fn change_password(
    State(state): State<AppState>,
    identity: Identity,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<MeDto>, AppError> {
    let user_id = identity.user_id;

    let account = load_account(state.pool(), user_id).await?;
    let new_login = initial_login(req.login.as_deref(), &account);
    validate_change(&req, &account, new_login.as_deref())?;
    if let Some(login) = new_login.as_deref() {
        ensure_login_available(state.pool(), user_id, login).await?;
    }

    let hash =
        password::hash_argon2(&req.new_password).map_err(|e| AppError::Internal(e.into()))?;

    let mut q = Update::new("users");
    q.set("password_hash", hash);
    if let Some(login) = new_login {
        q.set("login", login);
    }
    q.and_where("id = $", (user_id,));
    q.execute(state.pool()).await.map_err(into_internal)?;

    let body = fetch_me(state.pool(), user_id).await?.ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "user {user_id} vanished mid-password-change"
        ))
    })?;
    Ok(Json(body))
}

/// What the caller can sign in with today. All three are nullable: an imported
/// or OAuth account may have no password, no login, or no address.
#[derive(Debug, sqlx::FromRow)]
struct Account {
    login: Option<String>,
    email: Option<String>,
    password_hash: Option<String>,
}

async fn load_account(pool: &PgPool, user_id: i32) -> Result<Account, AppError> {
    sqlx::query_as::<_, Account>("SELECT login, email, password_hash FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(into_internal)?
        .ok_or(AppError::NotFound)
}

/// The initial login to set, if any. Set-once, so anything an account with a
/// login already sends is ignored rather than rejected.
fn initial_login(requested: Option<&str>, account: &Account) -> Option<String> {
    if account.login.is_some() {
        return None;
    }
    requested
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Format and credential checks, reported together as one 422. The
/// login-availability query is deliberately not among them — see
/// [`ensure_login_available`].
fn validate_change(
    req: &ChangePasswordRequest,
    account: &Account,
    new_login: Option<&str>,
) -> Result<(), AppError> {
    let mut errors = FieldErrors::new();

    if let Some(msg) = weak_password(&req.new_password) {
        errors.add("new_password", msg);
    }

    // A password-less account (OAuth / import) sets its first password here
    // with no current-password check. Otherwise verify it.
    if let Some(hash) = account.password_hash.as_deref() {
        if req.current_password.is_empty() {
            errors.add("current_password", "Enter your current password");
        } else {
            match password::verify(&req.current_password, hash) {
                Ok(Some(_)) => {}
                Ok(None) => errors.add("current_password", "Incorrect password"),
                Err(e) => return Err(AppError::Internal(e.into())),
            }
        }
    }

    // The account must be able to sign in afterward.
    if account.login.is_none() && new_login.is_none() && account.email.is_none() {
        errors.add("login", "Set a login so you can sign in");
    }

    errors.into_result()
}

/// Runs only after [`validate_change`] passes, so a caller who can't prove the
/// current password gets no way to probe which logins are taken. Matching is
/// case-insensitive, like the `users_login_key` functional index.
async fn ensure_login_available(pool: &PgPool, user_id: i32, login: &str) -> Result<(), AppError> {
    let taken: Option<i32> =
        sqlx::query_scalar("SELECT id FROM users WHERE LOWER(login) = LOWER($1) AND id <> $2")
            .bind(login)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(into_internal)?;
    if taken.is_none() {
        return Ok(());
    }
    let mut errors = FieldErrors::new();
    errors.add("login", "Already taken");
    errors.into_result()
}
