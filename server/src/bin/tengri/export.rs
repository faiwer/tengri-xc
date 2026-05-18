//! `tengri export` — parse a stored source flight file and write another format.

use std::{
    fs::File,
    io::{ErrorKind, Write},
    path::PathBuf,
};

use anyhow::Context;
use clap::ValueEnum;
use tengri_server::flight::{Track, igc, ingest::slice_time_range, store::fetch_source_track};

use super::shared::connect_pool;

pub async fn run(
    flight_id: String,
    format: ExportFormat,
    destination: Option<PathBuf>,
) -> anyhow::Result<()> {
    let pool = connect_pool().await?;
    let track = fetch_source_track(&pool, &flight_id).await?;
    let window = fetch_stored_window(&pool, &flight_id).await?;
    let track = slice_time_range(track, window.takeoff_at, window.landing_at)?;

    match destination {
        Some(path) => {
            let file =
                File::create(&path).with_context(|| format!("creating {}", path.display()))?;
            format.write(file, &track)?;
            println!("exported {flight_id} to {}", path.display());
        }
        None => {
            if let Err(e) = format.write(std::io::stdout().lock(), &track) {
                if e.kind() != ErrorKind::BrokenPipe {
                    return Err(e).context("writing exported flight to stdout");
                }
            }
        }
    }

    Ok(())
}

#[derive(sqlx::FromRow)]
struct StoredWindow {
    takeoff_at: i64,
    landing_at: i64,
}

async fn fetch_stored_window(pool: &sqlx::PgPool, flight_id: &str) -> anyhow::Result<StoredWindow> {
    sqlx::query_as::<_, StoredWindow>(
        "SELECT EXTRACT(EPOCH FROM takeoff_at)::bigint AS takeoff_at, \
                EXTRACT(EPOCH FROM landing_at)::bigint AS landing_at \
         FROM flights \
         WHERE id = $1",
    )
    .bind(flight_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("fetching stored window for flight {flight_id}"))?
    .ok_or_else(|| anyhow::anyhow!("no flight with id={flight_id}"))
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportFormat {
    Igc,
}

impl ExportFormat {
    fn write<W: Write>(self, writer: W, track: &Track) -> std::io::Result<()> {
        match self {
            ExportFormat::Igc => igc::write(writer, track),
        }
    }
}
