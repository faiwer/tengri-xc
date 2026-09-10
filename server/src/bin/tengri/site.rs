//! `tengri site` — read and write the `site_settings` singleton for
//! local/dev/test data setup.

use anyhow::Context;
use clap::Subcommand;
use tengri_server::site::{
    UpdateSiteRequest, apply_site_update, fetch_site_admin, validate_site_update,
};

use super::shared::connect_pool;

#[derive(Subcommand)]
pub enum Cmd {
    /// Print the full admin view of the settings as JSON.
    Show,

    /// Patch the settings from a JSON object shaped like the body of
    /// `PATCH /admin/site` — absent keys are left alone, `null` clears.
    ///
    ///   tengri site set '{"can_register":true}'
    ///   tengri site set '{"smtp_host":"127.0.0.1","smtp_port":1025,"smtp_tls":"none"}'
    Set {
        /// JSON object of fields to change.
        patch: String,
    },
}

pub async fn run(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Show => show().await,
        Cmd::Set { patch } => set(&patch).await,
    }
}

async fn show() -> anyhow::Result<()> {
    let pool = connect_pool().await?;
    let settings = fetch_site_admin(&pool).await.map_err(anyhow::Error::new)?;
    println!("{}", serde_json::to_string_pretty(&settings)?);
    Ok(())
}

async fn set(patch: &str) -> anyhow::Result<()> {
    let request: UpdateSiteRequest =
        serde_json::from_str(patch).context("parsing the patch as a site-settings JSON object")?;
    let update = validate_site_update(request)
        .map_err(|errors| anyhow::anyhow!("invalid settings: {errors:?}"))?;

    let pool = connect_pool().await?;
    apply_site_update(&pool, &update)
        .await
        .map_err(anyhow::Error::new)?;

    println!("site settings updated");
    Ok(())
}
