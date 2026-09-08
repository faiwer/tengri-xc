//! `POST /users/register` — create an unconfirmed account and mail it a
//! confirmation link. Always 204; there's deliberately no session, since `POST
//! /users/login` refuses an unconfirmed address anyway.

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use serde::Deserialize;

use super::into_internal;
use crate::{
    AppError, AppState,
    mail::{ConfirmRecipient, is_smtp_configured, send_confirmation_email},
    site::{fetch_site_admin, fetch_site_public},
    user::{
        CreateUser, CreateUserPassword, Permissions, UserSource, check_unique, create_user,
        validate_email, validate_login, validate_name, weak_password,
    },
    validation::FieldErrors,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/users/register", post(register))
}

async fn register(
    State(state): State<AppState>,
    Json(input): Json<RegisterRequest>,
) -> Result<StatusCode, AppError> {
    // The client hides its register link on the same flag; this is the gate
    // that actually holds.
    if !fetch_site_public(state.pool()).await?.can_register {
        return Err(AppError::Forbidden);
    }

    let valid = validate(input)?;

    // Uniqueness pre-check for clean per-field 422s; the unique indexes
    // (`users_name_key` / `users_login_key` / `users_email_key`) remain the
    // backstop for a race between this check and the insert.
    //
    // The email arm only sees *proven* addresses, since that's all `email`
    // ever holds. So registering an address nobody has confirmed is allowed
    // even if another pending signup already named it — whoever opens their
    // link first gets it, and only the mailbox owner can do that.
    let mut errors = FieldErrors::new();
    check_unique(
        state.pool(),
        Some(&valid.name),
        Some(&valid.login),
        Some(&valid.email),
        None,
        &mut errors,
    )
    .await?;
    errors.into_result()?;

    // Checked before anything is written: an account nobody can ever confirm
    // is worse than a refusal, because it's holding a login and a name.
    let stored = fetch_site_admin(state.pool()).await?;
    if !is_smtp_configured(&stored) {
        return Err(AppError::Conflict(
            "Registration is unavailable right now — outgoing mail isn't configured".into(),
        ));
    }

    let mut tx = state.pool().begin().await.map_err(into_internal)?;
    let created = create_user(
        &mut tx,
        CreateUser {
            id: None,
            name: valid.name.clone(),
            login: Some(valid.login),
            // Pending, not stored: an unconfirmed signup must not reserve an
            // address it can't prove it owns, or anyone could take a stranger's
            // email out of circulation from the registration form.
            email: None,
            pending_email: Some(valid.email.clone()),
            password: Some(CreateUserPassword::Plaintext(valid.password)),
            permissions: Permissions::default().bits(),
            source: UserSource::Internal,
            last_login_at: None,
            created_at: None,
        },
    )
    .await
    .map_err(AppError::Internal)?;

    // Sent inside the transaction so a relay that rejects the message rolls the
    // account back rather than stranding one that can neither log in nor
    // re-register. Costs holding the connection for the SMTP timeout.
    send_confirmation_email(
        &stored,
        state.api_public_url(),
        state.jwt_encoding_key(),
        ConfirmRecipient {
            user_id: created.id,
            name: &valid.name,
            email: &valid.email,
        },
    )
    .await?;

    tx.commit().await.map_err(into_internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    name: String,
    login: String,
    email: String,
    password: String,
}

struct ValidRegistration {
    name: String,
    login: String,
    email: String,
    password: String,
}

fn validate(input: RegisterRequest) -> Result<ValidRegistration, AppError> {
    let mut errors = FieldErrors::new();
    let name = validate_name(&input.name, &mut errors);
    let login = validate_login(&input.login, &mut errors);
    if let Some(message) = weak_password(&input.password) {
        errors.add("password", message);
    }
    // `validate_email` reads blank as "field absent" and stays quiet, which is
    // right for a PATCH and wrong here — there's no stored address to fall back
    // on, so requiring one is this caller's job.
    if input.email.trim().is_empty() {
        errors.add("email", "Cannot be empty");
    }
    let email = validate_email(Some(input.email), &mut errors);
    errors.into_result()?;

    Ok(ValidRegistration {
        name,
        login,
        email: email.ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!("email passed validation but produced none"))
        })?,
        password: input.password,
    })
}
