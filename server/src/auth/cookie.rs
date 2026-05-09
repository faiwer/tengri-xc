//! Cookie shape for the session JWT — name, flags, lifetime — in
//! one file. The actual decision of *when* to set, slide, or clear
//! the cookie lives elsewhere:
//!
//! - login/logout handlers in `routes::users` set/clear inline.
//! - the slide-on-active-request middleware in [`super::middleware`]
//!   re-mints in flight on every authed request whose token has
//!   crossed [`SLIDE_INTERVAL`].
//!
//! Why we hand-roll instead of pulling in `tower-cookies` /
//! `axum-extra::extract::cookie::SignedCookieJar`: those crates are
//! great if you have many cookies, but we have exactly one, and
//! writing it via [`cookie::Cookie::build`] is ~10 lines vs. one
//! extra layer to configure.

use std::time::Duration;

use cookie::{Cookie, SameSite};

use super::token::JWT_LIFETIME;

/// On any request, refresh the user's token if it's older than this.
pub const SLIDE_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub const SESSION_COOKIE_NAME: &str = "tengri-jwt";

/// `Set-Cookie` value for storing `jwt`. `https=true` adds the
/// `Secure` flag.
pub fn set_session(jwt: &str, https: bool) -> String {
    Cookie::build((SESSION_COOKIE_NAME, jwt.to_owned()))
        .http_only(true)
        .secure(https)
        // Lax so cross-site links (email, Telegram) still send the
        // cookie. Strict would log you out whenever you arrive from
        // elsewhere.
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(cookie::time::Duration::seconds(
            JWT_LIFETIME.as_secs() as i64
        ))
        .build()
        .to_string()
}

/// `Set-Cookie` value that deletes the session cookie.
pub fn clear_session(https: bool) -> String {
    Cookie::build((SESSION_COOKIE_NAME, ""))
        .http_only(true)
        .secure(https)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(cookie::time::Duration::seconds(0))
        .build()
        .to_string()
}
