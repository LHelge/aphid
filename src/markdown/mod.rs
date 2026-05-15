pub mod alerts;
pub mod anchors;
pub mod external_links;
pub mod highlight;
pub mod relative_urls;
pub mod rendered;
pub mod wikilinks;

pub use alerts::rewrite_alerts;
pub use anchors::{HeadingEntry, inject_heading_ids};
pub use external_links::rewrite_external_links;
pub use highlight::Highlighter;
pub use relative_urls::rewrite_relative_urls;
pub use rendered::{BrokenWikiLink, DiagnosticSource, Diagnostics, RenderedSite};
pub use wikilinks::{WikiLinkRef, extract_wiki_links, rewrite_wiki_links};

use std::sync::LazyLock;

use rayon::prelude::*;
use regex::Regex;

use pulldown_cmark::{Options, Parser, html};

use crate::config::Config;
use crate::content::Site;

/// Matches `href="…"` and `src="…"` attributes whose value is a root-relative
/// path (starts with `/`). Used to rewrite in-HTML URLs to absolute form for
/// contexts that fetch out of site scope (RSS/Atom feeds).
static ROOT_RELATIVE_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(href|src)="(/[^"]*)"#).unwrap());

pub(crate) fn render_html(events: Vec<pulldown_cmark::Event<'_>>) -> String {
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    output
}

fn markdown_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_WIKILINKS);
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options
}

/// The output of rendering one markdown body.
pub struct Rendered {
    /// Body HTML, ready to be wrapped in a Tera template.
    pub html: String,
    /// Headings in source order, for table-of-contents rendering.
    pub toc: Vec<HeadingEntry>,
    /// Targets of `[[wiki-links]]` that didn't resolve to any known slug.
    /// `build` mode treats these as fatal; `serve` mode logs them and
    /// continues so writing isn't blocked.
    pub broken_wiki_links: Vec<String>,
    /// Whether the body contains at least one ` ```mermaid ` block. Used by
    /// the template layer to load the Mermaid runtime only on pages that
    /// need it.
    pub contains_mermaid: bool,
}

impl Rendered {
    /// Return [`Rendered::html`] with every root-relative `href` / `src`
    /// attribute rewritten to a fully-qualified URL against the site
    /// `base_url`. Needed by RSS/Atom feeds, which crawlers fetch out of
    /// site context, so a `<a href="/wiki/foo/">` would otherwise have no
    /// resolvable origin.
    pub fn html_with_absolute_urls(&self, config: &Config) -> String {
        ROOT_RELATIVE_ATTR_RE
            .replace_all(&self.html, |caps: &regex::Captures| {
                format!("{}=\"{}\"", &caps[1], config.absolute_url(&caps[2]))
            })
            .into_owned()
    }
}

/// Markdown → HTML pipeline scoped to a [`Site`]: parses the body,
/// rewrites wiki-links against the site's slug index, injects heading
/// anchors, and runs syntax highlighting.
pub struct MarkdownRenderer<'a> {
    pub(crate) site: &'a Site,
    highlighter: Highlighter,
}

impl<'a> MarkdownRenderer<'a> {
    pub fn new(site: &'a Site) -> Self {
        Self {
            site,
            highlighter: Highlighter::new(),
        }
    }

    /// Render the markdown body to HTML. `body` must already have its YAML
    /// frontmatter stripped — `frontmatter::parse` does this at load time, so
    /// the `Page.body` invariant satisfies it.
    pub fn render(&self, body: &str) -> Rendered {
        let events: Vec<_> = Parser::new_ext(body, markdown_options()).collect();

        let events = rewrite_relative_urls(events);
        let (events, broken_wiki_links) = rewrite_wiki_links(events, self.site);
        let events = rewrite_external_links(events, &self.site.config.base_url);
        let events = rewrite_alerts(events);
        let (events, toc) = inject_heading_ids(events);
        let (events, contains_mermaid) = self.highlighter.transform(events);

        Rendered {
            html: render_html(events),
            toc,
            broken_wiki_links,
            contains_mermaid,
        }
    }

    /// Render every page body in the site through the markdown pipeline and
    /// return them joined to their source pages. Blog and wiki are walked in
    /// parallel via rayon (`par_iter` within each); standalone pages and
    /// `home.md` stay on the current thread because they're typically a
    /// handful of files and the parallelism overhead isn't worth it.
    pub fn render_site(&self) -> RenderedSite<'a> {
        let (blog, wiki) = rayon::join(
            || {
                self.site
                    .blog
                    .par_iter()
                    .map(|p| (p, self.render(&p.body)))
                    .collect::<Vec<_>>()
            },
            || {
                self.site
                    .wiki
                    .par_iter()
                    .map(|p| (p, self.render(&p.body)))
                    .collect::<Vec<_>>()
            },
        );

        let pages: Vec<_> = self
            .site
            .pages
            .iter()
            .map(|p| (p, self.render(&p.body)))
            .collect();

        let home = self.site.home.as_ref().map(|h| (h, self.render(&h.body)));
        let not_found = self
            .site
            .not_found
            .as_ref()
            .map(|n| (n, self.render(&n.body)));

        RenderedSite::from_parts(self.site, blog, wiki, pages, home, not_found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_site() -> Site {
        let config: crate::config::Config =
            "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        Site::from_parts(config, vec![], vec![], vec![]).unwrap()
    }

    fn rendered_with_html(html: &str) -> Rendered {
        Rendered {
            html: html.to_owned(),
            toc: vec![],
            broken_wiki_links: vec![],
            contains_mermaid: false,
        }
    }

    #[test]
    fn html_with_absolute_urls_rewrites_root_relative_attrs() {
        let cfg: crate::config::Config = "title = \"T\"\nbase_url = \"https://example.com\""
            .parse()
            .unwrap();
        let r = rendered_with_html(r#"<a href="/wiki/foo/">x</a><img src="/static/i.png">"#);
        let out = r.html_with_absolute_urls(&cfg);
        assert!(out.contains(r#"href="https://example.com/wiki/foo/""#));
        assert!(out.contains(r#"src="https://example.com/static/i.png""#));
    }

    #[test]
    fn html_with_absolute_urls_passes_through_absolute_attrs() {
        let cfg: crate::config::Config = "title = \"T\"\nbase_url = \"https://example.com\""
            .parse()
            .unwrap();
        let html = r#"<a href="https://other.com/page">link</a>"#;
        let out = rendered_with_html(html).html_with_absolute_urls(&cfg);
        assert_eq!(out, html);
    }

    #[test]
    fn renders_paragraph_and_list() {
        let body = "Hello world.\n\n- item one\n- item two\n";
        let site = empty_site();
        let rendered = MarkdownRenderer::new(&site).render(body);
        insta::assert_snapshot!(rendered.html);
    }

    #[test]
    fn broken_wiki_link_recorded_and_rendered_as_span() {
        let body = "See [[missing-page]] for details.\n";
        let site = empty_site();
        let rendered = MarkdownRenderer::new(&site).render(body);
        assert!(rendered.html.contains("class=\"wikilink broken\""));
        assert!(rendered.html.contains("missing-page"));
        assert_eq!(rendered.broken_wiki_links, vec!["missing-page"]);
    }

    #[test]
    fn toc_populated_from_headings() {
        let body = "# Section One\n\nContent.\n\n## Sub Section\n\nMore.\n";
        let site = empty_site();
        let rendered = MarkdownRenderer::new(&site).render(body);
        assert_eq!(rendered.toc.len(), 2);
        assert_eq!(rendered.toc[0].id, "section-one");
        assert_eq!(rendered.toc[0].level, 2);
        assert_eq!(rendered.toc[1].id, "sub-section");
        assert_eq!(rendered.toc[1].level, 3);
        assert!(rendered.html.contains("id=\"section-one\""));
    }

    fn site_with_wiki_page(slug: &str, title: &str) -> Site {
        use crate::content::WikiFrontmatter;
        use crate::content::page::Page;
        use std::path::PathBuf;

        let config: crate::config::Config =
            "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
        let page = Page {
            slug: slug.into(),
            body: String::new(),
            path: PathBuf::from(format!("content/wiki/{slug}.md")),
            frontmatter: WikiFrontmatter {
                title: title.to_owned(),
                category: None,
                created: None,
                updated: None,
                tags: vec![],
                draft: false,
            },
        };
        Site::from_parts(config, vec![], vec![page], vec![]).unwrap()
    }

    #[test]
    fn wiki_link_with_anchor_appends_fragment() {
        let site = site_with_wiki_page("glossary", "Glossary");
        let rendered = MarkdownRenderer::new(&site).render("See [[glossary#term]] for details.\n");
        assert!(
            rendered.html.contains(r#"href="/wiki/glossary/#term""#),
            "html was: {}",
            rendered.html
        );
        assert!(rendered.broken_wiki_links.is_empty());
    }

    #[test]
    fn wiki_link_anchor_slugifies_to_match_headings() {
        let site = site_with_wiki_page("glossary", "Glossary");
        let rendered =
            MarkdownRenderer::new(&site).render("See [[glossary#Hello World]] for details.\n");
        assert!(
            rendered
                .html
                .contains(r#"href="/wiki/glossary/#hello-world""#),
            "html was: {}",
            rendered.html
        );
    }

    #[test]
    fn wiki_link_anchor_default_display_includes_section() {
        let site = site_with_wiki_page("glossary", "Glossary");
        let rendered = MarkdownRenderer::new(&site).render("[[glossary#term]]\n");
        // Bare cross-page anchor renders as "Page Title > section"
        assert!(
            rendered.html.contains("Glossary &gt; term"),
            "html was: {}",
            rendered.html
        );
    }

    #[test]
    fn wiki_link_anchor_pipe_alias_wins() {
        let site = site_with_wiki_page("glossary", "Glossary");
        let rendered = MarkdownRenderer::new(&site).render("[[glossary#term|the term itself]]\n");
        assert!(rendered.html.contains(r#"href="/wiki/glossary/#term""#));
        assert!(rendered.html.contains(">the term itself</a>"));
    }

    #[test]
    fn same_page_anchor_link() {
        let site = empty_site();
        let rendered = MarkdownRenderer::new(&site).render("Jump to [[#summary]].\n");
        assert!(
            rendered.html.contains(r##"href="#summary""##),
            "html was: {}",
            rendered.html
        );
        assert!(rendered.broken_wiki_links.is_empty());
    }

    #[test]
    fn broken_wiki_link_with_anchor_reports_full_target() {
        let site = empty_site();
        let rendered =
            MarkdownRenderer::new(&site).render("See [[missing#section]] for details.\n");
        assert_eq!(rendered.broken_wiki_links, vec!["missing#section"]);
    }
}
