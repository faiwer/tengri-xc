//! Where a `users` row originated. Mirrors the `user_source` Postgres
//! enum from migration `0002_users_auth.sql`; the variants must stay
//! in lockstep with that type's `ENUM(...)` list.
//!
//! New variants get added to both sides at the same time. Renames
//! are awkward (Postgres `ALTER TYPE ... RENAME VALUE` exists since
//! 12 but is fiddly); avoid by picking durable names up front.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_source", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserSource {
    /// Created by our own signup / CLI flow. The expected default.
    Internal,
    /// Imported from a Leonardo XC instance via `leonardo migrate`.
    Leo,
}

impl UserSource {
    /// String accepted by the `user_source` Postgres enum. Same
    /// shape as `InputFormat::pg_enum_value`: keeps the binding site
    /// from having to know about `sqlx::Type`'s rename rules.
    pub fn pg_enum_value(self) -> &'static str {
        match self {
            UserSource::Internal => "internal",
            UserSource::Leo => "leo",
        }
    }
}
