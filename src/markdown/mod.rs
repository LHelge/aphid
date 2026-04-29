pub mod alerts;
pub mod anchors;
pub mod external_links;
pub mod highlight;
pub mod wikilinks;

pub use alerts::rewrite_alerts;
pub use anchors::{HeadingEntry, inject_heading_ids};
pub use external_links::rewrite_external_links;
pub use highlight::Highlighter;
pub use wikilinks::{WikiLinkRef, extract_wiki_links, rewrite_wiki_links};

use pulldown_cmark::{Options, Parser, html};

use crate::content::Site;

pub(crate) fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);
        opts.insert(Options::ENABLE_WIKILINKS);
        opts.insert(Options::ENABLE_GFM);
        opts.insert(Options::ENABLE_SMART_PUNCTUATION);

        let events: Vec<_> = Parser::new_ext(body, opts).collect();

        let (events, broken_wiki_links) = rewrite_wiki_links(events, self.site);
        let events = rewrite_external_links(events);
        let events = rewrite_alerts(events);
        let (events, toc) = inject_heading_ids(events);
        let events = self.highlighter.transform(events);

        let mut output = String::new();
        html::push_html(&mut output, events.into_iter());

        Rendered {
            html: output,
            toc,
            broken_wiki_links,
        }
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
}
