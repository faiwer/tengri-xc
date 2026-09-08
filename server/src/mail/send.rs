//! SMTP transport. Takes a resolved connection and a composed message and puts
//! it on the wire; every decision about *what* to send lives in the callers.

use std::time::Duration;

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::header::ContentType,
    transport::smtp::authentication::Credentials,
};

use crate::{
    AppError,
    site::{AdminSiteDto, dto::SmtpTls},
    user::blank_to_none,
};

/// Give up rather than hold the request open for lettre's 60 s default — a
/// misconfigured host is the expected failure here, not an unlucky one.
const SMTP_TIMEOUT: Duration = Duration::from_secs(15);

/// A resolved SMTP session. Ports and TLS mode are already defaulted, and the
/// password is the one to actually use (stored or freshly typed).
pub(super) struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub tls: SmtpTls,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
}

/// A composed message. `body_html` is already sanitized and substituted.
pub(super) struct OutgoingMail {
    pub to: String,
    pub subject: String,
    pub body_html: String,
}

/// Deliver `mail` over `smtp`.
///
/// Failures come back as [`AppError::BadRequest`] carrying the underlying SMTP
/// message: the operator's own settings are the input being tested, and
/// `Internal` would replace the one useful detail with "internal server error".
pub(super) async fn send_mail(smtp: &SmtpConfig, mail: OutgoingMail) -> Result<(), AppError> {
    let message =
        Message::builder()
            .from(smtp.from.parse().map_err(|_| {
                AppError::BadRequest(format!("Invalid from address: {}", smtp.from))
            })?)
            .to(mail
                .to
                .parse()
                .map_err(|_| AppError::BadRequest(format!("Invalid recipient: {}", mail.to)))?)
            .subject(mail.subject)
            .header(ContentType::TEXT_HTML)
            .body(mail.body_html)
            .map_err(|err| AppError::BadRequest(format!("Couldn't build the message: {err}")))?;

    let mut builder = match smtp.tls {
        SmtpTls::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host),
        SmtpTls::Starttls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host),
        SmtpTls::None => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
            &smtp.host,
        )),
    }
    .map_err(|err| AppError::BadRequest(format!("Couldn't reach {}: {err}", smtp.host)))?
    .port(smtp.port)
    .timeout(Some(SMTP_TIMEOUT));

    if let Some(ref username) = smtp.username {
        let password = smtp.password.clone().unwrap_or_default();
        builder = builder.credentials(Credentials::new(username.clone(), password));
    }

    builder
        .build()
        .send(message)
        .await
        .map_err(|err| AppError::BadRequest(format!("SMTP send failed: {err}")))?;

    Ok(())
}

/// Whether the saved settings can actually produce a send. Defined as "the
/// config builds", so a caller's pre-flight check can't drift out of step with
/// what [`SmtpConfig::from_stored`] demands — host, from-address, and a usable
/// port are all required, and a blank string counts as absent.
pub fn is_smtp_configured(stored: &AdminSiteDto) -> bool {
    SmtpConfig::from_stored(stored).is_ok()
}

impl SmtpConfig {
    /// Build from the saved settings. Every send except the operator's
    /// test-send goes through here.
    pub(super) fn from_stored(stored: &AdminSiteDto) -> Result<Self, AppError> {
        let host = blank_to_none(stored.smtp_host.clone())
            .ok_or_else(|| AppError::Conflict("Outgoing mail is not configured".into()))?;
        let from = blank_to_none(stored.from_address.clone())
            .ok_or_else(|| AppError::Conflict("Outgoing mail has no from-address".into()))?
            .to_ascii_lowercase();
        let (tls, port) = resolve_transport(stored.smtp_tls, stored.smtp_port)?;

        Ok(Self {
            host,
            port,
            tls,
            username: blank_to_none(stored.smtp_username.clone()),
            password: blank_to_none(stored.smtp_password.clone()),
            from,
        })
    }
}

/// Pick the encryption mode and port from whatever the operator filled in. An
/// unset mode is inferred from the port, so someone who typed 465 and left the
/// dropdown alone doesn't get STARTTLS on an implicit-TLS port.
pub(super) fn resolve_transport(
    tls: Option<SmtpTls>,
    port: Option<i32>,
) -> Result<(SmtpTls, u16), AppError> {
    let tls = match (tls, port) {
        (Some(tls), _) => tls,
        (None, Some(port)) if port == i32::from(PORT_IMPLICIT) => SmtpTls::Implicit,
        (None, _) => SmtpTls::Starttls,
    };

    let port = match port {
        None => default_port(tls),
        Some(port) => u16::try_from(port)
            .ok()
            .filter(|port| *port > 0)
            .ok_or_else(|| AppError::BadRequest(format!("Invalid SMTP port: {port}")))?,
    };

    Ok((tls, port))
}

fn default_port(tls: SmtpTls) -> u16 {
    match tls {
        SmtpTls::Implicit => PORT_IMPLICIT,
        SmtpTls::Starttls => PORT_STARTTLS,
        SmtpTls::None => PORT_PLAIN,
    }
}

const PORT_IMPLICIT: u16 = 465;
const PORT_STARTTLS: u16 = 587;
const PORT_PLAIN: u16 = 25;
