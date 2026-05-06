use std::sync::Arc;

use sqlx::PgPool;

/// Shared application state. Cheap to clone (everything inside an `Arc`).
///
/// Grow this by adding fields like an HTTP client, config snapshot, metrics
/// handles, etc. Keep them wrapped in `Arc<...>` so cloning stays O(1).
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    pool: PgPool,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            inner: Arc::new(AppStateInner { pool }),
        }
    }

    /// Postgres connection pool. Cloning the pool is cheap (it's an `Arc`
    /// internally), so callers can take a `&PgPool` from here freely.
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }
}
