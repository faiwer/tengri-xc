//! Site-wide singleton settings. One row in `site_settings`, fetched and
//! updated through `/site` (public, slim) and `/admin/site` (admin, full). See
//! the migration in `0005_site_settings.sql` for the storage shape and
//! rationale.

pub mod dto;
pub mod store;

pub use dto::{AdminSiteDto, DocKind, SiteDto};
pub use store::{
    UpdateSiteRequest, apply_site_update, fetch_site_admin, fetch_site_doc, fetch_site_public,
    validate_site_update,
};
