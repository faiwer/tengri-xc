//! The `oauth_provider` Postgres enum (`0024_user_oauth_links.sql`) as a
//! Rust type. Shared by the admin provider-settings store here and, later,
//! the per-user link flow.

use serde::{Deserialize, Serialize};

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
}
