//! `tengri` — flight-file tooling.
//!
//! Currently a single subcommand: `convert`, which parses a flight log
//! (today: IGC; later: GPX, KML, …) and writes a `.tengri` envelope holding
//! the compact track plus a sibling metadata block.

use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
    process,
};

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand};
use tengri_server::flight::{Metadata, TengriFile, Track, encode, parse_str};

#[derive(Parser)]
#[command(name = "tengri", version, about = "Tengri-XC flight tooling")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Convert a flight log into a `.tengri` envelope.
    Convert {
        /// Input file (.igc).
        input: PathBuf,
        /// Output path. Defaults to `<input>.tengri`.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Inspect a `.tengri` envelope without unpacking it.
    Inspect {
        /// `.tengri` file to read.
        input: PathBuf,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Convert { input, output } => convert(input, output),
        Cmd::Inspect { input } => inspect(input),
    }
}

fn convert(input: PathBuf, output: Option<PathBuf>) -> anyhow::Result<()> {
    let track = parse_input(&input)?;
    let n_points = track.points.len();

    let compact = encode(&track).context("encoding compact track")?;
    let envelope = TengriFile::new(Metadata::default(), compact);

    let output = output.unwrap_or_else(|| input.with_extension("tengri"));
    let f = File::create(&output).with_context(|| format!("creating {}", output.display()))?;
    envelope
        .write(BufWriter::new(f))
        .with_context(|| format!("writing {}", output.display()))?;

    let in_size = std::fs::metadata(&input)?.len();
    let out_size = std::fs::metadata(&output)?.len();
    let ratio = in_size as f64 / out_size as f64;

    println!(
        "{} → {}  ({} points, {} → {} bytes, {:.1}×)",
        input.display(),
        output.display(),
        n_points,
        in_size,
        out_size,
        ratio,
    );
    Ok(())
}

fn inspect(input: PathBuf) -> anyhow::Result<()> {
    let f = File::open(&input).with_context(|| format!("opening {}", input.display()))?;
    let envelope = TengriFile::read(BufReader::new(f))
        .with_context(|| format!("reading {}", input.display()))?;

    let body = match &envelope.track.track {
        tengri_server::flight::compact::TrackBody::Gps { fixes, coords } => {
            format!("Gps  {} fixes, {} coords", fixes.len(), coords.len())
        }
        tengri_server::flight::compact::TrackBody::Dual { fixes, coords } => {
            format!("Dual {} fixes, {} coords", fixes.len(), coords.len())
        }
    };

    println!("file        {}", input.display());
    println!("start_time  {}", envelope.track.start_time);
    println!("interval    {} s", envelope.track.interval);
    println!("body        {body}");
    println!("time_fixes  {}", envelope.track.time_fixes.len());
    Ok(())
}

fn parse_input(input: &PathBuf) -> anyhow::Result<Track> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());

    match ext.as_deref() {
        Some("igc") => {
            let raw = std::fs::read_to_string(input)
                .with_context(|| format!("reading {}", input.display()))?;
            Ok(parse_str(&raw).context("parsing IGC")?)
        }
        Some(other) => Err(anyhow!("unsupported input format: .{other}")),
        None => Err(anyhow!(
            "input has no extension; cannot detect format: {}",
            input.display()
        )),
    }
}
