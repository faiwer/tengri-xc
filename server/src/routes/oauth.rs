//! `/oauth/...` — the login/link redirect flow.
//!
//! - `GET /oauth/providers` — enabled providers, for the login picker + link
//!   settings. Anonymous-safe.
//! - `GET /oauth/links` — the caller's connected accounts. Requires a session.
//! - `GET /oauth/:provider/start` — begin a flow: set the signed flow cookie,
//!   303 to the provider. `?intent=link` needs a session; `?intent=login`
//!   (default) is anonymous. `?return_to=/path` is where the callback lands the
//!   browser.
//! - `GET /oauth/:provider/callback` — the provider bounce-back: verify the
//!   flow cookie + CSRF, exchange the code, then link (attach to the flow's
//!   user) or login (find the user the identity maps to, mint a session). Ends
//!   in a 303 back to `${APP_BASE_URL}${return_to}` with an `?oauth=` /
//!   `?oauth_error=` status the SPA toasts.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{COOKIE, LOCATION, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use cookie::Cookie;
use jsonwebtoken::DecodingKey;
use serde::Deserialize;

use crate::{
    AppError, AppState,
    auth::{Identity, token::mint_session_cookie},
    oauth::{
        FLOW_COOKIE_NAME, FlowState, LinkOutcome, LinkSnapshot, OAuthIdentity, OAuthIntent,
        OAuthProvider, OAuthVisibility, UnlinkOutcome, build_authorize_redirect, clear_flow_cookie,
        decode_flow_cookie, delete_link, exchange_and_identify, fetch_provider_credentials,
        fetch_visible_providers, find_user_by_link, list_links_for_user, register_oauth_user,
        resolve_return_to, set_flow_cookie, upsert_link,
    },
    site::fetch_site_public,
    user::{Permissions, find_user_id_by_email},
};

/// The callback sets the session cookie inline, so it lives outside the slide
/// middleware — same reasoning as `/users/login`.
pub fn public_router() -> Router<AppState> {
    Router::new().route("/oauth/{provider}/callback", get(callback))
}

/// Session-aware reads + `start`. `providers` is anonymous-safe; `links`
/// requires a session; `start` reads an optional session (required only for
/// `intent=link`).
pub fn session_router() -> Router<AppState> {
    Router::new()
        .route("/oauth/providers", get(list_providers))
        .route("/oauth/links", get(list_links))
        .route("/oauth/links/{provider}/{provider_user_id}", delete(unlink))
        .route("/oauth/{provider}/start", get(start))
}

async fn list_providers(
    State(state): State<AppState>,
    identity: Option<Identity>,
) -> Result<Json<Vec<OAuthProvider>>, AppError> {
    fetch_visible_providers(state.pool(), is_admin(identity.as_ref()))
        .await
        .map(Json)
}

/// Whether the caller may see `admins`-visibility providers: a session holding
/// `MANAGE_USERS`. Anonymous callers (and non-admins) get only `public` ones.
fn is_admin(identity: Option<&Identity>) -> bool {
    identity.is_some_and(|i| i.permissions.contains(Permissions::MANAGE_USERS))
}

async fn list_links(
    State(state): State<AppState>,
    identity: Identity,
) -> Result<Json<Vec<LinkSnapshot>>, AppError> {
    list_links_for_user(state.pool(), identity.user_id)
        .await
        .map(Json)
}

/// `DELETE /oauth/links/{provider}/{provider_user_id}` — unlink one of the
/// caller's connected accounts. Idempotent (204 whether or not the row was
/// there); refuses with 409 if it would leave a password-less account unable
/// to sign in.
async fn unlink(
    State(state): State<AppState>,
    identity: Identity,
    Path((provider, provider_user_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let provider = OAuthProvider::from_path(&provider).ok_or(AppError::NotFound)?;
    match delete_link(state.pool(), identity.user_id, provider, &provider_user_id).await? {
        UnlinkOutcome::Deleted | UnlinkOutcome::NotFound => Ok(StatusCode::NO_CONTENT),
        UnlinkOutcome::WouldBrick => Err(AppError::Conflict(
            "Set a password before unlinking your only sign-in method.".into(),
        )),
    }
}

#[derive(Debug, Deserialize)]
struct StartQuery {
    #[serde(default)]
    intent: Option<String>,
    #[serde(default)]
    return_to: Option<String>,
}

async fn start(
    State(state): State<AppState>,
    identity: Option<Identity>,
    Path(provider): Path<String>,
    Query(query): Query<StartQuery>,
) -> Result<Response, AppError> {
    let provider = OAuthProvider::from_path(&provider).ok_or(AppError::NotFound)?;

    let intent = match query.intent.as_deref() {
        Some("link") => OAuthIntent::Link,
        Some("login") | None => OAuthIntent::Login,
        Some(other) => return Err(AppError::BadRequest(format!("unknown intent {other:?}"))),
    };
    let caller_is_admin = is_admin(identity.as_ref());
    // Link attaches to the current user; login is (and must be able to be)
    // anonymous. A logged-in user may still start a login flow (e.g. from the
    // modal); we simply don't capture their id for it.
    let user_id = match intent {
        OAuthIntent::Link => Some(identity.ok_or(AppError::Unauthorized)?.user_id),
        OAuthIntent::Login => None,
    };

    let (visibility, creds) = fetch_provider_credentials(state.pool(), provider)
        .await?
        .ok_or_else(|| AppError::BadRequest("provider is not enabled".into()))?;
    // `admins` providers are only reachable by admins; treat a non-admin hitting
    // this URL directly the same as a disabled/unconfigured provider.
    match visibility {
        OAuthVisibility::Disabled => {
            return Err(AppError::BadRequest("provider is not enabled".into()));
        }
        OAuthVisibility::Admins if !caller_is_admin => {
            return Err(AppError::BadRequest("provider is not enabled".into()));
        }
        OAuthVisibility::Admins | OAuthVisibility::Public => {}
    }

    let return_to = resolve_return_to(query.return_to.as_deref().unwrap_or("/"));
    let redirect_uri = callback_uri(&state, provider);
    let (authorize_url, flow) =
        build_authorize_redirect(provider, &creds, &redirect_uri, intent, return_to, user_id)?;
    let cookie = set_flow_cookie(&flow, state.jwt_encoding_key(), state.https())?;

    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, header_value(&authorize_url)?);
    headers.insert(SET_COOKIE, header_value(&cookie)?);
    Ok((StatusCode::SEE_OTHER, headers).into_response())
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    state: Option<String>,
    /// Provider-side error (e.g. `access_denied` when the user cancels consent).
    #[serde(default)]
    error: Option<String>,
}

async fn callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, AppError> {
    let provider = OAuthProvider::from_path(&provider).ok_or(AppError::NotFound)?;
    // Single-use: always drop the flow cookie on the way out.
    let clear = clear_flow_cookie(state.https());

    let ValidCallback { flow, code } =
        match validate_callback(provider, &headers, query, state.jwt_decoding_key()) {
            Ok(valid) => valid,
            Err(reject) => {
                return Ok(redirect(
                    &state,
                    &reject.return_to,
                    ERR,
                    reject.code,
                    &[clear],
                ));
            }
        };

    // The signed flow cookie proves `start` already gated on visibility, so
    // here we only reject a provider that's since gone away or been disabled.
    let creds = match fetch_provider_credentials(state.pool(), provider).await? {
        Some((visibility, creds)) if visibility != OAuthVisibility::Disabled => creds,
        _ => return Ok(redirect(&state, &flow.return_to, ERR, "failed", &[clear])),
    };
    let redirect_uri = callback_uri(&state, provider);
    let identity = match exchange_and_identify(
        provider,
        &creds,
        &redirect_uri,
        code,
        flow.pkce_verifier.clone(),
    )
    .await
    {
        Ok(identity) => identity,
        Err(err) => {
            tracing::warn!(error = %err, provider = provider.pg_enum_value(), "oauth callback failed");
            return Ok(redirect(&state, &flow.return_to, ERR, "failed", &[clear]));
        }
    };

    let outcome = resolve_intent(&state, provider, &flow, &identity).await?;
    let mut cookies = vec![clear];
    if let Some(session) = outcome.session {
        cookies.push(session);
    }
    Ok(redirect(
        &state,
        &flow.return_to,
        outcome.param,
        outcome.value,
        &cookies,
    ))
}

/// What to report back after a verified callback: the status param/value the
/// SPA toasts, plus the session cookie to set on a successful login.
struct CallbackOutcome {
    param: &'static str,
    value: &'static str,
    session: Option<String>,
}

impl CallbackOutcome {
    fn ok(value: &'static str) -> Self {
        Self {
            param: OK,
            value,
            session: None,
        }
    }
    fn err(value: &'static str) -> Self {
        Self {
            param: ERR,
            value,
            session: None,
        }
    }
    fn logged_in(session: String) -> Self {
        Self {
            param: OK,
            value: "logged_in",
            session: Some(session),
        }
    }
    fn registered(session: String) -> Self {
        Self {
            param: OK,
            value: "registered",
            session: Some(session),
        }
    }
}

/// Act on a verified callback: link the identity to the flow's user, or log in
/// the user it resolves to.
async fn resolve_intent(
    state: &AppState,
    provider: OAuthProvider,
    flow: &FlowState,
    identity: &OAuthIdentity,
) -> Result<CallbackOutcome, AppError> {
    match flow.intent {
        OAuthIntent::Link => {
            let Some(user_id) = flow.user_id else {
                return Ok(CallbackOutcome::err("failed"));
            };
            let mut conn = acquire(state).await?;
            let outcome = match upsert_link(&mut conn, user_id, provider, identity).await? {
                LinkOutcome::TakenByOther => CallbackOutcome::err("link_taken"),
                LinkOutcome::Created | LinkOutcome::Refreshed => CallbackOutcome::ok("linked"),
            };
            Ok(outcome)
        }
        OAuthIntent::Login => resolve_login(state, provider, identity).await,
    }
}

/// Login intent: sign in an already-linked identity; else attach a verified
/// email to the existing account it matches and sign in; else register a fresh
/// account, when the site allows it.
async fn resolve_login(
    state: &AppState,
    provider: OAuthProvider,
    identity: &OAuthIdentity,
) -> Result<CallbackOutcome, AppError> {
    // 1. Identity already linked → straight login.
    if let Some(user_id) = find_user_by_link(state.pool(), provider, &identity.subject).await? {
        return Ok(login_outcome(mint_session_for(state, user_id).await?));
    }

    // 2. Verified email matches an existing account → attach + login. The
    // identity was unlinked at step 1, so `upsert_link` can't hit TakenByOther.
    if let Some(email) = identity.email.as_deref()
        && let Some(user_id) = find_user_id_by_email(state.pool(), email).await?
    {
        let mut conn = acquire(state).await?;
        upsert_link(&mut conn, user_id, provider, identity).await?;
        return Ok(login_outcome(mint_session_for(state, user_id).await?));
    }

    // 3. Unknown identity → register, gated by the site switch.
    if !fetch_site_public(state.pool()).await?.can_register {
        return Ok(CallbackOutcome::err("registration_disabled"));
    }
    let user_id = register_oauth_user(state.pool(), provider, identity).await?;
    match mint_session_for(state, user_id).await? {
        SessionOutcome::Ok(session) => Ok(CallbackOutcome::registered(session)),
        // A just-created user has CAN_AUTHORIZE and exists; a miss here is not
        // reachable in practice, so fall back to a generic failure.
        SessionOutcome::Banned | SessionOutcome::Gone => Ok(CallbackOutcome::err("failed")),
    }
}

/// A pooled connection for the link helpers, which take a `&mut PgConnection`
/// (usable standalone or inside a transaction) rather than a pool.
async fn acquire(state: &AppState) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, AppError> {
    state
        .pool()
        .acquire()
        .await
        .map_err(|e| AppError::Internal(e.into()))
}

/// Map a session mint for an *existing* account to its callback outcome.
fn login_outcome(outcome: SessionOutcome) -> CallbackOutcome {
    match outcome {
        SessionOutcome::Ok(session) => CallbackOutcome::logged_in(session),
        SessionOutcome::Banned => CallbackOutcome::err("banned"),
        SessionOutcome::Gone => CallbackOutcome::err("failed"),
    }
}

/// A parsed + verified callback: readable flow cookie, matching provider, a
/// present authorization `code`, and a CSRF `state` matching the cookie.
struct ValidCallback {
    flow: FlowState,
    code: String,
}

/// A rejected callback: the error code to show and where to land the user
/// (`/` when the flow cookie was unreadable, so we don't know their page).
struct CallbackReject {
    return_to: String,
    code: &'static str,
}

/// Verify the flow cookie + CSRF and pull out the authorization code. Doesn't
/// touch the DB or the provider — purely request validation.
fn validate_callback(
    provider: OAuthProvider,
    headers: &HeaderMap,
    query: CallbackQuery,
    decoding_key: &DecodingKey,
) -> Result<ValidCallback, CallbackReject> {
    let Some(flow) =
        read_flow_cookie(headers).and_then(|jwt| decode_flow_cookie(&jwt, decoding_key))
    else {
        return Err(reject("/", "bad_state"));
    };

    if flow.provider != provider {
        return Err(reject(&flow.return_to, "bad_state"));
    }
    if query.error.is_some() {
        return Err(reject(&flow.return_to, "denied"));
    }
    let (Some(code), Some(returned_state)) = (query.code, query.state) else {
        return Err(reject(&flow.return_to, "bad_state"));
    };
    if returned_state != flow.state {
        return Err(reject(&flow.return_to, "bad_state"));
    }

    Ok(ValidCallback { flow, code })
}

fn reject(return_to: &str, code: &'static str) -> CallbackReject {
    CallbackReject {
        return_to: return_to.to_owned(),
        code,
    }
}

/// Query-param keys the SPA reads after a callback bounce.
const OK: &str = "oauth";
const ERR: &str = "oauth_error";

/// `${API_PUBLIC_URL}/oauth/{provider}/callback`. Must match exactly what's
/// registered with the provider.
fn callback_uri(state: &AppState, provider: OAuthProvider) -> String {
    format!(
        "{}/oauth/{}/callback",
        state.api_public_url(),
        provider.pg_enum_value()
    )
}

/// Outcome of trying to log the linked user in.
enum SessionOutcome {
    Ok(String),
    /// Row exists but `CAN_AUTHORIZE` is cleared — a banned account.
    Banned,
    /// Row missing (deleted between the link lookup and now).
    Gone,
}

async fn mint_session_for(state: &AppState, user_id: i32) -> Result<SessionOutcome, AppError> {
    let row: Option<(String, i32)> =
        sqlx::query_as("SELECT name, permissions FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(state.pool())
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    let Some((name, permissions_bits)) = row else {
        return Ok(SessionOutcome::Gone);
    };

    let permissions = Permissions::from_bits_retain(permissions_bits);
    if !permissions.contains(Permissions::CAN_AUTHORIZE) {
        return Ok(SessionOutcome::Banned);
    }

    let cookie = mint_session_cookie(
        user_id,
        name,
        permissions,
        state.jwt_encoding_key(),
        state.https(),
    )
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(SessionOutcome::Ok(cookie))
}

/// 303 back to `${APP_BASE_URL}${return_to}` with `?{param}={value}` appended
/// and each of `cookies` set. `return_to` is re-sanitized defensively.
fn redirect(
    state: &AppState,
    return_to: &str,
    param: &str,
    value: &str,
    cookies: &[String],
) -> Response {
    let base = format!("{}{}", state.app_base_url(), resolve_return_to(return_to));
    let sep = if base.contains('?') { '&' } else { '?' };
    let location = format!("{base}{sep}{param}={value}");

    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&location) {
        headers.insert(LOCATION, value);
    }
    for cookie in cookies {
        if let Ok(value) = HeaderValue::from_str(cookie) {
            headers.append(SET_COOKIE, value);
        }
    }
    (StatusCode::SEE_OTHER, headers).into_response()
}

fn read_flow_cookie(headers: &HeaderMap) -> Option<String> {
    for raw in headers.get_all(COOKIE) {
        let Ok(text) = raw.to_str() else { continue };
        for part in Cookie::split_parse(text) {
            let Ok(cookie) = part else { continue };
            if cookie.name() == FLOW_COOKIE_NAME {
                return Some(cookie.value().to_owned());
            }
        }
    }
    None
}

fn header_value(value: &str) -> Result<HeaderValue, AppError> {
    HeaderValue::from_str(value).map_err(|e| AppError::Internal(e.into()))
}
