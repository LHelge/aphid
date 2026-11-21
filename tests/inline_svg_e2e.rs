//! End-to-end tests for build-time SVG inlining (`![alt](x.svg#inline)`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use aphid::testutil::write_file;

mod common;

const DRAWING: &str = r##"<?xml version="1.0"?>
<!-- generator comment -->
<svg xmlns="http://www.w3.org/2000/svg" class="wirebug" viewBox="0 0 10 10">
  <defs><marker id="tip"><path d="M0 0"/></marker></defs>
  <line class="wire" data-color="red" stroke="red" marker-end="url(#tip)"/>
</svg>
"##;

/// A writable site in a tempdir with one blog post whose body is `body`.
fn site_with_post(body: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let content_dir = dir.path().join("content");
    let config_path = common::write_fixture_config(dir.path(), &content_dir);
    write_post(&content_dir, body);
    (dir, config_path)
}

fn write_post(content_dir: &Path, body: &str) {
    write_file(
        &content_dir.join("blog/wiring.md"),
        &format!(
            "---\ntitle: Wiring\nslug: wiring\nauthor: Test\ncreated: 2026-01-01\n---\n{body}"
        ),
    );
}

#[tokio::test]
async fn build_inlines_marked_svgs_and_leaves_others_alone() {
    let (dir, config_path) = site_with_post(
        "\
![Harness A](/static/drawings/a.svg#inline \"The A harness\")

![Harness B](static/drawings/a.svg#inline)

![Logo](/static/drawings/a.svg)
",
    );
    write_file(&dir.path().join("static/drawings/a.svg"), DRAWING);
    let output = dir.path().join("dist");

    let options = aphid::BuildOptions::default().precompress(true);
    aphid::build_with(&config_path, &output, options)
        .await
        .unwrap();

    let html = fs::read_to_string(output.join("blog/wiring/index.html")).unwrap();
    assert_eq!(html.matches(r#"<figure class="inline-svg">"#).count(), 2);
    assert!(html.contains(r#"role="img" aria-label="Harness A""#));
    assert!(html.contains("<figcaption>The A harness</figcaption>"));
    assert_eq!(html.matches("<figcaption>").count(), 1);
    assert!(html.contains(r#"class="wirebug" viewBox="0 0 10 10""#));
    assert!(!html.contains("<?xml") && !html.contains("generator comment"));

    // The two instances' ids (and their references) don't collide.
    let ids: Vec<&str> = html
        .match_indices("<marker id=\"")
        .map(|(i, m)| {
            let rest = &html[i + m.len()..];
            &rest[..rest.find('"').unwrap()]
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    for id in &ids {
        assert!(html.contains(&format!("url(#{id})")), "reference to {id}");
    }

    // The unmarked image is still an <img>.
    assert!(html.contains(r#"<img src="/static/drawings/a.svg" alt="Logo" />"#));

    // The file is still copied, and precompression still runs.
    assert!(output.join("static/drawings/a.svg").exists());
    assert!(output.join("blog/wiring/index.html.br").exists());

    // Feeds carry the inlined markup too.
    let atom = fs::read_to_string(output.join("feed.xml")).unwrap();
    assert!(
        atom.contains("inline-svg"),
        "atom feed should embed the SVG"
    );
}

#[tokio::test]
async fn missing_svg_fails_build_naming_page_and_path() {
    let (dir, config_path) = site_with_post("![A](/static/nope.svg#inline)\n");
    let err = aphid::build(&config_path, &dir.path().join("dist"))
        .await
        .expect_err("build should fail on a missing inline SVG");
    let msg = err.to_string();
    assert!(msg.contains("wiring"), "should name the page: {msg}");
    assert!(
        msg.contains("/static/nope.svg"),
        "should name the path: {msg}"
    );
    assert!(msg.contains("not found"), "should say why: {msg}");
    assert!(!dir.path().join("dist/index.html").exists());
}

#[tokio::test]
async fn invalid_svg_fails_build() {
    let (dir, config_path) = site_with_post("![A](/static/bad.svg#inline)\n");
    write_file(&dir.path().join("static/bad.svg"), "<svg><g></svg>");
    let msg = aphid::build(&config_path, &dir.path().join("dist"))
        .await
        .expect_err("build should fail on an invalid inline SVG")
        .to_string();
    assert!(msg.contains("/static/bad.svg"), "{msg}");
    assert!(msg.contains("not valid SVG"), "{msg}");
}

#[tokio::test]
async fn serve_rebuilds_when_inlined_svg_changes() {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;

    let (dir, config_path) = site_with_post("![A](/static/live.svg#inline)\n");
    let svg_path = dir.path().join("static/live.svg");
    write_file(&svg_path, r#"<svg class="before"/>"#);

    let (port, _state, server_handle, watcher_handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test_with_watcher()
        .await
        .unwrap();

    let (mut ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .expect("WebSocket connect failed");
    tokio::time::sleep(Duration::from_millis(100)).await;

    write_file(&svg_path, r#"<svg class="after"/>"#);

    let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("timed out waiting for reload after SVG change")
        .expect("stream ended")
        .expect("ws error");
    assert_eq!(msg.to_text().unwrap(), "reload");

    let body = reqwest::get(format!("http://127.0.0.1:{port}/blog/wiring/"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains(r#"class="after""#), "got: {body}");

    server_handle.abort();
    watcher_handle.abort();
}

#[tokio::test]
async fn serve_keeps_running_when_inlined_svg_is_missing() {
    let (_dir, config_path) = site_with_post("![Pending drawing](/static/later.svg#inline)\n");
    let (port, _state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/blog/wiring/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains(r#"class="inline-svg broken""#), "got: {body}");
    assert!(body.contains("Pending drawing"));

    handle.abort();
}
