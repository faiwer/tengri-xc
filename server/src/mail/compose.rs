//! Drops a server-authored message into the operator's template shell. Shared
//! by every sender so the test-send and real mail apply identical rules to
//! identical input.

use crate::{AppError, mail::sanitize::sanitize_email_html, mail::send::OutgoingMail};

/// What to put inside the templates.
pub(super) struct Message<'a> {
    pub to: String,
    /// Plain text, substituted for `%title%`.
    pub subject: &'a str,
    /// Server-authored HTML, substituted for `%body%`.
    pub body_html: &'a str,
}

pub(super) fn compose(
    title_template: &str,
    body_template: &str,
    message: Message<'_>,
) -> Result<OutgoingMail, AppError> {
    if !title_template.contains(TITLE_PLACEHOLDER) {
        return Err(AppError::BadRequest(format!(
            "Subject template must contain {TITLE_PLACEHOLDER}"
        )));
    }
    if !body_template.contains(BODY_PLACEHOLDER) {
        return Err(AppError::BadRequest(format!(
            "Body template must contain {BODY_PLACEHOLDER}"
        )));
    }

    // A subject is a plain header, so it needs no HTML escaping — but a newline
    // anywhere in it, template or substitution, is header injection.
    let subject: String = title_template
        .replace(TITLE_PLACEHOLDER, message.subject)
        .chars()
        .filter(|c| !c.is_control())
        .collect();

    // Sanitize the template, then substitute: the same order as the write path
    // in `site::store`, and it keeps the server-authored body out of reach of
    // the allowlist that exists to bound *operator* markup.
    let body_html = sanitize_email_html(body_template).replace(BODY_PLACEHOLDER, message.body_html);

    Ok(OutgoingMail {
        to: message.to,
        subject,
        body_html,
    })
}

const TITLE_PLACEHOLDER: &str = "%title%";
const BODY_PLACEHOLDER: &str = "%body%";

#[cfg(test)]
mod tests {
    use super::*;

    fn message<'a>(subject: &'a str, body_html: &'a str) -> Message<'a> {
        Message {
            to: "pilot@example.com".to_owned(),
            subject,
            body_html,
        }
    }

    /// The column defaults in `0027_site_smtp.sql` are the bare placeholders,
    /// so this is the shape every send takes until an operator edits a template.
    #[test]
    fn bare_placeholder_templates_pass_the_message_through() {
        let mail = compose("%title%", "%body%", message("Confirm", "<p>Hi</p>")).unwrap();
        assert_eq!(mail.subject, "Confirm");
        assert_eq!(mail.body_html, "<p>Hi</p>");
    }

    #[test]
    fn wraps_the_message_in_the_operator_shell() {
        let mail = compose(
            "[Tengri] %title%",
            "<div class=\"wrap\">%body%<hr></div>",
            message("Confirm", "<p><a href=\"https://x/y\">Link</a></p>"),
        )
        .unwrap();
        assert_eq!(mail.subject, "[Tengri] Confirm");
        assert!(
            mail.body_html.contains("<a href=\"https://x/y\">Link</a>"),
            "the link must survive: {}",
            mail.body_html
        );
        assert!(mail.body_html.starts_with("<div"), "{}", mail.body_html);
    }

    /// A newline in the subject — from either half — would let the rest of the
    /// value be read as additional headers.
    #[test]
    fn strips_control_characters_from_the_subject() {
        let mail = compose(
            "%title%\r\nBcc: attacker@example.com",
            "%body%",
            message("Confirm\nX", ""),
        )
        .unwrap();
        assert_eq!(mail.subject, "ConfirmXBcc: attacker@example.com");
    }

    #[test]
    fn rejects_a_template_that_dropped_its_placeholder() {
        assert!(compose("no title here", "%body%", message("s", "b")).is_err());
        assert!(compose("%title%", "no body here", message("s", "b")).is_err());
    }
}
