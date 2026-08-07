//! DB reads + writes for `oauth_provider_settings`. Unlike `site_settings` (one
//! seeded singleton), rows here are created on demand: a provider gains a row
//! the first time an admin saves both credentials for it, and the read side
//! returns only the rows that exist. The client owns the canonical provider
//! list and fills the gaps for ones not configured yet.

use serde::Deserialize;

use crate::{AppError, db::Upsert, validation::FieldErrors};

use super::{dto::AdminOAuthProviderDto, provider::OAuthProvider};

/// Length cap on `client_id` / `client_secret`. Real values are well under
/// this; the cap just stops a paste-bomb from landing in the row.
const CREDENTIAL_MAX_LEN: usize = 512;

/// Fetch every configured provider (0..5 rows). Unconfigured providers are
/// simply absent — the FE renders empty forms for those from its own list.
pub async fn fetch_oauth_providers_admin(
    pool: &sqlx::PgPool,
) -> Result<Vec<AdminOAuthProviderDto>, AppError> {
    sqlx::query_as::<_, AdminOAuthProviderDto>(
        "SELECT provider, client_id, client_secret, enabled \
         FROM oauth_provider_settings \
         ORDER BY provider",
    )
    .fetch_all(pool)
    .await
    .map_err(into_internal)
}

/// Enabled providers, in display order — the set offered for login/link. Used
/// by the public `/oauth/providers` endpoint and to gate `start`/`callback`.
pub async fn fetch_enabled_providers(pool: &sqlx::PgPool) -> Result<Vec<OAuthProvider>, AppError> {
    let rows: Vec<(OAuthProvider,)> = sqlx::query_as(
        "SELECT provider FROM oauth_provider_settings WHERE enabled = TRUE ORDER BY provider",
    )
    .fetch_all(pool)
    .await
    .map_err(into_internal)?;
    Ok(rows.into_iter().map(|(p,)| p).collect())
}

/// Credentials for an *enabled* provider, or `None` when it's absent/disabled.
/// The flow requires enablement, so a disabled provider is indistinguishable
/// from an unconfigured one here.
pub async fn fetch_enabled_provider_credentials(
    pool: &sqlx::PgPool,
    provider: OAuthProvider,
) -> Result<Option<ProviderCredentials>, AppError> {
    sqlx::query_as::<_, ProviderCredentials>(
        "SELECT client_id, client_secret \
         FROM oauth_provider_settings \
         WHERE provider = $1::oauth_provider AND enabled = TRUE",
    )
    .bind(provider.pg_enum_value())
    .fetch_optional(pool)
    .await
    .map_err(into_internal)
}

/// `client_id` + `client_secret` for the authorization-code exchange.
#[derive(Debug, sqlx::FromRow)]
pub struct ProviderCredentials {
    pub client_id: String,
    pub client_secret: String,
}

/// PATCH body. Every field is optional: an absent (or empty-string) credential
/// means "leave unchanged", mirroring the password field in `admin/users.rs`,
/// so saving `enabled` alone doesn't wipe the stored secret.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateOAuthProviderRequest {
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// Validated update. Credentials are resolved to their final values (incoming
/// value, or the existing row's value when the field was left unchanged), so
/// `apply` can write a valid row in one UPSERT regardless of create-vs-edit.
#[derive(Debug)]
pub struct OAuthProviderUpdate {
    client_id: String,
    client_secret: String,
    /// `None` = leave `enabled` as-is (or default FALSE on create).
    enabled: Option<bool>,
}

/// Validate a PATCH against the current row.
///
/// - First-time config (no row): both credentials must be present — the pair
///   is required to create a row.
/// - Editing (row exists): either credential may be patched alone.
///
/// Because a created row always carries both credentials, enabling a provider
/// never needs a separate credential check: creds are guaranteed present the
/// moment a row exists.
pub async fn validate_oauth_provider_update(
    pool: &sqlx::PgPool,
    provider: OAuthProvider,
    input: UpdateOAuthProviderRequest,
) -> Result<OAuthProviderUpdate, AppError> {
    let client_id = normalise(input.client_id);
    let client_secret = normalise(input.client_secret);
    let enabled = input.enabled;

    if client_id.is_none() && client_secret.is_none() && enabled.is_none() {
        return Err(AppError::BadRequest(
            "PATCH body must include at least one settable field".into(),
        ));
    }

    let mut errors = FieldErrors::new();
    check_len(&mut errors, "client_id", client_id.as_deref());
    check_len(&mut errors, "client_secret", client_secret.as_deref());

    let existing: Option<(String, String)> = sqlx::query_as(
        "SELECT client_id, client_secret \
         FROM oauth_provider_settings \
         WHERE provider = $1::oauth_provider",
    )
    .bind(provider.pg_enum_value())
    .fetch_optional(pool)
    .await
    .map_err(into_internal)?;

    let final_client_id = client_id.or_else(|| existing.as_ref().map(|(id, _)| id.clone()));
    let final_client_secret =
        client_secret.or_else(|| existing.as_ref().map(|(_, secret)| secret.clone()));

    if final_client_id.is_none() {
        errors.add("client_id", "Set a client ID to configure this provider");
    }
    if final_client_secret.is_none() {
        errors.add(
            "client_secret",
            "Set a client secret to configure this provider",
        );
    }
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    Ok(OAuthProviderUpdate {
        client_id: final_client_id.expect("presence checked above"),
        client_secret: final_client_secret.expect("presence checked above"),
        enabled,
    })
}

/// Create or patch the provider's row. The resolved credentials are always
/// written (to the same value when unchanged, which the `updated_at` trigger's
/// `IS DISTINCT FROM` guard treats as a no-op), so the tentative INSERT always
/// satisfies the `NOT NULL` credential columns even on an enabled-only edit.
pub async fn apply_oauth_provider_update(
    pool: &sqlx::PgPool,
    provider: OAuthProvider,
    update: &OAuthProviderUpdate,
) -> Result<(), AppError> {
    let mut q = Upsert::into("oauth_provider_settings");
    q.value_cast("provider", provider.pg_enum_value(), "oauth_provider");
    q.value("client_id", update.client_id.as_str());
    q.value("client_secret", update.client_secret.as_str());
    if let Some(enabled) = update.enabled {
        q.value("enabled", enabled);
    }
    q.on_conflict("provider");
    q.update_excluded("client_id");
    q.update_excluded("client_secret");
    if update.enabled.is_some() {
        q.update_excluded("enabled");
    }

    q.execute(pool).await.map_err(into_internal)?;
    Ok(())
}

/// Trim and collapse an empty string to `None`. An empty credential from the
/// form is "unchanged", not "clear to empty" — the columns are `NOT NULL`.
fn normalise(value: Option<String>) -> Option<String> {
    value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn check_len(errors: &mut FieldErrors, field: &'static str, value: Option<&str>) {
    if let Some(v) = value
        && v.chars().count() > CREDENTIAL_MAX_LEN
    {
        errors.add(
            field,
            format!("Must be at most {CREDENTIAL_MAX_LEN} characters"),
        );
    }
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
