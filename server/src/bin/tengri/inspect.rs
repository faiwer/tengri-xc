//! `tengri inspect` — peek inside a `.tengri` envelope without unpacking it.

use std::{fs::File, io::BufReader, path::PathBuf};

use anyhow::Context;
use tengri_server::flight::{
    TengriFile,
    compact::{TasBody, TrackBody},
};

pub fn run(input: PathBuf) -> anyhow::Result<()> {
    let f = File::open(&input).with_context(|| format!("opening {}", input.display()))?;
    let envelope = TengriFile::read(BufReader::new(f))
        .with_context(|| format!("reading {}", input.display()))?;

    let body = match &envelope.track.track {
        TrackBody::Gps { fixes, coords } => {
            format!("Gps  {} fixes, {} coords", fixes.len(), coords.len())
        }
        TrackBody::Dual { fixes, coords } => {
            format!("Dual {} fixes, {} coords", fixes.len(), coords.len())
        }
    };

    let tas = match &envelope.track.tas {
        TasBody::None => String::from("None"),
        TasBody::Tas { fixes, deltas } => {
            format!("Tas  {} fixes, {} deltas", fixes.len(), deltas.len())
        }
    };

    println!("file        {}", input.display());
    println!("start_time  {}", envelope.track.start_time);
    println!("interval    {} s", envelope.track.interval);
    println!("body        {body}");
    println!("time_fixes  {}", envelope.track.time_fixes.len());
    println!("tas         {tas}");
    Ok(())
}
