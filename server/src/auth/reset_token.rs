//! Password-reset tokens. Signed with the same key as session JWTs and
//! confirmation links, so each carries its own purpose tag.

use std::time::Duration;

use chrono::Utc;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};

/// How long a reset link stays usable. Matches the confirmation link: long
/// enough that a mail read the next morning still works. What bounds abuse is
/// the resend ladder, not this.
pub const RESET_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Its own single-variant enum rather than a second variant on
/// `confirm_token::TokenKind`: sharing one enum would let a confirmation token
/// satisfy the purpose check here, and vice versa.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ResetKind {
    ResetPassword,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetClaims {
    /// User id (= `users.id`).
    pub sub: i32,
    /// Epoch micros of the `users.password_reset_sends` entry this link was
    /// minted for. The confirm route requires it to still be the last entry,
    /// which is what makes the link single-use.
    pub stamp: i64,
    kind: ResetKind,
    pub exp: i64,
}

pub fn mint_reset_token(
    user_id: i32,
    stamp: i64,
    encoding_key: &EncodingKey,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = ResetClaims {
        sub: user_id,
        stamp,
        kind: ResetKind::ResetPassword,
        exp: Utc::now().timestamp() + RESET_TTL.as_secs() as i64,
    };
    encode(&Header::new(Algorithm::HS256), &claims, encoding_key)
}

/// Verify signature, `exp`, and that the token was minted for a password reset.
pub fn verify_reset_token(
    token: &str,
    decoding_key: &DecodingKey,
) -> Result<ResetClaims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;
    let TokenData { claims, .. } = decode::<ResetClaims>(token, decoding_key, &validation)?;
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use jsonwebtoken::errors::ErrorKind;

    use super::{
        super::{
            confirm_token::{mint_confirm_token, verify_confirm_token},
            token::{Claims, encode_jwt},
        },
        *,
    };
    use crate::user::Permissions;

    fn key() -> (EncodingKey, DecodingKey) {
        let secret = b"\x00".repeat(32);
        (
            EncodingKey::from_secret(&secret),
            DecodingKey::from_secret(&secret),
        )
    }

    #[test]
    fn round_trip_preserves_subject_and_stamp() {
        let (enc, dec) = key();
        let token = mint_reset_token(42, 1_700_000_000_123_456, &enc).unwrap();
        let claims = verify_reset_token(&token, &dec).unwrap();
        assert_eq!(claims.sub, 42);
        assert_eq!(claims.stamp, 1_700_000_000_123_456);
    }

    #[test]
    fn expired_token_rejected() {
        let (enc, dec) = key();
        let claims = ResetClaims {
            sub: 1,
            stamp: 1,
            kind: ResetKind::ResetPassword,
            exp: 1_400_000_000,
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &enc).unwrap();
        let err = verify_reset_token(&token, &dec).unwrap_err();
        assert!(matches!(err.kind(), ErrorKind::ExpiredSignature));
    }

    #[test]
    fn forged_signature_rejected() {
        let (enc, _dec) = key();
        let other = DecodingKey::from_secret(b"\x01".repeat(32).as_ref());
        let token = mint_reset_token(1, 1, &enc).unwrap();
        verify_reset_token(&token, &other).expect_err("must reject a mismatched key");
    }

    #[test]
    fn session_token_rejected() {
        let (enc, dec) = key();
        let session = Claims::new(
            1,
            "Pilot".into(),
            Permissions::CAN_AUTHORIZE,
            Utc::now().timestamp(),
        );
        let token = encode_jwt(&session, &enc).unwrap();
        verify_reset_token(&token, &dec).expect_err("must reject a session token");
    }

    /// The reason [`ResetKind`] isn't a variant on the confirmation enum.
    #[test]
    fn confirm_token_rejected() {
        let (enc, dec) = key();
        let token = mint_confirm_token(1, "a@example.com", &enc).unwrap();
        verify_reset_token(&token, &dec).expect_err("must reject a confirmation token");
    }

    #[test]
    fn reset_token_is_not_a_confirmation_token() {
        let (enc, dec) = key();
        let token = mint_reset_token(1, 1, &enc).unwrap();
        verify_confirm_token(&token, &dec).expect_err("must reject a reset token");
    }
}
