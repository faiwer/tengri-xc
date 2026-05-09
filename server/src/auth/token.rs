//! Issue and verify session JWTs. Tiny payload (`sub`/`name`/`p`),
//! HS256, 6-month lifetime. Mutable user fields (email, profile)
//! deliberately stay out — they live in the DB and are read on
//! demand.

use std::time::Duration;

use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};

use crate::user::Permissions;

pub const JWT_LIFETIME: Duration = Duration::from_secs(60 * 60 * 24 * 30 * 6);

/// Field names use JWT-standard short forms (`sub`/`iat`/`exp`)
/// for tooling compatibility; `name` and `p` are custom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// User id (= `users.id`). Standard JWT "subject" claim.
    pub sub: i32,
    /// Cached display name so the navbar doesn't query `users` on
    /// every page. Refreshed on the next slide.
    pub name: String,
    /// Permissions bitfield, raw `i32` from `users.permissions`.
    pub p: i32,
    pub iat: i64,
    pub exp: i64,
}

impl Claims {
    pub fn new(user_id: i32, name: String, permissions: Permissions, now: i64) -> Self {
        Self {
            sub: user_id,
            name,
            p: permissions.bits(),
            iat: now,
            exp: now + JWT_LIFETIME.as_secs() as i64,
        }
    }

    /// Decode `p` into [`Permissions`]. `from_bits_retain` keeps
    /// unknown bits so a newer server's privilege bit isn't
    /// silently dropped when read by an older build.
    pub fn permissions(&self) -> Permissions {
        Permissions::from_bits_retain(self.p)
    }
}

pub fn encode_jwt(
    claims: &Claims,
    encoding_key: &EncodingKey,
) -> Result<String, jsonwebtoken::errors::Error> {
    encode(&Header::new(Algorithm::HS256), claims, encoding_key)
}

/// Verify signature + `exp`. "Is this account still allowed?" is
/// the slide middleware's problem, not this function's.
pub fn decode_jwt(
    token: &str,
    decoding_key: &DecodingKey,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;
    let TokenData { claims, .. } = decode::<Claims>(token, decoding_key, &validation)?;
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> (EncodingKey, DecodingKey) {
        let secret = b"\x00".repeat(32);
        (
            EncodingKey::from_secret(&secret),
            DecodingKey::from_secret(&secret),
        )
    }

    /// Anchor used as `iat` for round-trip tests. Pinned far in
    /// the future (year 2100) so the JWT's `exp = iat +
    /// JWT_LIFETIME` stays valid relative to wall clock without
    /// our tests baking in a "today" snapshot that goes stale.
    const FUTURE_NOW: i64 = 4_102_444_800;

    #[test]
    fn round_trip_preserves_claims() {
        let (enc, dec) = key();
        let claims = Claims::new(
            42,
            "Test User".into(),
            Permissions::CAN_AUTHORIZE | Permissions::MANAGE_TRACKS,
            FUTURE_NOW,
        );
        let token = encode_jwt(&claims, &enc).unwrap();
        let back = decode_jwt(&token, &dec).unwrap();
        assert_eq!(back.sub, 42);
        assert_eq!(back.name, "Test User");
        assert_eq!(back.iat, FUTURE_NOW);
        assert_eq!(
            back.permissions(),
            Permissions::CAN_AUTHORIZE | Permissions::MANAGE_TRACKS
        );
    }

    #[test]
    fn expired_token_rejected() {
        let (enc, dec) = key();
        // exp ten years in the past.
        let claims = Claims {
            sub: 1,
            name: "stale".into(),
            p: 1,
            iat: 1_400_000_000,
            exp: 1_400_000_001,
        };
        let token = encode_jwt(&claims, &enc).unwrap();
        let err = decode_jwt(&token, &dec).unwrap_err();
        assert!(matches!(
            err.kind(),
            jsonwebtoken::errors::ErrorKind::ExpiredSignature
        ));
    }

    #[test]
    fn forged_signature_rejected() {
        let (enc, _dec) = key();
        let other_dec = DecodingKey::from_secret(b"\x01".repeat(32).as_ref());
        let claims = Claims::new(1, "x".into(), Permissions::CAN_AUTHORIZE, FUTURE_NOW);
        let token = encode_jwt(&claims, &enc).unwrap();
        decode_jwt(&token, &other_dec).expect_err("must reject mismatched key");
    }
}
