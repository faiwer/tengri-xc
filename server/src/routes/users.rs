//! `/users/*` — auth and current-user.
//!
//! - `POST /users/login`  — `{ identifier, password }` → cookie + `/users/me`
//!   body. Identifier matches `login` or `email`, case-insensitively. 403 when
//!   the account holds an unconfirmed address; see `routes::register`.
//! - `POST /users/logout` — clear the cookie. Always 204.
//! - `GET  /users/me`     — current user, or `null` if anonymous. Always 200.
//! - `PATCH /users/me`    — owner-self profile/preferences edit. A changed
//!   email goes to `pending_email` and gets a confirmation link mailed to it
//!   rather than being written straight through; see [`plan_email_edit`].
//! - `POST /users/me/password` — owner-self change/set password. Sets an
//!   initial `login` too when the account has none. Returns the refreshed
//!   `/users/me` body.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::{
    AppError, AppState,
    auth::{
        Identity,
        cookie::clear_session,
        password::{self, Verified},
        token::mint_session_cookie,
    },
    db::Update,
    mail::{ConfirmRecipient, is_smtp_configured, send_confirmation_email},
    site::{AdminSiteDto, fetch_site_admin},
    user::{
        AccountUpdate, MeDto, Permissions, PreferencesUpdate, ProfileUpdate,
        UpdatePreferencesRequest, UpdateProfileRequest, UserSex, apply_account_update,
        apply_preferences_update, apply_profile_update, check_unique, fetch_me, plan_email_edit,
        validate_email, validate_name, validate_preferences_update, validate_profile_update,
        weak_password,
    },
    validation::FieldErrors,
};

/// `error` code on login's unconfirmed-address 403. The SPA keys its "check
/// your inbox" copy off this string, so the two have to move together.
const EMAIL_UNCONFIRMED: &str = "email_unconfirmed";

/// Routes that set/clear the cookie inline; mounted *outside*
/// the slide middleware.
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/users/login", post(login))
        .route("/users/logout", post(logout))
}

/// Routes that read identity from extensions; mounted behind the
/// slide middleware.
pub fn session_router() -> Router<AppState> {
    Router::new()
        .route("/users/me", get(me).patch(update_me))
        .route("/users/me/password", post(change_password))
}

// -----------------------------------------------------------------
// POST /users/login
// -----------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Matched against `users.login` (case-insensitive, via the
    /// `users_login_key` functional index) or `users.email`
    /// (stored lowercased).
    pub identifier: String,
    pub password: String,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Response, AppError> {
    let identifier = req.identifier.trim();
    if identifier.is_empty() || req.password.is_empty() {
        return Err(AppError::BadRequest(
            "identifier and password are required".into(),
        ));
    }

    // Try login *and* email in one query. `users_login_key` is on
    // `LOWER(login)`; email is stored lowercased.
    let row = sqlx::query(
        "SELECT id, name, permissions, password_hash, \
                email IS NULL AND pending_email IS NOT NULL AS needs_confirmation \
         FROM users \
         WHERE LOWER(login) = LOWER($1) \
            OR email        = LOWER($1) \
         LIMIT 1",
    )
    .bind(identifier)
    .fetch_optional(state.pool())
    .await
    .map_err(into_internal)?;

    // Same 401 for "no such user" and "wrong password" so
    // attackers can't tell logins apart by response.
    let Some(row) = row else {
        return Err(AppError::Unauthorized);
    };

    let user_id: i32 = row.try_get("id").map_err(sqlx_to_internal)?;
    let name: String = row.try_get("name").map_err(sqlx_to_internal)?;
    let permissions_bits: i32 = row.try_get("permissions").map_err(sqlx_to_internal)?;
    let stored_hash: Option<String> = row.try_get("password_hash").map_err(sqlx_to_internal)?;

    // OAuth-only account with no password set.
    let Some(stored_hash) = stored_hash else {
        return Err(AppError::Unauthorized);
    };

    let verified = match password::verify(&req.password, &stored_hash) {
        Ok(Some(v)) => v,
        Ok(None) => return Err(AppError::Unauthorized),
        Err(e) => {
            // Corrupt stored hash. Fail closed.
            tracing::error!(user_id, error = %e, "password verify failed on stored hash");
            return Err(AppError::Unauthorized);
        }
    };

    let permissions = Permissions::from_bits_retain(permissions_bits);
    // Banned/soft-disabled. Same 401 as wrong password.
    if !permissions.contains(Permissions::CAN_AUTHORIZE) {
        return Err(AppError::Unauthorized);
    }

    // Registered but never clicked the confirmation link. 403 with a reason,
    // not the 401s above: the password was already correct, so telling them to
    // check their inbox gives nothing away.
    let needs_confirmation: bool = row
        .try_get("needs_confirmation")
        .map_err(sqlx_to_internal)?;
    if needs_confirmation {
        return Err(AppError::ForbiddenReason {
            code: EMAIL_UNCONFIRMED,
            message: "Confirm your email address before signing in — check your inbox for the link"
                .into(),
        });
    }

    // Persist the rehash *before* minting so a panic between
    // verify and write doesn't leave the row stuck on phpass.
    // A failed update is fine — we'll retry next login.
    if let Verified {
        rehashed: Some(new_hash),
    } = &verified
    {
        if let Err(e) = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(new_hash)
            .bind(user_id)
            .execute(state.pool())
            .await
        {
            tracing::error!(user_id, error = %e, "rehash write failed; will retry next login");
        } else {
            tracing::info!(user_id, "rehashed phpass → argon2");
        }
    }

    if let Err(e) = sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(state.pool())
        .await
    {
        tracing::warn!(user_id, error = %e, "failed to update last_login_at");
    }

    let cookie = mint_session_cookie(
        user_id,
        name,
        permissions,
        state.jwt_encoding_key(),
        state.https(),
    )
    .map_err(jwt_to_internal)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(into_internal)?,
    );

    let body = fetch_me(state.pool(), user_id)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("user {user_id} vanished mid-login")))?;
    Ok((StatusCode::OK, headers, Json(body)).into_response())
}

// -----------------------------------------------------------------
// POST /users/logout
// -----------------------------------------------------------------

/// 204 even if there was no session — idempotent, and avoids
/// noisy 401s when the client logs out twice.
async fn logout(State(state): State<AppState>) -> Result<Response, AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&clear_session(state.https())).map_err(into_internal)?,
    );
    Ok((StatusCode::NO_CONTENT, headers).into_response())
}

// -----------------------------------------------------------------
// GET /users/me
// -----------------------------------------------------------------

/// `null` for anonymous, current user otherwise. Always 200 —
/// "nobody" is a valid answer here, and 401 would just spam the
/// browser console with red errors on every anon SPA boot.
async fn me(
    State(state): State<AppState>,
    identity: Option<Identity>,
) -> Result<Json<Option<MeDto>>, AppError> {
    let Some(identity) = identity else {
        return Ok(Json(None));
    };
    // Row missing = user hard-deleted between the last slide and
    // now. Treat as anonymous; the next request slides cleanly.
    Ok(Json(fetch_me(state.pool(), identity.user_id).await?))
}

// -----------------------------------------------------------------
// PATCH /users/me
// -----------------------------------------------------------------

/// Owner-edit envelope. Each top-level block is optional and applied
/// independently — the FE form for preferences sends only `preferences`,
/// the profile form sends only `profile`, and a "save everything" flow
/// can send both. Empty body = 400 (no-op PATCH is a misuse).
///
/// Admin endpoints don't share this struct: the admin form has its own
/// envelope (with `permissions`, etc.) but reuses the same per-section
/// validators and appliers from `user::account` / `user::profile` /
/// `user::preferences`.
#[derive(Debug, Deserialize)]
pub struct UpdateMeRequest {
    #[serde(default)]
    pub profile: Option<MeProfileUpdate>,
    #[serde(default)]
    pub preferences: Option<UpdatePreferencesRequest>,
}

/// The self-service `profile` block. Flat on the wire, but spans two tables:
/// `name` / `email` live on `users`, the rest on `user_profiles`. `name` /
/// `email` are a plain `Option` (self-service never clears them); the
/// `user_profiles` fields keep the triple-state `Option<Option<T>>` (absent =
/// leave alone, `null` = clear, value = set).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MeProfileUpdate {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default, deserialize_with = "crate::user::profile::deserialize_some")]
    pub civl_id: Option<Option<i32>>,
    #[serde(default, deserialize_with = "crate::user::profile::deserialize_some")]
    pub country: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::user::profile::deserialize_some")]
    pub sex: Option<Option<UserSex>>,
}

/// `PATCH /users/me` response: the refreshed [`MeDto`], flattened to the same
/// shape `GET /users/me` returns, plus the address this request left awaiting
/// confirmation (`null` when the email didn't change). `me.email` still holds
/// the old address in that case, so the client needs both to explain the state.
#[derive(Debug, Serialize)]
struct UpdateMeResponse {
    #[serde(flatten)]
    me: MeDto,
    pending_email: Option<String>,
}

async fn update_me(
    State(state): State<AppState>,
    identity: Identity,
    Json(req): Json<UpdateMeRequest>,
) -> Result<Response, AppError> {
    if req.profile.is_none() && req.preferences.is_none() {
        return Err(AppError::BadRequest(
            "PATCH body must include at least one of: profile, preferences".into(),
        ));
    }

    let update = validate_me_update(&state, &identity, req).await?;

    // Reissue the session cookie only when the display name actually changes.
    let name_changed = update.name().is_some_and(|new| new != identity.name);
    let pending_email = update.pending_email().map(str::to_owned);

    // No mail server means no confirmation link, and without that link the new
    // address can never be confirmed. Better to fail now than to save a change
    // the user can't finish. `POST /users/register` bails for the same reason.
    let mail_settings = if pending_email.is_some() {
        Some(get_smtp_settings(state.pool()).await?)
    } else {
        None
    };

    // Single transaction so a profile-write that succeeds is rolled back if a
    // later write trips on a constraint (or vice versa). Failures here are
    // infra-level (DB went away mid-request); user-input failures already
    // turned into 422 above.
    let mut tx = state.pool().begin().await.map_err(sqlx_to_internal)?;
    apply_me_update(&mut tx, identity.user_id, &update).await?;

    // Sent inside the transaction so a relay that rejects the message rolls the
    // pending address back rather than leaving one the user can't act on.
    if let (Some(stored), Some(address)) = (&mail_settings, &pending_email) {
        send_confirmation_email(
            stored,
            state.api_public_url(),
            state.jwt_encoding_key(),
            ConfirmRecipient {
                user_id: identity.user_id,
                name: update.name().unwrap_or(&identity.name),
                email: address,
            },
        )
        .await?;
    }

    tx.commit().await.map_err(sqlx_to_internal)?;

    let body = fetch_me(state.pool(), identity.user_id)
        .await?
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "user {} vanished mid-update",
                identity.user_id
            ))
        })?;

    let mut headers = HeaderMap::new();
    if name_changed {
        let cookie = mint_session_cookie(
            identity.user_id,
            body.user.name.clone(),
            identity.permissions,
            state.jwt_encoding_key(),
            state.https(),
        )
        .map_err(jwt_to_internal)?;
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_str(&cookie).map_err(into_internal)?,
        );
    }

    let response = UpdateMeResponse {
        me: body,
        pending_email,
    };
    Ok((StatusCode::OK, headers, Json(response)).into_response())
}

/// A validated `PATCH /users/me`, split by the table each part writes.
struct MeUpdate {
    account: Option<AccountUpdate>,
    profile: Option<ProfileUpdate>,
    preferences: Option<PreferencesUpdate>,
}

impl MeUpdate {
    /// The display name this request writes, if any.
    fn name(&self) -> Option<&str> {
        self.account.as_ref()?.name.as_deref()
    }

    /// The address this request leaves awaiting confirmation, if any.
    fn pending_email(&self) -> Option<&str> {
        self.account.as_ref()?.pending_email.as_deref()
    }
}

/// Validate every block before any of it is written, so one bad field can't
/// half-apply the request. Each block's errors are namespaced under its own
/// key, which is what lets the FE keep a single `fieldPrefix` per block.
async fn validate_me_update(
    state: &AppState,
    identity: &Identity,
    req: UpdateMeRequest,
) -> Result<MeUpdate, AppError> {
    let mut errors = FieldErrors::new();
    let mut account = None;
    let mut profile = None;

    if let Some(input) = req.profile {
        let mut section = FieldErrors::new();

        let name = input.name.map(|raw| validate_name(&raw, &mut section));
        let email = validate_email(input.email, &mut section);
        match validate_profile_update(UpdateProfileRequest {
            civl_id: input.civl_id,
            country: input.country,
            sex: input.sex,
        }) {
            Ok(u) => profile = Some(u),
            Err(field_errors) => {
                for (key, message) in field_errors.fields {
                    section.add(key, message);
                }
            }
        }

        // Uniqueness (DB reads) — only the values we'd actually write. `login`
        // isn't editable here. `validate_email` already errored on a malformed
        // address, so a `None` email skips the check rather than
        // double-reporting.
        check_unique(
            state.pool(),
            name.as_deref(),
            None,
            email.as_deref(),
            Some(identity.user_id),
            &mut section,
        )
        .await?;

        errors.merge_prefixed("profile", section);

        // A new address only goes pending, never straight to `email`: the
        // confirmation link is what promotes it, so the address the user can
        // still receive mail at stays the one on file until then.
        let pending_email =
            plan_email_edit(state.pool(), identity.user_id, email.as_deref()).await?;

        account = Some(AccountUpdate {
            name,
            pending_email,
        });
    }

    let mut preferences = None;
    if let Some(input) = req.preferences {
        match validate_preferences_update(input) {
            Ok(u) => preferences = Some(u),
            Err(field_errors) => errors.merge_prefixed("preferences", field_errors),
        }
    }

    errors.into_result()?;
    Ok(MeUpdate {
        account,
        profile,
        preferences,
    })
}

async fn apply_me_update(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i32,
    update: &MeUpdate,
) -> Result<(), AppError> {
    if let Some(u) = &update.account {
        apply_account_update(tx, user_id, u).await?;
    }
    if let Some(u) = &update.profile {
        apply_profile_update(tx, user_id, u).await?;
    }
    if let Some(u) = &update.preferences {
        apply_preferences_update(tx, user_id, u).await?;
    }
    Ok(())
}

/// The stored mail settings, or a 409 saying the address change can't start.
async fn get_smtp_settings(pool: &PgPool) -> Result<AdminSiteDto, AppError> {
    let stored = fetch_site_admin(pool).await?;
    if !is_smtp_configured(&stored) {
        return Err(AppError::Conflict(
            "Can't change your email address right now — outgoing mail isn't configured".into(),
        ));
    }
    Ok(stored)
}

/// Change (or, for a password-less account, set) the caller's password. `login`
/// is honored only when the account has none yet — a set-once initial login;
/// once a login exists the client sends nothing and any value here is ignored.
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    #[serde(default)]
    login: Option<String>,
    #[serde(default)]
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<AppState>,
    identity: Identity,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<MeDto>, AppError> {
    let user_id = identity.user_id;

    let row = sqlx::query("SELECT login, email, password_hash FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(state.pool())
        .await
        .map_err(into_internal)?
        .ok_or(AppError::NotFound)?;
    let stored_login: Option<String> = row.try_get("login").map_err(sqlx_to_internal)?;
    let stored_email: Option<String> = row.try_get("email").map_err(sqlx_to_internal)?;
    let stored_hash: Option<String> = row.try_get("password_hash").map_err(sqlx_to_internal)?;

    // Stage one: format + credential checks. The login-availability query is
    // deliberately *not* here — it runs last, so a caller who fails these can't
    // use this endpoint to enumerate taken logins.
    let mut errors = FieldErrors::new();

    if let Some(msg) = weak_password(&req.new_password) {
        errors.add("new_password", msg);
    }

    // A password-less account (OAuth / import) sets its first password here
    // with no current-password check. Otherwise verify it.
    if let Some(hash) = stored_hash.as_deref() {
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

    // Login is set-once: editable only while NULL.
    let new_login = if stored_login.is_none() {
        req.login
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    } else {
        None
    };

    // The account must be able to sign in afterward.
    if stored_login.is_none() && new_login.is_none() && stored_email.is_none() {
        errors.add("login", "Set a login so you can sign in");
    }

    errors.into_result()?;

    // Runs last, and only when setting an initial login. Case-insensitive,
    // matching the `users_login_key` functional index.
    if let Some(login) = new_login.as_deref() {
        let taken: Option<i32> =
            sqlx::query_scalar("SELECT id FROM users WHERE LOWER(login) = LOWER($1) AND id <> $2")
                .bind(login)
                .bind(user_id)
                .fetch_optional(state.pool())
                .await
                .map_err(into_internal)?;
        if taken.is_some() {
            let mut errors = FieldErrors::new();
            errors.add("login", "Already taken");
            errors.into_result()?;
        }
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

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
fn sqlx_to_internal(e: sqlx::Error) -> AppError {
    AppError::Internal(anyhow::Error::new(e))
}
fn jwt_to_internal(e: jsonwebtoken::errors::Error) -> AppError {
    AppError::Internal(anyhow::Error::new(e))
}
