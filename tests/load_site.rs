use std::fs;

use tempfile::TempDir;

use aphid::Error;
use aphid::content::{PageAny, Site, Slug};

mod common;

fn fixtures_config() -> aphid::config::Config {
    let mut cfg: aphid::config::Config = "title = \"Test Site\"\nbase_url = \"http://localhost\""
        .parse()
        .unwrap();
    cfg.source_dir = common::fixtures_dir();
    cfg
}

#[test]
fn page_counts_and_slugs() {
    let site = Site::load(fixtures_config()).unwrap();

    assert_eq!(site.blog.len(), 2);
    assert_eq!(site.wiki.len(), 3);
    assert_eq!(site.pages.len(), 1);

    let blog_slugs: Vec<String> = site.blog.iter().map(|p| p.slug.to_string()).collect();
    assert_eq!(blog_slugs, ["second-post", "first-post"]);

    let wiki_slugs: Vec<String> = site.wiki.iter().map(|p| p.slug.to_string()).collect();
    assert_eq!(wiki_slugs, ["glossary", "internals", "syntax"]);

    assert_eq!(site.pages[0].slug.to_string(), "about");
}

#[test]
fn site_get_resolves_all_variants() {
    let site = Site::load(fixtures_config()).unwrap();

    assert!(matches!(
        site.get(&Slug::from("first-post")),
        Some(PageAny::Blog(_))
    ));
    assert!(matches!(
        site.get(&Slug::from("second-post")),
        Some(PageAny::Blog(_))
    ));
    assert!(matches!(
        site.get(&Slug::from("glossary")),
        Some(PageAny::Wiki(_))
    ));
    assert!(matches!(
        site.get(&Slug::from("syntax")),
        Some(PageAny::Wiki(_))
    ));
    assert!(matches!(
        site.get(&Slug::from("internals")),
        Some(PageAny::Wiki(_))
    ));
    assert!(matches!(
        site.get(&Slug::from("about")),
        Some(PageAny::Page(_))
    ));
    assert!(site.get(&Slug::from("nonexistent")).is_none());
}

#[test]
fn tag_index_merges_blog_and_wiki() {
    let site = Site::load(fixtures_config()).unwrap();

    // "rust" tag appears on both blog posts; standalone pages are excluded
    let mut rust_slugs: Vec<String> = site
        .tag_index
        .get("rust")
        .unwrap()
        .iter()
        .map(|s| s.to_string())
        .collect();
    rust_slugs.sort();
    assert_eq!(rust_slugs, ["first-post", "second-post"]);

    // "advanced" tag appears on second-post (blog) and internals (wiki)
    let mut adv_slugs: Vec<String> = site
        .tag_index
        .get("advanced")
        .unwrap()
        .iter()
        .map(|s| s.to_string())
        .collect();
    adv_slugs.sort();
    assert_eq!(adv_slugs, ["internals", "second-post"]);

    // "reference" tag is wiki-only
    let ref_slugs: Vec<String> = site
        .tag_index
        .get("reference")
        .unwrap()
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(ref_slugs, ["glossary"]);

    // standalone pages contribute no tags
    assert!(
        !site
            .tag_index
            .values()
            .flatten()
            .any(|s| s.as_ref() == "about")
    );
}

#[test]
fn backlinks_for_glossary() {
    let site = Site::load(fixtures_config()).unwrap();

    // first-post, second-post, internals, about all link to [[glossary]]
    let mut refs: Vec<String> = site
        .backlinks
        .get(&Slug::from("glossary"))
        .unwrap()
        .iter()
        .map(|s| s.to_string())
        .collect();
    refs.sort();
    assert_eq!(refs, ["about", "first-post", "internals", "second-post"]);
}

#[test]
fn backlinks_for_syntax() {
    let site = Site::load(fixtures_config()).unwrap();

    // second-post and glossary link to [[syntax]]
    let mut refs: Vec<String> = site
        .backlinks
        .get(&Slug::from("syntax"))
        .unwrap()
        .iter()
        .map(|s| s.to_string())
        .collect();
    refs.sort();
    assert_eq!(refs, ["glossary", "second-post"]);
}

#[test]
fn slug_collision_is_error() {
    let dir = TempDir::new().unwrap();
    let src = dir.path();

    // blog post with slug "about"
    fs::create_dir_all(src.join("blog")).unwrap();
    fs::write(
        src.join("blog/about.md"),
        "\
---
title: About Post
slug: about
author: Alice
created: 2024-01-01
---
Body.
",
    )
    .unwrap();

    // standalone page whose filename stem is also "about"
    fs::create_dir_all(src.join("pages")).unwrap();
    fs::write(
        src.join("pages/about.md"),
        "\
---
title: About Page
---
Body.
",
    )
    .unwrap();

    let mut cfg: aphid::config::Config = "title = \"T\"\nbase_url = \"http://x\"".parse().unwrap();
    cfg.source_dir = src.to_path_buf();

    assert!(matches!(Site::load(cfg), Err(Error::SlugCollision { .. })));
}

#[test]
fn wiki_categories_loaded() {
    let site = Site::load(fixtures_config()).unwrap();

    let glossary = site.wiki.iter().find(|p| p.slug == "glossary").unwrap();
    assert_eq!(glossary.frontmatter.category.as_deref(), Some("Reference"));

    let internals = site.wiki.iter().find(|p| p.slug == "internals").unwrap();
    assert_eq!(internals.frontmatter.category.as_deref(), Some("Advanced"));

    let syntax = site.wiki.iter().find(|p| p.slug == "syntax").unwrap();
    assert!(syntax.frontmatter.category.is_none());
}
