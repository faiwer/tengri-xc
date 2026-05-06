use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Shared application state. Cheap to clone (everything inside an `Arc`).
///
/// Grow this by adding fields like a database pool, HTTP client, config snapshot,
/// metrics handles, etc. Keep them wrapped in `Arc<...>` so cloning stays O(1).
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    request_counter: AtomicU64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                request_counter: AtomicU64::new(1),
            }),
        }
    }

    /// Monotonically increasing request id. Useful as a placeholder until a real
    /// id source (UUID, request-scoped span, etc.) is wired in.
    pub fn next_request_id(&self) -> u64 {
        self.inner.request_counter.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
