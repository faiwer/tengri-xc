//! `tengri` — flight-file tooling.
//!
//! Subcommands:
//! - `convert` — parse a flight log (IGC today; later GPX/KML) and write a
//!   `.tengri` envelope.
//! - `inspect` — peek inside a `.tengri` envelope without unpacking it.
//! - `add` — ingest a flight log into the database for a given user: gzipped
//!   source goes into `flight_sources`; the encoded `.tengri` HTTP wire form
//!   goes into `flight_tracks` (kind = `full`).
//! - `delete` — remove a flight by id (cascades to its track + source rows).
//! - `upgrade-tracks` — re-encode every `flight_tracks` row whose `version`
//!   lags behind the current build, sourcing the original bytes from
//!   `flight_sources`.
//!
//! Each subcommand lives in its own sibling module; cross-cutting helpers
//! (format detection, gzip, NanoID, Postgres connection) are in `shared`.

mod add;
mod convert;
mod delete;
mod inspect;
mod shared;
mod upgrade;

use std::{path::PathBuf, process};

use clap::{Parser, Subcommand};

use shared::run_async;

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

    /// Ingest a flight log into the database under the given user.
    /// Inserts a `flights` row, the gzipped source into `flight_sources`,
    /// and the encoded HTTP wire form into `flight_tracks` (kind = `full`).
    /// All three writes happen in a single transaction; on failure nothing
    /// is committed.
    Add {
        /// Input flight log (.igc).
        input: PathBuf,

        /// Owning user id (`users.id`). The user must already exist.
        #[arg(long = "user-id")]
        user_id: i32,
    },

    /// Delete a flight by id. Cascades to `flight_tracks` and
    /// `flight_sources` via the schema's `ON DELETE CASCADE`.
    Delete {
        /// Flight id to delete (`flights.id`, an 8-char NanoID).
        #[arg(long = "flight-id")]
        flight_id: String,
    },

    /// Re-encode every `flight_tracks` row whose `version` lags behind the
    /// current build. The fresh bytes are derived from the matching
    /// `flight_sources` row (we can't re-decode the stale blob — the wire
    /// format changed, that's the whole reason for the upgrade).
    UpgradeTracks {
        /// Print what would change without writing to the database.
        #[arg(long)]
        dry_run: bool,
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
        Cmd::Convert { input, output } => convert::run(input, output),
        Cmd::Inspect { input } => inspect::run(input),
        Cmd::Add { input, user_id } => run_async(add::run(input, user_id)),
        Cmd::Delete { flight_id } => run_async(delete::run(flight_id)),
        Cmd::UpgradeTracks { dry_run } => run_async(upgrade::run(dry_run)),
    }
}
