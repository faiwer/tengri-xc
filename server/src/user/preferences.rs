//! Per-user display preferences. Wire shape mirrors the
//! `user_preferences` columns 1:1; the literal `'system'` is a
//! sentinel meaning "follow the user's locale", resolved client-side.
//!
//! Read-only here for now: the settings UI + `PATCH` endpoint land
//! in a follow-up. The trigger in `0004_user_preferences.sql`
//! guarantees one row per user, so the fetch never `LEFT JOIN`s.

use serde::Serialize;

use crate::AppError;

/// Wire shape for `/users/me`'s `preferences` block.
#[derive(Debug, Serialize, Clone, Copy)]
pub struct PreferencesDto {
    /// One of `'system' | 'h12' | 'h24'`.
    pub time_format: &'static str,
    /// One of `'system' | 'dmy' | 'mdy'`.
    pub date_format: &'static str,
    /// One of `'system' | 'metric' | 'imperial'`. Drives both
    /// altitude (m vs ft) and XC distance (km vs mi).
    pub units: &'static str,
    /// One of `'system' | 'mps' | 'fpm'`. Independent of `units`
    /// because instrument-driven preferences exist (a metric pilot
    /// flying with an imported imperial vario, etc.).
    pub vario_unit: &'static str,
    /// One of `'system' | 'kmh' | 'mph'`.
    pub speed_unit: &'static str,
    /// One of `'system' | 'mon' | 'sun'`.
    pub week_start: &'static str,
}

/// Fetch the preferences row for `user_id`. Trigger guarantees the
/// row exists for any extant user; the `Option` is here purely so a
/// caller passing a stale id (just-deleted user) still gets `None`
/// instead of an internal error.
pub async fn fetch_preferences(
    pool: &sqlx::PgPool,
    user_id: i32,
) -> Result<Option<PreferencesDto>, AppError> {
    let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT time_format, date_format, units, vario_unit, speed_unit, week_start \
         FROM user_preferences \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::Error::new(e)))?;

    Ok(row.map(
        |(time_format, date_format, units, vario_unit, speed_unit, week_start)| PreferencesDto {
            time_format: intern(&time_format, &["system", "h12", "h24"]),
            date_format: intern(&date_format, &["system", "dmy", "mdy"]),
            units: intern(&units, &["system", "metric", "imperial"]),
            vario_unit: intern(&vario_unit, &["system", "mps", "fpm"]),
            speed_unit: intern(&speed_unit, &["system", "kmh", "mph"]),
            week_start: intern(&week_start, &["system", "mon", "sun"]),
        },
    ))
}

/// Map a DB string back to the matching `&'static str` from the
/// allow-list. Falls back to `'system'` if the DB ever holds a
/// value outside the CHECK set (shouldn't happen — the constraint
/// rejects bad inserts — but a safe default beats a panic).
fn intern(value: &str, allowed: &[&'static str]) -> &'static str {
    allowed
        .iter()
        .copied()
        .find(|candidate| *candidate == value)
        .unwrap_or("system")
}
