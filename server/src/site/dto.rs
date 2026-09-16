//! Wire shapes for site settings. Two DTOs because the public boot payload and
//! the admin editor have different visibility needs: the public one ships short
//! scalars + booleans, the admin one carries markdown and SMTP credentials.

use serde::{Deserialize, Serialize};

/// Returned by `GET /site` (unauthenticated). Loaded once per app boot.
/// `has_tos` / `has_privacy` drive footer-link visibility without paying for
/// the markdown bytes on every page load — the content is fetched on demand
/// from `/site/tos` / `/site/privacy` when a visitor navigates to the document
/// page.
#[derive(Debug, Serialize)]
pub struct SiteDto {
    pub site_name: String,
    pub can_register: bool,
    pub has_tos: bool,
    pub has_privacy: bool,
}

/// How the SMTP client speaks TLS (`smtp_tls` Postgres enum). `Implicit` is
/// SMTPS (typically 465); `Starttls` upgrades a plain connection (typically
/// 587); `None` is plaintext.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "smtp_tls", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum SmtpTls {
    Implicit,
    Starttls,
    None,
}

/// Returned by `GET /admin/site` and `PATCH /admin/site`. Includes markdown
/// and SMTP credentials so the operator editor can prefill. Sensitive fields
/// stay here, never on [`SiteDto`].
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminSiteDto {
    pub site_name: String,
    pub site_description: Option<String>,
    pub can_register: bool,
    pub tos_md: Option<String>,
    pub privacy_md: Option<String>,
    /// SMTP credentials
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_tls: Option<SmtpTls>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub from_address: Option<String>,
    /// Mail templates
    pub title_template: String,
    pub body_template: String,
}

/// Which long-form document a public `GET /site/:kind` is asking about. The
/// router maps the URL segment to this enum so the handler stays a single
/// function parameterised by kind.
#[derive(Debug, Clone, Copy)]
pub enum DocKind {
    Tos,
    Privacy,
}

impl DocKind {
    pub fn column(self) -> &'static str {
        match self {
            DocKind::Tos => "tos_md",
            DocKind::Privacy => "privacy_md",
        }
    }
}
