use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tengri_server::{AppState, Config, build_app, telemetry};
use tokio::{net::TcpListener, signal};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load `.env` if present. Missing file is not an error: production deploys
    // typically inject env vars directly.
    let _ = dotenvy::dotenv();

    telemetry::init();

    let config = Config::from_env().context("loading config from environment")?;

    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&config.database_url)
        .await
        .context("connecting to Postgres")?;
    tracing::info!("postgres pool ready");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("running migrations")?;
    tracing::info!("migrations applied");

    let state = AppState::new(pool);
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
