use crate::config::Config;

/// A generated `robots.txt` that allows all crawlers and references the
/// sitemap.
pub struct Robots {
    content: String,
}

impl Robots {
    /// Build a permissive `robots.txt` pointing at the sitemap.
    pub fn new(base_url: &str) -> Self {
        tracing::debug!("generating robots.txt");
        let base_url = Config::normalize_base_url(base_url);
        Self {
            content: format!("User-agent: *\nAllow: /\n\nSitemap: {base_url}/sitemap.xml\n"),
        }
    }

    /// Consume the value and return the raw text content.
    pub fn into_bytes(self) -> Vec<u8> {
        self.content.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_sitemap() {
        let r = Robots::new("https://example.com");
        let txt = String::from_utf8(r.into_bytes()).unwrap();
        assert!(txt.contains("User-agent: *"));
        assert!(txt.contains("Allow: /"));
        assert!(txt.contains("Sitemap: https://example.com/sitemap.xml"));
    }

    #[test]
    fn strips_trailing_slash() {
        let r = Robots::new("https://example.com/");
        let txt = String::from_utf8(r.into_bytes()).unwrap();
        assert!(txt.contains("Sitemap: https://example.com/sitemap.xml"));
    }
}
