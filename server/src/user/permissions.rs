//! Per-user capability bitfield, stored in `users.permissions` as a
//! single Postgres `int`. The migration's COMMENT mirrors the layout
//! below; if you change the bit assignments, change both.
//!
//! Why bits and not roles: roles imply a ladder (member ≤ moderator ≤
//! admin) and we don't want one. A user can be allowed to manage
//! tracks without managing users, and clearing bit 0 alone is the
//! soft-disable mechanism (no separate `disabled_at` needed).
//!
//! Why bits and not a list of named flags in the JWT: the wire form
//! collapses to one integer (`{ "p": 13 }`), so the token stays
//! compact regardless of how many capabilities we add. The auth
//! middleware reconstructs `Permissions` with a single
//! `Permissions::from_bits_retain(p)` and runs structured `.contains()`
//! checks from there. Adding bit 4 next year is a one-line schema
//! comment update; the JWT shape doesn't move.
//!
//! The `bitflags!` type-checks the operations and gives us
//! `contains` / `insert` / `remove` semantics, so app code never
//! does raw `& 4 == 4` arithmetic. The on-disk representation is
//! still an `int` — `Permissions::bits()` for write,
//! `Permissions::from_bits_retain` for read.

use bitflags::bitflags;

bitflags! {
    /// Capability flags carried on a user row.
    ///
    /// **Bit 0 (`CAN_AUTHORIZE`)** is the "can this account log in?"
    /// switch — clearing it is how we soft-disable a user without
    /// adding a `disabled_at` column. New users (CLI, future signup,
    /// imports of active accounts) get it set.
    ///
    /// **Bits 1-3** are manage-X powers, granted explicitly.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Permissions: i32 {
        /// The account is allowed to authenticate. Default for new
        /// rows. Clear this bit (and only this bit) to soft-ban an
        /// account while preserving its other capabilities for
        /// audit / restoration.
        const CAN_AUTHORIZE     = 1 << 0;
        /// Edit / delete any flight, not just the user's own.
        const MANAGE_TRACKS     = 1 << 1;
        /// Create / suspend / promote other users.
        const MANAGE_USERS      = 1 << 2;
        /// Change global project settings (rate limits, feature
        /// flags, anything the operator console exposes).
        const MANAGE_SETTINGS   = 1 << 3;
        /// View / edit the canonical brand + glider-model dictionary.
        /// Read-only for now; gates the `/admin/gliders` endpoint and
        /// the matching settings nav item.
        const MANAGE_GLIDERS    = 1 << 4;
    }
}

impl Default for Permissions {
    /// Default for a freshly-created user: can log in, no manage
    /// powers. Matches the SQL default on `users.permissions`.
    fn default() -> Self {
        Permissions::CAN_AUTHORIZE
    }
}
