use pulldown_cmark::{Event, LinkType, Tag, TagEnd};

use crate::html::escape_html;

/// Rewrite `http://` and `https://` markdown links to open in a new tab
/// with `rel="noopener noreferrer"`. Wiki-links are left alone — they're
/// always internal and are handled by [`super::wikilinks`] earlier in the
/// pipeline. Markdown disallows nested links, so a single boolean is
/// enough to pair the rewritten `End(Link)` with its `Start(Link)`.
pub fn rewrite_external_links(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut out = Vec::with_capacity(events.len());
    let mut inside_external = false;

    for event in events {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            }) if !matches!(link_type, LinkType::WikiLink { .. }) && is_external(&dest_url) => {
                let title_attr = if title.is_empty() {
                    String::new()
                } else {
                    format!(" title=\"{}\"", escape_html(&title))
                };
                let html = format!(
                    "<a href=\"{}\"{} target=\"_blank\" rel=\"noopener noreferrer\">",
                    escape_html(&dest_url),
                    title_attr
                );
                out.push(Event::InlineHtml(html.into()));
                inside_external = true;
            }
            Event::End(TagEnd::Link) if inside_external => {
                out.push(Event::InlineHtml("</a>".into()));
                inside_external = false;
            }
            other => out.push(other),
        }
    }

    out
}

fn is_external(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Options, Parser};

    fn render(input: &str) -> String {
        let events: Vec<_> = Parser::new_ext(input, Options::ENABLE_WIKILINKS).collect();
        let events = rewrite_external_links(events);
        crate::markdown::render_html(events)
    }

    #[test]
    fn https_link_gets_target_and_rel() {
        let html = render("See [example](https://example.com) please.");
        assert!(html.contains("href=\"https://example.com\""));
        assert!(html.contains("target=\"_blank\""));
        assert!(html.contains("rel=\"noopener noreferrer\""));
    }

    #[test]
    fn http_link_gets_target_and_rel() {
        let html = render("See [example](http://example.com).");
        assert!(html.contains("target=\"_blank\""));
        assert!(html.contains("rel=\"noopener noreferrer\""));
    }

    #[test]
    fn relative_link_left_alone() {
        let html = render("See [about](/about/).");
        assert!(!html.contains("target=\"_blank\""));
        assert!(!html.contains("rel=\"noopener"));
        assert!(html.contains("href=\"/about/\""));
    }

    #[test]
    fn fragment_and_mailto_left_alone() {
        let html = render("[top](#top) and [mail](mailto:a@b.c)");
        assert!(!html.contains("target=\"_blank\""));
    }

    #[test]
    fn link_title_is_preserved() {
        let html = render("[ex](https://example.com \"Example Site\")");
        assert!(html.contains("title=\"Example Site\""));
        assert!(html.contains("target=\"_blank\""));
    }

    #[test]
    fn link_text_formatting_is_preserved() {
        let html = render("[**bold** text](https://example.com)");
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("target=\"_blank\""));
    }

    #[test]
    fn multiple_links_in_one_paragraph() {
        let html = render("[a](https://a.example) and [b](/b) and [c](https://c.example).");
        let blanks = html.matches("target=\"_blank\"").count();
        assert_eq!(blanks, 2);
    }
}
