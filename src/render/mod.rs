pub mod context;
pub mod theme;

pub use context::*;
pub use theme::Theme;

use std::collections::HashMap;

use rayon::prelude::*;
use serde::Serialize;
use tera::Context;

use crate::Error;
use crate::artifacts;
use crate::config::Config;
use crate::content::{PageView, Site};
use crate::favicon::FaviconSet;
use crate::markdown::{Diagnostics, MarkdownRenderer, RenderedSite};

/// A fully built site, ready to be written to disk or served over HTTP.
///
/// Carries diagnostics from pass 1 (markdown rendering) alongside the
/// output artifacts. Callers decide what to do with the diagnostics:
/// [`crate::build`] turns broken wiki-links into an error, the dev server
/// logs them as warnings and serves the pages anyway.
pub struct BuiltSite {
    /// URL path -> rendered HTML (e.g. "/blog/hello/" -> "<html>...</html>")
    pub pages: HashMap<String, String>,
    /// Pre-rendered 404 page HTML.
    pub not_found_html: String,
    /// Files written at the site root (e.g. `favicon.ico`, `robots.txt`,
    /// `sitemap.xml`). Each entry is `(filename, bytes)`.
    pub root_files: Vec<(String, Vec<u8>)>,
    /// Build-time signals collected during pass 1.
    pub diagnostics: Diagnostics,
}

impl BuiltSite {
    /// Load content from `config` and render every page against `theme`.
    pub fn build(config: &Config, theme: &Theme) -> Result<Self, Error> {
        let favicon = config
            .favicon
            .as_ref()
            .map(|p| FaviconSet::generate(p, &config.title))
            .transpose()?;
        Self::build_with_favicon(config, theme, favicon)
    }

    /// Like [`build`](Self::build) but accepts a pre-built [`FaviconSet`].
    ///
    /// Serve-mode rebuilds pass the cached set from the initial render so
    /// the expensive image-processing step is not repeated on every file
    /// change.
    pub fn build_with_favicon(
        config: &Config,
        theme: &Theme,
        favicon: Option<FaviconSet>,
    ) -> Result<Self, Error> {
        tracing::info!(source = %config.source_dir.display(), "loading content");
        let site = Site::load(config.clone())?;
        tracing::info!("rendering markdown");
        let rendered = MarkdownRenderer::new(&site).render_site();
        Renderer::new(theme).render_all(&rendered, favicon)
    }

    /// Look up rendered HTML for a URL path, normalising trailing slashes.
    pub fn lookup(&self, path: &str) -> Option<&str> {
        if path.ends_with('/') {
            self.pages.get(path).map(String::as_str)
        } else {
            self.pages.get(&format!("{path}/")).map(String::as_str)
        }
    }
}

/// Pass-2 orchestrator: takes a [`RenderedSite`] (markdown bodies already
/// rendered, joined to their pages) and a [`Theme`], and produces a
/// fully-built [`BuiltSite`]. Runs per-page Tera template rendering in
/// parallel via rayon, then the index/tag/404 pages, then assembles the
/// root files.
pub struct Renderer<'a> {
    theme: &'a Theme,
}

impl<'a> Renderer<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }

    /// Apply templates and assemble root files. Pass 1 is already done — the
    /// [`RenderedSite`] argument carries the markdown bodies and any
    /// diagnostics; this method neither extends nor inspects the
    /// diagnostics, just forwards them onto the [`BuiltSite`].
    fn render_all(
        &self,
        rendered: &RenderedSite<'_>,
        favicon: Option<FaviconSet>,
    ) -> Result<BuiltSite, Error> {
        let site = rendered.site();
        let site_ctx = SiteContext::from_config(&site.config, &site.pages, favicon.as_ref());
        let wiki_categories = WikiCategory::from_site(site);

        let (blog_html, wiki_html) = rayon::join(
            || self.render_blog_pages(rendered, &site_ctx),
            || self.render_wiki_pages(rendered, &site_ctx, &wiki_categories),
        );
        let mut pages = blog_html?;
        pages.extend(wiki_html?);
        pages.extend(self.render_standalone_pages(rendered, &site_ctx)?);
        pages.extend(self.render_tag_pages(site, &site_ctx)?);
        pages.extend(self.render_index_pages(rendered, &site_ctx, &wiki_categories)?);

        let not_found_rendered = rendered.not_found().map(|(_, r)| r);
        let not_found_content = not_found_rendered.map(NotFoundContent::from);
        let not_found_ctx = NotFoundContext {
            site: site_ctx,
            not_found: not_found_content,
            contains_mermaid: not_found_rendered.is_some_and(|r| r.contains_mermaid),
        };
        let not_found_html = self.render_template("404.html", &not_found_ctx)?;

        // ── Root files (favicon, then every RootArtifact) ───────────────
        let mut root_files: Vec<(String, Vec<u8>)> = Vec::new();
        if let Some(set) = favicon {
            root_files.extend(set.files.clone());
        }
        root_files.extend(artifacts::render_all(rendered));

        Ok(BuiltSite {
            pages,
            not_found_html,
            root_files,
            diagnostics: rendered.diagnostics().clone(),
        })
    }

    fn render_blog_pages(
        &self,
        rendered: &RenderedSite<'_>,
        site_ctx: &SiteContext,
    ) -> Result<HashMap<String, String>, Error> {
        let site = rendered.site();
        rendered
            .blog()
            .par_bridge()
            .map(|(page, r)| {
                let ctx = BlogPostContext::from_page(page, r, site, site_ctx);
                let url = page.url_path();
                let html = self.render_template("blog_post.html", &ctx)?;
                Ok((url, html))
            })
            .collect()
    }

    fn render_wiki_pages(
        &self,
        rendered: &RenderedSite<'_>,
        site_ctx: &SiteContext,
        wiki_categories: &[WikiCategory],
    ) -> Result<HashMap<String, String>, Error> {
        let site = rendered.site();
        rendered
            .wiki()
            .par_bridge()
            .map(|(page, r)| {
                let ctx =
                    WikiPageContext::from_page(page, r, site, site_ctx, wiki_categories.to_vec());
                let url = page.url_path();
                let html = self.render_template("wiki_page.html", &ctx)?;
                Ok((url, html))
            })
            .collect()
    }

    fn render_standalone_pages(
        &self,
        rendered: &RenderedSite<'_>,
        site_ctx: &SiteContext,
    ) -> Result<HashMap<String, String>, Error> {
        rendered
            .pages()
            .par_bridge()
            .map(|(page, r)| {
                let ctx = StandalonePageContext::from_page(page, r, rendered.site(), site_ctx);
                let url = page.url_path();
                let html = self.render_template("page.html", &ctx)?;
                Ok((url, html))
            })
            .collect()
    }

    fn render_tag_pages(
        &self,
        site: &Site,
        site_ctx: &SiteContext,
    ) -> Result<HashMap<String, String>, Error> {
        let mut pages = HashMap::new();
        let mut all_tags: Vec<TagEntry> = Vec::new();
        let per_page = site.config.posts_per_page.max(1);

        for (tag, slugs) in &site.tag_index {
            let posts: Vec<PostEntry> = slugs
                .iter()
                .filter_map(|slug| {
                    site.blog_post(slug)
                        .map(PostEntry::from_blog_page)
                        .or_else(|| site.wiki_page(slug).map(PostEntry::from_wiki_page))
                })
                .collect();

            let tag_entry = TagEntry::new(tag, posts.len());
            let tag_slug = tag_entry.slug.clone();
            all_tags.push(tag_entry);

            let base_path = format!("/tags/{tag_slug}/");
            let chunks = paginate(&posts, per_page);
            let total = chunks.len();
            for (i, chunk) in chunks.iter().enumerate() {
                let current = i + 1;
                let pagination = Pagination::build(&base_path, current, total);
                let ctx = TagPageContext {
                    site: site_ctx.clone(),
                    tag: tag.clone(),
                    tag_slug: tag_slug.clone(),
                    posts: chunk.to_vec(),
                    pagination,
                };
                let html = self.render_template("tag.html", &ctx)?;
                let url = if current == 1 {
                    base_path.clone()
                } else {
                    format!("{base_path}page/{current}/")
                };
                pages.insert(url, html);
            }
        }

        all_tags.sort_by(|a, b| a.name.cmp(&b.name));
        let ctx = TagsIndexContext {
            site: site_ctx.clone(),
            tags: all_tags,
        };
        let html = self.render_template("tags_index.html", &ctx)?;
        pages.insert("/tags/".into(), html);

        Ok(pages)
    }

    fn render_index_pages(
        &self,
        rendered: &RenderedSite<'_>,
        site_ctx: &SiteContext,
        wiki_categories: &[WikiCategory],
    ) -> Result<HashMap<String, String>, Error> {
        let site = rendered.site();
        let mut pages = HashMap::new();

        let posts = PostEntry::from_blog_pages(&site.blog);
        let per_page = site.config.posts_per_page.max(1);

        let home_rendered = rendered.home().map(|(_, r)| r);
        let home_content = home_rendered.map(HomeContent::from);
        let contains_mermaid = home_rendered.is_some_and(|r| r.contains_mermaid);
        let home_ctx = HomeContext {
            site: site_ctx.clone(),
            posts: posts.clone(),
            home: home_content,
            contains_mermaid,
        };
        pages.insert("/".into(), self.render_template("home.html", &home_ctx)?);

        let chunks = paginate(&posts, per_page);
        let total = chunks.len();
        for (i, chunk) in chunks.iter().enumerate() {
            let current = i + 1;
            let pagination = Pagination::build("/blog/", current, total);
            let blog_ctx = BlogIndexContext {
                site: site_ctx.clone(),
                posts: chunk.to_vec(),
                pagination,
            };
            let url = if current == 1 {
                "/blog/".to_string()
            } else {
                format!("/blog/page/{current}/")
            };
            pages.insert(url, self.render_template("blog_index.html", &blog_ctx)?);
        }

        let wiki_ctx = WikiIndexContext {
            site: site_ctx.clone(),
            categories: wiki_categories.to_vec(),
        };
        pages.insert(
            "/wiki/".into(),
            self.render_template("wiki_index.html", &wiki_ctx)?,
        );

        Ok(pages)
    }

    fn render_template<T: Serialize>(&self, template: &str, ctx: &T) -> Result<String, Error> {
        let tera_ctx = Context::from_serialize(ctx)?;
        let html = self.theme.tera.render(template, &tera_ctx)?;
        Ok(html)
    }
}

/// Split `items` into chunks of `per_page`, always yielding at least one
/// (possibly empty) slice so the canonical page exists for an empty list.
/// Caller must ensure `per_page >= 1`.
fn paginate<T>(items: &[T], per_page: usize) -> Vec<&[T]> {
    if items.is_empty() {
        return vec![&[]];
    }
    items.chunks(per_page).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::page::Page;

    #[test]
    fn paginate_empty_yields_one_empty_page() {
        let chunks = paginate::<i32>(&[], 5);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].is_empty());
    }

    #[test]
    fn paginate_splits_into_chunks() {
        let items = [1, 2, 3, 4, 5, 6, 7];
        let chunks = paginate(&items, 3);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2, 3]);
        assert_eq!(chunks[1], &[4, 5, 6]);
        assert_eq!(chunks[2], &[7]);
    }

    fn make_blog_post(slug: &str, year: i32, day: u32) -> Page<crate::content::BlogFrontmatter> {
        use crate::content::BlogFrontmatter;
        use chrono::NaiveDate;
        use std::path::PathBuf;
        Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/blog/{slug}.md")),
            frontmatter: BlogFrontmatter {
                title: format!("Post {slug}"),
                slug: slug.into(),
                author: "Tester".into(),
                created: NaiveDate::from_ymd_opt(year, 1, day).unwrap(),
                updated: None,
                image: None,
                description: None,
                tags: vec!["rust".into()],
                draft: false,
            },
        }
    }

    fn theme_with_stub_blog_index() -> Theme {
        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "blog_index.html",
            "{% if pagination %}page {{ pagination.current }}/{{ pagination.total }}{% else %}single{% endif %}: {% for p in posts %}{{ p.title }};{% endfor %}",
        )
        .unwrap();
        tera.add_raw_template("home.html", "home").unwrap();
        tera.add_raw_template("wiki_index.html", "wiki").unwrap();
        tera.add_raw_template(
            "tag.html",
            "{% if pagination %}p{{ pagination.current }}/{{ pagination.total }}{% else %}single{% endif %}: {% for p in posts %}{{ p.title }};{% endfor %}",
        )
        .unwrap();
        tera.add_raw_template("tags_index.html", "tags").unwrap();
        Theme {
            meta: theme::ThemeMeta {
                name: "test".into(),
                version: "0.1.0".into(),
                description: None,
            },
            tera,
            static_dir: None,
            embedded_static: vec![],
        }
    }

    fn site_ctx_for_test() -> SiteContext {
        SiteContext {
            site_title: "Test".into(),
            site_description: None,
            social_image_url: None,
            version: "0.0.0".into(),
            nav_pages: vec![],
            socials: vec![],
            favicon_tags: String::new(),
            feed_atom_url: "/feed.xml".into(),
            feed_rss_url: "/rss.xml".into(),
        }
    }

    #[test]
    fn blog_index_paginates_into_multiple_pages() {
        let blog: Vec<_> = (0..25)
            .map(|i| make_blog_post(&format!("post-{i:02}"), 2026, (i % 28 + 1) as u32))
            .collect();
        let mut config: Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        config.posts_per_page = 10;
        let mut site = Site::from_parts(config, blog, vec![], vec![]).unwrap();
        site.blog.sort();

        let theme = theme_with_stub_blog_index();
        let renderer = Renderer::new(&theme);
        let rendered = MarkdownRenderer::new(&site).render_site();
        let pages = renderer
            .render_index_pages(&rendered, &site_ctx_for_test(), &[])
            .unwrap();

        assert!(pages.contains_key("/blog/"));
        assert!(pages.contains_key("/blog/page/2/"));
        assert!(pages.contains_key("/blog/page/3/"));
        assert!(!pages.contains_key("/blog/page/1/"));
        assert!(!pages.contains_key("/blog/page/4/"));

        let p1 = pages.get("/blog/").unwrap();
        assert!(p1.starts_with("page 1/3:"));
        let p3 = pages.get("/blog/page/3/").unwrap();
        assert!(p3.starts_with("page 3/3:"));
    }

    #[test]
    fn blog_index_skips_pagination_when_under_limit() {
        let blog: Vec<_> = (0..3)
            .map(|i| make_blog_post(&format!("post-{i:02}"), 2026, (i + 1) as u32))
            .collect();
        let mut config: Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        config.posts_per_page = 10;
        let mut site = Site::from_parts(config, blog, vec![], vec![]).unwrap();
        site.blog.sort();

        let theme = theme_with_stub_blog_index();
        let renderer = Renderer::new(&theme);
        let rendered = MarkdownRenderer::new(&site).render_site();
        let pages = renderer
            .render_index_pages(&rendered, &site_ctx_for_test(), &[])
            .unwrap();

        assert!(pages.contains_key("/blog/"));
        assert!(!pages.keys().any(|k| k.starts_with("/blog/page/")));
        let p1 = pages.get("/blog/").unwrap();
        assert!(p1.starts_with("single:"));
    }

    #[test]
    fn tag_pages_paginate_with_correct_urls() {
        let blog: Vec<_> = (0..12)
            .map(|i| make_blog_post(&format!("post-{i:02}"), 2026, (i + 1) as u32))
            .collect();
        let mut config: Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        config.posts_per_page = 5;
        let mut site = Site::from_parts(config, blog, vec![], vec![]).unwrap();
        site.blog.sort();

        let theme = theme_with_stub_blog_index();
        let renderer = Renderer::new(&theme);
        let pages = renderer
            .render_tag_pages(&site, &site_ctx_for_test())
            .unwrap();

        assert!(pages.contains_key("/tags/rust/"));
        assert!(pages.contains_key("/tags/rust/page/2/"));
        assert!(pages.contains_key("/tags/rust/page/3/"));
        assert!(!pages.contains_key("/tags/rust/page/1/"));
        assert!(!pages.contains_key("/tags/rust/page/4/"));
    }

    #[test]
    fn nav_entries_sorted_by_order_then_title() {
        use crate::content::PageFrontmatter;
        use crate::content::page::Page;
        use std::path::PathBuf;

        let pages = vec![
            Page {
                slug: "contact".into(),
                body: String::new(),
                path: PathBuf::from("content/pages/contact.md"),
                frontmatter: PageFrontmatter {
                    title: "Contact".into(),
                    order: Some(2),
                    draft: false,
                },
            },
            Page {
                slug: "about".into(),
                body: String::new(),
                path: PathBuf::from("content/pages/about.md"),
                frontmatter: PageFrontmatter {
                    title: "About".into(),
                    order: Some(1),
                    draft: false,
                },
            },
            Page {
                slug: "faq".into(),
                body: String::new(),
                path: PathBuf::from("content/pages/faq.md"),
                frontmatter: PageFrontmatter {
                    title: "FAQ".into(),
                    order: None,
                    draft: false,
                },
            },
        ];

        let nav = NavEntry::from_pages(&pages);
        assert_eq!(nav.len(), 3);
        assert_eq!(nav[0].title, "About");
        assert_eq!(nav[0].url, "/about/");
        assert_eq!(nav[1].title, "Contact");
        assert_eq!(nav[1].url, "/contact/");
        assert_eq!(nav[2].title, "FAQ"); // order=None → MAX → last
    }
}
