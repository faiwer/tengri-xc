use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use jsonwebtoken::{DecodingKey, EncodingKey};
use sqlx::PgPool;

use crate::{
    config::SatelliteMap,
    flight::{ScoringQueue, queue::default_worker_count},
    html::HtmlShell,
    site::SiteMeta,
};

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
    /// Imagery behind the flight preview image; `None` renders it on white.
    /// See [`Config::satellite_map`](crate::Config).
    satellite_map: Option<SatelliteMap>,
    /// Stand-in for the providers' own OAuth endpoints; `None` everywhere but
    /// the E2E harness. See [`Config::oauth_endpoint_base`](crate::Config).
    oauth_endpoint_base: Option<String>,
    /// Global route-scoring queue; drains its worker pool in the background.
    scoring_queue: ScoringQueue,
    /// The SPA's `index.html`, fetched from `app_base_url` on the first
    /// document request. A failed fetch isn't cached, so a server that starts
    /// before the static host recovers on the next request.
    html_shell: RwLock<Option<Arc<HtmlShell>>>,
    /// Site name + description for the rendered `<head>`. Every document
    /// request reads it, so it's cached rather than re-queried per page view.
    site_meta: RwLock<Option<(Instant, Arc<SiteMeta>)>>,
}

/// How long a cached [`SiteMeta`] stays usable. Editing the settings clears the
/// entry outright, so this only bounds staleness from writes that bypass
/// `PATCH /admin/site` (the `tengri site set` CLI, a manual UPDATE).
const SITE_META_TTL: Duration = Duration::from_secs(5 * 60);

/// Stand-in URL for tests whose route never touches Postgres: `connect_lazy`
/// defers the connection, and nothing in the test triggers one. Pair with
/// [`AppState::new_for_tests`].
pub const PLACEHOLDER_DB_URL: &str = "postgres://test:test@localhost/test";

impl AppState {
    /// Minimal constructor for tests: no client origins, no OAuth URLs, and no
    /// basemap, so renders stay offline. The prod path uses
    /// [`with_origins`](Self::with_origins) with values from `Config`.
    pub fn new_for_tests(pool: PgPool, jwt_secret: &[u8], https: bool) -> Self {
        Self::with_origins(
            pool,
            jwt_secret,
            https,
            Vec::new(),
            None,
            String::new(),
            String::new(),
            None,
            None,
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
        satellite_map: Option<SatelliteMap>,
        oauth_endpoint_base: Option<String>,
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
                satellite_map,
                oauth_endpoint_base,
                scoring_queue,
                html_shell: RwLock::new(None),
                site_meta: RwLock::new(None),
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

    pub fn satellite_map(&self) -> Option<&SatelliteMap> {
        self.inner.satellite_map.as_ref()
    }

    pub fn oauth_endpoint_base(&self) -> Option<&str> {
        self.inner.oauth_endpoint_base.as_deref()
    }

    pub(crate) fn html_shell(&self) -> Option<Arc<HtmlShell>> {
        self.inner
            .html_shell
            .read()
            .expect("html shell lock poisoned")
            .clone()
    }

    pub(crate) fn cache_html_shell(&self, shell: Arc<HtmlShell>) {
        *self
            .inner
            .html_shell
            .write()
            .expect("html shell lock poisoned") = Some(shell);
    }

    /// Next document request re-fetches. Called by `POST /api/html/reload`
    /// after the static host gets a new `index.html`.
    pub(crate) fn clear_html_shell(&self) {
        *self
            .inner
            .html_shell
            .write()
            .expect("html shell lock poisoned") = None;
    }

    /// `None` once the entry has aged past [`SITE_META_TTL`].
    pub(crate) fn site_meta(&self) -> Option<Arc<SiteMeta>> {
        self.inner
            .site_meta
            .read()
            .expect("site meta lock poisoned")
            .as_ref()
            .filter(|(cached_at, _)| cached_at.elapsed() < SITE_META_TTL)
            .map(|(_, site)| site.clone())
    }

    pub(crate) fn cache_site_meta(&self, site: Arc<SiteMeta>) {
        *self
            .inner
            .site_meta
            .write()
            .expect("site meta lock poisoned") = Some((Instant::now(), site));
    }

    /// Called after `PATCH /admin/site` so an operator's edit shows up on the
    /// next reload instead of within [`SITE_META_TTL`].
    pub(crate) fn clear_site_meta(&self) {
        *self
            .inner
            .site_meta
            .write()
            .expect("site meta lock poisoned") = None;
    }
}
