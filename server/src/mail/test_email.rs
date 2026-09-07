//! `POST /admin/site/test-email` — prove a relay works before trusting it with
//! real mail.
//!
//! The request carries the operator's *unsaved* editor values so a config can
//! be verified before it's committed. That's also why the template is sanitized
//! here rather than relying on the write path: on this route it never passed
//! through storage.

use serde::Deserialize;

use crate::{
    AppError,
    mail::{
        sanitize::{escape_email_text, sanitize_email_html},
        send::{OutgoingMail, SmtpConfig, send_mail},
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
    let mail = compose(&request, stored, to)?;
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

fn compose(
    request: &TestEmailRequest,
    stored: &AdminSiteDto,
    to: String,
) -> Result<OutgoingMail, AppError> {
    if !request.title_template.contains(TITLE_PLACEHOLDER) {
        return Err(AppError::BadRequest(format!(
            "Subject template must contain {TITLE_PLACEHOLDER}"
        )));
    }
    if !request.body_template.contains(BODY_PLACEHOLDER) {
        return Err(AppError::BadRequest(format!(
            "Body template must contain {BODY_PLACEHOLDER}"
        )));
    }

    // A subject is a plain header, so the site name needs no HTML escaping —
    // but it must not smuggle in a newline, which is header injection.
    let site_name: String = stored
        .site_name
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let subject = request
        .title_template
        .replace(TITLE_PLACEHOLDER, &format!("Test message from {site_name}"));

    let body = format!(
        "<p>This is a test message from <strong>{site}</strong>.</p>\
         <p>If you are reading it, outgoing mail is configured correctly.</p>",
        site = escape_email_text(&site_name),
    );
    // Sanitize the template, then substitute — the same order as the write
    // path, so both routes apply identical rules to identical input.
    let body_html = sanitize_email_html(&request.body_template).replace(BODY_PLACEHOLDER, &body);

    Ok(OutgoingMail {
        to,
        subject,
        body_html,
    })
}

fn resolve_smtp(request: TestEmailRequest, stored: &AdminSiteDto) -> Result<SmtpConfig, AppError> {
    let host = blank_to_none(Some(request.smtp_host))
        .ok_or_else(|| AppError::BadRequest("SMTP host is not set".into()))?;
    let from = blank_to_none(Some(request.from_address))
        .ok_or_else(|| AppError::BadRequest("From address is not set".into()))?
        .to_ascii_lowercase();

    // An unset encryption mode is inferred from the port, so an operator who
    // typed 465 and left the dropdown alone doesn't get STARTTLS on an
    // implicit-TLS port.
    let tls = match (request.smtp_tls, request.smtp_port) {
        (Some(tls), _) => tls,
        (None, Some(port)) if port == i32::from(PORT_IMPLICIT) => SmtpTls::Implicit,
        (None, _) => SmtpTls::Starttls,
    };

    let port = match request.smtp_port {
        None => default_port(tls),
        Some(port) => u16::try_from(port)
            .ok()
            .filter(|port| *port > 0)
            .ok_or_else(|| AppError::BadRequest(format!("Invalid SMTP port: {port}")))?,
    };

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

fn default_port(tls: SmtpTls) -> u16 {
    match tls {
        SmtpTls::Implicit => PORT_IMPLICIT,
        SmtpTls::Starttls => PORT_STARTTLS,
        SmtpTls::None => PORT_PLAIN,
    }
}

const TITLE_PLACEHOLDER: &str = "%title%";
const BODY_PLACEHOLDER: &str = "%body%";

const PORT_IMPLICIT: u16 = 465;
const PORT_STARTTLS: u16 = 587;
const PORT_PLAIN: u16 = 25;
