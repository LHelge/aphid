use pulldown_cmark::{CowStr, Event, LinkType, Tag};

/// Rewrite relative URLs in link and image events to be root-relative.
/// A path like `static/blog/image.png` becomes `/static/blog/image.png`.
/// Already-absolute paths (`/...`), external URLs (`http(s)://`), fragments
/// (`#`), and schemes like `mailto:` are left untouched.
pub fn rewrite_relative_urls(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    events
        .into_iter()
        .map(|event| match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) if !matches!(link_type, LinkType::WikiLink { .. }) && needs_rewrite(&dest_url) => {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url: prepend_slash(dest_url),
                    title,
                    id,
                })
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) if needs_rewrite(&dest_url) => Event::Start(Tag::Image {
                link_type,
                dest_url: prepend_slash(dest_url),
                title,
                id,
            }),
            other => other,
        })
        .collect()
}

fn needs_rewrite(url: &str) -> bool {
    !url.is_empty()
        && !url.starts_with('/')
        && !url.starts_with('#')
        && !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.contains(':')
}

fn prepend_slash(url: CowStr<'_>) -> CowStr<'_> {
    CowStr::from(format!("/{url}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Options, Parser};

    fn render(input: &str) -> String {
        let events: Vec<_> = Parser::new_ext(input, Options::empty()).collect();
        let events = rewrite_relative_urls(events);
        crate::markdown::render_html(events)
    }

    #[test]
    fn relative_image_gets_leading_slash() {
        let html = render("![alt](static/blog/image.png)");
        assert!(html.contains("src=\"/static/blog/image.png\""));
    }

    #[test]
    fn relative_link_gets_leading_slash() {
        let html = render("[click](pages/about.html)");
        assert!(html.contains("href=\"/pages/about.html\""));
    }

    #[test]
    fn root_relative_unchanged() {
        let html = render("![alt](/static/blog/image.png)");
        assert!(html.contains("src=\"/static/blog/image.png\""));
    }

    #[test]
    fn absolute_url_unchanged() {
        let html = render("[ex](https://example.com/img.png)");
        assert!(html.contains("href=\"https://example.com/img.png\""));
    }

    #[test]
    fn fragment_unchanged() {
        let html = render("[top](#heading)");
        assert!(html.contains("href=\"#heading\""));
    }

    #[test]
    fn mailto_unchanged() {
        let html = render("[mail](mailto:a@b.c)");
        assert!(html.contains("href=\"mailto:a@b.c\""));
    }
}
