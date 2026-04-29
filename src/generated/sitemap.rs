use crate::content::Site;
use crate::content::page::PageKind;
use crate::content::slug::Slug;

/// A generated `sitemap.xml` built from the loaded site content.
///
/// Blog posts always carry `<lastmod>` (from `updated` or `created`). Wiki
/// pages include it only when a date is present in their frontmatter.
/// Standalone pages, index pages, and tag pages omit `<lastmod>`.
pub struct Sitemap {
    xml: String,
}

struct Entry {
    loc: String,
    lastmod: Option<String>,
}

impl Entry {
    fn new(base: &str, url_path: &str, lastmod: Option<String>) -> Self {
        Self {
            loc: format!("{base}{url_path}"),
            lastmod,
        }
    }
}

impl Sitemap {
    /// Build a sitemap from the fully-loaded site.
    pub fn new(site: &Site) -> Self {
        tracing::debug!("generating sitemap.xml");
        let base = site.config.normalized_base_url();
        let mut entries = Vec::new();

        // Home
        entries.push(Entry::new(base, "/", None));

        // Blog index
        entries.push(Entry::new(base, "/blog/", None));

        // Blog posts — always have a date
        for post in &site.blog {
            let url = PageKind::Blog.url_path(&post.slug);
            let date = post.frontmatter.updated.unwrap_or(post.frontmatter.created);
            entries.push(Entry::new(base, &url, Some(date.to_string())));
        }

        // Wiki index
        entries.push(Entry::new(base, "/wiki/", None));

        // Wiki pages — date only when present
        for page in &site.wiki {
            let url = PageKind::Wiki.url_path(&page.slug);
            let date = page
                .frontmatter
                .updated
                .or(page.frontmatter.created)
                .map(|d| d.to_string());
            entries.push(Entry::new(base, &url, date));
        }

        // Standalone pages
        for page in &site.pages {
            let url = PageKind::Page.url_path(&page.slug);
            entries.push(Entry::new(base, &url, None));
        }

        // Tags index
        if !site.tag_index.is_empty() {
            entries.push(Entry::new(base, "/tags/", None));
        }

        // Individual tag pages
        let mut tags: Vec<&String> = site.tag_index.keys().collect();
        tags.sort();
        for tag in tags {
            let slug: Slug = tag.as_str().into();
            entries.push(Entry::new(base, &format!("/tags/{slug}/"), None));
        }

        Self {
            xml: Self::render(&entries),
        }
    }

    /// Consume the value and return the raw XML bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.xml.into_bytes()
    }

    fn render(entries: &[Entry]) -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
        );
        for entry in entries {
            xml.push_str("  <url>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", escape_xml(&entry.loc)));
            if let Some(ref date) = entry.lastmod {
                xml.push_str(&format!("    <lastmod>{date}</lastmod>\n"));
            }
            xml.push_str("  </url>\n");
        }
        xml.push_str("</urlset>\n");
        xml
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_formed() {
        let config: crate::config::Config = "title = \"T\"\nbase_url = \"https://example.com\""
            .parse()
            .unwrap();
        let site = Site::from_parts(config, vec![], vec![], vec![]).unwrap();
        let xml = String::from_utf8(Sitemap::new(&site).into_bytes()).unwrap();

        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<urlset"));
        assert!(xml.contains("</urlset>"));
        assert!(xml.contains("<loc>https://example.com/</loc>"));
        assert!(xml.contains("<loc>https://example.com/blog/</loc>"));
        assert!(xml.contains("<loc>https://example.com/wiki/</loc>"));
    }

    #[test]
    fn includes_blog_dates() {
        use crate::content::frontmatter::BlogFrontmatter;
        use crate::content::page::Page;
        use chrono::NaiveDate;
        use std::path::PathBuf;

        let config: crate::config::Config = "title = \"T\"\nbase_url = \"https://example.com\""
            .parse()
            .unwrap();
        let blog = vec![Page {
            slug: "hello".into(),
            body: String::new(),
            path: PathBuf::from("content/blog/hello.md"),
            frontmatter: BlogFrontmatter {
                title: "Hello".into(),
                slug: "hello".into(),
                author: "A".into(),
                created: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
                updated: None,
                image: None,
                description: None,
                tags: vec![],
            },
        }];
        let site = Site::from_parts(config, blog, vec![], vec![]).unwrap();
        let xml = String::from_utf8(Sitemap::new(&site).into_bytes()).unwrap();

        assert!(xml.contains("<loc>https://example.com/blog/hello/</loc>"));
        assert!(xml.contains("<lastmod>2026-01-15</lastmod>"));
    }

    #[test]
    fn escapes_ampersands() {
        let entries = vec![Entry {
            loc: "https://example.com/a&b/".into(),
            lastmod: None,
        }];
        let xml = Sitemap::render(&entries);
        assert!(xml.contains("https://example.com/a&amp;b/"));
    }
}
