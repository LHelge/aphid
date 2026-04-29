use std::fs;
use std::path::PathBuf;

use insta::assert_snapshot;
use tempfile::TempDir;

use aphid::testutil::write_file;

mod common;

/// Slice the `<main>...</main>` element out of a full HTML page so snapshots
/// don't include nav/header/footer boilerplate that's irrelevant to the
/// thing under test.
fn extract_main(html: &str) -> &str {
    let start = html.find("<main").expect("no <main> tag in output");
    let end = html.rfind("</main>").expect("no </main> tag in output");
    &html[start..end + "</main>".len()]
}

/// Build a site from the shared fixtures into a tempdir and return the
/// (tempdir, output_dir) so assertions can inspect the result.
async fn build_fixture_site() -> (TempDir, PathBuf) {
    let (dir, config_path) = common::setup_with_shared_fixtures();
    let output = dir.path().join("dist");
    write_file(&dir.path().join("static/style.css"), "body { margin: 0; }");

    aphid::build(&config_path, &output).await.unwrap();
    (dir, output)
}

#[tokio::test]
async fn blog_post_rendered_with_wiki_link() {
    let (_dir, output) = build_fixture_site().await;
    let html = fs::read_to_string(output.join("blog/first-post/index.html")).unwrap();

    assert!(html.contains("First Post"), "title missing from blog post");
    assert!(
        html.contains(r#"href="/wiki/glossary/""#),
        "expected resolved wiki-link href to glossary"
    );
}

#[tokio::test]
async fn wiki_page_has_backlinks() {
    let (_dir, output) = build_fixture_site().await;
    let html = fs::read_to_string(output.join("wiki/glossary/index.html")).unwrap();
    assert_snapshot!("wiki_glossary_main", extract_main(&html));
}

#[tokio::test]
async fn tag_pages_generated() {
    let (_dir, output) = build_fixture_site().await;

    let rust_tag = output.join("tags/rust/index.html");
    let html = fs::read_to_string(&rust_tag).expect("tag page for 'rust' missing");
    assert_snapshot!("tags_rust_main", extract_main(&html));

    assert!(
        output.join("tags/advanced/index.html").exists(),
        "tag page for 'advanced' missing"
    );

    let tags_index = output.join("tags/index.html");
    let html = fs::read_to_string(&tags_index).expect("tags index missing");
    assert!(html.contains("rust"));
}

#[tokio::test]
async fn blog_and_wiki_indexes_generated() {
    let (_dir, output) = build_fixture_site().await;

    assert!(
        output.join("index.html").exists(),
        "home page (root index.html) missing"
    );

    let blog_index = output.join("blog/index.html");
    let html = fs::read_to_string(&blog_index).expect("blog index missing");
    assert!(
        html.contains(r#"href="/blog/first-post/""#),
        "blog index should link to first-post"
    );

    let wiki_index = output.join("wiki/index.html");
    let html = fs::read_to_string(&wiki_index).expect("wiki index missing");
    assert!(
        html.contains(r#"href="/wiki/glossary/""#),
        "wiki index should link to glossary"
    );
}

#[tokio::test]
async fn standalone_page_rendered() {
    let (_dir, output) = build_fixture_site().await;
    let html = fs::read_to_string(output.join("about/index.html")).unwrap();
    assert!(html.contains("About"), "standalone page title missing");
}

#[tokio::test]
async fn static_files_copied() {
    let (_dir, output) = build_fixture_site().await;

    assert_eq!(
        fs::read_to_string(output.join("static/style.css")).unwrap(),
        "body { margin: 0; }",
        "user static file content mismatch"
    );
    assert!(
        output.join("static/css/theme.css").exists(),
        "embedded theme CSS missing"
    );
}

#[tokio::test]
async fn four_oh_four_page_generated() {
    let (_dir, output) = build_fixture_site().await;
    let html = fs::read_to_string(output.join("404.html")).unwrap();
    assert!(html.contains("Not Found") || html.contains("404"));
}

#[tokio::test]
async fn html_is_well_formed() {
    let (_dir, output) = build_fixture_site().await;

    for entry in walkdir::WalkDir::new(&output)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
    {
        let content = fs::read_to_string(entry.path()).unwrap();
        assert!(
            content.contains("<!DOCTYPE html>") || content.contains("<!doctype html>"),
            "{} missing DOCTYPE",
            entry.path().display()
        );
        assert!(
            content.contains("</html>"),
            "{} missing closing </html>",
            entry.path().display()
        );
    }
}

#[tokio::test]
async fn broken_wiki_link_fails_build() {
    let dir = TempDir::new().unwrap();
    let content_dir = dir.path().join("content");
    let config_path = common::write_fixture_config(dir.path(), &content_dir);

    write_file(
        &content_dir.join("blog/broken.md"),
        "\
---
title: Broken
slug: broken
author: Test
created: 2026-01-01
---
See [[nonexistent]] for details.",
    );

    let result = aphid::build(&config_path, &dir.path().join("dist")).await;
    let err = result.expect_err("build should fail on broken wiki-link");
    let msg = err.to_string();
    assert!(
        msg.contains("nonexistent"),
        "error should mention the broken target: {msg}"
    );
    assert!(
        msg.contains("broken"),
        "error should mention the source page: {msg}"
    );
}

#[tokio::test]
async fn build_output_does_not_contain_live_reload_script() {
    let (_dir, output) = build_fixture_site().await;

    // The live-reload script must never appear in build output — it is
    // a serve-mode-only transformation.
    for entry in walkdir::WalkDir::new(&output)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
    {
        let content = fs::read_to_string(entry.path()).unwrap();
        assert!(
            !content.contains("WebSocket"),
            "{} contains live-reload WebSocket script — must not appear in build output",
            entry.path().display()
        );
    }
}

// ── robots.txt & sitemap.xml ────────────────────────────────────────────

#[tokio::test]
async fn robots_txt_generated() {
    let (_dir, output) = build_fixture_site().await;
    let robots = fs::read_to_string(output.join("robots.txt")).unwrap();
    assert!(robots.contains("User-agent: *"));
    assert!(robots.contains("Allow: /"));
    assert!(robots.contains("Sitemap: https://example.com/sitemap.xml"));
}

#[tokio::test]
async fn sitemap_xml_generated() {
    let (_dir, output) = build_fixture_site().await;
    let sitemap = fs::read_to_string(output.join("sitemap.xml")).unwrap();

    assert!(sitemap.starts_with("<?xml"));
    assert!(sitemap.contains("<urlset"));
    // Home, blog index, wiki index should be present
    assert!(
        sitemap.contains("<loc>https://example.com/</loc>"),
        "sitemap should contain home URL"
    );
    assert!(
        sitemap.contains("<loc>https://example.com/blog/</loc>"),
        "sitemap should contain blog index URL"
    );
    assert!(
        sitemap.contains("<loc>https://example.com/wiki/</loc>"),
        "sitemap should contain wiki index URL"
    );
    // Blog posts should have lastmod
    assert!(
        sitemap.contains("<lastmod>"),
        "blog posts in sitemap should have lastmod"
    );
}

// ── Favicon ─────────────────────────────────────────────────────────────

/// Build with a test favicon and verify all generated files.
#[tokio::test]
async fn favicon_files_generated() {
    let dir = TempDir::new().unwrap();
    let config_path = common::write_fixture_config(dir.path(), &common::fixtures_dir());

    // Create a tiny 4×4 red PNG as the favicon source.
    let icon_path = dir.path().join("icon.png");
    write_tiny_png(&icon_path);

    // Add favicon to config
    append_config(
        &config_path,
        &format!("favicon = \"{}\"", icon_path.display()),
    );

    let output = dir.path().join("dist");
    write_file(&dir.path().join("static/empty"), "");
    aphid::build(&config_path, &output).await.unwrap();

    assert!(output.join("favicon.ico").exists(), "favicon.ico missing");
    assert!(
        output.join("apple-touch-icon.png").exists(),
        "apple-touch-icon.png missing"
    );
    assert!(
        output.join("android-chrome-192x192.png").exists(),
        "android-chrome-192x192.png missing"
    );
    assert!(
        output.join("android-chrome-512x512.png").exists(),
        "android-chrome-512x512.png missing"
    );
    assert!(
        output.join("site.webmanifest").exists(),
        "site.webmanifest missing"
    );
}

#[tokio::test]
async fn favicon_html_tags_injected() {
    let dir = TempDir::new().unwrap();
    let config_path = common::write_fixture_config(dir.path(), &common::fixtures_dir());

    let icon_path = dir.path().join("icon.png");
    write_tiny_png(&icon_path);
    append_config(
        &config_path,
        &format!("favicon = \"{}\"", icon_path.display()),
    );

    let output = dir.path().join("dist");
    write_file(&dir.path().join("static/empty"), "");
    aphid::build(&config_path, &output).await.unwrap();

    let html = fs::read_to_string(output.join("blog/first-post/index.html")).unwrap();
    assert!(
        html.contains(r#"href="/favicon.ico""#),
        "favicon.ico link missing from HTML head"
    );
    assert!(
        html.contains(r#"href="/apple-touch-icon.png""#),
        "apple-touch-icon link missing from HTML head"
    );
    assert!(
        html.contains(r#"href="/site.webmanifest""#),
        "webmanifest link missing from HTML head"
    );
}

#[tokio::test]
async fn no_favicon_when_not_configured() {
    let (_dir, output) = build_fixture_site().await;
    assert!(
        !output.join("favicon.ico").exists(),
        "favicon.ico should not exist when favicon is not configured"
    );
    assert!(
        !output.join("apple-touch-icon.png").exists(),
        "apple-touch-icon.png should not exist when favicon is not configured"
    );

    // robots.txt and sitemap.xml should still be present
    assert!(output.join("robots.txt").exists());
    assert!(output.join("sitemap.xml").exists());
}

/// Write a tiny 4×4 red PNG for test fixtures.
fn write_tiny_png(path: &std::path::Path) {
    use std::io::Cursor;
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        4,
        4,
        image::Rgba([255, 0, 0, 255]),
    ));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    fs::write(path, buf.into_inner()).unwrap();
}

fn append_config(config_path: &std::path::Path, extra: &str) {
    let mut content = fs::read_to_string(config_path).unwrap();
    content.push('\n');
    content.push_str(extra);
    content.push('\n');
    fs::write(config_path, content).unwrap();
}
