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
use crate::content::slug::Slug;
use crate::content::{PageAny, Site};
use crate::markdown::{MarkdownRenderer, Rendered};

/// In-memory representation of the rendered site.
pub struct RenderedSite {
    /// URL path -> rendered HTML (e.g. "/blog/hello/" -> "<html>...</html>")
    pub pages: HashMap<String, String>,
    /// Pre-rendered 404 page HTML.
    pub not_found_html: String,
}

impl RenderedSite {
    /// Load content from `config` and render every page against `theme`.
    ///
    /// If `fail_on_broken_links` is `true`, returns [`Error::BrokenWikiLinks`]
    /// when any wiki-link target is missing. When `false`, broken links are
    /// logged as warnings and rendered with a `class="wikilink broken"` span.
    pub fn build(
        config: &Config,
        theme: &Theme,
        fail_on_broken_links: bool,
    ) -> Result<Self, Error> {
        tracing::info!(source = %config.source_dir.display(), "loading content");
        let site = Site::load(config.clone())?;
        Renderer::new(theme).render_all(&site, fail_on_broken_links)
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
    pub fn render_all(
        &self,
        site: &Site,
        fail_on_broken_links: bool,
    ) -> Result<RenderedSite, Error> {
        tracing::info!("rendering markdown");
        let rendered = Self::render_markdown(site);
        Self::check_broken_links(&rendered, site, fail_on_broken_links)?;

        let site_ctx = SiteContext::from_config(&site.config, &site.pages);
        let mut pages = self.render_content_pages(&rendered, site, &site_ctx)?;
        pages.extend(self.render_tag_pages(site, &site_ctx)?);
        pages.extend(self.render_index_pages(site, &site_ctx)?);

        let not_found_html =
            self.render_template("404.html", &NotFoundContext { site: site_ctx })?;

        Ok(RenderedSite {
            pages,
            not_found_html,
        })
    }

    fn render_markdown(site: &Site) -> RenderedPages {
        let md = MarkdownRenderer::new(site);
        // Blog and wiki are typically the bulk of a site; render them
        // concurrently. Standalone pages are usually one or two — sequential
        // afterwards is fine.
        let (blog, wiki) = rayon::join(
            || site.blog.par_iter().map(|p| md.render(&p.body)).collect(),
            || site.wiki.par_iter().map(|p| md.render(&p.body)).collect(),
        );
        let pages = site.pages.iter().map(|p| md.render(&p.body)).collect();
        RenderedPages { blog, wiki, pages }
    }

    fn check_broken_links(rendered: &RenderedPages, site: &Site, fail: bool) -> Result<(), Error> {
        let all_broken = site
            .blog
            .iter()
            .map(|p| &p.slug)
            .zip(&rendered.blog)
            .chain(site.wiki.iter().map(|p| &p.slug).zip(&rendered.wiki))
            .chain(site.pages.iter().map(|p| &p.slug).zip(&rendered.pages))
            .flat_map(|(slug, r)| r.broken_wiki_links.iter().map(move |t| (slug, t)));

        if fail {
            let broken: Vec<(String, String)> = all_broken
                .map(|(slug, target)| (slug.to_string(), target.clone()))
                .collect();
            if !broken.is_empty() {
                return Err(Error::BrokenWikiLinks(broken));
            }
        } else {
            for (slug, target) in all_broken {
                tracing::warn!(page = %slug, target, "broken wiki-link");
            }
        }
        Ok(())
    }

    fn render_content_pages(
        &self,
        rendered: &RenderedPages,
        site: &Site,
        site_ctx: &SiteContext,
    ) -> Result<HashMap<String, String>, Error> {
        let all_pages: Vec<(PageAny<'_>, &Rendered)> = site
            .blog
            .iter()
            .map(PageAny::Blog)
            .zip(&rendered.blog)
            .chain(site.wiki.iter().map(PageAny::Wiki).zip(&rendered.wiki))
            .chain(site.pages.iter().map(PageAny::Page).zip(&rendered.pages))
            .collect();

        all_pages
            .into_par_iter()
            .map(|(page, md)| {
                let ctx = PageContext::from_page(&page, md, site, site_ctx);
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

        for (tag, slugs) in &site.tag_index {
            let posts: Vec<PostEntry> = slugs
                .iter()
                .filter_map(|s| site.get(s).map(|any| PostEntry::from_page(&any)))
                .collect();

            let tag_slug: Slug = tag.as_str().into();
            all_tags.push(TagEntry {
                name: tag.clone(),
                slug: tag_slug.clone(),
                count: posts.len(),
            });

            let ctx = TagPageContext {
                site: site_ctx.clone(),
                tag: tag.clone(),
                tag_slug: tag_slug.clone(),
                posts,
            };
            let html = self.render_template("tag.html", &ctx)?;
            pages.insert(format!("/tags/{tag_slug}/"), html);
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
        site_ctx: &SiteContext,
    ) -> Result<HashMap<String, String>, Error> {
        let mut pages = HashMap::new();

        let posts: Vec<PostEntry> = site
            .blog
            .iter()
            .map(|p| PostEntry::from_page(&PageAny::Blog(p)))
            .collect();

        let home_content = site.home.as_ref().map(|h| {
            let rendered = MarkdownRenderer::new(site).render(&h.body);
            HomeContent {
                content: rendered.html,
            }
        });
        let home_ctx = HomeContext {
            site: site_ctx.clone(),
            posts: posts.clone(),
            home: home_content,
        };
        pages.insert("/".into(), self.render_template("home.html", &home_ctx)?);

        let blog_ctx = BlogIndexContext {
            site: site_ctx.clone(),
            posts,
        };
        pages.insert(
            "/blog/".into(),
            self.render_template("blog_index.html", &blog_ctx)?,
        );

        let wiki_ctx = WikiIndexContext {
            site: site_ctx.clone(),
            categories: WikiCategory::from_site(site),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::page::PageKind;

    #[test]
    fn page_context_selects_correct_template() {
        let site = SiteContext {
            site_title: "Test".into(),
            base_url: "http://localhost".into(),
            version: "0.0.0".into(),
            nav_pages: vec![],
            socials: vec![],
        };
        let ctx = PageContext {
            site: site.clone(),
            title: "Hello".into(),
            url: "/blog/hello/".into(),
            kind: PageKind::Blog,
            content: "<p>Hello</p>".into(),
            toc: vec![],
            backlinks: vec![],
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
            },
            title: "My Post".into(),
            url: "/blog/my-post/".into(),
            kind: PageKind::Blog,
            content: "<p>Body here</p>".into(),
            toc: vec![],
            backlinks: vec![],
            category: None,
            wiki_categories: vec![],
            author: Some("Alice".into()),
            image: None,
            description: None,
            created: Some("2026-01-01".into()),
            updated: None,
            tags: vec![],
        };

        let html = renderer.render_template(ctx.template_name(), &ctx).unwrap();
        assert!(html.contains("<h1>My Post</h1>"));
        assert!(html.contains("<p>Body here</p>"));
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
                },
            },
            Page {
                slug: "about".into(),
                body: String::new(),
                path: PathBuf::from("content/pages/about.md"),
                frontmatter: PageFrontmatter {
                    title: "About".into(),
                    order: Some(1),
                },
            },
            Page {
                slug: "faq".into(),
                body: String::new(),
                path: PathBuf::from("content/pages/faq.md"),
                frontmatter: PageFrontmatter {
                    title: "FAQ".into(),
                    order: None,
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
