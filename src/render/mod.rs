pub mod context;
pub mod theme;

pub use context::*;
pub use theme::Theme;

use std::collections::HashMap;

use rayon::prelude::*;
use serde::Serialize;
use tera::Context;

use crate::Error;
use crate::config::Config;
use crate::content::{PageAny, Site};
use crate::generated::{AtomFeed, FaviconSet, Robots, RssFeed, Sitemap};
use crate::markdown::{MarkdownRenderer, Rendered};

/// Controls mode-specific behaviour during rendering.
///
/// In **build** mode broken wiki-links are fatal errors and RSS/Atom feeds
/// are generated. In **serve** mode broken links are logged as warnings
/// (so writing can continue) and feed generation is skipped for faster
/// rebuilds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Build,
    Serve,
}

/// In-memory representation of the rendered site.
pub struct RenderedSite {
    /// URL path -> rendered HTML (e.g. "/blog/hello/" -> "<html>...</html>")
    pub pages: HashMap<String, String>,
    /// Pre-rendered 404 page HTML.
    pub not_found_html: String,
    /// Files written at the site root (e.g. `favicon.ico`, `robots.txt`,
    /// `sitemap.xml`). Each entry is `(filename, bytes)`.
    pub root_files: Vec<(String, Vec<u8>)>,
}

impl RenderedSite {
    /// Load content from `config` and render every page against `theme`.
    ///
    /// In [`Mode::Build`] broken wiki-links are fatal and feeds are
    /// generated. In [`Mode::Serve`] broken links are warnings and feeds
    /// are skipped.
    pub fn build(config: &Config, theme: &Theme, mode: Mode) -> Result<Self, Error> {
        let favicon = config
            .favicon
            .as_ref()
            .map(|p| FaviconSet::generate(p, &config.title))
            .transpose()?;
        Self::build_with_favicon(config, theme, mode, favicon)
    }

    /// Like [`build`](Self::build) but accepts a pre-built [`FaviconSet`].
    ///
    /// Serve-mode rebuilds pass the cached set from the initial render so
    /// the expensive image-processing step is not repeated on every file
    /// change.
    pub fn build_with_favicon(
        config: &Config,
        theme: &Theme,
        mode: Mode,
        favicon: Option<FaviconSet>,
    ) -> Result<Self, Error> {
        tracing::info!(source = %config.source_dir.display(), "loading content");
        let site = Site::load(config.clone())?;
        Renderer::new(theme).render_all(&site, config, mode, favicon)
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

struct RenderedPages {
    blog: Vec<Rendered>,
    wiki: Vec<Rendered>,
    pages: Vec<Rendered>,
    home: Option<Rendered>,
}

impl RenderedPages {
    fn iter_pages(&self) -> impl Iterator<Item = &Rendered> {
        self.blog
            .iter()
            .chain(self.wiki.iter())
            .chain(self.pages.iter())
    }
}

/// Pass-2 orchestrator: takes a [`Site`] and a [`Theme`] and produces a
/// fully-rendered [`RenderedSite`]. Internally runs the markdown pipeline
/// per page (in parallel via rayon), then per-page Tera template
/// rendering, then the index/tag/404 pages.
pub struct Renderer<'a> {
    theme: &'a Theme,
}

impl<'a> Renderer<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }

    /// Render the entire site: markdown pass, broken-link policy, template rendering.
    fn render_all(
        &self,
        site: &Site,
        config: &Config,
        mode: Mode,
        favicon: Option<FaviconSet>,
    ) -> Result<RenderedSite, Error> {
        tracing::info!("rendering markdown");
        let rendered = Self::render_markdown(site);
        Self::check_broken_links(&rendered, site, mode)?;

        let site_ctx = SiteContext::from_config(&site.config, &site.pages, favicon.as_ref());
        let wiki_categories = WikiCategory::from_site(site);
        let mut pages = self.render_content_pages(&rendered, site, &site_ctx, &wiki_categories)?;
        pages.extend(self.render_tag_pages(site, &site_ctx)?);
        pages.extend(self.render_index_pages(site, &rendered, &site_ctx, &wiki_categories)?);

        let not_found_html =
            self.render_template("404.html", &NotFoundContext { site: site_ctx })?;

        // ── Root files (favicon, robots.txt, sitemap.xml, feeds) ────────
        let mut root_files: Vec<(String, Vec<u8>)> = Vec::new();

        if let Some(set) = favicon {
            root_files.extend(set.files.clone());
        }

        root_files.push((
            "robots.txt".into(),
            Robots::new(config.normalized_base_url()).into_bytes(),
        ));
        root_files.push(("sitemap.xml".into(), Sitemap::new(site).into_bytes()));
        root_files.push((
            "feed.xml".into(),
            AtomFeed::new(site, &rendered.blog).into_bytes(),
        ));
        root_files.push((
            "rss.xml".into(),
            RssFeed::new(site, &rendered.blog).into_bytes(),
        ));

        Ok(RenderedSite {
            pages,
            not_found_html,
            root_files,
        })
    }

    fn render_markdown(site: &Site) -> RenderedPages {
        let md = MarkdownRenderer::new(site);
        // Page-body rendering preserves slice order, so blog/wiki/pages can
        // all render in parallel without disturbing page-to-render alignment.
        let (blog, wiki) = rayon::join(
            || Self::render_page_bodies(&site.blog, &md),
            || Self::render_page_bodies(&site.wiki, &md),
        );
        let pages = Self::render_page_bodies(&site.pages, &md);
        let home = site.home.as_ref().map(|page| md.render(&page.body));
        RenderedPages {
            blog,
            wiki,
            pages,
            home,
        }
    }

    fn render_page_bodies<F: Sync>(
        pages: &[crate::content::page::Page<F>],
        md: &MarkdownRenderer<'_>,
    ) -> Vec<Rendered> {
        pages.par_iter().map(|page| md.render(&page.body)).collect()
    }

    fn check_broken_links(rendered: &RenderedPages, site: &Site, mode: Mode) -> Result<(), Error> {
        let all_broken = site
            .iter_pages()
            .zip(rendered.iter_pages())
            .flat_map(|(page, rendered_page)| {
                rendered_page
                    .broken_wiki_links
                    .iter()
                    .map(move |target| (page.slug().to_string(), target.clone()))
            })
            .chain(
                rendered
                    .home
                    .iter()
                    .flat_map(|r| r.broken_wiki_links.iter().cloned())
                    .map(|target| ("home.md".to_string(), target)),
            );

        if mode == Mode::Build {
            let broken: Vec<(String, String)> = all_broken.collect();
            if !broken.is_empty() {
                return Err(Error::BrokenWikiLinks(broken));
            }
        } else {
            for (slug, target) in all_broken {
                tracing::warn!(page = slug, target, "broken wiki-link");
            }
        }
        Ok(())
    }

    fn render_content_pages(
        &self,
        rendered: &RenderedPages,
        site: &Site,
        site_ctx: &SiteContext,
        wiki_categories: &[WikiCategory],
    ) -> Result<HashMap<String, String>, Error> {
        let all_pages: Vec<(PageAny<'_>, &Rendered)> =
            site.iter_pages().zip(rendered.iter_pages()).collect();

        all_pages
            .into_par_iter()
            .map(|(page, md)| {
                let ctx = PageContext::from_page(&page, md, site, site_ctx, wiki_categories);
                let html = self.render_template(ctx.template_name(), &ctx)?;
                Ok((ctx.url, html))
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
            let posts = PostEntry::from_pages(slugs.iter().filter_map(|slug| site.get(slug)));

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
        site: &Site,
        rendered: &RenderedPages,
        site_ctx: &SiteContext,
        wiki_categories: &[WikiCategory],
    ) -> Result<HashMap<String, String>, Error> {
        let mut pages = HashMap::new();

        let posts = PostEntry::from_pages(site.blog.iter().map(PageAny::Blog));
        let per_page = site.config.posts_per_page.max(1);

        let home_rendered = site.home.as_ref().and(rendered.home.as_ref());
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
    use crate::content::page::{Page, PageKind};

    #[test]
    fn page_context_selects_correct_template() {
        let site = SiteContext {
            site_title: "Test".into(),
            base_url: "http://localhost".into(),
            version: "0.0.0".into(),
            nav_pages: vec![],
            socials: vec![],
            favicon_tags: String::new(),
            feed_atom_url: "http://localhost/feed.xml".into(),
            feed_rss_url: "http://localhost/rss.xml".into(),
        };
        let ctx = PageContext {
            site: site.clone(),
            title: "Hello".into(),
            url: "/blog/hello/".into(),
            kind: PageKind::Blog,
            content: "<p>Hello</p>".into(),
            toc: vec![],
            backlinks: vec![],
            contains_mermaid: false,
            category: None,
            wiki_categories: vec![],
            author: Some("Alice".into()),
            image: None,
            description: None,
            created: Some("2026-01-01".into()),
            updated: None,
            tags: vec![TagRef {
                name: "rust".into(),
                slug: "rust".into(),
            }],
            newer_post: None,
            older_post: None,
        };
        assert_eq!(ctx.template_name(), "blog_post.html");

        let wiki_ctx = PageContext {
            site,
            kind: PageKind::Wiki,
            ..ctx
        };
        assert_eq!(wiki_ctx.template_name(), "wiki_page.html");
    }

    #[test]
    fn render_page_with_inline_template() {
        let mut tera = tera::Tera::default();
        tera.add_raw_template(
            "blog_post.html",
            "<h1>{{ title }}</h1><div>{{ content | safe }}</div>",
        )
        .unwrap();

        // Build a Theme with the inline Tera (bypass from_dir)
        let theme = Theme {
            meta: theme::ThemeMeta {
                name: "test".into(),
                version: "0.1.0".into(),
                description: None,
            },
            tera,
            static_dir: None,
            embedded_static: vec![],
        };

        let renderer = Renderer::new(&theme);
        let ctx = PageContext {
            site: SiteContext {
                site_title: "Test Site".into(),
                base_url: "http://localhost".into(),
                version: "0.0.0".into(),
                nav_pages: vec![],
                socials: vec![],
                favicon_tags: String::new(),
                feed_atom_url: "http://localhost/feed.xml".into(),
                feed_rss_url: "http://localhost/rss.xml".into(),
            },
            title: "My Post".into(),
            url: "/blog/my-post/".into(),
            kind: PageKind::Blog,
            content: "<p>Body here</p>".into(),
            toc: vec![],
            backlinks: vec![],
            contains_mermaid: false,
            category: None,
            wiki_categories: vec![],
            author: Some("Alice".into()),
            image: None,
            description: None,
            created: Some("2026-01-01".into()),
            updated: None,
            tags: vec![],
            newer_post: None,
            older_post: None,
        };

        let html = renderer.render_template(ctx.template_name(), &ctx).unwrap();
        assert!(html.contains("<h1>My Post</h1>"));
        assert!(html.contains("<p>Body here</p>"));
    }

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
            base_url: "http://localhost".into(),
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
        let rendered = Renderer::render_markdown(&site);
        let pages = renderer
            .render_index_pages(&site, &rendered, &site_ctx_for_test(), &[])
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
        let rendered = Renderer::render_markdown(&site);
        let pages = renderer
            .render_index_pages(&site, &rendered, &site_ctx_for_test(), &[])
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
