//! `tengri` — flight-file tooling.
//!
//! Subcommands:
//! - `convert` — parse a flight log (IGC, KML, KMZ, GPX) and write a `.tengri`
//!   envelope.
//! - `inspect` — peek inside a `.tengri` envelope without unpacking it.
//! - `add` — ingest a flight log into the database for a given user: gzipped
//!   source goes into `flight_sources`; the encoded `.tengri` HTTP wire form
//!   goes into `flight_tracks` (kind = `full`).
//! - `delete` — remove a flight by id (cascades to its track + source rows).
//! - `migrate` — apply outstanding SQL migrations to the configured DB, then
//!   run any Rust-side data backfills that depend on those schema changes (e.g.
//!   re-encoding `.tengri` blobs after a version bump).
//! - `prune` — wipe every data row from the configured DB (keeping the schema
//!   intact). Useful for resetting between Leonardo imports.
//! - `import-gliders` — load `brands` + `glider_models` from one JSON
//!   dictionary (`--kind=<hg|pg|sp> --file=<path>`). One invocation per kind;
//!   idempotent, UPSERT-based.
//! - `db` — open psql against the configured database (or run a one-shot query
//!   via `tengri db -- -c 'SELECT …'`).
//!
//! Each subcommand lives in its own sibling module; cross-cutting helpers
//! (format detection, gzip, NanoID, Postgres connection) are in `shared`.

mod add;
mod convert;
mod db;
mod delete;
mod import_gliders;
mod inspect;
mod migrate;
mod prune;
mod shared;

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
        /// Input file (.igc, .kml, .kmz, .gpx).
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
        /// Input flight log (.igc, .kml, .kmz, .gpx).
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

    /// Apply outstanding SQL migrations from `server/migrations/`, then
    /// run any Rust-side data backfills that depend on those schema
    /// changes (e.g. re-encoding `.tengri` blobs after a `VERSION`
    /// bump). The HTTP server runs the same code path on startup; this
    /// subcommand is for migrating without booting the server (e.g.
    /// after a manual schema reset).
    Migrate,

    /// Wipe every data row from the database while keeping the schema
    /// intact. Truncates `users`, `flights`, `flight_tracks`,
    /// `flight_sources` (cascading) and resets identity sequences.
    /// Pass `--yes` to skip the confirmation prompt.
    Prune {
        /// Skip the interactive confirmation. Use in scripts/CI.
        #[arg(long)]
        yes: bool,
    },

    /// Open psql against the configured database. Anything after `--` is
    /// forwarded verbatim to psql, so:
    ///   tengri db                            # interactive shell
    ///   tengri db -- -c 'SELECT 1;'          # one-shot query
    ///   tengri db -- -f script.sql           # run a script
    Db {
        /// Forwarded directly to `psql`. Use `--` to separate from clap's flags.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Load brands + canonical glider models for one kind (`hg`, `pg`, or
    /// `sp`) from a JSON dictionary file. Run once per kind. Idempotent —
    /// re-running picks up JSON edits; `class` / `is_tandem` changes fan
    /// out to existing `gliders` rows via the `sync_glider_denorm` trigger
    /// from migration `0009`.
    ImportGliders {
        /// Glider kind the file describes.
        #[arg(long, value_parser = ["hg", "pg", "sp"])]
        kind: String,
        /// Path to the JSON dictionary for this kind.
        #[arg(long)]
        file: PathBuf,
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
        Cmd::Migrate => run_async(migrate::run()),
        Cmd::Prune { yes } => run_async(prune::run(yes)),
        Cmd::Db { args } => db::run(args),
        Cmd::ImportGliders { kind, file } => run_async(import_gliders::run(kind, file)),
    }
}
