use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tengri_server::{AppState, Config, build_app, migrate, telemetry};
use tokio::{net::TcpListener, signal};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load `<crate-root>/.env` if present. We resolve the path at compile time
    // via `CARGO_MANIFEST_DIR` so the binary finds its dev `.env` regardless of
    // the process's working directory (e.g. running it from the repo root, or
    // from `cargo test` invoked elsewhere). Missing file is not an error:
    // production deploys inject env vars directly.
    let _ = dotenvy::from_filename(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));

    telemetry::init();

    let config = Config::from_env().context("loading config from environment")?;

    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&config.database_url)
        .await
        .context("connecting to Postgres")?;
    tracing::info!("postgres pool ready");

    migrate::apply(&sqlx::migrate!("./migrations"), &pool).await?;
    tracing::info!("migrations applied");

    let backfilled = tengri_server::flight::backfill::run(&pool)
        .await
        .context("backfilling flights")?;
    if backfilled > 0 {
        tracing::info!(n = backfilled, "flights backfilled");
    }

    let state = AppState::with_origins(
        pool,
        &config.jwt_secret,
        config.https,
        config.client_origins.clone(),
    );
    let app = build_app(state);

    let listener = TcpListener::bind(config.server_addr)
        .await
        .with_context(|| format!("binding to {}", config.server_addr))?;

    let local_addr = listener.local_addr().context("reading local addr")?;
    tracing::info!(%local_addr, "server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    tracing::info!("server stopped cleanly");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl-C, shutting down"),
        _ = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}
