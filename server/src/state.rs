use std::sync::Arc;

use jsonwebtoken::{DecodingKey, EncodingKey};
use sqlx::PgPool;

use crate::flight::{ScoringQueue, queue::default_worker_count};

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
    /// Cross-origin browsers allowed to send the session cookie.
    client_origins: Vec<String>,
    /// Keep to be able to clear the cookie.
    leonardo_cookie_domain: Option<String>,
    /// Public API base URL (matches the SPA's `VITE_SERVER_URL`); OAuth
    /// redirect URIs are built from it. Trailing slash already trimmed by
    /// `Config`.
    api_public_url: String,
    /// Public SPA origin; OAuth callbacks redirect the browser back under it.
    /// Trailing slash already trimmed by `Config`.
    app_base_url: String,
    /// Global route-scoring queue; drains its worker pool in the background.
    scoring_queue: ScoringQueue,
}

impl AppState {
    /// Minimal constructor for tests: no client origins, no OAuth URLs. The
    /// prod path uses [`with_origins`](Self::with_origins) with values from
    /// `Config`.
    pub fn new_for_tests(pool: PgPool, jwt_secret: &[u8], https: bool) -> Self {
        Self::with_origins(
            pool,
            jwt_secret,
            https,
            Vec::new(),
            None,
            String::new(),
            String::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_origins(
        pool: PgPool,
        jwt_secret: &[u8],
        https: bool,
        client_origins: Vec<String>,
        leonardo_cookie_domain: Option<String>,
        api_public_url: String,
        app_base_url: String,
    ) -> Self {
        let scoring_queue = ScoringQueue::spawn(pool.clone(), default_worker_count());
        Self {
            inner: Arc::new(AppStateInner {
                pool,
                jwt_encoding_key: EncodingKey::from_secret(jwt_secret),
                jwt_decoding_key: DecodingKey::from_secret(jwt_secret),
                https,
                client_origins,
                leonardo_cookie_domain,
                api_public_url,
                app_base_url,
                scoring_queue,
            }),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    pub fn scoring_queue(&self) -> &ScoringQueue {
        &self.inner.scoring_queue
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

    pub fn client_origins(&self) -> &[String] {
        &self.inner.client_origins
    }

    pub fn leonardo_cookie_domain(&self) -> Option<&str> {
        self.inner.leonardo_cookie_domain.as_deref()
    }

    pub fn api_public_url(&self) -> &str {
        &self.inner.api_public_url
    }

    pub fn app_base_url(&self) -> &str {
        &self.inner.app_base_url
    }
}
