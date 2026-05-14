use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

use crate::content::{Site, Slug};
use crate::html::escape_html;

/// A `[[wiki-link]]` parsed out of a markdown body.
///
/// `target` is the page slug portion — empty for same-page anchor links
/// like `[[#section]]`. `anchor` is the in-page anchor text as written
/// by the author (slugified at render time to match heading IDs).
/// `display` is `Some` only for pipe-aliased links
/// (`[[slug|Display Text]]`); when the link author wrote a bare form,
/// `display` is `None` and a sensible default is chosen at render time.
#[derive(Debug, PartialEq)]
pub struct WikiLinkRef {
    pub target: String,
    pub anchor: Option<String>,
    pub display: Option<String>,
}

impl WikiLinkRef {
    /// Build a `WikiLinkRef` from the raw pieces emitted by pulldown-cmark
    /// for a single `[[…]]` link: `raw_target` is the verbatim `dest_url`
    /// (which may include a `#anchor` suffix); `label` is the accumulated
    /// inner text of the link (equal to `raw_target` for bare links, the
    /// alias for pipe-aliased ones).
    fn from_raw(raw_target: String, label: String) -> Self {
        let aliased = label != raw_target;
        let (slug, anchor) = match raw_target.split_once('#') {
            Some((slug, anchor)) => (slug.to_owned(), Some(anchor.to_owned())),
            None => (raw_target, None),
        };
        Self {
            target: slug,
            anchor,
            display: aliased.then_some(label),
        }
    }
}

/// One step of a wiki-link-aware walk over a markdown event stream.
enum WikiEvent<'a> {
    /// A non-wiki-link event that should be passed through unchanged.
    Other(Event<'a>),
    /// A completed wiki link.
    Link(WikiLinkRef),
}

/// Walk an event stream, detecting wiki links by their `Start`/`End`
/// boundaries. Events that are *part of* a wiki link (the `Start`, the
/// inner `Text`, the `End`) are folded into a single `WikiEvent::Link`;
/// everything else is yielded as `WikiEvent::Other`.
fn walk_with_wiki_links<'a>(
    events: impl IntoIterator<Item = Event<'a>>,
    mut handle: impl FnMut(WikiEvent<'a>),
) {
    // (raw_target_with_optional_anchor, accumulated_label_text)
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
            (Some((raw_target, label)), Event::End(TagEnd::Link)) => {
                handle(WikiEvent::Link(WikiLinkRef::from_raw(raw_target, label)));
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
/// the backlink graph before any rendering happens. Same-page anchor
/// links like `[[#section]]` are excluded — they don't backlink to
/// another page.
pub fn extract_wiki_links(input: &str) -> Vec<WikiLinkRef> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_WIKILINKS);

    let mut refs = Vec::new();
    walk_with_wiki_links(Parser::new_ext(input, opts), |event| {
        if let WikiEvent::Link(link) = event
            && !link.target.is_empty()
        {
            refs.push(link);
        }
    });
    refs
}

/// Rewrite `[[wiki-links]]` in an event stream against `site`'s slug
/// index. Resolved links become `<a class="wikilink">` HTML with an
/// optional `#anchor` fragment; unresolved targets become
/// `<span class="wikilink broken">` and are also returned in the second
/// tuple element so callers can apply the build-vs-serve broken-link
/// policy. Same-page anchor links (`[[#section]]`) render as plain
/// in-page anchors and never count as broken.
pub fn rewrite_wiki_links<'a>(
    events: Vec<Event<'a>>,
    site: &Site,
) -> (Vec<Event<'a>>, Vec<String>) {
    let mut out = Vec::with_capacity(events.len());
    let mut broken = Vec::new();

    walk_with_wiki_links(events, |event| match event {
        WikiEvent::Other(e) => out.push(e),
        WikiEvent::Link(WikiLinkRef {
            target,
            anchor,
            display,
        }) => {
            let anchor_slug = anchor.as_deref().map(|a| Slug::from(a).to_string());
            let html = if target.is_empty() {
                // Same-page anchor: [[#section]] — always renders, never broken.
                let fragment = anchor_slug.as_deref().unwrap_or("");
                let text = display
                    .or_else(|| anchor.clone())
                    .unwrap_or_else(|| fragment.to_owned());
                format!(
                    "<a href=\"#{}\" class=\"wikilink\">{}</a>",
                    fragment,
                    escape_html(&text)
                )
            } else {
                let slug: Slug = target.as_str().into();
                match site.get(&slug) {
                    Some(page) => {
                        let url = match &anchor_slug {
                            Some(a) => format!("{}#{}", page.url_path(), a),
                            None => page.url_path(),
                        };
                        let text = display.unwrap_or_else(|| match &anchor {
                            Some(a) => format!("{} > {}", page.title(), a),
                            None => page.title().to_owned(),
                        });
                        format!(
                            "<a href=\"{}\" class=\"wikilink\">{}</a>",
                            url,
                            escape_html(&text)
                        )
                    }
                    None => {
                        let broken_target = match &anchor {
                            Some(a) => format!("{target}#{a}"),
                            None => target.clone(),
                        };
                        let text = display.unwrap_or_else(|| broken_target.clone());
                        broken.push(broken_target);
                        format!(
                            "<span class=\"wikilink broken\">{}</span>",
                            escape_html(&text)
                        )
                    }
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
                anchor: None,
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
                anchor: None,
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
                anchor: None,
                display: None,
            }
        );
        assert_eq!(
            refs[1],
            WikiLinkRef {
                target: "bar".into(),
                anchor: None,
                display: Some("Bar Page".into()),
            }
        );
        assert_eq!(
            refs[2],
            WikiLinkRef {
                target: "baz".into(),
                anchor: None,
                display: None,
            }
        );
    }

    #[test]
    fn link_inside_code_block_ignored() {
        let refs = extract_wiki_links("```\n[[not-a-link]]\n```");
        assert!(refs.is_empty());
    }

    #[test]
    fn cross_page_anchor_extracted() {
        let refs = extract_wiki_links("See [[glossary#term]] for details.");
        assert_eq!(
            refs,
            vec![WikiLinkRef {
                target: "glossary".into(),
                anchor: Some("term".into()),
                display: None,
            }]
        );
    }

    #[test]
    fn cross_page_anchor_with_pipe_alias() {
        let refs = extract_wiki_links("See [[glossary#term|that term]] for details.");
        assert_eq!(
            refs,
            vec![WikiLinkRef {
                target: "glossary".into(),
                anchor: Some("term".into()),
                display: Some("that term".into()),
            }]
        );
    }

    #[test]
    fn same_page_anchor_excluded_from_backlinks() {
        // [[#section]] is in-page navigation, not a backlink to another page.
        let refs = extract_wiki_links("Jump to [[#summary]] for the gist.");
        assert!(refs.is_empty());
    }
}
