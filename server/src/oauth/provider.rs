//! The `oauth_provider` Postgres enum (`0024_user_oauth_links.sql`) as a Rust
//! type, plus each provider's endpoints and userinfo parsing.

use serde::{Deserialize, Serialize};

use crate::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "oauth_provider", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Google,
    Facebook,
    X,
    Microsoft,
    Github,
}

/// Who a configured provider is offered to (`oauth_visibility` enum,
/// `0026_oauth_provider_visibility.sql`). `Admins` gates the provider behind
/// `MANAGE_USERS`; `Public` offers it to everyone; `Disabled` hides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "oauth_visibility", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum OAuthVisibility {
    Disabled,
    Admins,
    Public,
}

impl OAuthVisibility {
    /// String accepted by the `oauth_visibility` Postgres enum, for binding a
    /// `&str` + `::oauth_visibility` cast. Mirrors [`OAuthProvider::pg_enum_value`].
    pub fn pg_enum_value(self) -> &'static str {
        match self {
            OAuthVisibility::Disabled => "disabled",
            OAuthVisibility::Admins => "admins",
            OAuthVisibility::Public => "public",
        }
    }
}

/// Fixed OAuth 2.0 endpoints + scopes for a provider. Static because these are
/// public, well-known URLs — only `client_id`/`client_secret` are per-install
/// (those live in `oauth_provider_settings`).
pub struct ProviderEndpoints {
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub userinfo_url: &'static str,
    /// Scopes requested at authorize time. Kept minimal: a stable subject id
    /// plus (where the provider offers it) an email + display name for the
    /// link snapshot.
    pub scopes: &'static [&'static str],
}

/// Normalized identity extracted from a provider's userinfo response. `subject`
/// is the stable provider id (never email); `email`/`display_name` are
/// display-only snapshots and may be absent (X gives no email, some omit name).
pub struct OAuthIdentity {
    pub subject: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
}

impl OAuthProvider {
    /// String accepted by the `oauth_provider` Postgres enum. Same shape as
    /// [`UserSex::pg_enum_value`](crate::user::UserSex::pg_enum_value): keeps
    /// binding sites from having to know `sqlx::Type`'s rename rules, and lets
    /// us bind a `&str` + `::oauth_provider` cast where the driver can't infer
    /// the enum from a generic bind.
    pub fn pg_enum_value(self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::Facebook => "facebook",
            OAuthProvider::X => "x",
            OAuthProvider::Microsoft => "microsoft",
            OAuthProvider::Github => "github",
        }
    }

    /// Parse a `provider` path segment back into the enum, for
    /// `/oauth/:provider/...` routes. Mirrors [`pg_enum_value`](Self::pg_enum_value).
    pub fn from_path(s: &str) -> Option<Self> {
        Some(match s {
            "google" => OAuthProvider::Google,
            "facebook" => OAuthProvider::Facebook,
            "x" => OAuthProvider::X,
            "microsoft" => OAuthProvider::Microsoft,
            "github" => OAuthProvider::Github,
            _ => return None,
        })
    }

    /// Authorize / token / userinfo URLs + scopes per provider.
    pub fn endpoints(self) -> ProviderEndpoints {
        match self {
            OAuthProvider::Google => ProviderEndpoints {
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
                token_url: "https://oauth2.googleapis.com/token",
                userinfo_url: "https://openidconnect.googleapis.com/v1/userinfo",
                scopes: &["openid", "email", "profile"],
            },
            OAuthProvider::Facebook => ProviderEndpoints {
                auth_url: "https://www.facebook.com/v19.0/dialog/oauth",
                token_url: "https://graph.facebook.com/v19.0/oauth/access_token",
                userinfo_url: "https://graph.facebook.com/me?fields=id,name,email",
                scopes: &["email", "public_profile"],
            },
            OAuthProvider::X => ProviderEndpoints {
                auth_url: "https://twitter.com/i/oauth2/authorize",
                token_url: "https://api.twitter.com/2/oauth2/token",
                userinfo_url: "https://api.twitter.com/2/users/me",
                scopes: &["tweet.read", "users.read"],
            },
            OAuthProvider::Microsoft => ProviderEndpoints {
                auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
                token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
                userinfo_url: "https://graph.microsoft.com/oidc/userinfo",
                scopes: &["openid", "email", "profile"],
            },
            OAuthProvider::Github => ProviderEndpoints {
                auth_url: "https://github.com/login/oauth/authorize",
                token_url: "https://github.com/login/oauth/access_token",
                userinfo_url: "https://api.github.com/user",
                scopes: &["read:user", "user:email"],
            },
        }
    }

    /// Map a provider's userinfo JSON to the subject + display name. Returns
    /// `None` when the stable subject id is missing/blank — without it there's
    /// no identity to link against. Email is resolved separately by
    /// [`resolve_email`](Self::resolve_email), which is the only place trust is
    /// judged.
    pub fn parse_userinfo(self, body: &serde_json::Value) -> Option<(String, Option<String>)> {
        // X nests everything under `data`; everyone else is flat.
        let root = match self {
            OAuthProvider::X => body.get("data")?,
            _ => body,
        };

        let subject = match self {
            // OIDC providers use `sub`; GitHub a numeric `id`; X/Facebook `id`.
            OAuthProvider::Google | OAuthProvider::Microsoft => str_field(root, "sub"),
            OAuthProvider::Github => root.get("id").and_then(json_id_to_string),
            OAuthProvider::Facebook | OAuthProvider::X => str_field(root, "id"),
        };
        let subject = subject.filter(|s| !s.is_empty())?;

        let display_name = match self {
            OAuthProvider::Github => str_field(root, "name").or_else(|| str_field(root, "login")),
            OAuthProvider::X => str_field(root, "name").or_else(|| str_field(root, "username")),
            _ => str_field(root, "name"),
        };

        Some((subject, display_name))
    }

    /// Resolve a *trustworthy* email for this identity, or `None`. Auto-linking
    /// an OAuth identity to an existing account by email is an account-takeover
    /// vector when the address is unverified, so an untrusted email is dropped
    /// here rather than carried downstream: a `Some` from this fn is always a
    /// verified address.
    ///
    /// Only GitHub needs the network (its userinfo email is unverified; the
    /// verified set lives behind `/user/emails`); the rest answer from `body`.
    pub async fn resolve_email(
        self,
        http: &reqwest::Client,
        access_token: &str,
        body: &serde_json::Value,
    ) -> Result<Option<String>, AppError> {
        Ok(match self {
            // OIDC: trust the email only when the provider asserts it. Microsoft
            // omits `email_verified` on `oidc/userinfo` → `None` → safe.
            OAuthProvider::Google | OAuthProvider::Microsoft => {
                if body.get("email_verified").and_then(|v| v.as_bool()) == Some(true) {
                    str_field(body, "email")
                } else {
                    None
                }
            }
            // Graph only returns `email` once it's confirmed.
            OAuthProvider::Facebook => str_field(body, "email"),
            OAuthProvider::Github => github_verified_email(http, access_token).await?,
            // We don't request an email scope for X.
            OAuthProvider::X => None,
        })
    }
}

/// GitHub's primary email, but only when GitHub marks it verified. Reads
/// `/user/emails` (covered by our `user:email` scope) rather than the
/// unverified public email on `/user`.
async fn github_verified_email(
    http: &reqwest::Client,
    access_token: &str,
) -> Result<Option<String>, AppError> {
    #[derive(Deserialize)]
    struct GithubEmail {
        email: String,
        primary: bool,
        verified: bool,
    }

    let emails: Vec<GithubEmail> = http
        .get("https://api.github.com/user/emails")
        .bearer_auth(access_token)
        .header(reqwest::header::ACCEPT, "application/json")
        // GitHub rejects requests without a User-Agent.
        .header(reqwest::header::USER_AGENT, "tengri-xc")
        .send()
        .await
        .map_err(into_internal)?
        .error_for_status()
        .map_err(into_internal)?
        .json()
        .await
        .map_err(into_internal)?;

    Ok(emails
        .iter()
        .find(|e| e.primary && e.verified)
        .map(|e| e.email.clone()))
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}

/// A trimmed non-empty string field, or `None`.
fn str_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// GitHub's `id` is a JSON number; accept either number or string form.
fn json_id_to_string(value: &serde_json::Value) -> Option<String> {
    if let Some(n) = value.as_i64() {
        Some(n.to_string())
    } else {
        value.as_str().map(str::to_owned)
    }
}
