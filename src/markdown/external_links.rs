use pulldown_cmark::{Event, LinkType, Tag, TagEnd};
use url::Url;

use crate::html::escape_html;

/// Rewrite `http(s)://` markdown links that point off-site to open in a
/// new tab with `rel="noopener noreferrer"`. Absolute URLs whose host
/// matches the site's `base_url` are treated as internal — same site,
/// no `_blank`. Wiki-links are left alone — they're always internal and
/// are handled by [`super::wikilinks`] earlier in the pipeline. Markdown
/// disallows nested links, so a single boolean is enough to pair the
/// rewritten `End(Link)` with its `Start(Link)`.
pub fn rewrite_external_links<'a>(events: Vec<Event<'a>>, base_url: &Url) -> Vec<Event<'a>> {
    let mut out = Vec::with_capacity(events.len());
    let mut inside_external = false;

    for event in events {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            }) if !matches!(link_type, LinkType::WikiLink { .. })
                && is_external(&dest_url, base_url) =>
            {
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

/// An `http(s)://` URL that points to a host other than the site's own.
/// Anything that doesn't parse, doesn't use http(s), or shares a host
/// with `base_url` is treated as internal.
fn is_external(url: &str, base_url: &Url) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return false;
    }
    parsed.host() != base_url.host()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Options, Parser};

    fn render(input: &str) -> String {
        render_with_base(input, "https://aphid.example")
    }

    fn render_with_base(input: &str, base_url: &str) -> String {
        let base = Url::parse(base_url).unwrap();
        let events: Vec<_> = Parser::new_ext(input, Options::ENABLE_WIKILINKS).collect();
        let events = rewrite_external_links(events, &base);
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

    #[test]
    fn absolute_link_to_own_host_treated_as_internal() {
        // A markdown link written with the site's full URL (e.g. copied
        // from the published version) shouldn't open in a new tab — it's
        // the same site.
        let html = render_with_base(
            "See [self](https://aphid.example/wiki/foo/).",
            "https://aphid.example",
        );
        assert!(!html.contains("target=\"_blank\""));
        assert!(!html.contains("rel=\"noopener"));
    }

    #[test]
    fn different_host_is_external_even_when_subdomain() {
        // www.x.com vs x.com are different hosts per the URL spec.
        let html = render_with_base(
            "[other](https://www.aphid.example/page)",
            "https://aphid.example",
        );
        assert!(html.contains("target=\"_blank\""));
    }
}
