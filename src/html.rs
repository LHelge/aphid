pub(crate) fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn insert_before_closing_tag(
    html: &mut String,
    closing_tag: &str,
    content: &str,
) -> bool {
    if let Some(position) = html.find(closing_tag) {
        html.insert_str(position, content);
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{escape_html, insert_before_closing_tag};

    #[test]
    fn escapes_html_text_and_attributes() {
        assert_eq!(
            escape_html("Tom & \"Jerry\" <tag>"),
            "Tom &amp; &quot;Jerry&quot; &lt;tag&gt;"
        );
    }

    #[test]
    fn inserts_before_matching_closing_tag() {
        let mut html = String::from("<html><head></head><body></body></html>");

        let inserted = insert_before_closing_tag(&mut html, "</head>", "<meta>");

        assert!(inserted);
        assert_eq!(html, "<html><head><meta></head><body></body></html>");
    }

    #[test]
    fn returns_false_when_closing_tag_is_missing() {
        let mut html = String::from("<html><body></body></html>");

        let inserted = insert_before_closing_tag(&mut html, "</head>", "<meta>");

        assert!(!inserted);
        assert_eq!(html, "<html><body></body></html>");
    }
}
