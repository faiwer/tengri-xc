use std::sync::Arc;

use jsonwebtoken::{DecodingKey, EncodingKey};
use sqlx::PgPool;

/// Shared app state. Cheap to clone — everything's behind `Arc`.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    pool: PgPool,
    jwt_encoding_key: EncodingKey,
    jwt_decoding_key: DecodingKey,
    /// `true` → session cookies get the `Secure` flag.
    https: bool,
}

impl AppState {
    pub fn new(pool: PgPool, jwt_secret: &[u8], https: bool) -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                pool,
                jwt_encoding_key: EncodingKey::from_secret(jwt_secret),
                jwt_decoding_key: DecodingKey::from_secret(jwt_secret),
                https,
            }),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    pub fn jwt_encoding_key(&self) -> &EncodingKey {
        &self.inner.jwt_encoding_key
    }

    pub fn jwt_decoding_key(&self) -> &DecodingKey {
        &self.inner.jwt_decoding_key
    }

    pub fn https(&self) -> bool {
        self.inner.https
    }
}
