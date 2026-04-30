use std::io::Cursor;

use chrono::NaiveDate;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesText};

use crate::content::Site;
use crate::content::frontmatter::BlogFrontmatter;
use crate::content::page::{Page, PageKind};
use crate::markdown::Rendered;

const ATOM_NS: &str = "http://www.w3.org/2005/Atom";
const DC_NS: &str = "http://purl.org/dc/elements/1.1/";

/// Convert a `NaiveDate` to midnight UTC and format as RFC 3339 (Atom).
fn rfc3339(date: NaiveDate) -> String {
    date.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339()
}

/// Convert a `NaiveDate` to midnight UTC and format as RFC 2822 (RSS).
fn rfc2822(date: NaiveDate) -> String {
    date.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc2822()
}

/// Collect the blog entries that should appear in the feed, pairing each
/// post with its rendered HTML body.
fn feed_entries<'a>(
    site: &'a Site,
    blog_rendered: &'a [Rendered],
) -> Vec<(&'a Page<BlogFrontmatter>, &'a Rendered)> {
    let limit = site.config.feed_limit;
    let iter = site.blog.iter().zip(blog_rendered.iter());
    if limit == 0 {
        iter.collect()
    } else {
        iter.take(limit).collect()
    }
}

/// A generated Atom 1.0 feed (`feed.xml`) for blog posts.
pub struct AtomFeed {
    bytes: Vec<u8>,
}

impl AtomFeed {
    pub fn new(site: &Site, blog_rendered: &[Rendered]) -> Self {
        tracing::debug!("generating feed.xml (Atom)");
        let entries = feed_entries(site, blog_rendered);
        Self {
            bytes: Self::render(site, &entries),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn render(site: &Site, entries: &[(&Page<BlogFrontmatter>, &Rendered)]) -> Vec<u8> {
        let base = site.config.normalized_base_url();
        let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

        writer
            .write_event(quick_xml::events::Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                None,
            )))
            .expect("write XML decl");

        writer
            .create_element("feed")
            .with_attribute(("xmlns", ATOM_NS))
            .write_inner_content(|w| {
                // Channel metadata
                w.create_element("title")
                    .write_text_content(BytesText::new(&site.config.title))?;

                w.create_element("link")
                    .with_attribute(("href", format!("{base}/feed.xml").as_str()))
                    .with_attribute(("rel", "self"))
                    .with_attribute(("type", "application/atom+xml"))
                    .write_empty()?;

                w.create_element("link")
                    .with_attribute(("href", format!("{base}/").as_str()))
                    .with_attribute(("rel", "alternate"))
                    .with_attribute(("type", "text/html"))
                    .write_empty()?;

                w.create_element("id")
                    .write_text_content(BytesText::new(&format!("{base}/")))?;

                // Feed-level <updated>: most recent post's date, or omit
                if let Some((post, _)) = entries.first() {
                    let date = post.frontmatter.updated.unwrap_or(post.frontmatter.created);
                    w.create_element("updated")
                        .write_text_content(BytesText::new(&rfc3339(date)))?;
                }

                if let Some(ref desc) = site.config.description {
                    w.create_element("subtitle")
                        .write_text_content(BytesText::new(desc))?;
                }

                // Entries
                for (post, rendered) in entries {
                    let url = format!("{base}{}", PageKind::Blog.url_path(&post.slug));
                    let updated = post.frontmatter.updated.unwrap_or(post.frontmatter.created);

                    w.create_element("entry").write_inner_content(|w| {
                        w.create_element("title")
                            .write_text_content(BytesText::new(&post.frontmatter.title))?;

                        w.create_element("link")
                            .with_attribute(("href", url.as_str()))
                            .with_attribute(("rel", "alternate"))
                            .with_attribute(("type", "text/html"))
                            .write_empty()?;

                        w.create_element("id")
                            .write_text_content(BytesText::new(&url))?;

                        w.create_element("published")
                            .write_text_content(BytesText::new(&rfc3339(
                                post.frontmatter.created,
                            )))?;

                        w.create_element("updated")
                            .write_text_content(BytesText::new(&rfc3339(updated)))?;

                        w.create_element("author").write_inner_content(|w| {
                            w.create_element("name")
                                .write_text_content(BytesText::new(&post.frontmatter.author))?;
                            Ok(())
                        })?;

                        if let Some(ref desc) = post.frontmatter.description {
                            w.create_element("summary")
                                .with_attribute(("type", "text"))
                                .write_text_content(BytesText::new(desc))?;
                        }

                        w.create_element("content")
                            .with_attribute(("type", "html"))
                            .write_text_content(BytesText::new(&rendered.html))?;

                        for tag in &post.frontmatter.tags {
                            w.create_element("category")
                                .with_attribute(("term", tag.as_str()))
                                .write_empty()?;
                        }

                        Ok(())
                    })?;
                }

                Ok(())
            })
            .expect("write Atom feed");

        let mut bytes = writer.into_inner().into_inner();
        bytes.push(b'\n');
        bytes
    }
}

/// A generated RSS 2.0 feed (`rss.xml`) for blog posts.
pub struct RssFeed {
    bytes: Vec<u8>,
}

impl RssFeed {
    pub fn new(site: &Site, blog_rendered: &[Rendered]) -> Self {
        tracing::debug!("generating rss.xml (RSS 2.0)");
        let entries = feed_entries(site, blog_rendered);
        Self {
            bytes: Self::render(site, &entries),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn render(site: &Site, entries: &[(&Page<BlogFrontmatter>, &Rendered)]) -> Vec<u8> {
        let base = site.config.normalized_base_url();
        let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

        writer
            .write_event(quick_xml::events::Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                None,
            )))
            .expect("write XML decl");

        writer
            .create_element("rss")
            .with_attribute(("version", "2.0"))
            .with_attribute(("xmlns:atom", ATOM_NS))
            .with_attribute(("xmlns:dc", DC_NS))
            .write_inner_content(|w| {
                w.create_element("channel").write_inner_content(|w| {
                    w.create_element("title")
                        .write_text_content(BytesText::new(&site.config.title))?;

                    w.create_element("link")
                        .write_text_content(BytesText::new(&format!("{base}/")))?;

                    let desc = site
                        .config
                        .description
                        .as_deref()
                        .unwrap_or(&site.config.title);
                    w.create_element("description")
                        .write_text_content(BytesText::new(desc))?;

                    // Atom self-link for RSS feed autodiscovery
                    w.create_element("atom:link")
                        .with_attribute(("href", format!("{base}/rss.xml").as_str()))
                        .with_attribute(("rel", "self"))
                        .with_attribute(("type", "application/rss+xml"))
                        .write_empty()?;

                    // <lastBuildDate> from most recent post
                    if let Some((post, _)) = entries.first() {
                        let date = post.frontmatter.updated.unwrap_or(post.frontmatter.created);
                        w.create_element("lastBuildDate")
                            .write_text_content(BytesText::new(&rfc2822(date)))?;
                    }

                    // Items
                    for (post, rendered) in entries {
                        let url = format!("{base}{}", PageKind::Blog.url_path(&post.slug));

                        w.create_element("item").write_inner_content(|w| {
                            w.create_element("title")
                                .write_text_content(BytesText::new(&post.frontmatter.title))?;

                            w.create_element("link")
                                .write_text_content(BytesText::new(&url))?;

                            w.create_element("guid")
                                .with_attribute(("isPermaLink", "true"))
                                .write_text_content(BytesText::new(&url))?;

                            w.create_element("pubDate")
                                .write_text_content(BytesText::new(&rfc2822(
                                    post.frontmatter.created,
                                )))?;

                            w.create_element("dc:creator")
                                .write_text_content(BytesText::new(&post.frontmatter.author))?;

                            w.create_element("description")
                                .write_text_content(BytesText::new(&rendered.html))?;

                            for tag in &post.frontmatter.tags {
                                w.create_element("category")
                                    .write_text_content(BytesText::new(tag))?;
                            }

                            Ok(())
                        })?;
                    }

                    Ok(())
                })?;

                Ok(())
            })
            .expect("write RSS feed");

        let mut bytes = writer.into_inner().into_inner();
        bytes.push(b'\n');
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Site;
    use chrono::NaiveDate;
    use std::path::PathBuf;

    fn test_config() -> crate::config::Config {
        "title = \"Test Blog\"\nbase_url = \"https://example.com\"\ndescription = \"A test blog\""
            .parse()
            .unwrap()
    }

    fn blog_post(
        slug: &str,
        title: &str,
        created: NaiveDate,
        tags: Vec<&str>,
    ) -> Page<BlogFrontmatter> {
        Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/blog/{slug}.md")),
            frontmatter: BlogFrontmatter {
                title: title.into(),
                slug: slug.into(),
                author: "Alice".into(),
                created,
                updated: None,
                image: None,
                description: Some(format!("About {title}")),
                tags: tags.into_iter().map(Into::into).collect(),
            },
        }
    }

    fn rendered_html(content: &str) -> Rendered {
        Rendered {
            html: content.into(),
            toc: vec![],
            broken_wiki_links: vec![],
            contains_mermaid: false,
        }
    }

    #[test]
    fn atom_well_formed() {
        let site = Site::from_parts(test_config(), vec![], vec![], vec![]).unwrap();
        let xml = String::from_utf8(AtomFeed::new(&site, &[]).into_bytes()).unwrap();

        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<feed xmlns=\"http://www.w3.org/2005/Atom\">"));
        assert!(xml.contains("<title>Test Blog</title>"));
        assert!(xml.contains("<subtitle>A test blog</subtitle>"));
        assert!(xml.contains("</feed>"));
    }

    #[test]
    fn rss_well_formed() {
        let site = Site::from_parts(test_config(), vec![], vec![], vec![]).unwrap();
        let xml = String::from_utf8(RssFeed::new(&site, &[]).into_bytes()).unwrap();

        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<rss version=\"2.0\""));
        assert!(xml.contains("<title>Test Blog</title>"));
        assert!(xml.contains("<description>A test blog</description>"));
        assert!(xml.contains("</channel>"));
        assert!(xml.contains("</rss>"));
    }

    #[test]
    fn atom_entries_contain_content() {
        let blog = vec![blog_post(
            "hello",
            "Hello World",
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            vec!["rust"],
        )];
        let rendered = vec![rendered_html("<p>Hello, world!</p>")];
        let site = Site::from_parts(test_config(), blog, vec![], vec![]).unwrap();
        let xml = String::from_utf8(AtomFeed::new(&site, &rendered).into_bytes()).unwrap();

        assert!(xml.contains("<title>Hello World</title>"));
        assert!(xml.contains("https://example.com/blog/hello/"));
        assert!(xml.contains("<published>2026-04-20T00:00:00+00:00</published>"));
        assert!(xml.contains("<name>Alice</name>"));
        assert!(xml.contains("&lt;p&gt;Hello, world!&lt;/p&gt;"));
        assert!(xml.contains("<category term=\"rust\""));
        assert!(xml.contains("<summary type=\"text\">About Hello World</summary>"));
    }

    #[test]
    fn rss_entries_contain_content() {
        let blog = vec![blog_post(
            "hello",
            "Hello World",
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            vec!["rust"],
        )];
        let rendered = vec![rendered_html("<p>Hello, world!</p>")];
        let site = Site::from_parts(test_config(), blog, vec![], vec![]).unwrap();
        let xml = String::from_utf8(RssFeed::new(&site, &rendered).into_bytes()).unwrap();

        assert!(xml.contains("<title>Hello World</title>"));
        assert!(xml.contains("<link>https://example.com/blog/hello/</link>"));
        assert!(xml.contains("<guid isPermaLink=\"true\">https://example.com/blog/hello/</guid>"));
        assert!(xml.contains("&lt;p&gt;Hello, world!&lt;/p&gt;"));
        assert!(xml.contains("<dc:creator>Alice</dc:creator>"));
        assert!(xml.contains("<category>rust</category>"));
    }

    #[test]
    fn feed_limit_respected() {
        let mut config = test_config();
        config.feed_limit = 1;
        let blog = vec![
            blog_post(
                "a",
                "First",
                NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
                vec![],
            ),
            blog_post(
                "b",
                "Second",
                NaiveDate::from_ymd_opt(2026, 4, 19).unwrap(),
                vec![],
            ),
        ];
        let rendered = vec![rendered_html("<p>A</p>"), rendered_html("<p>B</p>")];
        let site = Site::from_parts(config, blog, vec![], vec![]).unwrap();

        let atom = String::from_utf8(AtomFeed::new(&site, &rendered).into_bytes()).unwrap();
        assert!(atom.contains("First"));
        assert!(!atom.contains("Second"));

        let rss = String::from_utf8(RssFeed::new(&site, &rendered).into_bytes()).unwrap();
        assert!(rss.contains("First"));
        assert!(!rss.contains("Second"));
    }

    #[test]
    fn feed_limit_zero_includes_all() {
        let mut config = test_config();
        config.feed_limit = 0;
        let blog = vec![
            blog_post(
                "a",
                "First",
                NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
                vec![],
            ),
            blog_post(
                "b",
                "Second",
                NaiveDate::from_ymd_opt(2026, 4, 19).unwrap(),
                vec![],
            ),
        ];
        let rendered = vec![rendered_html("<p>A</p>"), rendered_html("<p>B</p>")];
        let site = Site::from_parts(config, blog, vec![], vec![]).unwrap();

        let atom = String::from_utf8(AtomFeed::new(&site, &rendered).into_bytes()).unwrap();
        assert!(atom.contains("First"));
        assert!(atom.contains("Second"));
    }

    #[test]
    fn escapes_special_characters() {
        let blog = vec![blog_post(
            "test",
            "A & B <C>",
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            vec![],
        )];
        let rendered = vec![rendered_html("<p>x &amp; y</p>")];
        let site = Site::from_parts(test_config(), blog, vec![], vec![]).unwrap();

        let atom = String::from_utf8(AtomFeed::new(&site, &rendered).into_bytes()).unwrap();
        assert!(atom.contains("A &amp; B &lt;C&gt;"));

        let rss = String::from_utf8(RssFeed::new(&site, &rendered).into_bytes()).unwrap();
        assert!(rss.contains("A &amp; B &lt;C&gt;"));
    }
}
