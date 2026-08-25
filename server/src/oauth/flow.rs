//! The authorize → callback redirect dance: build the provider authorize URL
//! (with PKCE + CSRF), stash the flow state in a signed short-lived cookie,
//! then on callback exchange the code and read the provider's userinfo.
//!
//! Flow state lives in a cookie rather than a DB table to match the stateless
//! JWT-session design: it must survive the bounce to the provider and back, is
//! single-use, and is meaningless after ~20 minutes.

use std::time::Duration;

use chrono::Utc;
use cookie::{Cookie, SameSite};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl, basic::BasicClient,
};
use serde::{Deserialize, Serialize};

use crate::AppError;

use super::{
    provider::{OAuthIdentity, OAuthProvider},
    store::ProviderCredentials,
};

pub const FLOW_COOKIE_NAME: &str = "tengri-oauth";

/// How long a started flow stays valid. Long enough for a human to complete the
/// provider consent screen, short enough that a leaked cookie is near-useless.
const FLOW_TTL: Duration = Duration::from_secs(20 * 60);

/// Why the flow was started. Drives what the callback does with the resolved
/// identity: attach it to the current user, or look up which user it logs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthIntent {
    /// Anonymous "Sign in with …": the identity must already map to a user.
    Login,
    /// Logged-in "connect this account": attach the identity to `user_id`.
    Link,
}

/// Everything the callback needs, carried across the provider bounce in a
/// signed cookie. `exp` is a real JWT claim so `jsonwebtoken` enforces the TTL.
#[derive(Debug, Serialize, Deserialize)]
pub struct FlowState {
    pub provider: OAuthProvider,
    pub intent: OAuthIntent,
    /// CSRF token echoed back as `?state=`; must match on callback.
    pub state: String,
    /// PKCE verifier, replayed at token-exchange time.
    pub pkce_verifier: String,
    /// SPA-relative path to return the browser to (validated on the way out).
    pub return_to: String,
    /// The user who started a `Link` flow. `None` for `Login`.
    pub user_id: Option<i32>,
    pub exp: i64,
}

/// Build the provider authorize URL and the matching [`FlowState`]. The caller
/// sets the flow cookie from the returned state and 303s the browser to the URL.
pub fn build_authorize_redirect(
    provider: OAuthProvider,
    creds: &ProviderCredentials,
    redirect_uri: &str,
    intent: OAuthIntent,
    return_to: String,
    user_id: Option<i32>,
) -> Result<(String, FlowState), AppError> {
    let endpoints = provider.endpoints();
    let client = BasicClient::new(ClientId::new(creds.client_id.clone()))
        .set_client_secret(ClientSecret::new(creds.client_secret.clone()))
        .set_auth_uri(AuthUrl::new(endpoints.auth_url.to_owned()).map_err(into_internal)?)
        .set_token_uri(TokenUrl::new(endpoints.token_url.to_owned()).map_err(into_internal)?)
        .set_redirect_uri(RedirectUrl::new(redirect_uri.to_owned()).map_err(into_internal)?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(endpoints.scopes.iter().map(|s| Scope::new((*s).to_owned())))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let flow = FlowState {
        provider,
        intent,
        state: csrf.secret().clone(),
        pkce_verifier: pkce_verifier.secret().clone(),
        return_to,
        user_id,
        exp: Utc::now().timestamp() + FLOW_TTL.as_secs() as i64,
    };
    Ok((authorize_url.to_string(), flow))
}

/// Exchange the authorization `code` for an access token, then read the
/// provider's userinfo into a normalized [`OAuthIdentity`]. `Err` covers every
/// remote failure (bad code, network, malformed userinfo); the caller maps it
/// to a single "failed" outcome.
pub async fn exchange_and_identify(
    provider: OAuthProvider,
    creds: &ProviderCredentials,
    redirect_uri: &str,
    code: String,
    pkce_verifier: String,
) -> Result<OAuthIdentity, AppError> {
    let endpoints = provider.endpoints();
    let client = BasicClient::new(ClientId::new(creds.client_id.clone()))
        .set_client_secret(ClientSecret::new(creds.client_secret.clone()))
        .set_auth_uri(AuthUrl::new(endpoints.auth_url.to_owned()).map_err(into_internal)?)
        .set_token_uri(TokenUrl::new(endpoints.token_url.to_owned()).map_err(into_internal)?)
        .set_redirect_uri(RedirectUrl::new(redirect_uri.to_owned()).map_err(into_internal)?);

    // Following redirects on the token endpoint would open us to SSRF.
    let http = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(into_internal)?;

    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(&http)
        .await
        .map_err(|e| AppError::BadRequest(format!("oauth token exchange failed: {e}")))?;

    let body: serde_json::Value = http
        .get(endpoints.userinfo_url)
        .bearer_auth(token.access_token().secret())
        .header(reqwest::header::ACCEPT, "application/json")
        // GitHub rejects requests without a User-Agent; harmless elsewhere.
        .header(reqwest::header::USER_AGENT, "tengri-xc")
        .send()
        .await
        .map_err(into_internal)?
        .error_for_status()
        .map_err(into_internal)?
        .json()
        .await
        .map_err(into_internal)?;

    let (subject, display_name) = provider
        .parse_userinfo(&body)
        .ok_or_else(|| AppError::BadRequest("oauth userinfo missing subject id".into()))?;
    let email = provider
        .resolve_email(&http, token.access_token().secret(), &body)
        .await?;

    Ok(OAuthIdentity {
        subject,
        email,
        display_name,
    })
}

/// `Set-Cookie` carrying the signed flow state. `SameSite=Lax` so the top-level
/// GET navigation back from the provider still sends it.
pub fn set_flow_cookie(
    flow: &FlowState,
    encoding_key: &EncodingKey,
    https: bool,
) -> Result<String, AppError> {
    let jwt = encode(&Header::new(Algorithm::HS256), flow, encoding_key).map_err(into_internal)?;
    Ok(Cookie::build((FLOW_COOKIE_NAME, jwt))
        .http_only(true)
        .secure(https)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(cookie::time::Duration::seconds(FLOW_TTL.as_secs() as i64))
        .build()
        .to_string())
}

/// Decode + verify (signature + `exp`) a flow cookie value. `None` on any
/// tamper/expiry — the callback treats that as a bad-state error.
pub fn decode_flow_cookie(jwt: &str, decoding_key: &DecodingKey) -> Option<FlowState> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;
    let TokenData { claims, .. } = decode::<FlowState>(jwt, decoding_key, &validation).ok()?;
    Some(claims)
}

/// `Set-Cookie` that deletes the flow cookie — the flow is single-use.
pub fn clear_flow_cookie(https: bool) -> String {
    Cookie::build((FLOW_COOKIE_NAME, ""))
        .http_only(true)
        .secure(https)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(cookie::time::Duration::seconds(0))
        .build()
        .to_string()
}

/// Sanitize a caller-supplied return path to a same-origin SPA path. Rejects
/// absolute/protocol-relative URLs (open-redirect guard), falling back to `/`.
pub fn resolve_return_to(return_to: &str) -> String {
    let trimmed = return_to.trim();
    let safe = trimmed.starts_with('/')
        && !trimmed.starts_with("//")
        && !trimmed.contains('\\')
        && !trimmed.contains(char::is_control);
    if safe {
        trimmed.to_owned()
    } else {
        "/".to_owned()
    }
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
