//! Allowlist sanitizer for operator-authored mail HTML, plus the escaper for
//! values interpolated into it. The two defend different things: the sanitizer
//! bounds what markup a template may contain, the escaper stops a value from
//! breaking out of the element or attribute it lands in.

use std::{collections::HashSet, sync::LazyLock};

use ammonia::{Builder, UrlRelative};

/// Sanitize an operator-authored HTML mail template.
///
/// Runs on the template — before placeholder substitution — so that both the
/// `PATCH /admin/site` write path and the test-send path (which carries an
/// unsaved template that never reached storage) apply identical rules.
pub fn sanitize_email_html(html: &str) -> String {
    SANITIZER.clean(html).to_string()
}

/// Escape a value for interpolation into element content. Sanitizing the
/// surrounding template is not enough on its own: a raw value can close a tag
/// early and produce markup the allowlist has no reason to object to.
///
/// Element content only — escaping is context-sensitive, so a value destined
/// for an attribute or a URL needs its own encoder, not this one.
pub(super) fn escape_email_text(value: &str) -> String {
    html_escape::encode_text(value).into_owned()
}

static SANITIZER: LazyLock<Builder<'static>> = LazyLock::new(|| {
    let mut builder = Builder::default();
    builder
        // Mail clients largely ignore stylesheets, so inline styles are the
        // only styling that works — but `style` is not in ammonia's default
        // attribute set, and allowing it without filtering the properties
        // would let arbitrary CSS through.
        .add_generic_attributes(["style"])
        .filter_style_properties(STYLE_PROPERTIES.clone())
        // The default list carries 24 schemes (bitcoin, magnet, ssh, sms, …).
        // Mail needs three.
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        // A relative href has no base document once the message has left, so
        // it can only resolve somewhere unintended.
        .url_relative(UrlRelative::Deny)
        // `rel` does nothing in a mail client.
        .link_rel(None);
    builder
});

/// CSS properties permitted inside `style`. Layout and typography only:
/// `background` / `background-image` are excluded because they carry `url()`,
/// and `position` / `z-index` because stacking games are how a template would
/// disguise one thing as another.
static STYLE_PROPERTIES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "background-color",
        "border",
        "border-bottom",
        "border-collapse",
        "border-color",
        "border-left",
        "border-radius",
        "border-right",
        "border-spacing",
        "border-style",
        "border-top",
        "border-width",
        "color",
        "display",
        "font",
        "font-family",
        "font-size",
        "font-style",
        "font-variant",
        "font-weight",
        "height",
        "letter-spacing",
        "line-height",
        "margin",
        "margin-bottom",
        "margin-left",
        "margin-right",
        "margin-top",
        "max-height",
        "max-width",
        "min-height",
        "min-width",
        "padding",
        "padding-bottom",
        "padding-left",
        "padding-right",
        "padding-top",
        "text-align",
        "text-decoration",
        "text-indent",
        "text-transform",
        "vertical-align",
        "white-space",
        "width",
        "word-break",
    ])
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_elements_and_their_contents() {
        let out = sanitize_email_html("<p>hi</p><script>alert(1)</script>");
        assert_eq!(out, "<p>hi</p>");
    }

    #[test]
    fn strips_event_handlers() {
        let out = sanitize_email_html(r#"<img src="https://e.com/a.png" onerror="alert(1)">"#);
        assert!(!out.contains("onerror"));
        assert!(out.contains("https://e.com/a.png"));
    }

    #[test]
    fn strips_javascript_urls_but_keeps_https_and_mailto() {
        assert!(
            !sanitize_email_html(r#"<a href="javascript:alert(1)">x</a>"#).contains("javascript")
        );
        assert!(sanitize_email_html(r#"<a href="https://e.com">x</a>"#).contains("https://e.com"));
        assert!(
            sanitize_email_html(r#"<a href="mailto:a@e.com">x</a>"#).contains("mailto:a@e.com")
        );
    }

    #[test]
    fn keeps_allowed_style_properties_and_drops_the_rest() {
        let out = sanitize_email_html(r#"<p style="color: red; position: fixed">x</p>"#);
        assert!(out.contains("color"));
        assert!(!out.contains("position"));
    }

    #[test]
    fn keeps_the_body_placeholder_intact() {
        assert!(sanitize_email_html("<p>%body%</p>").contains("%body%"));
    }

    #[test]
    fn escaping_prevents_breaking_out_of_markup() {
        let escaped = escape_email_text("</p><script>alert(1)</script>");
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
    }
}
