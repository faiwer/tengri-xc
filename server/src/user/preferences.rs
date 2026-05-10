//! Per-user display preferences. Wire shape mirrors the
//! `user_preferences` columns 1:1; the literal `'system'` is a
//! sentinel meaning "follow the user's locale", resolved client-side.
//!
//! Read-only here for now: the settings UI + `PATCH` endpoint land
//! in a follow-up. The trigger in `0004_user_preferences.sql`
//! guarantees one row per user, so the fetch never `LEFT JOIN`s.

use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

use crate::{AppError, db::Update, validation::FieldErrors};

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

// ---------------------------------------------------------------------------
// Update support
// ---------------------------------------------------------------------------
//
// Owner-only — preferences are private, so this lives off the self-edit
// endpoint and is *not* exposed through admin routes. The shape uses
// `Option<EnumType>` (single Option) instead of the double-Option used by
// `profile`: every preference column is `NOT NULL DEFAULT 'system'` in the
// DB, so there's no "clear to NULL" intent to express. Absent = unchanged,
// present = set.

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct UpdatePreferencesRequest {
    #[serde(default)]
    pub time_format: Option<TimeFormat>,
    #[serde(default)]
    pub date_format: Option<DateFormat>,
    #[serde(default)]
    pub units: Option<Units>,
    #[serde(default)]
    pub vario_unit: Option<VarioUnit>,
    #[serde(default)]
    pub speed_unit: Option<SpeedUnit>,
    #[serde(default)]
    pub week_start: Option<WeekStart>,
}

/// Mirrors the column's `CHECK` set. Rendered to the DB via
/// [`AsRef<str>`]; the same value is what flows back out on read.
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TimeFormat {
    System,
    H12,
    H24,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DateFormat {
    System,
    Dmy,
    Mdy,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    System,
    Metric,
    Imperial,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum VarioUnit {
    System,
    Mps,
    Fpm,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum SpeedUnit {
    System,
    Kmh,
    Mph,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum WeekStart {
    System,
    Mon,
    Sun,
}

trait PgEnumValue {
    fn pg_value(self) -> &'static str;
}

impl PgEnumValue for TimeFormat {
    fn pg_value(self) -> &'static str {
        match self {
            TimeFormat::System => "system",
            TimeFormat::H12 => "h12",
            TimeFormat::H24 => "h24",
        }
    }
}
impl PgEnumValue for DateFormat {
    fn pg_value(self) -> &'static str {
        match self {
            DateFormat::System => "system",
            DateFormat::Dmy => "dmy",
            DateFormat::Mdy => "mdy",
        }
    }
}
impl PgEnumValue for Units {
    fn pg_value(self) -> &'static str {
        match self {
            Units::System => "system",
            Units::Metric => "metric",
            Units::Imperial => "imperial",
        }
    }
}
impl PgEnumValue for VarioUnit {
    fn pg_value(self) -> &'static str {
        match self {
            VarioUnit::System => "system",
            VarioUnit::Mps => "mps",
            VarioUnit::Fpm => "fpm",
        }
    }
}
impl PgEnumValue for SpeedUnit {
    fn pg_value(self) -> &'static str {
        match self {
            SpeedUnit::System => "system",
            SpeedUnit::Kmh => "kmh",
            SpeedUnit::Mph => "mph",
        }
    }
}
impl PgEnumValue for WeekStart {
    fn pg_value(self) -> &'static str {
        match self {
            WeekStart::System => "system",
            WeekStart::Mon => "mon",
            WeekStart::Sun => "sun",
        }
    }
}

/// Validated projection of [`UpdatePreferencesRequest`]. Nothing
/// beyond serde's enum-shape check happens today — the shape is here
/// so `validate_preferences_update` can grow rules later (e.g. "h12
/// requires en-locale", whatever) without changing the call sites.
#[derive(Debug, Default)]
pub struct PreferencesUpdate {
    pub time_format: Option<TimeFormat>,
    pub date_format: Option<DateFormat>,
    pub units: Option<Units>,
    pub vario_unit: Option<VarioUnit>,
    pub speed_unit: Option<SpeedUnit>,
    pub week_start: Option<WeekStart>,
}

impl PreferencesUpdate {
    pub fn is_noop(&self) -> bool {
        self.time_format.is_none()
            && self.date_format.is_none()
            && self.units.is_none()
            && self.vario_unit.is_none()
            && self.speed_unit.is_none()
            && self.week_start.is_none()
    }
}

/// Validate the request body without touching the DB. Currently a
/// pure shape-pass-through (serde already enforced the enum set);
/// kept as a function so future cross-field rules have a home.
pub fn validate_preferences_update(
    input: UpdatePreferencesRequest,
) -> Result<PreferencesUpdate, FieldErrors> {
    Ok(PreferencesUpdate {
        time_format: input.time_format,
        date_format: input.date_format,
        units: input.units,
        vario_unit: input.vario_unit,
        speed_unit: input.speed_unit,
        week_start: input.week_start,
    })
}

/// Apply a validated update inside a transaction. The trigger from
/// `0004_user_preferences` guarantees a row exists, so this is a
/// straight UPDATE — no UPSERT needed.
pub async fn apply_preferences_update(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i32,
    update: &PreferencesUpdate,
) -> Result<(), AppError> {
    if update.is_noop() {
        return Ok(());
    }

    let mut q = Update::new("user_preferences");
    if let Some(v) = update.time_format {
        q.set("time_format", v.pg_value());
    }
    if let Some(v) = update.date_format {
        q.set("date_format", v.pg_value());
    }
    if let Some(v) = update.units {
        q.set("units", v.pg_value());
    }
    if let Some(v) = update.vario_unit {
        q.set("vario_unit", v.pg_value());
    }
    if let Some(v) = update.speed_unit {
        q.set("speed_unit", v.pg_value());
    }
    if let Some(v) = update.week_start {
        q.set("week_start", v.pg_value());
    }
    q.and_where("user_id = $", (user_id,));

    q.execute_tx(tx)
        .await
        .map_err(|e| AppError::Internal(anyhow::Error::new(e)))?;
    Ok(())
}
