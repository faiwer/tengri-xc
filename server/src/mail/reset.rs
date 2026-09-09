//! The "reset your password" message. Unlike the confirmation mail, its link
//! points at the SPA (`/reset-password`), not the API: the click has no side
//! effect, it just opens the form that posts the new password.

use jsonwebtoken::EncodingKey;

use crate::{
    AppError,
    auth::mint_reset_token,
    mail::{
        compose::{Message, compose},
        sanitize::escape_email_text,
        send::{SmtpConfig, send_mail},
    },
    site::AdminSiteDto,
};

/// Who the reset is for. `stamp` is the `users.password_reset_sends` entry this
/// link is minted against, which is what ties the link to a single send.
pub struct ResetRecipient<'a> {
    pub user_id: i32,
    pub name: &'a str,
    pub email: &'a str,
    pub stamp: i64,
}

pub async fn send_password_reset_email(
    stored: &AdminSiteDto,
    app_base_url: &str,
    encoding_key: &EncodingKey,
    recipient: ResetRecipient<'_>,
) -> Result<(), AppError> {
    let token = mint_reset_token(recipient.user_id, recipient.stamp, encoding_key)
        .map_err(into_internal)?;
    // Safe to interpolate unescaped: the origin comes from config and a JWT is
    // base64url plus dots, so neither can carry a quote or an angle bracket.
    let link = format!("{app_base_url}/reset-password?token={token}");

    let subject = format!("Reset your {} password", stored.site_name);
    let body = format!(
        "<p>Hey {name},</p>\
         <p>Someone asked to reset the password on your <strong>{site}</strong> account. \
         Open this link to choose a new one:</p>\
         <p><a href=\"{link}\">Choose a new password</a></p>\
         <p>The link works for 24 hours and only once. If this wasn't you, ignore this \
         message — your password stays as it is.</p>",
        name = escape_email_text(recipient.name),
        site = escape_email_text(&stored.site_name),
    );

    let mail = compose(
        &stored.title_template,
        &stored.body_template,
        Message {
            to: recipient.email.to_owned(),
            subject: &subject,
            body_html: &body,
        },
    )?;
    send_mail(&SmtpConfig::from_stored(stored)?, mail).await
}

fn into_internal<E: Into<anyhow::Error>>(e: E) -> AppError {
    AppError::Internal(e.into())
}
