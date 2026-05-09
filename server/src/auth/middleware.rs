//! Session middleware. Mounted on every route except
//! `/users/{login,logout}`.
//!
//! Per request:
//!
//! ```text
//!   no cookie / decode fails       → no identity, response untouched
//!   age < SLIDE_INTERVAL           → identity from JWT, response untouched
//!   age ≥ SLIDE_INTERVAL           → DB lookup by sub:
//!     row gone OR !CAN_AUTHORIZE   → no identity, clear cookie
//!     row ok                       → identity + re-mint cookie with
//!                                    fresh name/permissions
//! ```
//!
//! The decision is computed *before* the handler runs so a
//! revoked-mid-request user sees "logged out" in both the response
//! body and the cookie — no stale-data + clear-cookie inconsistency.

use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, header::SET_COOKIE},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use cookie::Cookie;
use sqlx::Row;

use crate::{AppState, user::Permissions};

use super::{
    cookie::{SESSION_COOKIE_NAME, SLIDE_INTERVAL, clear_session, set_session},
    extractor::Identity,
    token::{Claims, decode_jwt, encode_jwt},
};

/// Per-request outcome: identity to inject (or not), cookie to set
/// on the response (or not).
struct Decision {
    identity: Option<Identity>,
    cookie: Option<String>,
}

impl Decision {
    fn passthrough() -> Self {
        Self {
            identity: None,
            cookie: None,
        }
    }
    fn keep(claims: Claims) -> Self {
        Self {
            identity: Some(Identity::from_claims(claims)),
            cookie: None,
        }
    }
    fn renewed(identity: Identity, set_cookie: String) -> Self {
        Self {
            identity: Some(identity),
            cookie: Some(set_cookie),
        }
    }
    fn revoked(clear_cookie: String) -> Self {
        Self {
            identity: None,
            cookie: Some(clear_cookie),
        }
    }
}

pub async fn session_layer(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = extract_session_token(request.headers());
    let claims = token
        .as_deref()
        .and_then(|jwt| decode_jwt(jwt, state.jwt_decoding_key()).ok());

    let decision = match claims {
        None => Decision::passthrough(),
        Some(c) => decide(&state, c).await,
    };

    if let Some(identity) = decision.identity {
        request.extensions_mut().insert(identity);
    }

    let mut response = next.run(request).await;

    if let Some(cookie) = decision.cookie
        && let Ok(v) = HeaderValue::from_str(&cookie)
    {
        response.headers_mut().insert(SET_COOKIE, v);
    }
    response
}

async fn decide(state: &AppState, claims: Claims) -> Decision {
    let now = Utc::now().timestamp();
    let age = now.saturating_sub(claims.iat);

    if age < SLIDE_INTERVAL.as_secs() as i64 {
        return Decision::keep(claims);
    }

    let row = sqlx::query("SELECT name, permissions FROM users WHERE id = $1")
        .bind(claims.sub)
        .fetch_optional(state.pool())
        .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => {
            tracing::info!(user_id = claims.sub, "session revoked: user row missing");
            return Decision::revoked(clear_session(state.https()));
        }
        // DB blip: keep the existing identity rather than logging
        // the user out over our outage.
        Err(e) => {
            tracing::warn!(user_id = claims.sub, error = %e, "slide DB lookup failed; leaving cookie unchanged");
            return Decision::keep(claims);
        }
    };

    let (name, permissions_bits): (String, i32) = match (
        row.try_get::<String, _>("name"),
        row.try_get::<i32, _>("permissions"),
    ) {
        (Ok(n), Ok(p)) => (n, p),
        (n, p) => {
            tracing::error!(
                user_id = claims.sub,
                name_err = ?n.err(),
                perms_err = ?p.err(),
                "slide: malformed user row"
            );
            return Decision::keep(claims);
        }
    };

    let permissions = Permissions::from_bits_retain(permissions_bits);
    if !permissions.contains(Permissions::CAN_AUTHORIZE) {
        tracing::info!(
            user_id = claims.sub,
            "session revoked: CAN_AUTHORIZE bit cleared"
        );
        return Decision::revoked(clear_session(state.https()));
    }

    let fresh_claims = Claims::new(claims.sub, name, permissions, now);
    match encode_jwt(&fresh_claims, state.jwt_encoding_key()) {
        Ok(jwt) => {
            let identity = Identity::from_claims(fresh_claims);
            Decision::renewed(identity, set_session(&jwt, state.https()))
        }
        Err(e) => {
            tracing::error!(error = %e, "slide: JWT signing failed; leaving cookie unchanged");
            Decision::keep(claims)
        }
    }
}

fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    for raw in headers.get_all(axum::http::header::COOKIE) {
        let Ok(s) = raw.to_str() else { continue };
        for part in Cookie::split_parse(s) {
            let Ok(cookie) = part else { continue };
            if cookie.name() == SESSION_COOKIE_NAME {
                return Some(cookie.value().to_owned());
            }
        }
    }
    None
}
