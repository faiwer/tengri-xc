//! `POST /admin/site/test-email` — prove a relay works before trusting it with
//! real mail.
//!
//! The request carries the operator's *unsaved* editor values so a config can
//! be verified before it's committed. That's also why the template is sanitized
//! on this route rather than relying on the write path: here it never passed
//! through storage.

use serde::Deserialize;

use crate::{
    AppError,
    mail::{
        compose::{Message, compose},
        sanitize::escape_email_text,
        send::{SmtpConfig, resolve_transport, send_mail},
    },
    site::{AdminSiteDto, dto::SmtpTls},
    user::blank_to_none,
    validation::{FieldErrors, looks_like_email},
};

/// Unsaved connection settings plus a recipient. Every field is present because
/// the editor always submits its whole state; `smtp_port` and `smtp_tls` are
/// nullable in both the form and the column, and clearing an AntD control
/// yields `undefined`, which `JSON.stringify` drops from the body entirely.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TestEmailRequest {
    pub to: String,
    pub smtp_host: String,
    #[serde(default)]
    pub smtp_port: Option<i32>,
    #[serde(default)]
    pub smtp_tls: Option<SmtpTls>,
    pub smtp_username: String,
    /// Blank means the operator didn't retype it — fall back to the stored one.
    pub smtp_password: String,
    pub from_address: String,
    pub title_template: String,
    pub body_template: String,
}

/// Compose and deliver the test message. `stored` supplies exactly two things:
/// the fallback password and the site name; every connection setting and both
/// templates come from the request.
pub async fn send_test_email(
    request: TestEmailRequest,
    stored: &AdminSiteDto,
) -> Result<(), AppError> {
    let to = validate_recipient(&request.to)?;
    let subject = format!("Test message from {}", stored.site_name);
    let body = format!(
        "<p>This is a test message from <strong>{site}</strong>.</p>\
         <p>If you are reading it, outgoing mail is configured correctly.</p>",
        site = escape_email_text(&stored.site_name),
    );

    let mail = compose(
        &request.title_template,
        &request.body_template,
        Message {
            to,
            subject: &subject,
            body_html: &body,
        },
    )?;
    let smtp = resolve_smtp(request, stored)?;
    send_mail(&smtp, mail).await
}

fn validate_recipient(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if looks_like_email(&trimmed) {
        return Ok(trimmed);
    }

    // Keyed to `to` so it lands on the modal's own input via `Form.setFields`.
    let mut errors = FieldErrors::new();
    errors.add("to", "Enter a valid email address");
    Err(AppError::Validation(errors))
}

fn resolve_smtp(request: TestEmailRequest, stored: &AdminSiteDto) -> Result<SmtpConfig, AppError> {
    let host = blank_to_none(Some(request.smtp_host))
        .ok_or_else(|| AppError::BadRequest("SMTP host is not set".into()))?;
    let from = blank_to_none(Some(request.from_address))
        .ok_or_else(|| AppError::BadRequest("From address is not set".into()))?
        .to_ascii_lowercase();
    let (tls, port) = resolve_transport(request.smtp_tls, request.smtp_port)?;

    Ok(SmtpConfig {
        host,
        port,
        tls,
        username: blank_to_none(Some(request.smtp_username)),
        // Blank means "unchanged", matching how the editor leaves the box empty
        // when a password is already stored.
        password: blank_to_none(Some(request.smtp_password))
            .or_else(|| blank_to_none(stored.smtp_password.clone())),
        from,
    })
}
