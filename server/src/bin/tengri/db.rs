//! `tengri db` — open `psql` against the configured database, forwarding
//! any extra arguments through. The connection string is read from
//! `server/.env` (`DATABASE_URL`), so a bare `tengri db` opens an
//! interactive shell against the same database the server uses.
//!
//! Examples:
//!   tengri db                                      # interactive psql
//!   tengri db -- -c 'SELECT id FROM users;'        # one-shot query
//!   tengri db -- -f migrations/0001_init.sql       # run a script
//!
//! We deliberately don't reimplement query/format/repl logic in Rust:
//! psql is the right tool. This subcommand exists so the connection
//! string lives in one place (`server/.env`) and so the agent's
//! allowlist can target a single, narrow command (`tengri db ...`)
//! instead of an open-ended `psql ...`.

use std::process::Command;

use anyhow::{Context, anyhow};

use super::shared::database_url;

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let url = database_url()?;
    let status = Command::new("psql")
        .arg(&url)
        .args(&args)
        .status()
        .context("spawning psql (is it on PATH?)")?;
    if !status.success() {
        return Err(anyhow!(
            "psql exited with {}",
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "<no code>".into())
        ));
    }
    Ok(())
}
