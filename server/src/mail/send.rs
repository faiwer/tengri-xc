//! SMTP transport. Takes a resolved connection and a composed message and puts
//! it on the wire; every decision about *what* to send lives in the callers.

use std::time::Duration;

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::header::ContentType,
    transport::smtp::authentication::Credentials,
};

use crate::{AppError, site::dto::SmtpTls};

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
