//! OAuth: admin-editable per-provider credentials (`oauth_provider_settings`,
//! `0025`) surfaced through `/admin/oauth-providers`, plus the login/link
//! redirect flow (`/oauth/...`) that consumes them. `flow` owns the authorize →
//! callback dance (PKCE, the signed flow cookie, userinfo); `links` owns the
//! `user_oauth_links` reads/writes.

mod dto;
mod flow;
mod links;
mod provider;
mod store;

pub use dto::AdminOAuthProviderDto;
pub use flow::{
    FLOW_COOKIE_NAME, FlowState, OAuthIntent, build_authorize_redirect, clear_flow_cookie,
    decode_flow_cookie, exchange_and_identify, resolve_return_to, set_flow_cookie,
};
pub use links::{
    LinkOutcome, LinkSnapshot, UnlinkOutcome, delete_link, find_user_by_link, list_links_for_user,
    register_oauth_user, upsert_link,
};
pub use provider::{OAuthIdentity, OAuthProvider};
pub use store::{
    ProviderCredentials, UpdateOAuthProviderRequest, apply_oauth_provider_update,
    fetch_enabled_provider_credentials, fetch_enabled_providers, fetch_oauth_providers_admin,
    validate_oauth_provider_update,
};
