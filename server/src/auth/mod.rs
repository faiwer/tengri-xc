//! Session auth: JWT in an HTTP-only cookie, no refresh token, no
//! server-side session table.
//!
//! - `routes::users::login`/`logout` set/clear the cookie inline.
//! - All other routes run behind [`middleware::session_layer`],
//!   which decodes the cookie into request extensions and renews
//!   it from the DB once it's older than [`cookie::SLIDE_INTERVAL`].
//! - Handlers read the resolved user via [`Identity`] (required,
//!   401 on miss) or `Option<Identity>` (conditional auth).

pub mod cookie;
pub mod extractor;
pub mod middleware;
pub mod password;
pub mod token;

pub use extractor::Identity;
pub use middleware::session_layer;
pub use token::{Claims, JWT_LIFETIME};
