use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initialize the global tracing subscriber.
///
/// Honors the `RUST_LOG` env var (e.g. `tengri_server=debug,tower_http=info`).
/// Falls back to `info` when unset/invalid so production deploys without an
/// explicit filter still emit useful logs.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true).with_level(true))
        .init();
}
