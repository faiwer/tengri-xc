//! `leonardo` — one-way importer from a Leonardo XC MySQL database into
//! our own platform.
//!
//! The binary will eventually grow a tree of subcommands for users,
//! flights, comments, photos, etc. For now the only command is
//! `check-db`, which validates that we can reach the configured MySQL
//! instance and prints a small summary of what's in there. Use it as a
//! smoke test before running anything destructive.
//!
//! Connection strings come from `server/.env` — same file the rest of
//! the workspace uses. The Leonardo source MySQL lives under
//! `LEONARDO_MYSQL_URL`; `DATABASE_URL` keeps pointing at our Postgres.

mod check_db;
mod shared;

use std::process;

use clap::{Parser, Subcommand};

use shared::run_async;

#[derive(Parser)]
#[command(
    name = "leonardo",
    version,
    about = "Import data from a Leonardo XC MySQL database into Tengri-XC"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Connect to the configured Leonardo MySQL and print a short
    /// summary (server version, table count, flights/pilots row counts).
    /// Returns non-zero if the connection or any of the probe queries
    /// fail, which makes it usable as a CI/healthcheck step.
    CheckDb,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::CheckDb => run_async(check_db::run()),
    }
}
