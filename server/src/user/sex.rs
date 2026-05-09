//! Self-described gender on `user_profiles.sex`. Mirrors the
//! `user_sex` Postgres enum from `0003_user_profiles.sql`.
//!
//! `Diverse` covers everyone who doesn't pick `Male` / `Female` —
//! non-binary, prefer-not-to-say-but-still-want-the-form-saved,
//! etc. The Leonardo importer only ever produces `Male` / `Female`
//! / NULL because the source column is `varchar(6)` storing
//! `M`/`F`/empty.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_sex", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserSex {
    Male,
    Female,
    Diverse,
}

impl UserSex {
    /// String accepted by the `user_sex` Postgres enum. Same
    /// shape as [`UserSource::pg_enum_value`]: keeps the binding
    /// site from having to know about `sqlx::Type`'s rename
    /// rules.
    ///
    /// [`UserSource::pg_enum_value`]: super::UserSource::pg_enum_value
    pub fn pg_enum_value(self) -> &'static str {
        match self {
            UserSex::Male => "male",
            UserSex::Female => "female",
            UserSex::Diverse => "diverse",
        }
    }

    /// Convert Leonardo's `Sex varchar(6)` value (`M`/`F`/empty/
    /// other) to our enum. Trims whitespace, case-insensitive.
    /// Anything we don't recognise — including the empty string
    /// and Leonardo's all-pad rows — returns `None`, leaving
    /// the destination column NULL.
    pub fn from_leonardo(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "m" | "male" => Some(UserSex::Male),
            "f" | "female" => Some(UserSex::Female),
            // We don't see this in the source today, but if a
            // newer Leonardo extension uses 'D' / 'X' for diverse,
            // pass it through cleanly.
            "d" | "x" | "diverse" => Some(UserSex::Diverse),
            _ => None,
        }
    }
}
