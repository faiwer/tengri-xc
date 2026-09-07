//! DB reads + writes for `site_settings`. The migration inserts a single row at
//! deploy time, so every read is a single-row SELECT — readers can `.fetch_one`
//! without worrying about an empty table.

use serde::{Deserialize, Deserializer};
use sqlx::Row;

use crate::{
    AppError,
    db::Update,
    mail::sanitize_email_html,
    site::dto::{AdminSiteDto, DocKind, SiteDto, SmtpTls},
    validation::{FieldErrors, looks_like_email},
};

/// Length cap on `site_name`. Short enough that the header layout can't be
/// broken by a 10 KB paste, long enough for any real name.
const SITE_NAME_MAX_LEN: usize = 64;

/// Length cap on each long-form document. 64 KB is far more than any real ToS /
/// Privacy text needs and guards the row from accidental paste-bombs.
const DOC_MAX_LEN: usize = 64 * 1024;

/// Cap on single-line settings values (hostnames, usernames, addresses).
const SHORT_TEXT_MAX_LEN: usize = 512;

/// TCP port range. The column is `int4` because Postgres has no unsigned types,
/// so the bounds have to be checked rather than encoded in the type.
const PORT_MIN: i32 = 1;
const PORT_MAX: i32 = 65535;

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

/// Fetch the full admin view, including raw markdown and SMTP credentials.
pub async fn fetch_site_admin(pool: &sqlx::PgPool) -> Result<AdminSiteDto, AppError> {
    sqlx::query_as::<_, AdminSiteDto>(
        "SELECT site_name, can_register, tos_md, privacy_md, \
                smtp_host, smtp_port, smtp_tls, smtp_username, smtp_password, \
                from_address, title_template, body_template \
         FROM site_settings \
         WHERE id = TRUE",
    )
    .fetch_one(pool)
    .await
    .map_err(into_internal)
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

// Same `Option<Option<T>>` recipe as `user::profile`: outer `None` = field
// absent in the PATCH (leave alone), `Some(None)` = explicit JSON `null` (clear
// to NULL), `Some(Some(v))` = set. Short scalars (`site_name`, `can_register`)
// and the NOT NULL templates use plain `Option` since they have no "clear to
// null" intent. `smtp_password` treats `""` as unchanged, matching OAuth
// secrets, so a form that prefills and re-submits an empty box doesn't wipe
// the stored password.

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
    /// SMTP credentials
    #[serde(default, deserialize_with = "deserialize_some")]
    pub smtp_host: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub smtp_port: Option<Option<i32>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub smtp_tls: Option<Option<SmtpTls>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub smtp_username: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub smtp_password: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub from_address: Option<Option<String>>,
    /// Mail templates
    #[serde(default)]
    pub title_template: Option<String>,
    #[serde(default)]
    pub body_template: Option<String>,
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
    /// SMTP credentials
    pub smtp_host: Option<Option<String>>,
    pub smtp_port: Option<Option<i32>>,
    pub smtp_tls: Option<Option<SmtpTls>>,
    pub smtp_username: Option<Option<String>>,
    pub smtp_password: Option<Option<String>>,
    pub from_address: Option<Option<String>>,
    /// Mail templates
    pub title_template: Option<String>,
    pub body_template: Option<String>,
}

impl SiteUpdate {
    pub fn is_noop(&self) -> bool {
        self.site_name.is_none()
            && self.can_register.is_none()
            && self.tos_md.is_none()
            && self.privacy_md.is_none()
            && self.smtp_host.is_none()
            && self.smtp_port.is_none()
            && self.smtp_tls.is_none()
            && self.smtp_username.is_none()
            && self.smtp_password.is_none()
            && self.from_address.is_none()
            && self.title_template.is_none()
            && self.body_template.is_none()
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

    let smtp_host = validate_optional_text(&mut errors, "smtp_host", input.smtp_host);
    let smtp_username = validate_optional_text(&mut errors, "smtp_username", input.smtp_username);
    let smtp_password = validate_smtp_password(&mut errors, input.smtp_password);
    let from_address = validate_from_address(&mut errors, input.from_address);
    let smtp_port = validate_port(&mut errors, "smtp_port", input.smtp_port);
    let smtp_tls = input.smtp_tls;

    let title_template = validate_template(
        &mut errors,
        "title_template",
        "%title%",
        input.title_template,
    );
    let body_template = validate_body_template(&mut errors, input.body_template);

    if errors.is_empty() {
        Ok(SiteUpdate {
            site_name,
            can_register: input.can_register,
            tos_md,
            privacy_md,
            smtp_host,
            smtp_port,
            smtp_tls,
            smtp_username,
            smtp_password,
            from_address,
            title_template,
            body_template,
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

fn validate_optional_text(
    errors: &mut FieldErrors,
    field: &'static str,
    value: Option<Option<String>>,
) -> Option<Option<String>> {
    match value {
        None => None,
        Some(None) => Some(None),
        Some(Some(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Some(None)
            } else if trimmed.chars().count() > SHORT_TEXT_MAX_LEN {
                errors.add(
                    field,
                    format!("Must be at most {SHORT_TEXT_MAX_LEN} characters"),
                );
                None
            } else {
                Some(Some(trimmed.to_owned()))
            }
        }
    }
}

/// Empty / whitespace-only password means "leave unchanged", not "clear".
fn validate_smtp_password(
    errors: &mut FieldErrors,
    value: Option<Option<String>>,
) -> Option<Option<String>> {
    match value {
        None => None,
        Some(None) => Some(None),
        Some(Some(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.chars().count() > SHORT_TEXT_MAX_LEN {
                errors.add(
                    "smtp_password",
                    format!("Must be at most {SHORT_TEXT_MAX_LEN} characters"),
                );
                None
            } else {
                Some(Some(trimmed.to_owned()))
            }
        }
    }
}

fn validate_from_address(
    errors: &mut FieldErrors,
    value: Option<Option<String>>,
) -> Option<Option<String>> {
    match value {
        None => None,
        Some(None) => Some(None),
        Some(Some(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Some(None)
            } else {
                let lowered = trimmed.to_ascii_lowercase();
                if !looks_like_email(&lowered) {
                    errors.add("from_address", "Enter a valid email address");
                    None
                } else if lowered.chars().count() > SHORT_TEXT_MAX_LEN {
                    errors.add(
                        "from_address",
                        format!("Must be at most {SHORT_TEXT_MAX_LEN} characters"),
                    );
                    None
                } else {
                    Some(Some(lowered))
                }
            }
        }
    }
}

fn validate_port(
    errors: &mut FieldErrors,
    field: &'static str,
    value: Option<Option<i32>>,
) -> Option<Option<i32>> {
    match value {
        None => None,
        Some(None) => Some(None),
        Some(Some(port)) if !(PORT_MIN..=PORT_MAX).contains(&port) => {
            errors.add(field, format!("Must be between {PORT_MIN} and {PORT_MAX}"));
            None
        }
        Some(Some(port)) => Some(Some(port)),
    }
}

/// The body is HTML, so it's sanitized before storage rather than on the way
/// out: `PATCH` returns the stored value and the editor mirrors it back, so the
/// operator sees what will actually be sent instead of discovering at delivery
/// time that markup was dropped. The subject template is a plain header and is
/// deliberately left alone — running an HTML sanitizer over it would mangle an
/// innocent subject like `3 < 5`.
fn validate_body_template(errors: &mut FieldErrors, value: Option<String>) -> Option<String> {
    let validated = validate_template(errors, "body_template", "%body%", value)?;
    let sanitized = sanitize_email_html(&validated);
    if !sanitized.contains("%body%") {
        // Reachable when the placeholder sits inside markup that's removed
        // wholesale, e.g. `<style>%body%</style>`.
        errors.add(
            "body_template",
            "Must contain %body% outside of markup that email doesn't allow",
        );
        return None;
    }
    Some(sanitized)
}

fn validate_template(
    errors: &mut FieldErrors,
    field: &'static str,
    placeholder: &str,
    value: Option<String>,
) -> Option<String> {
    match value {
        None => None,
        Some(s) if s.is_empty() => {
            errors.add(field, "Cannot be empty");
            None
        }
        Some(s) if s.len() > DOC_MAX_LEN => {
            errors.add(field, format!("Must be at most {} KB", DOC_MAX_LEN / 1024));
            None
        }
        Some(s) if !s.contains(placeholder) => {
            errors.add(field, format!("Must contain {placeholder}"));
            None
        }
        Some(s) => Some(s),
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
    if let Some(ref v) = update.smtp_host {
        q.set("smtp_host", v.as_deref());
    }
    if let Some(v) = update.smtp_port {
        q.set("smtp_port", v);
    }
    if let Some(v) = update.smtp_tls {
        q.set("smtp_tls", v);
    }
    if let Some(ref v) = update.smtp_username {
        q.set("smtp_username", v.as_deref());
    }
    if let Some(ref v) = update.smtp_password {
        q.set("smtp_password", v.as_deref());
    }
    if let Some(ref v) = update.from_address {
        q.set("from_address", v.as_deref());
    }
    if let Some(ref v) = update.title_template {
        q.set("title_template", v.as_str());
    }
    if let Some(ref v) = update.body_template {
        q.set("body_template", v.as_str());
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
