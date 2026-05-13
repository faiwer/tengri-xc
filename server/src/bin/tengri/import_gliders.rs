//! `tengri import-gliders --kind=<hg|pg|sp> --file=<path>` — load brands +
//! canonical glider models for one kind from a JSON dictionary. Thin wrapper
//! around `tengri_server::glider::import::run`; the real logic lives in the
//! library so the Leonardo flight importer can call the same function.
//!
//! One invocation, one file. Run once per kind to seed each catalogue.

use std::path::PathBuf;

use anyhow::Context;
use tengri_server::glider::import;

use super::shared::connect_pool;

pub async fn run(kind: String, file: PathBuf) -> anyhow::Result<()> {
    let json =
        std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;

    let pool = connect_pool().await?;
    let s = import::run(&pool, &json, &kind)
        .await
        .with_context(|| format!("importing {kind} gliders from {}", file.display()))?;

    println!(
        "imported {} brands ({} new, {} updated)",
        s.brands_total, s.brands_new, s.brands_updated
    );
    println!(
        "imported {} {} models ({} new, {} updated, {} tandem)",
        s.models_total, kind, s.models_new, s.models_updated, s.models_tandem
    );
    Ok(())
}
