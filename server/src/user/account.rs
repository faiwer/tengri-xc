//! Owner-editable identity columns of the `users` table (`name` / `login` /
//! `email`): validation, uniqueness, and the self-service apply path. Shared by
//! the admin user editor (`/admin/users`) and the owner-self profile form
//! (`PATCH /users/me`) so the name/email rules live in exactly one place.

use sqlx::{Postgres, Transaction};

use crate::{
    AppError,
    db::Update,
    validation::{FieldErrors, looks_like_email},
};

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

/// Trim `raw`; record a `login` error when it's the wrong length or holds a
/// character outside [`is_login_char`]. Returns the trimmed value regardless.
///
/// Excluding `@` from the charset is the load-bearing part. `POST /users/login`
/// resolves its identifier with `LOWER(login) = LOWER($1) OR email = LOWER($1)`,
/// so a login shaped like somebody else's address makes the row that query
/// returns ambiguous — enough to stop the address's real owner signing in by
/// email.
pub fn validate_login(raw: &str, errors: &mut FieldErrors) -> String {
    let trimmed = raw.trim();
    let length = trimmed.chars().count();
    if trimmed.is_empty() {
        errors.add("login", "Cannot be empty");
    } else if !(LOGIN_MIN_LEN..=LOGIN_MAX_LEN).contains(&length) {
        errors.add(
            "login",
            format!("Use {LOGIN_MIN_LEN} to {LOGIN_MAX_LEN} characters"),
        );
    } else if !trimmed.chars().all(is_login_char) {
        errors.add(
            "login",
            "Use only letters, digits, periods, hyphens, or underscores",
        );
    }
    trimmed.to_owned()
}

/// Characters allowed in a login. ASCII only: the column is folded with
/// `LOWER()` for uniqueness, and how to case-fold other scripts isn't a call
/// this needs to make.
fn is_login_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')
}

const LOGIN_MIN_LEN: usize = 3;
const LOGIN_MAX_LEN: usize = 32;

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

/// `None` when `password` meets the policy (>= 8 chars, at least one letter and
/// one digit), otherwise the message to surface on the field. Mirrored
/// client-side in the Authorization and Register forms.
pub fn weak_password(password: &str) -> Option<&'static str> {
    if password.chars().count() < 8 {
        return Some("At least 8 characters");
    }
    let has_letter = password.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_letter || !has_digit {
        return Some("Must include a letter and a digit");
    }
    None
}

/// Trim, then collapse an empty / all-whitespace string to `None`.
pub fn blank_to_none(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// The user whose stored email matches `email`, or `None`. Addresses are stored
/// lowercased, so `LOWER($1)` case-folds the incoming value.
///
/// OAuth sign-in uses this to attach a provider identity to an existing
/// account, which makes `users.email` a join key and its trustworthiness a
/// security property: an address nobody proved would let someone pre-register
/// a copy of a victim's email and collect the victim's real OAuth sign-in.
/// Nothing writes an unproven address there — [`plan_email_edit`] routes
/// self-service changes through `pending_email`, and the remaining writers
/// (admin, OAuth, import) set an address they vouch for.
pub async fn find_user_id_by_email(
    pool: &sqlx::PgPool,
    email: &str,
) -> Result<Option<i32>, AppError> {
    sqlx::query_scalar::<_, i32>("SELECT id FROM users WHERE email = LOWER($1) LIMIT 1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(into_internal)
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

/// What to write to `pending_email` when a user edits the address in their own
/// profile form: the new address, or `None` when there's nothing to do — the
/// request carried no address, or it's the one they already have.
///
/// It never returns an address to write to `email`. That column only holds
/// addresses someone proved by clicking a confirmation link, and a typo in the
/// profile form must not overwrite it: login refuses an unproven address, and
/// the mail that would prove it goes to the typo, so the user is locked out
/// with no way to fix the field. [`find_user_id_by_email`] matches OAuth
/// sign-ins against `email` on the same assumption.
///
/// Admins who need to set an address nobody proved use `/admin/users`, which
/// has an explicit "verified" flag.
pub async fn plan_email_edit(
    pool: &sqlx::PgPool,
    user_id: i32,
    new_email: Option<&str>,
) -> Result<Option<String>, AppError> {
    let Some(new_email) = new_email else {
        return Ok(None);
    };
    let current: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(into_internal)?;
    if current.as_deref() == Some(new_email) {
        return Ok(None);
    }
    Ok(Some(new_email.to_owned()))
}

/// Validated projection of the owner-editable `users` identity columns. There
/// is no `email` field on purpose: a self-service change only ever reaches
/// `pending_email`, see [`plan_email_edit`].
#[derive(Debug)]
pub struct AccountUpdate {
    /// `None` leaves the column alone.
    pub name: Option<String>,
    /// `None` leaves the column alone. Neither column is ever *cleared* here —
    /// emptying `pending_email` is part of the promote in
    /// `GET /users/confirm-email`, which is the only thing that finishes an
    /// address change.
    pub pending_email: Option<String>,
}

impl AccountUpdate {
    fn is_noop(&self) -> bool {
        self.name.is_none() && self.pending_email.is_none()
    }
}

/// Apply the owner-editable `users` columns inside a transaction. Only the
/// `Some` fields are written. No-op (and no SQL) when nothing is set.
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
    if let Some(pending_email) = update.pending_email.clone() {
        q.set("pending_email", pending_email);
    }
    q.and_where("id = $", (user_id,));
    q.execute_tx(tx)
        .await
        .map_err(|e| AppError::Internal(anyhow::Error::new(e)))?;
    Ok(())
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

    fn login_error(raw: &str) -> Option<String> {
        let mut errors = FieldErrors::new();
        validate_login(raw, &mut errors);
        errors.fields.get("login").cloned()
    }

    #[test]
    fn accepts_handle_shaped_logins() {
        for login in ["alice", "Agent007", "jean.luc", "o-brien", "snake_case"] {
            assert!(
                login_error(login).is_none(),
                "expected {login:?} to be valid"
            );
        }
    }

    #[test]
    fn rejects_logins_that_could_be_mistaken_for_an_email() {
        assert!(login_error("victim@example.com").is_some());
        assert!(login_error("a@b").is_some());
    }

    #[test]
    fn rejects_blank_wrong_length_and_stray_symbols() {
        assert_eq!(login_error("   ").as_deref(), Some("Cannot be empty"));
        assert!(login_error("ab").is_some());
        assert!(login_error(&"a".repeat(33)).is_some());
        assert!(login_error("has space").is_some());
        assert!(login_error("renée").is_some());
    }
}
