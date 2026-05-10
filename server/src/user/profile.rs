//! Owner-editable surface of `user_profiles`. Designed to be reused
//! by `PATCH /users/me` (owner-self) and a future `PATCH /admin/users/:id`
//! (admin-edit) — neither should know about the other's envelope.
//!
//! Wire shape uses `Option<Option<T>>` for each nullable field so the
//! API can express three intents cleanly:
//! - field absent → no change
//! - field `null` → clear it (`SET col = NULL`)
//! - field present with a value → set it
//!
//! The `serde(default, deserialize_with = "deserialize_some")` recipe
//! turns a missing JSON key into `None` and a present-but-`null` JSON
//! value into `Some(None)`.

use serde::{Deserialize, Deserializer};
use sqlx::{Postgres, Transaction};

use crate::{AppError, db::Upsert, user::UserSex, validation::FieldErrors};

/// Wire shape for the profile section of an update request.
///
/// Empty struct (no fields present) is allowed and is a no-op — the
/// composing endpoint can decide whether to reject empty bodies.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateProfileRequest {
    #[serde(default, deserialize_with = "deserialize_some")]
    pub civl_id: Option<Option<i32>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub country: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub sex: Option<Option<UserSex>>,
}

/// Validated + normalised projection of [`UpdateProfileRequest`].
/// Country is uppercased, CIVL id is range-checked. Identical
/// `Option<Option<T>>` shape as the input — `None` means "leave the
/// column alone", `Some(None)` means "set NULL", `Some(Some(v))`
/// means "set v".
#[derive(Debug, Default)]
pub struct ProfileUpdate {
    pub civl_id: Option<Option<i32>>,
    pub country: Option<Option<String>>,
    pub sex: Option<Option<UserSex>>,
}

impl ProfileUpdate {
    pub fn is_noop(&self) -> bool {
        self.civl_id.is_none() && self.country.is_none() && self.sex.is_none()
    }
}

/// Validate the request body without touching the DB. Returns bare
/// field names (e.g. `country`); the caller adds any namespace prefix
/// via [`FieldErrors::merge_prefixed`].
pub fn validate_profile_update(input: UpdateProfileRequest) -> Result<ProfileUpdate, FieldErrors> {
    let mut errors = FieldErrors::new();

    let civl_id = match input.civl_id {
        None => None,
        Some(None) => Some(None),
        Some(Some(v)) if v <= 0 => {
            errors.add("civl_id", "Must be a positive number");
            None
        }
        Some(Some(v)) => Some(Some(v)),
    };

    let country = match input.country {
        None => None,
        Some(None) => Some(None),
        Some(Some(raw)) => match validate_country(&raw) {
            Ok(c) => Some(Some(c)),
            Err(msg) => {
                errors.add("country", msg);
                None
            }
        },
    };

    // `sex` is enum-typed at deserialise time, so by the time it reaches
    // here it's either a known variant or the request 400'd. No further
    // validation needed.
    let sex = input.sex;

    if errors.is_empty() {
        Ok(ProfileUpdate {
            civl_id,
            country,
            sex,
        })
    } else {
        Err(errors)
    }
}

/// Apply a validated update inside a transaction. Uses `INSERT … ON
/// CONFLICT DO UPDATE` so a user without a profile row gets one, and
/// a user with one gets only the requested columns touched.
///
/// No-op if `update.is_noop()` — caller should short-circuit before
/// calling, but defensively skip the SQL to keep `updated_at` stable.
pub async fn apply_profile_update(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i32,
    update: &ProfileUpdate,
) -> Result<(), AppError> {
    if update.is_noop() {
        return Ok(());
    }

    // For UPSERT we need *some* values for the INSERT row. Use NULL
    // for any column the caller didn't touch, then only update the
    // touched columns on conflict.
    let civl_id_insert: Option<i32> = update.civl_id.flatten();
    let country_insert: Option<&str> = update.country.as_ref().and_then(Option::as_deref);
    let sex_insert: Option<&str> = update.sex.flatten().map(UserSex::pg_enum_value);

    let mut q = Upsert::into("user_profiles");
    q.value("user_id", user_id);
    q.value("civl_id", civl_id_insert);
    q.value("country", country_insert);
    q.value_cast("sex", sex_insert, "user_sex");
    q.on_conflict("user_id");
    if update.civl_id.is_some() {
        q.update_excluded("civl_id");
    }
    if update.country.is_some() {
        q.update_excluded("country");
    }
    if update.sex.is_some() {
        q.update_excluded("sex");
    }

    q.execute_tx(tx)
        .await
        .map_err(|e| AppError::Internal(anyhow::Error::new(e)))?;
    Ok(())
}

fn validate_country(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Cannot be empty — use null to clear");
    }
    if trimmed.len() != 2 {
        return Err("Must be a 2-letter ISO country code");
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("Must be a 2-letter ISO country code");
    }
    Ok(trimmed.to_ascii_uppercase())
}

/// Standard serde recipe for distinguishing "field absent" from
/// "field present and explicitly null" in JSON. Used with
/// `#[serde(default, deserialize_with = "deserialize_some")]`.
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}
