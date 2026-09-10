//! `tengri oauth` — read and write `oauth_provider_settings` for local/dev/test
//! data setup.

use anyhow::{Context, anyhow};
use clap::Subcommand;
use tengri_server::oauth::{
    OAuthProvider, UpdateOAuthProviderRequest, apply_oauth_provider_update,
    fetch_oauth_providers_admin, validate_oauth_provider_update,
};

use super::shared::connect_pool;

#[derive(Subcommand)]
pub enum Cmd {
    /// Print every configured provider as JSON.
    Show,

    /// Patch one provider from a JSON object shaped like the body of
    /// `PATCH /admin/oauth-providers/{provider}` — absent keys are left alone.
    ///
    ///   tengri oauth set google '{"client_id":"id","client_secret":"shh","visibility":"public"}'
    Set {
        /// One of `google`, `facebook`, `x`, `microsoft`, `github`.
        provider: String,

        /// JSON object of fields to change.
        patch: String,
    },
}

pub async fn run(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Show => show().await,
        Cmd::Set { provider, patch } => set(&provider, &patch).await,
    }
}

async fn show() -> anyhow::Result<()> {
    let pool = connect_pool().await?;
    let providers = fetch_oauth_providers_admin(&pool)
        .await
        .map_err(anyhow::Error::new)?;
    println!("{}", serde_json::to_string_pretty(&providers)?);
    Ok(())
}

async fn set(provider: &str, patch: &str) -> anyhow::Result<()> {
    let provider =
        OAuthProvider::from_path(provider).ok_or_else(|| anyhow!("unknown provider {provider}"))?;
    let request: UpdateOAuthProviderRequest =
        serde_json::from_str(patch).context("parsing the patch as a provider JSON object")?;

    let pool = connect_pool().await?;
    let update = validate_oauth_provider_update(&pool, provider, request)
        .await
        .map_err(anyhow::Error::new)?;
    apply_oauth_provider_update(&pool, provider, &update)
        .await
        .map_err(anyhow::Error::new)?;

    println!("{} settings updated", provider.pg_enum_value());
    Ok(())
}
