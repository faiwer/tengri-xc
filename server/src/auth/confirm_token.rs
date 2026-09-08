//! Email-confirmation tokens. Signed with the same key as session JWTs but
//! carrying a purpose tag, so neither kind can stand in for the other.

use std::time::Duration;

use chrono::Utc;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};

/// How long a confirmation link stays usable. Long enough to survive a mail
/// that lands overnight, short enough that an old inbox isn't a standing key.
const CONFIRM_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// What a token is for. Both kinds are signed with `JWT_SECRET`, so a valid
/// signature only proves *we* minted it, not what we minted it for. A
/// single-variant enum moves that check into `serde`: any other value fails to
/// deserialize, so there's no comparison for a caller to forget.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TokenKind {
    ConfirmEmail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmClaims {
    /// User id (= `users.id`).
    pub sub: i32,
    /// The address the link was issued for. The confirm route matches it
    /// against the stored one, so a stale link can't confirm an address the
    /// user has since changed to.
    pub email: String,
    kind: TokenKind,
    pub exp: i64,
}

pub fn mint_confirm_token(
    user_id: i32,
    email: &str,
    encoding_key: &EncodingKey,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = ConfirmClaims {
        sub: user_id,
        email: email.to_owned(),
        kind: TokenKind::ConfirmEmail,
        exp: Utc::now().timestamp() + CONFIRM_TTL.as_secs() as i64,
    };
    encode(&Header::new(Algorithm::HS256), &claims, encoding_key)
}

/// Verify signature, `exp`, and that the token was minted for confirmation.
pub fn verify_confirm_token(
    token: &str,
    decoding_key: &DecodingKey,
) -> Result<ConfirmClaims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;
    let TokenData { claims, .. } = decode::<ConfirmClaims>(token, decoding_key, &validation)?;
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use jsonwebtoken::errors::ErrorKind;

    use super::*;
    use crate::{
        auth::token::{Claims, encode_jwt},
        user::Permissions,
    };

    fn key() -> (EncodingKey, DecodingKey) {
        let secret = b"\x00".repeat(32);
        (
            EncodingKey::from_secret(&secret),
            DecodingKey::from_secret(&secret),
        )
    }

    #[test]
    fn round_trip_preserves_subject_and_email() {
        let (enc, dec) = key();
        let token = mint_confirm_token(42, "pilot@example.com", &enc).unwrap();
        let claims = verify_confirm_token(&token, &dec).unwrap();
        assert_eq!(claims.sub, 42);
        assert_eq!(claims.email, "pilot@example.com");
    }

    #[test]
    fn expired_token_rejected() {
        let (enc, dec) = key();
        let claims = ConfirmClaims {
            sub: 1,
            email: "stale@example.com".into(),
            kind: TokenKind::ConfirmEmail,
            exp: 1_400_000_000,
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &enc).unwrap();
        let err = verify_confirm_token(&token, &dec).unwrap_err();
        assert!(matches!(err.kind(), ErrorKind::ExpiredSignature));
    }

    #[test]
    fn forged_signature_rejected() {
        let (enc, _dec) = key();
        let other = DecodingKey::from_secret(b"\x01".repeat(32).as_ref());
        let token = mint_confirm_token(1, "a@example.com", &enc).unwrap();
        verify_confirm_token(&token, &other).expect_err("must reject a mismatched key");
    }

    /// The point of [`TokenKind`]: a session cookie is signed with the same key
    /// and would otherwise be a structurally acceptable confirmation token.
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
        verify_confirm_token(&token, &dec).expect_err("must reject a session token");
    }

    /// And the reverse, so the confirm link can't be pasted into the cookie.
    #[test]
    fn confirm_token_is_not_a_session_token() {
        use crate::auth::token::decode_jwt;

        let (enc, dec) = key();
        let token = mint_confirm_token(1, "a@example.com", &enc).unwrap();
        decode_jwt(&token, &dec).expect_err("must reject a confirmation token");
    }
}
