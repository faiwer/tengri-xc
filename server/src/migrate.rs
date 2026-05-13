//! Wrapper around `sqlx::Migrator::run` that turns the otherwise-cryptic
//! "migration N was previously applied but has been modified" into an
//! actionable message: both checksums in hex plus the copy-pasteable SQL to
//! reseat the recorded value when the diff is cosmetic.
//!
//! Every other migration error falls through unchanged.

use std::fmt::Write;

use anyhow::Context;
use sqlx::PgPool;
use sqlx::migrate::{MigrateError, Migrator};

/// Apply pending migrations, intercepting checksum-mismatch errors to print
/// both hashes and a reseat command. Returns the original `anyhow::Error` so
/// the caller's exit-code behaviour is unchanged.
pub async fn apply(migrator: &Migrator, pool: &PgPool) -> anyhow::Result<()> {
    match migrator.run(pool).await {
        Ok(()) => Ok(()),
        Err(MigrateError::VersionMismatch(version)) => {
            print_mismatch(migrator, pool, version).await;
            Err(anyhow::anyhow!(
                "migration {version} checksum mismatch (see details above)"
            ))
        }
        Err(e) => Err(anyhow::Error::new(e)).context("running migrations"),
    }
}

async fn print_mismatch(migrator: &Migrator, pool: &PgPool, version: i64) {
    let expected = migrator
        .iter()
        .find(|m| m.version == version)
        .map(|m| hex(&m.checksum))
        .unwrap_or_else(|| "<not in binary>".to_string());

    let recorded: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = $1")
            .bind(version)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let recorded = recorded
        .as_deref()
        .map(hex)
        .unwrap_or_else(|| "<not recorded>".to_string());

    eprintln!(
        "\n\
─── migration {version} ──────────────────────────────────────────────────\n\
The on-disk migration file's SHA-384 differs from what was recorded in\n\
`_sqlx_migrations` when this DB was first migrated to version {version}.\n\
\n\
  current file:  {expected}\n\
  DB recorded:   {recorded}\n\
\n\
If the diff is cosmetic (comments / whitespace / formatting) and the\n\
schema already in the DB matches the current SQL, reseat the checksum:\n\
\n\
  UPDATE _sqlx_migrations SET checksum = decode(\n\
      '{expected}', 'hex'\n\
  ) WHERE version = {version};\n\
\n\
If the SQL itself diverged, add a new migration instead. See\n\
.cursor/rules/migrations.mdc for the recovery policy.\n\
─────────────────────────────────────────────────────────────────────────\n\
"
    );
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(&mut s, "{:02x}", b).expect("write to String");
    }
    s
}
