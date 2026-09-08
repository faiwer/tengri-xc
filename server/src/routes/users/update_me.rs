use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};

use super::{into_internal, jwt_to_internal, sqlx_to_internal};
use crate::{
    AppError, AppState,
    auth::{Identity, token::mint_session_cookie},
    mail::{ConfirmRecipient, is_smtp_configured, send_confirmation_email},
    site::{AdminSiteDto, fetch_site_admin},
    user::{
        AccountUpdate, MeDto, PreferencesUpdate, ProfileUpdate, UpdatePreferencesRequest,
        UpdateProfileRequest, UserSex, apply_account_update, apply_preferences_update,
        apply_profile_update, check_unique, fetch_me, plan_email_edit, validate_email,
        validate_name, validate_preferences_update, validate_profile_update,
    },
    validation::FieldErrors,
};

/// Owner-edit envelope. Each top-level block is optional and applied
/// independently — the FE form for preferences sends only `preferences`, the
/// profile form sends only `profile`, and a "save everything" flow can send
/// both. Empty body = 400 (no-op PATCH is a misuse).
///
/// Admin endpoints don't share this struct: the admin form has its own envelope
/// (with `permissions`, etc.) but reuses the same per-section validators and
/// appliers from `user::account` / `user::profile` / `user::preferences`.
#[derive(Debug, Deserialize)]
pub(super) struct UpdateMeRequest {
    #[serde(default)]
    profile: Option<MeProfileUpdate>,
    #[serde(default)]
    preferences: Option<UpdatePreferencesRequest>,
}

/// The self-service `profile` block. Flat on the wire, but spans two tables:
/// `name` / `email` live on `users`, the rest on `user_profiles`. `name` /
/// `email` are a plain `Option` (self-service never clears them); the
/// `user_profiles` fields keep the triple-state `Option<Option<T>>` (absent =
/// leave alone, `null` = clear, value = set).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct MeProfileUpdate {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default, deserialize_with = "crate::user::profile::deserialize_some")]
    civl_id: Option<Option<i32>>,
    #[serde(default, deserialize_with = "crate::user::profile::deserialize_some")]
    country: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::user::profile::deserialize_some")]
    sex: Option<Option<UserSex>>,
}

/// `PATCH /users/me` response: the refreshed [`MeDto`], flattened to the same
/// shape `GET /users/me` returns, plus where *this* request mailed a
/// confirmation link (`null` when the email didn't change).
///
/// Not the same as the `pending_email` inside the user record: that one holds
/// any change still in flight, including one started by an earlier request, and
/// resending its toast on every unrelated save would be noise.
#[derive(Debug, Serialize)]
struct UpdateMeResponse {
    #[serde(flatten)]
    me: MeDto,
    confirmation_sent_to: Option<String>,
}

pub(super) async fn update_me(
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
        confirmation_sent_to: pending_email,
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
