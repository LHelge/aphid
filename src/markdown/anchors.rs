use pulldown_cmark::{Event, Tag, TagEnd};
use std::collections::HashMap;

use crate::content::Slug;
use crate::html::escape_html;

fn slugify(text: &str) -> String {
    Slug::from(text).to_string()
}

/// One heading captured during rendering. `level` is the *output* level
/// (after the +1 shift, capped at 6), `id` is the anchor — either the
/// slugified heading text or an author-supplied `{#custom-id}` attribute.
pub struct HeadingEntry {
    pub level: u8,
    pub text: String,
    pub id: Slug,
}

/// Heading level shift: `#` in markdown becomes `<h2>`, `##` → `<h3>`, etc.
/// The page title occupies h1 and is rendered by the template, so body headings
/// start at h2. Levels are capped at h6.
const HEADING_LEVEL_OFFSET: u8 = 1;

/// Per-heading parsing state buffered between `Start(Heading)` and
/// `End(Heading)` events. Inner events are held back rather than emitted
/// inline so the opening `<hN id="…">` tag can be written first.
struct HeadingBuf<'a> {
    source_level: u8,
    /// Raw `{#…}` attribute from the source, if any. Used verbatim
    /// (modulo collision-suffixing) — not slugified, so authors keep
    /// control over the exact anchor.
    custom_id: Option<String>,
    /// Plain-text accumulator used to slugify the heading text when no
    /// custom id is supplied. Inline-code backticks are stripped.
    text: String,
    /// Inner events buffered for emission between the open and close tags.
    inner: Vec<Event<'a>>,
}

/// Walk an event stream, replacing each heading with raw HTML carrying an
/// `id` attribute, and return the collected headings for use in a
/// table-of-contents. Heading levels are shifted up by one (so `#`
/// renders as `<h2>` — the page title is `<h1>`, supplied by the
/// template) and capped at `<h6>`. The id is the author-supplied
/// `{#custom-id}` attribute when present, otherwise the slugified
/// heading text. Both share a single namespace: any collision (auto or
/// custom) gets a `-2`, `-3`, … suffix.
pub fn inject_heading_ids<'a>(events: Vec<Event<'a>>) -> (Vec<Event<'a>>, Vec<HeadingEntry>) {
    let mut out = Vec::with_capacity(events.len());
    let mut toc = Vec::new();
    let mut used_ids: HashMap<String, usize> = HashMap::new();

    let mut heading_buf: Option<HeadingBuf<'a>> = None;

    for event in events {
        match heading_buf.take() {
            None => match event {
                Event::Start(Tag::Heading { level, id, .. }) => {
                    heading_buf = Some(HeadingBuf {
                        source_level: level as u8,
                        custom_id: id.map(|s| s.to_string()),
                        text: String::new(),
                        inner: Vec::new(),
                    });
                }
                other => out.push(other),
            },
            Some(mut state) => match event {
                Event::End(TagEnd::Heading(_)) => {
                    let rendered_level = (state.source_level + HEADING_LEVEL_OFFSET).min(6);
                    let base_id = state.custom_id.unwrap_or_else(|| slugify(&state.text));
                    let id = Slug::new_raw(unique_id(&base_id, &mut used_ids));
                    toc.push(HeadingEntry {
                        level: rendered_level,
                        text: state.text,
                        id: id.clone(),
                    });
                    out.push(Event::Html(
                        format!("<h{} id=\"{}\">", rendered_level, escape_html(id.as_ref())).into(),
                    ));
                    out.extend(state.inner);
                    out.push(Event::Html(format!("</h{}>", rendered_level).into()));
                }
                other => {
                    // Accumulate text content for slug generation (strips inline-code backticks)
                    if let Event::Text(ref t) | Event::Code(ref t) = other {
                        state.text.push_str(t);
                    }
                    state.inner.push(other);
                    heading_buf = Some(state);
                }
            },
        }
    }

    (out, toc)
}

fn unique_id(base: &str, used: &mut HashMap<String, usize>) -> String {
    let count = used.entry(base.to_string()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base.to_string()
    } else {
        format!("{}-{}", base, *count)
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use pulldown_cmark::{Options, Parser};

    use super::*;

    fn render_with_anchors(input: &str) -> (String, Vec<HeadingEntry>) {
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        let events: Vec<_> = Parser::new_ext(input, opts).collect();
        let (events, toc) = inject_heading_ids(events);
        (crate::markdown::render_html(events), toc)
    }

    #[test]
    fn simple_heading_gets_id() {
        // ## in source → <h3> after +1 shift
        let (html, toc) = render_with_anchors("## Hello World\n\nSome text.");
        assert_snapshot!(html);
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].level, 3);
        assert_eq!(toc[0].text, "Hello World");
        assert_eq!(toc[0].id, "hello-world");
    }

    #[test]
    fn duplicate_headings_get_suffixed() {
        let (html, toc) = render_with_anchors("## Intro\n\n## Intro\n");
        assert_snapshot!(html);
        assert_eq!(toc[0].id, "intro");
        assert_eq!(toc[1].id, "intro-2");
        assert_eq!(toc[0].level, 3);
        assert_eq!(toc[1].level, 3);
    }

    #[test]
    fn heading_levels_shifted_by_one() {
        // # → h2, ### → h4 after +1 shift
        let (html, toc) = render_with_anchors("# H1\n\n### H3\n");
        assert_snapshot!(html);
        assert_eq!(toc.len(), 2);
        assert_eq!(toc[0].level, 2);
        assert_eq!(toc[0].id, "h1");
        assert_eq!(toc[1].level, 4);
        assert_eq!(toc[1].id, "h3");
    }

    #[test]
    fn all_source_headings_included_in_toc() {
        // All heading levels appear in TOC after shift
        let (html, toc) = render_with_anchors("# Title\n\n## Section\n\n### Sub\n");
        assert_snapshot!(html);
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].level, 2);
        assert_eq!(toc[0].text, "Title");
        assert_eq!(toc[1].level, 3);
        assert_eq!(toc[1].text, "Section");
        assert_eq!(toc[2].level, 4);
        assert_eq!(toc[2].text, "Sub");
    }

    #[test]
    fn h5_h6_capped_at_h6() {
        // ##### → h6, ###### → h6 (capped)
        let (html, toc) = render_with_anchors("##### Deep\n\n###### Deepest\n");
        assert_snapshot!(html);
        assert_eq!(toc[0].level, 6);
        assert_eq!(toc[1].level, 6);
    }

    #[test]
    fn toc_order_matches_source() {
        let (_, toc) = render_with_anchors("# A\n\n## B\n\n# C\n\n### D\n");
        let ids: Vec<&str> = toc.iter().map(|h| h.id.as_ref()).collect();
        assert_eq!(ids, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn inline_formatting_stripped_from_toc_text() {
        let (html, toc) = render_with_anchors("# _Intro_\n");
        // TOC text is plain; HTML body keeps the formatting.
        assert_snapshot!(html);
        assert_eq!(toc[0].text, "Intro");
        assert_eq!(toc[0].level, 2);
    }

    #[test]
    fn special_chars_stripped_from_slug() {
        let (_, toc) = render_with_anchors("# Hello, World!\n");
        assert_eq!(toc[0].id, "hello-world");
    }

    #[test]
    fn no_headings_produces_empty_toc() {
        let (_, toc) = render_with_anchors("Just a paragraph.\n");
        assert!(toc.is_empty());
    }

    #[test]
    fn custom_id_overrides_slug() {
        let (_, toc) = render_with_anchors("## Hello World {#greet}\n");
        assert_eq!(toc[0].id, "greet");
        assert_eq!(toc[0].text, "Hello World");
    }

    #[test]
    fn custom_id_used_verbatim_not_slugified() {
        // Custom IDs are trusted as-is: case, dots, underscores preserved.
        let (_, toc) = render_with_anchors("## Section {#API_v2.1}\n");
        assert_eq!(toc[0].id, "API_v2.1");
    }

    #[test]
    fn duplicate_custom_ids_get_suffixed() {
        let (_, toc) = render_with_anchors("## First {#same}\n\n## Second {#same}\n");
        assert_eq!(toc[0].id, "same");
        assert_eq!(toc[1].id, "same-2");
    }

    #[test]
    fn custom_id_collides_with_auto_id() {
        // Auto and custom IDs share one namespace, so a custom id matching
        // an earlier heading's slug gets suffixed.
        let (_, toc) = render_with_anchors("## intro\n\n## Other {#intro}\n");
        assert_eq!(toc[0].id, "intro");
        assert_eq!(toc[1].id, "intro-2");
    }

    #[test]
    fn custom_id_html_escaped_in_output() {
        // Defense in depth: even though pulldown-cmark won't produce
        // exotic IDs through normal `{#…}` syntax, the renderer must not
        // emit raw `"` or `<` into the attribute value.
        let (html, _) = render_with_anchors("## Heading {#weird&id}\n");
        assert!(html.contains(r#"id="weird&amp;id""#), "html was: {html}");
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(Slug::from("Hello World").to_string(), "hello-world");
        assert_eq!(
            Slug::from("The `foo` Function").to_string(),
            "the-foo-function"
        );
        assert_eq!(Slug::from("foo-bar").to_string(), "foo-bar");
        assert_eq!(Slug::from("Héllo").to_string(), "héllo");
    }
}
