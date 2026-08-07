//! OAuth provider configuration. Currently just the admin-editable per-provider
//! credentials (`oauth_provider_settings`, `0025`), surfaced through
//! `/admin/oauth-providers`. The login/link redirect flow that consumes these
//! credentials is a later phase.

mod dto;
mod provider;
mod store;

pub use dto::AdminOAuthProviderDto;
pub use provider::OAuthProvider;
pub use store::{
    UpdateOAuthProviderRequest, apply_oauth_provider_update, fetch_oauth_providers_admin,
    validate_oauth_provider_update,
};
