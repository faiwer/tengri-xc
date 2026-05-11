//! DB reads + writes for `site_settings`. The migration inserts a single row at
//! deploy time, so every read is a single-row SELECT — readers can `.fetch_one`
//! without worrying about an empty table.

use serde::{Deserialize, Deserializer};
use sqlx::Row;

use crate::{
    AppError,
    db::Update,
    site::dto::{AdminSiteDto, DocKind, SiteDto},
    validation::FieldErrors,
};

/// Length cap on `site_name`. Short enough that the header layout can't be
/// broken by a 10 KB paste, long enough for any real name.
const SITE_NAME_MAX_LEN: usize = 64;

/// Length cap on each long-form document. 64 KB is far more than any real ToS /
/// Privacy text needs and guards the row from accidental paste-bombs.
const DOC_MAX_LEN: usize = 64 * 1024;

/// Fetch the slim public view of `site_settings`. The migration guarantees one
/// row; missing rows here are a schema invariant violation and surface as 500.
pub async fn fetch_site_public(pool: &sqlx::PgPool) -> Result<SiteDto, AppError> {
    let row = sqlx::query(
        "SELECT site_name, can_register, \
                tos_md     IS NOT NULL AS has_tos, \
                privacy_md IS NOT NULL AS has_privacy \
         FROM site_settings \
         WHERE id = TRUE",
    )
    .fetch_one(pool)
    .await
    .map_err(into_internal)?;

    Ok(SiteDto {
        site_name: row.try_get("site_name").map_err(sqlx_to_internal)?,
        can_register: row.try_get("can_register").map_err(sqlx_to_internal)?,
        has_tos: row.try_get("has_tos").map_err(sqlx_to_internal)?,
        has_privacy: row.try_get("has_privacy").map_err(sqlx_to_internal)?,
    })
}

/// Fetch the full admin view, including raw markdown.
pub async fn fetch_site_admin(pool: &sqlx::PgPool) -> Result<AdminSiteDto, AppError> {
    let row = sqlx::query(
        "SELECT site_name, can_register, tos_md, privacy_md \
         FROM site_settings \
         WHERE id = TRUE",
    )
    .fetch_one(pool)
    .await
    .map_err(into_internal)?;

    Ok(AdminSiteDto {
        site_name: row.try_get("site_name").map_err(sqlx_to_internal)?,
        can_register: row.try_get("can_register").map_err(sqlx_to_internal)?,
        tos_md: row.try_get("tos_md").map_err(sqlx_to_internal)?,
        privacy_md: row.try_get("privacy_md").map_err(sqlx_to_internal)?,
    })
}

/// Fetch a single document column. `Ok(None)` when the column is NULL — caller
/// decides whether that's a 404 or an empty payload.
pub async fn fetch_site_doc(
    pool: &sqlx::PgPool,
    kind: DocKind,
) -> Result<Option<String>, AppError> {
    // The column name is a compile-time constant from `DocKind`, not user input
    // — safe to interpolate into the SQL string.
    let sql = format!(
        "SELECT {col} FROM site_settings WHERE id = TRUE",
        col = kind.column()
    );
    let row = sqlx::query(&sql)
        .fetch_one(pool)
        .await
        .map_err(into_internal)?;

    row.try_get::<Option<String>, _>(0)
        .map_err(sqlx_to_internal)
}

// ---------------------------------------------------------------------------
// Update support
// ---------------------------------------------------------------------------
//
// Same `Option<Option<T>>` recipe as `user::profile`: outer `None` = field
// absent in the PATCH (leave alone), `Some(None)` = explicit JSON `null` (clear
// to NULL), `Some(Some(v))` = set. Short scalars (`site_name`, `can_register`)
// use plain `Option` since their columns are NOT NULL — they have no "clear to
// null" intent.

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateSiteRequest {
    #[serde(default)]
    pub site_name: Option<String>,
    #[serde(default)]
    pub can_register: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub tos_md: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub privacy_md: Option<Option<String>>,
}

/// Validated projection. Same triple-state shape as the request, values
/// normalised (site_name trimmed; markdown left as-is so authors keep their
/// whitespace).
#[derive(Debug, Default)]
pub struct SiteUpdate {
    pub site_name: Option<String>,
    pub can_register: Option<bool>,
    pub tos_md: Option<Option<String>>,
    pub privacy_md: Option<Option<String>>,
}

impl SiteUpdate {
    pub fn is_noop(&self) -> bool {
        self.site_name.is_none()
            && self.can_register.is_none()
            && self.tos_md.is_none()
            && self.privacy_md.is_none()
    }
}

pub fn validate_site_update(input: UpdateSiteRequest) -> Result<SiteUpdate, FieldErrors> {
    let mut errors = FieldErrors::new();

    let site_name = match input.site_name {
        None => None,
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                errors.add("site_name", "Cannot be empty");
                None
            } else if trimmed.chars().count() > SITE_NAME_MAX_LEN {
                errors.add(
                    "site_name",
                    format!("Must be at most {SITE_NAME_MAX_LEN} characters"),
                );
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
    };

    let tos_md = validate_doc(&mut errors, "tos_md", input.tos_md);
    let privacy_md = validate_doc(&mut errors, "privacy_md", input.privacy_md);

    if errors.is_empty() {
        Ok(SiteUpdate {
            site_name,
            can_register: input.can_register,
            tos_md,
            privacy_md,
        })
    } else {
        Err(errors)
    }
}

/// Treat the empty string from the client as "clear" (Some(None)) — the form
/// submits an empty textarea as `""`, and we want that to behave the same as an
/// explicit JSON null. Above the cap, error.
fn validate_doc(
    errors: &mut FieldErrors,
    field: &'static str,
    value: Option<Option<String>>,
) -> Option<Option<String>> {
    match value {
        None => None,
        Some(None) => Some(None),
        Some(Some(s)) if s.is_empty() => Some(None),
        Some(Some(s)) if s.len() > DOC_MAX_LEN => {
            errors.add(field, format!("Must be at most {} KB", DOC_MAX_LEN / 1024));
            None
        }
        Some(Some(s)) => Some(Some(s)),
    }
}

/// Apply a validated update. Single UPDATE — the singleton row is guaranteed by
/// the migration, no UPSERT shape needed.
pub async fn apply_site_update(pool: &sqlx::PgPool, update: &SiteUpdate) -> Result<(), AppError> {
    if update.is_noop() {
        return Ok(());
    }

    let mut q = Update::new("site_settings");
    if let Some(ref v) = update.site_name {
        q.set("site_name", v.as_str());
    }
    if let Some(v) = update.can_register {
        q.set("can_register", v);
    }
    if let Some(ref v) = update.tos_md {
        q.set("tos_md", v.as_deref());
    }
    if let Some(ref v) = update.privacy_md {
        q.set("privacy_md", v.as_deref());
    }
    q.and_where("id = $", (true,));

    q.execute(pool).await.map_err(into_internal)?;
    Ok(())
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}

fn sqlx_to_internal(e: sqlx::Error) -> AppError {
    AppError::Internal(anyhow::Error::new(e))
}

/// Standard serde recipe for distinguishing "field absent" from "field present
/// and explicitly null". Used with `#[serde(default, deserialize_with =
/// "deserialize_some")]`.
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}
