//! Wire shapes for site settings. Two DTOs because the public boot payload and
//! the admin editor have different visibility needs: the public one ships short
//! scalars + booleans, the admin one carries the long-form markdown the editor
//! populates.

use serde::Serialize;

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

/// Returned by `GET /admin/site` and `PATCH /admin/site`. Carries the markdown
/// so the admin form can populate its textareas. Will grow sensitive fields
/// (SMTP password, OAuth client secrets) in future migrations — those will live
/// here, never on [`SiteDto`].
#[derive(Debug, Serialize)]
pub struct AdminSiteDto {
    pub site_name: String,
    pub can_register: bool,
    pub tos_md: Option<String>,
    pub privacy_md: Option<String>,
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
