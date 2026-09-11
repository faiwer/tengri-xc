//! `tengri oauth` — read and write `oauth_provider_settings` and the per-user
//! links that hang off it, for local/dev/test data setup.

use anyhow::{Context, anyhow};
use clap::Subcommand;
use tengri_server::oauth::{
    LinkOutcome, OAuthIdentity, OAuthProvider, UpdateOAuthProviderRequest,
    apply_oauth_provider_update, fetch_oauth_providers_admin, upsert_link,
    validate_oauth_provider_update,
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

    /// Attach a provider identity to an existing user, as a completed link
    /// flow would.
    ///
    ///   tengri oauth link --login pilot --provider google --subject 12345
    Link {
        /// Login of the user to attach the identity to.
        #[arg(long)]
        login: String,

        /// One of `google`, `facebook`, `x`, `microsoft`, `github`.
        #[arg(long)]
        provider: String,

        /// Provider-side user id (`sub`) — what the link is keyed on.
        #[arg(long)]
        subject: String,

        /// Address snapshot, as the provider reported it.
        #[arg(long)]
        email: Option<String>,

        /// Display-name snapshot, as the provider reported it.
        #[arg(long)]
        name: Option<String>,
    },
}

pub async fn run(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Show => show().await,
        Cmd::Set { provider, patch } => set(&provider, &patch).await,
        Cmd::Link {
            login,
            provider,
            subject,
            email,
            name,
        } => {
            link(LinkArgs {
                login,
                provider,
                subject,
                email,
                name,
            })
            .await
        }
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

struct LinkArgs {
    login: String,
    provider: String,
    subject: String,
    email: Option<String>,
    name: Option<String>,
}

async fn link(args: LinkArgs) -> anyhow::Result<()> {
    let provider = parse_provider(&args.provider)?;
    let identity = OAuthIdentity {
        subject: args.subject,
        email: args.email,
        display_name: args.name,
    };

    let pool = connect_pool().await?;
    let user_id: Option<i32> =
        sqlx::query_scalar("SELECT id FROM users WHERE LOWER(login) = LOWER($1)")
            .bind(&args.login)
            .fetch_optional(&pool)
            .await?;
    let user_id = user_id.ok_or_else(|| anyhow!("no user with login {}", args.login))?;

    let mut conn = pool.acquire().await?;
    match upsert_link(&mut conn, user_id, provider, &identity)
        .await
        .map_err(anyhow::Error::new)?
    {
        LinkOutcome::TakenByOther => Err(anyhow!(
            "{} subject {} is already linked to another user",
            provider.pg_enum_value(),
            identity.subject
        )),
        LinkOutcome::Created | LinkOutcome::Refreshed => {
            println!(
                "linked {} to user {user_id} ({})",
                provider.pg_enum_value(),
                identity.subject
            );
            Ok(())
        }
    }
}

async fn set(provider: &str, patch: &str) -> anyhow::Result<()> {
    let provider = parse_provider(provider)?;
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

fn parse_provider(provider: &str) -> anyhow::Result<OAuthProvider> {
    OAuthProvider::from_path(provider).ok_or_else(|| anyhow!("unknown provider {provider}"))
}
