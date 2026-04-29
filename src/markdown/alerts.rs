use pulldown_cmark::{BlockQuoteKind, Event, Tag, TagEnd};

/// Rewrite GFM alert blockquotes (`> [!NOTE]`, `> [!TIP]`, etc.) into
/// styled `<div>` elements. Regular blockquotes (`BlockQuote(None)`)
/// pass through unchanged.
///
/// pulldown-cmark emits `Tag::BlockQuote(Some(kind))` when
/// `Options::ENABLE_GFM` is active and the blockquote starts with a
/// `[!TYPE]` marker. This transformation replaces the start/end tags
/// with raw HTML that themes can style via `.markdown-alert` classes.
pub fn rewrite_alerts(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut out = Vec::with_capacity(events.len());
    // Track nesting depth of alert blockquotes so we can pair the
    // correct `End(BlockQuote)` with the opening alert.
    let mut alert_depth: usize = 0;

    for event in events {
        match event {
            Event::Start(Tag::BlockQuote(Some(kind))) => {
                alert_depth += 1;
                let (type_str, title) = alert_meta(kind);
                let html = format!(
                    "<div class=\"markdown-alert markdown-alert-{type_str}\">\n\
                     <p class=\"markdown-alert-title\">{title}</p>\n"
                );
                out.push(Event::Html(html.into()));
            }
            Event::End(TagEnd::BlockQuote(_)) if alert_depth > 0 => {
                alert_depth -= 1;
                out.push(Event::Html("</div>\n".into()));
            }
            other => out.push(other),
        }
    }

    out
}

fn alert_meta(kind: BlockQuoteKind) -> (&'static str, &'static str) {
    match kind {
        BlockQuoteKind::Note => ("note", "Note"),
        BlockQuoteKind::Tip => ("tip", "Tip"),
        BlockQuoteKind::Important => ("important", "Important"),
        BlockQuoteKind::Warning => ("warning", "Warning"),
        BlockQuoteKind::Caution => ("caution", "Caution"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Options, Parser, html};

    fn render(input: &str) -> String {
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_GFM);
        let events: Vec<_> = Parser::new_ext(input, opts).collect();
        let events = rewrite_alerts(events);
        let mut output = String::new();
        html::push_html(&mut output, events.into_iter());
        output
    }

    #[test]
    fn note_alert() {
        let html = render("> [!NOTE]\n> Important info here.\n");
        assert!(html.contains("class=\"markdown-alert markdown-alert-note\""));
        assert!(html.contains("class=\"markdown-alert-title\">Note</p>"));
        assert!(html.contains("Important info here."));
        assert!(html.contains("</div>"));
    }

    #[test]
    fn tip_alert() {
        let html = render("> [!TIP]\n> A helpful tip.\n");
        assert!(html.contains("markdown-alert-tip"));
        assert!(html.contains(">Tip</p>"));
    }

    #[test]
    fn important_alert() {
        let html = render("> [!IMPORTANT]\n> Crucial info.\n");
        assert!(html.contains("markdown-alert-important"));
        assert!(html.contains(">Important</p>"));
    }

    #[test]
    fn warning_alert() {
        let html = render("> [!WARNING]\n> Be careful.\n");
        assert!(html.contains("markdown-alert-warning"));
        assert!(html.contains(">Warning</p>"));
    }

    #[test]
    fn caution_alert() {
        let html = render("> [!CAUTION]\n> Dangerous action.\n");
        assert!(html.contains("markdown-alert-caution"));
        assert!(html.contains(">Caution</p>"));
    }

    #[test]
    fn regular_blockquote_unchanged() {
        let html = render("> Just a normal quote.\n");
        assert!(html.contains("<blockquote>"));
        assert!(!html.contains("markdown-alert"));
    }

    #[test]
    fn multi_paragraph_alert() {
        let input = "> [!NOTE]\n> First paragraph.\n>\n> Second paragraph.\n";
        let html = render(input);
        assert!(html.contains("markdown-alert-note"));
        assert!(html.contains("First paragraph."));
        assert!(html.contains("Second paragraph."));
    }
}
