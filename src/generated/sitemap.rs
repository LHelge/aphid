use std::io::Cursor;

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesText};

use crate::content::Site;
use crate::content::page::PageKind;
use crate::content::slug::Slug;

const SITEMAP_NS: &str = "http://www.sitemaps.org/schemas/sitemap/0.9";

/// A generated `sitemap.xml` built from the loaded site content.
///
/// Blog posts always carry `<lastmod>` (from `updated` or `created`). Wiki
/// pages include it only when a date is present in their frontmatter.
/// Standalone pages, index pages, and tag pages omit `<lastmod>`.
pub struct Sitemap {
    bytes: Vec<u8>,
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
            bytes: Self::render(&entries),
        }
    }

    /// Consume the value and return the raw XML bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn render(entries: &[Entry]) -> Vec<u8> {
        let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

        writer
            .write_event(quick_xml::events::Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                None,
            )))
            .expect("write XML decl");

        writer
            .create_element("urlset")
            .with_attribute(("xmlns", SITEMAP_NS))
            .write_inner_content(|w| {
                for entry in entries {
                    w.create_element("url").write_inner_content(|w| {
                        w.create_element("loc")
                            .write_text_content(BytesText::new(&entry.loc))?;
                        if let Some(ref date) = entry.lastmod {
                            w.create_element("lastmod")
                                .write_text_content(BytesText::new(date))?;
                        }
                        Ok(())
                    })?;
                }
                Ok(())
            })
            .expect("write sitemap XML");

        let mut bytes = writer.into_inner().into_inner();
        bytes.push(b'\n');
        bytes
    }
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
                draft: false,
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
        let xml = String::from_utf8(Sitemap::render(&entries)).unwrap();
        assert!(xml.contains("https://example.com/a&amp;b/"));
    }
}
