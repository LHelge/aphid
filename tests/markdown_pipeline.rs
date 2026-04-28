use std::path::PathBuf;

use chrono::NaiveDate;
use insta::assert_snapshot;

use aphid::config::Config;
use aphid::content::page::Page;
use aphid::content::{BlogFrontmatter, PageFrontmatter, Site, WikiFrontmatter};
use aphid::markdown::MarkdownRenderer;

fn test_config() -> Config {
    "title = \"Test\"\nbase_url = \"http://localhost\""
        .parse()
        .unwrap()
}

fn blog_page(slug: &str, title: &str, body: &str) -> Page<BlogFrontmatter> {
    Page {
        slug: slug.into(),
        body: body.into(),
        path: PathBuf::from(format!("content/blog/{slug}.md")),
        frontmatter: BlogFrontmatter {
            title: title.into(),
            slug: slug.into(),
            author: "Alice".into(),
            created: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            updated: None,
            image: None,
            description: None,
            tags: vec![],
        },
    }
}

fn wiki_page(slug: &str, body: &str) -> Page<WikiFrontmatter> {
    Page {
        slug: slug.into(),
        body: body.into(),
        path: PathBuf::from(format!("content/wiki/{slug}.md")),
        frontmatter: WikiFrontmatter {
            title: None,
            category: None,
            created: None,
            updated: None,
            tags: vec![],
        },
    }
}

fn build_test_site() -> aphid::content::Site {
    let hello_body = "\
# Heading A

Check out [[glossary]] and [[missing-page]].

## Heading B

```rust
fn main() {}
```
";
    let followup_body = "A simple follow-up post.\n";
    let glossary_body = "See [[hello|the first post]] for context.\n";
    let internals_body = "\
# Section

```text
some plain text
```
";

    let blog = vec![
        blog_page("hello", "Hello World", hello_body),
        blog_page("followup", "Follow Up", followup_body),
    ];
    let wiki = vec![
        wiki_page("glossary", glossary_body),
        wiki_page("internals", internals_body),
    ];
    let pages: Vec<Page<PageFrontmatter>> = vec![];

    Site::from_parts(test_config(), blog, wiki, pages).unwrap()
}

#[test]
fn snapshot_hello_blog_post() {
    let site = build_test_site();
    let rendered = MarkdownRenderer::new(&site).render(&site.blog[0].body);
    assert_snapshot!(rendered.html);
}

#[test]
fn snapshot_glossary_wiki_page() {
    let site = build_test_site();
    let rendered = MarkdownRenderer::new(&site).render(&site.wiki[0].body);
    assert_snapshot!(rendered.html);
}

#[test]
fn snapshot_internals_wiki_page() {
    let site = build_test_site();
    let rendered = MarkdownRenderer::new(&site).render(&site.wiki[1].body);
    assert_snapshot!(rendered.html);
}

#[test]
fn broken_links_collected() {
    let site = build_test_site();
    let rendered = MarkdownRenderer::new(&site).render(&site.blog[0].body);
    assert_eq!(rendered.broken_wiki_links, vec!["missing-page"]);
}

#[test]
fn toc_extracted_for_headings() {
    let site = build_test_site();
    let rendered = MarkdownRenderer::new(&site).render(&site.blog[0].body);

    assert_eq!(rendered.toc.len(), 2);
    assert_eq!(rendered.toc[0].level, 2);
    assert_eq!(rendered.toc[0].text, "Heading A");
    assert_eq!(rendered.toc[0].id, "heading-a");
    assert_eq!(rendered.toc[1].level, 3);
    assert_eq!(rendered.toc[1].text, "Heading B");
    assert_eq!(rendered.toc[1].id, "heading-b");

    let internals = MarkdownRenderer::new(&site).render(&site.wiki[1].body);
    assert_eq!(internals.toc.len(), 1);
    assert_eq!(internals.toc[0].level, 2);
    assert_eq!(internals.toc[0].text, "Section");
    assert_eq!(internals.toc[0].id, "section");
}
