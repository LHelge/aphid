use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

use crate::content::{Site, Slug};
use crate::html::escape_html;

/// A `[[wiki-link]]` parsed out of a markdown body. `display` is `Some`
/// only for pipe-aliased links (`[[slug|Display Text]]`) — when the
/// link author wrote bare `[[slug]]`, `display` is `None` and the
/// target's title is used at render time.
#[derive(Debug, PartialEq)]
pub struct WikiLinkRef {
    pub target: String,
    pub display: Option<String>,
}

/// One step of a wiki-link-aware walk over a markdown event stream.
enum WikiEvent<'a> {
    /// A non-wiki-link event that should be passed through unchanged.
    Other(Event<'a>),
    /// A completed wiki link with its target slug and optional display text.
    Link {
        target: String,
        display: Option<String>,
    },
}

/// Walk an event stream, detecting wiki links by their `Start`/`End`
/// boundaries. Events that are *part of* a wiki link (the `Start`, the
/// inner `Text`, the `End`) are folded into a single `WikiEvent::Link`;
/// everything else is yielded as `WikiEvent::Other`.
fn walk_with_wiki_links<'a>(
    events: impl IntoIterator<Item = Event<'a>>,
    mut handle: impl FnMut(WikiEvent<'a>),
) {
    // (target_slug_string, accumulated_label_text)
    let mut state: Option<(String, String)> = None;

    for event in events {
        match (state.take(), event) {
            (
                None,
                Event::Start(Tag::Link {
                    link_type: LinkType::WikiLink { .. },
                    dest_url,
                    ..
                }),
            ) => {
                state = Some((dest_url.to_string(), String::new()));
            }
            (None, other) => handle(WikiEvent::Other(other)),
            (Some(mut s), Event::Text(text)) => {
                s.1.push_str(&text);
                state = Some(s);
            }
            (Some(s), Event::End(TagEnd::Link)) => {
                let (target, label) = s;
                let display = if label == target { None } else { Some(label) };
                handle(WikiEvent::Link { target, display });
            }
            (Some(s), other) => {
                // Unexpected event inside a link — keep state, pass through.
                state = Some(s);
                handle(WikiEvent::Other(other));
            }
        }
    }
}

/// Scan a markdown body for `[[wiki-links]]`. Used in pass 1 to build
/// the backlink graph before any rendering happens.
pub fn extract_wiki_links(input: &str) -> Vec<WikiLinkRef> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_WIKILINKS);

    let mut refs = Vec::new();
    walk_with_wiki_links(Parser::new_ext(input, opts), |event| {
        if let WikiEvent::Link { target, display } = event {
            refs.push(WikiLinkRef { target, display });
        }
    });
    refs
}

/// Rewrite `[[wiki-links]]` in an event stream against `site`'s slug
/// index. Resolved links become `<a class="wikilink">` HTML; unresolved
/// targets become `<span class="wikilink broken">` and are also returned
/// in the second tuple element so callers can apply the build-vs-serve
/// broken-link policy.
pub fn rewrite_wiki_links<'a>(
    events: Vec<Event<'a>>,
    site: &Site,
) -> (Vec<Event<'a>>, Vec<String>) {
    let mut out = Vec::with_capacity(events.len());
    let mut broken = Vec::new();

    walk_with_wiki_links(events, |event| match event {
        WikiEvent::Other(e) => out.push(e),
        WikiEvent::Link { target, display } => {
            let slug: Slug = target.clone().into();
            let html = match site.get(&slug) {
                Some(page) => {
                    let url = page.url_path();
                    let text = display.unwrap_or_else(|| page.title().to_owned());
                    format!(
                        "<a href=\"{}\" class=\"wikilink\">{}</a>",
                        url,
                        escape_html(&text)
                    )
                }
                None => {
                    let text = display.unwrap_or_else(|| target.clone());
                    broken.push(target);
                    format!(
                        "<span class=\"wikilink broken\">{}</span>",
                        escape_html(&text)
                    )
                }
            };
            out.push(Event::InlineHtml(html.into()));
        }
    });

    (out, broken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_link() {
        let refs = extract_wiki_links("See [[glossary]] for details.");
        assert_eq!(
            refs,
            vec![WikiLinkRef {
                target: "glossary".into(),
                display: None,
            }]
        );
    }

    #[test]
    fn pipe_aliased_link() {
        let refs = extract_wiki_links("See [[glossary|the glossary]] for details.");
        assert_eq!(
            refs,
            vec![WikiLinkRef {
                target: "glossary".into(),
                display: Some("the glossary".into()),
            }]
        );
    }

    #[test]
    fn no_links() {
        let refs = extract_wiki_links("No wiki links here.");
        assert!(refs.is_empty());
    }

    #[test]
    fn multiple_links() {
        let refs = extract_wiki_links("[[foo]] and [[bar|Bar Page]] and [[baz]].");
        assert_eq!(refs.len(), 3);
        assert_eq!(
            refs[0],
            WikiLinkRef {
                target: "foo".into(),
                display: None
            }
        );
        assert_eq!(
            refs[1],
            WikiLinkRef {
                target: "bar".into(),
                display: Some("Bar Page".into()),
            }
        );
        assert_eq!(
            refs[2],
            WikiLinkRef {
                target: "baz".into(),
                display: None
            }
        );
    }

    #[test]
    fn link_inside_code_block_ignored() {
        let refs = extract_wiki_links("```\n[[not-a-link]]\n```");
        assert!(refs.is_empty());
    }
}
