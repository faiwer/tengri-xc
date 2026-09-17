use crate::site::SiteMeta;

/// Site-wide preview image. Absolute URLs only — crawlers don't resolve
/// relative `og:image` paths.
const OG_IMAGE_PATH: &str = "/images/appIcon-1024.png";

/// What a single document response says about itself.
pub(super) struct PageMeta {
    pub(super) title: String,
    pub(super) description: Option<String>,
    /// The page's own preview, absolute. `None` falls back to the site icon.
    pub(super) image: Option<String>,
}

pub(super) fn image_url(app_base_url: &str) -> String {
    format!("{app_base_url}{OG_IMAGE_PATH}")
}

impl PageMeta {
    /// Fallback for every route without a resolver of its own.
    pub(super) fn site(site: &SiteMeta) -> Self {
        Self {
            title: site.site_name.clone(),
            description: site.site_description.clone(),
            image: None,
        }
    }

    /// The `<title>` plus the tag block, indented to sit where the shell's own
    /// title did. `site_image_url` is used by every page that has no preview of
    /// its own.
    pub(super) fn render(&self, site_image_url: &str) -> String {
        let mut out = format!("<title>{}</title>", html_escape::encode_text(&self.title));

        if let Some(ref description) = self.description {
            push_tag(&mut out, "name", "description", description);
            push_tag(&mut out, "property", "og:description", description);
        }
        push_tag(&mut out, "property", "og:title", &self.title);
        push_tag(
            &mut out,
            "property",
            "og:image",
            self.image.as_deref().unwrap_or(site_image_url),
        );
        push_tag(&mut out, "property", "og:type", "website");
        // The site icon is square and says nothing; a page that brings its own
        // picture has something worth the big card.
        let card = match self.image {
            Some(_) => "summary_large_image",
            None => "summary",
        };
        push_tag(&mut out, "name", "twitter:card", card);

        out
    }
}

fn push_tag(out: &mut String, attribute: &str, name: &str, content: &str) {
    out.push_str("\n    <meta ");
    out.push_str(attribute);
    out.push_str("=\"");
    out.push_str(name);
    out.push_str("\" content=\"");
    out.push_str(&html_escape::encode_double_quoted_attribute(content));
    out.push_str("\" />");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_the_title_and_the_attributes() {
        let meta = PageMeta {
            title: "Bob & \"Ann\"".to_owned(),
            description: Some("<script>".to_owned()),
            image: None,
        };

        let rendered = meta.render("https://example.test/i.png");

        assert!(rendered.contains("<title>Bob &amp; \"Ann\"</title>"));
        assert!(rendered.contains(r#"content="Bob &amp; &quot;Ann&quot;""#));
        assert!(rendered.contains(r#"content="&lt;script&gt;""#));
    }

    #[test]
    fn omits_the_description_tags_when_there_is_none() {
        let meta = PageMeta {
            title: "Tengri XC".to_owned(),
            description: None,
            image: None,
        };

        let rendered = meta.render("https://example.test/i.png");

        assert!(!rendered.contains("description"));
        assert!(rendered.contains(r#"<meta property="og:title" content="Tengri XC" />"#));
    }

    #[test]
    fn a_page_of_its_own_image_gets_it_and_the_big_card() {
        let meta = PageMeta {
            title: "Flight".to_owned(),
            description: None,
            image: Some("https://example.test/api/tracks/abc123/og.jpg".to_owned()),
        };

        let rendered = meta.render("https://example.test/i.png");

        assert!(rendered.contains(
            r#"<meta property="og:image" content="https://example.test/api/tracks/abc123/og.jpg" />"#
        ));
        assert!(rendered.contains(r#"content="summary_large_image""#));
    }

    #[test]
    fn a_page_without_one_falls_back_to_the_site_icon() {
        let rendered = PageMeta {
            title: "Tengri XC".to_owned(),
            description: None,
            image: None,
        }
        .render("https://example.test/i.png");

        assert!(
            rendered
                .contains(r#"<meta property="og:image" content="https://example.test/i.png" />"#)
        );
        assert!(rendered.contains(r#"content="summary""#));
    }
}
