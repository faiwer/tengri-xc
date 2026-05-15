use std::sync::OnceLock;

use axum::http::HeaderMap;
use cookie::Cookie;
use regex::Regex;
use sqlx::FromRow;

use crate::{
    AppState,
    user::{Permissions, UserSource},
};

use super::{
    cookie::clear_legacy_leonardo,
    extractor::Identity,
    token::{Claims, encode_jwt},
};

const LEONARDO_DATA_COOKIE_NAME: &str = "leonardo_data";

#[derive(Debug)]
pub struct Handoff {
    pub identity: Identity,
    pub cookies: Vec<String>,
}

#[derive(Debug)]
pub enum Attempt {
    NoCookie,
    Failed(Vec<String>),
    Authorized(Handoff),
}

#[derive(Debug)]
struct Autologin {
    user_id: i32,
    password_hash: String,
}

#[derive(FromRow)]
struct UserRow {
    id: i32,
    name: String,
    permissions: i32,
    password_hash: Option<String>,
}

pub async fn attempt(headers: &HeaderMap, state: &AppState, now: i64) -> Attempt {
    let Some(raw) = extract_leonardo_data(headers) else {
        return Attempt::NoCookie;
    };

    let clear_cookies = clear_legacy_leonardo(state.leonardo_cookie_domain(), state.https());
    let decoded = percent_decode(&raw).unwrap_or(raw);
    let Some(autologin) = parse_autologin(&decoded) else {
        tracing::info!("legacy Leonardo handoff rejected: malformed cookie");
        return Attempt::Failed(clear_cookies);
    };

    let Some(row) = fetch_leonardo_user(state, autologin.user_id).await else {
        return Attempt::Failed(clear_cookies);
    };

    let permissions = Permissions::from_bits_retain(row.permissions);
    if !permissions.contains(Permissions::CAN_AUTHORIZE) {
        tracing::info!(
            user_id = row.id,
            "legacy Leonardo handoff rejected: CAN_AUTHORIZE bit cleared"
        );
        return Attempt::Failed(clear_cookies);
    }

    let Some(stored_hash) = row.password_hash else {
        tracing::info!(
            user_id = row.id,
            "legacy Leonardo handoff rejected: no password hash"
        );
        return Attempt::Failed(clear_cookies);
    };

    if stored_hash != autologin.password_hash {
        tracing::info!(
            user_id = row.id,
            "legacy Leonardo handoff rejected: autologin hash mismatch"
        );
        return Attempt::Failed(clear_cookies);
    }

    let claims = Claims::new(row.id, row.name, permissions, now);
    let jwt = match encode_jwt(&claims, state.jwt_encoding_key()) {
        Ok(jwt) => jwt,
        Err(e) => {
            tracing::error!(user_id = row.id, error = %e, "legacy Leonardo JWT signing failed");
            return Attempt::Failed(clear_cookies);
        }
    };

    let identity = Identity::from_claims(claims);
    let mut cookies = Vec::with_capacity(clear_cookies.len() + 1);
    cookies.push(super::cookie::set_session(&jwt, state.https()));
    cookies.extend(clear_cookies);
    Attempt::Authorized(Handoff { identity, cookies })
}

async fn fetch_leonardo_user(state: &AppState, user_id: i32) -> Option<UserRow> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, name, permissions, password_hash \
         FROM users \
         WHERE id = $1 \
           AND source = $2::user_source",
    )
    .bind(user_id)
    .bind(UserSource::Leo.pg_enum_value())
    .fetch_optional(state.pool())
    .await;

    match row {
        Ok(Some(row)) => Some(row),
        Ok(None) => {
            tracing::info!(
                user_id,
                "legacy Leonardo handoff rejected: user row missing"
            );
            None
        }
        Err(e) => {
            tracing::warn!(
                user_id,
                error = %e,
                "legacy Leonardo handoff DB lookup failed"
            );
            None
        }
    }
}

fn extract_leonardo_data(headers: &HeaderMap) -> Option<String> {
    for raw in headers.get_all(axum::http::header::COOKIE) {
        let Ok(s) = raw.to_str() else { continue };
        for part in Cookie::split_parse(s) {
            let Ok(cookie) = part else { continue };
            if cookie.name() == LEONARDO_DATA_COOKIE_NAME {
                return Some(cookie.value().to_owned());
            }
        }
    }
    None
}

fn parse_autologin(raw: &str) -> Option<Autologin> {
    let user_id = captures_user_id(raw)?.parse::<i32>().ok()?;
    let password_hash = captures_autologinid(raw)?.to_owned();
    Some(Autologin {
        user_id,
        password_hash,
    })
}

fn captures_user_id(raw: &str) -> Option<&str> {
    static USER_ID_RE: OnceLock<Regex> = OnceLock::new();
    let re = USER_ID_RE.get_or_init(|| {
        Regex::new(r#"s:6:"userid";(?:i:(\d+);|s:\d+:"(\d+)";)"#).expect("userid regex")
    });
    let captures = re.captures(raw)?;
    captures
        .get(1)
        .or_else(|| captures.get(2))
        .map(|m| m.as_str())
}

fn captures_autologinid(raw: &str) -> Option<&str> {
    static AUTOLOGIN_RE: OnceLock<Regex> = OnceLock::new();
    let re = AUTOLOGIN_RE.get_or_init(|| {
        Regex::new(r#"s:11:"autologinid";s:\d+:"([^"]+)";"#).expect("autologinid regex")
    });
    re.captures(raw)?.get(1).map(|m| m.as_str())
}

/// ~decodeURIComponent
fn percent_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = *bytes.get(i + 1)?;
            let lo = *bytes.get(i + 2)?;
            out.push(hex(hi)? << 4 | hex(lo)?);
            i += 3;
            changed = true;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    if changed {
        String::from_utf8(out).ok()
    } else {
        None
    }
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_autologin;

    #[test]
    fn parses_leonardo_data_autologin() {
        let parsed = parse_autologin(
            r#"a:2:{s:11:"autologinid";s:34:"$H$9abcdefgh1234567890123456789012";s:6:"userid";i:42;}"#,
        )
        .expect("parse autologin");

        assert_eq!(parsed.user_id, 42);
        assert_eq!(parsed.password_hash, "$H$9abcdefgh1234567890123456789012");
    }

    #[test]
    fn rejects_cookie_without_autologinid() {
        assert!(parse_autologin(r#"a:1:{s:6:"userid";i:42;}"#).is_none());
    }

    #[test]
    fn decodes_percent_encoded_cookie_value() {
        assert_eq!(
            super::percent_decode("a%3A1%3A%7Bs%3A6%3A%22userid%22%3Bi%3A42%3B%7D"),
            Some(r#"a:1:{s:6:"userid";i:42;}"#.to_owned())
        );
    }
}
