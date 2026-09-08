//! The "confirm your email" message sent at registration. Its link points at
//! `GET /users/confirm-email`, handled by `routes::register`.

use jsonwebtoken::EncodingKey;

use crate::{
    AppError,
    auth::mint_confirm_token,
    mail::{
        compose::{Message, compose},
        sanitize::escape_email_text,
        send::{SmtpConfig, send_mail},
    },
    site::AdminSiteDto,
};

/// Who the confirmation is for. `email` is both the recipient and the address
/// baked into the token, so a link only ever confirms what it was minted for.
pub struct ConfirmRecipient<'a> {
    pub user_id: i32,
    pub name: &'a str,
    pub email: &'a str,
}

/// Compose and deliver the confirmation message using the *stored* SMTP
/// settings. `api_public_url` is the public API origin the link has to resolve
/// against from inside a mail client, so it can't be derived from the request.
pub async fn send_confirmation_email(
    stored: &AdminSiteDto,
    api_public_url: &str,
    encoding_key: &EncodingKey,
    recipient: ConfirmRecipient<'_>,
) -> Result<(), AppError> {
    let token = mint_confirm_token(recipient.user_id, recipient.email, encoding_key)
        .map_err(into_internal)?;
    // Safe to interpolate unescaped: the origin comes from config and a JWT is
    // base64url plus dots, so neither can carry a quote or an angle bracket.
    let link = format!("{api_public_url}/users/confirm-email?token={token}");

    let subject = format!("Confirm your {} account", stored.site_name);
    let body = format!(
        "<p>Hey {name},</p>\
         <p>Confirm this address to finish setting up your <strong>{site}</strong> account:</p>\
         <p><a href=\"{link}\">Confirm the email address</a></p>\
         <p>The link works for 24 hours. If you didn't sign up, ignore this message.</p>",
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
