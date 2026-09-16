use crate::site::SiteMeta;

/// Site-wide preview image. Absolute URLs only — crawlers don't resolve
/// relative `og:image` paths.
const OG_IMAGE_PATH: &str = "/images/appIcon-1024.png";

/// What a single document response says about itself. The image is site-wide
/// for now, so it isn't here; see [`image_url`].
pub(super) struct PageMeta {
    pub(super) title: String,
    pub(super) description: Option<String>,
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
        }
    }

    /// The `<title>` plus the tag block, indented to sit where the shell's own
    /// title did.
    pub(super) fn render(&self, image_url: &str) -> String {
        let mut out = format!("<title>{}</title>", html_escape::encode_text(&self.title));

        if let Some(ref description) = self.description {
            push_tag(&mut out, "name", "description", description);
            push_tag(&mut out, "property", "og:description", description);
        }
        push_tag(&mut out, "property", "og:title", &self.title);
        push_tag(&mut out, "property", "og:image", image_url);
        push_tag(&mut out, "property", "og:type", "website");
        // `summary`, not `summary_large_image`: the icon is square.
        push_tag(&mut out, "name", "twitter:card", "summary");

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
        };

        let rendered = meta.render("https://example.test/i.png");

        assert!(!rendered.contains("description"));
        assert!(rendered.contains(r#"<meta property="og:title" content="Tengri XC" />"#));
    }
}
