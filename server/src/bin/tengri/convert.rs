//! `tengri convert` — parse a flight log and write a `.tengri` envelope.

use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::Context;
use tengri_server::flight::{Metadata, TengriFile, encode};

use super::shared::parse_input;

pub fn run(input: PathBuf, output: Option<PathBuf>) -> anyhow::Result<()> {
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
