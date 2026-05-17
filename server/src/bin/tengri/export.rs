//! `tengri export` — parse a stored source flight file and write another format.

use std::{
    fs::File,
    io::{ErrorKind, Write},
    path::PathBuf,
};

use anyhow::Context;
use clap::ValueEnum;
use tengri_server::flight::{Track, igc, store::fetch_source_track};

use super::shared::connect_pool;

pub async fn run(
    flight_id: String,
    format: ExportFormat,
    destination: Option<PathBuf>,
) -> anyhow::Result<()> {
    let pool = connect_pool().await?;
    let track = fetch_source_track(&pool, &flight_id).await?;

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
