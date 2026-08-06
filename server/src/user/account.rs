//! Owner-editable identity columns of the `users` table (`name` / `login` /
//! `email`): validation, uniqueness, and the self-service apply path. Shared by
//! the admin user editor (`/admin/users`) and the owner-self profile form
//! (`PATCH /users/me`) so the name/email rules live in exactly one place.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};

use crate::{AppError, db::Update, user::Permissions, validation::FieldErrors};

/// Trim `raw`; record a `name` error when it's blank or contains a character
/// outside [`is_name_char`]. Returns the trimmed value regardless (used
/// verbatim on the happy path). Callers namespace the error key via
/// [`FieldErrors::merge_prefixed`] as needed.
pub fn validate_name(raw: &str, errors: &mut FieldErrors) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        errors.add("name", "Cannot be empty");
    } else if !trimmed.chars().all(is_name_char) {
        errors.add(
            "name",
            "Use only letters, spaces, hyphens, apostrophes, periods, or underscores",
        );
    }
    trimmed.to_owned()
}

/// Characters allowed in a display name: any Unicode letter (so non-Latin and
/// accented names work) plus the punctuation that shows up in real names —
/// space, hyphen, apostrophe (straight and typographic), period — and
/// underscore for handle-style names. Digits and other symbols are rejected.
fn is_name_char(c: char) -> bool {
    c.is_alphabetic() || matches!(c, ' ' | '-' | '_' | '.' | '\'' | '\u{2019}')
}

/// Normalise an optional email: blank/absent → `None`, otherwise trim +
/// lowercase and reject anything that doesn't [`looks_like_email`]
/// (recording an `email` error and returning `None`).
pub fn validate_email(raw: Option<String>, errors: &mut FieldErrors) -> Option<String> {
    match blank_to_none(raw) {
        None => None,
        Some(raw) => {
            let lowered = raw.to_ascii_lowercase();
            if looks_like_email(&lowered) {
                Some(lowered)
            } else {
                errors.add("email", "Enter a valid email address");
                None
            }
        }
    }
}

/// Trim, then collapse an empty / all-whitespace string to `None`.
pub fn blank_to_none(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Add a `name` / `login` / `email` field error for any value already taken by
/// another row. `exclude` is the row being edited (skipped on PATCH); `None` on
/// create. Name and login fold case (`users_name_key` / `users_login_key` are
/// on `LOWER(...)`); email is stored lowercased so a plain `=` matches its
/// index. Predicates are static literals — no user input reaches the SQL text.
pub async fn check_unique(
    pool: &sqlx::PgPool,
    name: Option<&str>,
    login: Option<&str>,
    email: Option<&str>,
    exclude: Option<i32>,
    errors: &mut FieldErrors,
) -> Result<(), AppError> {
    let exclude = exclude.unwrap_or(0);

    let mut checks: Vec<(&str, &str, &str)> = Vec::new();
    if let Some(name) = name {
        checks.push(("name", "LOWER(name) = LOWER($1)", name));
    }
    if let Some(login) = login {
        checks.push(("login", "LOWER(login) = LOWER($1)", login));
    }
    if let Some(email) = email {
        checks.push(("email", "email = $1", email));
    }

    for (field, predicate, value) in checks {
        let taken: Option<i32> = sqlx::query_scalar(&format!(
            "SELECT id FROM users WHERE {predicate} AND id <> $2"
        ))
        .bind(value)
        .bind(exclude)
        .fetch_optional(pool)
        .await
        .map_err(into_internal)?;
        if taken.is_some() {
            errors.add(field, "Already taken");
        }
    }
    Ok(())
}

/// Whether a self-service email edit must drop the stored confirmation. True
/// only when the editor lacks [`MANAGE_USERS`](Permissions::MANAGE_USERS) *and*
/// `new_email` differs from the address on file — re-saving the same address,
/// or an admin editing, leaves `email_verified_at` alone. The one extra
/// `SELECT` is what lets an unchanged re-save keep a verified badge.
pub async fn should_clear_email_verification(
    pool: &sqlx::PgPool,
    user_id: i32,
    permissions: Permissions,
    new_email: Option<&str>,
) -> Result<bool, AppError> {
    let Some(new_email) = new_email else {
        return Ok(false);
    };
    if permissions.contains(Permissions::MANAGE_USERS) {
        return Ok(false);
    }
    let current: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(into_internal)?;
    Ok(current.as_deref() != Some(new_email))
}

/// Validated projection of the owner-editable `users` identity columns. `None`
/// on a field means "leave the column alone" — self-service never clears `name`
/// (`NOT NULL`) nor `email` (clearing an address has no practical meaning),
/// which is why both are a plain `Option`.
#[derive(Debug, Default)]
pub struct AccountUpdate {
    pub name: Option<String>,
    pub email: Option<String>,
    /// When set alongside a written `email`, also null `email_verified_at` so
    /// the new address starts unverified. The self-service path sets it for
    /// non-admin editors who changed their address; admins manage verification
    /// explicitly, so they leave it `false`.
    pub clear_email_verification: bool,
}

impl AccountUpdate {
    pub fn is_noop(&self) -> bool {
        self.name.is_none() && self.email.is_none()
    }
}

/// Apply the owner-editable `users` columns inside a transaction. Only the
/// `Some` fields are written, so a blank email leaves the stored address
/// untouched. No-op (and no SQL) when nothing is set.
pub async fn apply_account_update(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i32,
    update: &AccountUpdate,
) -> Result<(), AppError> {
    if update.is_noop() {
        return Ok(());
    }

    let mut q = Update::new("users");
    if let Some(name) = update.name.clone() {
        q.set("name", name);
    }
    if let Some(email) = update.email.clone() {
        q.set("email", email);
        if update.clear_email_verification {
            q.set("email_verified_at", None::<DateTime<Utc>>);
        }
    }
    q.and_where("id = $", (user_id,));
    q.execute_tx(tx)
        .await
        .map_err(|e| AppError::Internal(anyhow::Error::new(e)))?;
    Ok(())
}

/// Deliberately loose: exactly one `@`, non-empty local + domain, no
/// whitespace. Real validity is confirmed by a verification mail, not a regex —
/// this only catches obvious typos.
fn looks_like_email(value: &str) -> bool {
    let mut parts = value.split('@');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => {
            !local.is_empty() && !domain.is_empty() && !value.contains(char::is_whitespace)
        }
        _ => false,
    }
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_error(raw: &str) -> Option<String> {
        let mut errors = FieldErrors::new();
        validate_name(raw, &mut errors);
        errors.fields.get("name").cloned()
    }

    #[test]
    fn accepts_ordinary_and_international_names() {
        for name in [
            "Alice",
            "O'Brien-Smith",
            "Jean-Luc",
            "Renée",
            "Æsir",
            "李雷",
            "J. Doe",
            "snake_case",
        ] {
            assert!(name_error(name).is_none(), "expected {name:?} to be valid");
        }
    }

    #[test]
    fn rejects_blank_and_disallowed_characters() {
        assert_eq!(name_error("   ").as_deref(), Some("Cannot be empty"));
        // Digits and stray symbols are out.
        assert!(name_error("Agent007").is_some());
        assert!(name_error("a@b").is_some());
        assert!(name_error("na!me").is_some());
    }
}
