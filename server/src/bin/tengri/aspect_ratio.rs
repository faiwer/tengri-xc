use std::path::PathBuf;

use anyhow::Context;
use tengri_server::{
    flight::{
        find_flight_window,
        ingest::{parse_input, slice_flight_window},
    },
    geo::track_aspect_ratio,
};

pub fn run(input: PathBuf) -> anyhow::Result<()> {
    let track = parse_input(&input).with_context(|| format!("parsing {}", input.display()))?;
    let window = find_flight_window(&track).context("detecting flight window")?;
    let track = slice_flight_window(track, window);

    match track_aspect_ratio(&track) {
        Some(ratio) => println!("{ratio}"),
        None => println!("none"),
    }

    Ok(())
}
