use std::{env, net::SocketAddr};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_addr: SocketAddr,
    pub database_url: String,
    /// HS256 signing key for session JWTs. Base64-decoded on
    /// startup so token signing/verification doesn't have to
    /// re-decode per request. Sized to the HS256 minimum (32
    /// bytes) so a short value can't slip through and weaken
    /// the signature.
    pub jwt_secret: Vec<u8>,
    /// `true` when the server is reachable over TLS (directly or
    /// behind a terminating proxy). Drives the `Secure` flag on
    /// session cookies and any future https-aware behavior. Read
    /// from the `HTTPS` env var; defaults to `false` so local dev
    /// over plain HTTP just works.
    pub https: bool,
}

/// Minimum key length for HS256. RFC 8725 §3.1 says "the keys
/// used MUST be of size equal to or greater than the size of the
/// HMAC output", which for SHA-256 is 32 bytes. Less than that
/// is a configuration bug, not an unusual deployment.
const JWT_SECRET_MIN_BYTES: usize = 32;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid value for {var}: {source}")]
    InvalidValue {
        var: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("required env var {0} is not set")]
    Missing(&'static str),

    #[error(
        "JWT_SECRET is too short ({got} bytes after base64-decoding, need at least {min}); \
         generate one with: head -c 32 /dev/urandom | base64"
    )]
    JwtSecretTooShort { got: usize, min: usize },
}

#[derive(Debug, Error)]
#[error("expected true/false/1/0/yes/no, got {0:?}")]
struct BoolParseError(String);

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let server_addr = parse_env("SERVER_ADDR", "0.0.0.0:3000")?;
        let database_url =
            env::var("DATABASE_URL").map_err(|_| ConfigError::Missing("DATABASE_URL"))?;
        let jwt_secret = load_jwt_secret()?;
        let https = parse_bool_env("HTTPS", false)?;
        Ok(Self {
            server_addr,
            database_url,
            jwt_secret,
            https,
        })
    }
}

/// Parse a boolean env var. Accepts the same values `serde-toml` and
/// most CI tooling do: `true`/`false`, `1`/`0`, `yes`/`no`,
/// case-insensitive. Anything else fails loudly because silently
/// defaulting `HTTPS=YEs` to `false` is a security foot-gun.
fn parse_bool_env(var: &'static str, default: bool) -> Result<bool, ConfigError> {
    let Ok(raw) = env::var(var) else {
        return Ok(default);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(ConfigError::InvalidValue {
            var,
            source: Box::new(BoolParseError(other.to_owned())),
        }),
    }
}

/// Read `JWT_SECRET`, base64-decode, sanity-check the length.
/// Failing here is good — the alternative is a server that boots
/// fine and starts issuing weak signatures.
fn load_jwt_secret() -> Result<Vec<u8>, ConfigError> {
    let raw = env::var("JWT_SECRET").map_err(|_| ConfigError::Missing("JWT_SECRET"))?;
    let bytes = B64
        .decode(raw.trim())
        .map_err(|e| ConfigError::InvalidValue {
            var: "JWT_SECRET",
            source: Box::new(e),
        })?;
    if bytes.len() < JWT_SECRET_MIN_BYTES {
        return Err(ConfigError::JwtSecretTooShort {
            got: bytes.len(),
            min: JWT_SECRET_MIN_BYTES,
        });
    }
    Ok(bytes)
}

fn parse_env<T>(var: &'static str, default: &str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let raw = env::var(var).unwrap_or_else(|_| default.to_owned());
    raw.parse::<T>().map_err(|e| ConfigError::InvalidValue {
        var,
        source: Box::new(e),
    })
}
