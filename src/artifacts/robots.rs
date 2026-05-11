use super::RootArtifact;
use crate::markdown::RenderedSite;

/// A permissive `robots.txt` that allows all crawlers and references the
/// sitemap.
pub(super) struct Robots;

impl RootArtifact for Robots {
    fn filename(&self) -> &'static str {
        "robots.txt"
    }

    fn render(&self, rendered: &RenderedSite<'_>) -> Vec<u8> {
        tracing::debug!("generating robots.txt");
        let base = rendered.site().config.normalized_base_url();
        format!("User-agent: *\nAllow: /\n\nSitemap: {base}/sitemap.xml\n").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Site;
    use crate::markdown::MarkdownRenderer;

    fn rendered_for(base_url: &str) -> Site {
        let config: crate::config::Config = format!("title = \"T\"\nbase_url = \"{base_url}\"")
            .parse()
            .unwrap();
        Site::from_parts(config, vec![], vec![], vec![]).unwrap()
    }

    #[test]
    fn references_sitemap() {
        let site = rendered_for("https://example.com");
        let rendered = MarkdownRenderer::new(&site).render_site();
        let txt = String::from_utf8(Robots.render(&rendered)).unwrap();
        assert!(txt.contains("User-agent: *"));
        assert!(txt.contains("Allow: /"));
        assert!(txt.contains("Sitemap: https://example.com/sitemap.xml"));
    }

    #[test]
    fn strips_trailing_slash() {
        let site = rendered_for("https://example.com/");
        let rendered = MarkdownRenderer::new(&site).render_site();
        let txt = String::from_utf8(Robots.render(&rendered)).unwrap();
        assert!(txt.contains("Sitemap: https://example.com/sitemap.xml"));
    }
}
