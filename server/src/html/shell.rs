use anyhow::Context;

use crate::html::meta::PageMeta;

/// The SPA's `index.html`, split once at its `<title>` tag. A document response
/// is then two string pushes around freshly rendered head tags instead of a
/// parse per request.
pub(crate) struct HtmlShell {
    /// Everything before `<title>`.
    head: String,
    /// Everything from after `</title>` onward.
    tail: String,
}

const TITLE_OPEN: &str = "<title>";
const TITLE_CLOSE: &str = "</title>";

impl HtmlShell {
    /// Errors when the shell has no title tag. We ship that `index.html`, so a
    /// missing anchor is our own bug and deserves a loud 500 rather than a
    /// silent fallback.
    pub(crate) fn parse(raw: &str) -> anyhow::Result<Self> {
        let open = raw
            .find(TITLE_OPEN)
            .context("no <title> in the SPA shell")?;
        let close = raw[open..]
            .find(TITLE_CLOSE)
            .map(|offset| open + offset)
            .context("no </title> in the SPA shell")?;

        Ok(Self {
            head: raw[..open].to_owned(),
            tail: raw[close + TITLE_CLOSE.len()..].to_owned(),
        })
    }

    /// Everything goes in at the title's position: `<meta>` order inside
    /// `<head>` is irrelevant to crawlers, and `<meta charset>` already sits
    /// above the title — so `</head>` never has to be found.
    pub(super) fn render(&self, meta: &PageMeta, image_url: &str) -> String {
        let tags = meta.render(image_url);
        let mut out = String::with_capacity(self.head.len() + tags.len() + self.tail.len());
        out.push_str(&self.head);
        out.push_str(&tags);
        out.push_str(&self.tail);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = "<!doctype html>\n<html>\n  <head>\n    <meta charset=\"UTF-8\" />\n    \
                       <title>Tengri XC</title>\n  </head>\n  <body></body>\n</html>\n";

    fn meta() -> PageMeta {
        PageMeta {
            title: "Flight".to_owned(),
            description: Some("A flight".to_owned()),
            image: None,
        }
    }

    #[test]
    fn replaces_the_title_instead_of_duplicating_it() {
        let rendered = HtmlShell::parse(RAW).unwrap().render(&meta(), "img");

        assert!(rendered.contains("<title>Flight</title>"));
        assert!(!rendered.contains("Tengri XC"));
    }

    #[test]
    fn injects_the_tags_inside_the_head() {
        let rendered = HtmlShell::parse(RAW).unwrap().render(&meta(), "img");

        let tag = rendered.find(r#"<meta property="og:title""#).unwrap();
        assert!(tag < rendered.find("</head>").unwrap());
        assert!(tag > rendered.find("<meta charset").unwrap());
    }

    #[test]
    fn keeps_everything_around_the_title() {
        let rendered = HtmlShell::parse(RAW).unwrap().render(&meta(), "img");

        assert!(rendered.starts_with("<!doctype html>"));
        assert!(rendered.ends_with("</html>\n"));
    }

    #[test]
    fn a_shell_without_a_title_is_an_error() {
        assert!(HtmlShell::parse("<html><head></head></html>").is_err());
        assert!(HtmlShell::parse("<html><head><title>oops</head></html>").is_err());
    }
}
