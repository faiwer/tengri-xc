//! Wire shape for `GET /admin/oauth-providers`.

use serde::Serialize;

use super::provider::{OAuthProvider, OAuthVisibility};

/// One configured provider, as returned to the admin editor. Only providers
/// that have a row appear; a row exists only when both credentials are set, so
/// `client_id` / `client_secret` are non-optional.
///
/// The secret is serialized: this is a `MANAGE_SETTINGS`-only endpoint and the
/// editor prefills the field from it. The cost is that the secret reaches the
/// browser — acceptable for an admin-only screen.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminOAuthProviderDto {
    pub provider: OAuthProvider,
    pub client_id: String,
    pub client_secret: String,
    pub visibility: OAuthVisibility,
}
