use std::time::Duration;

use tempfile::TempDir;

use aphid::testutil::write_file;

mod common;

/// Build a fixture site in a tempdir, return (tempdir, config_path).
fn setup_fixture() -> (TempDir, std::path::PathBuf) {
    let (dir, config_path) = common::setup_with_shared_fixtures();
    write_file(&dir.path().join("static/style.css"), "body { margin: 0; }");
    (dir, config_path)
}

#[tokio::test]
async fn serve_blog_post_returns_200() {
    let (_dir, config_path) = setup_fixture();
    let (port, _state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/blog/first-post/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert!(body.contains("</html>"), "should be full HTML page");

    handle.abort();
}

#[tokio::test]
async fn serve_wiki_page_returns_200() {
    let (_dir, config_path) = setup_fixture();
    let (port, _state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/wiki/glossary/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    handle.abort();
}

#[tokio::test]
async fn serve_static_asset_returns_200() {
    let (_dir, config_path) = setup_fixture();
    let (port, _state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/static/style.css"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert!(body.contains("margin: 0"), "should serve the CSS file");

    handle.abort();
}

#[tokio::test]
async fn serve_unknown_path_returns_404() {
    let (_dir, config_path) = setup_fixture();
    let (port, _state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/nonexistent/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    handle.abort();
}

#[tokio::test]
async fn serve_html_contains_live_reload_script() {
    let (_dir, config_path) = setup_fixture();
    let (port, _state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/blog/first-post/"))
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("WebSocket"),
        "serve-mode HTML should contain live-reload script"
    );

    handle.abort();
}

#[tokio::test]
async fn serve_broken_wiki_link_still_serves() {
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
See [[missing]] for details.",
    );

    let (port, _state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/blog/broken/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "broken link should not prevent serving");

    let body = resp.text().await.unwrap();
    assert!(
        body.contains("wikilink broken"),
        "HTML should contain broken wikilink span"
    );

    handle.abort();
}

#[tokio::test]
async fn serve_broken_wiki_link_in_home_still_serves() {
    let dir = TempDir::new().unwrap();
    let content_dir = dir.path().join("content");
    let config_path = common::write_fixture_config(dir.path(), &content_dir);
    let theme_dir = dir.path().join("theme");

    write_file(
        &content_dir.join("home.md"),
        "# Welcome\n\nSee [[missing-home-link]] for details.\n",
    );
    write_file(
        &theme_dir.join("theme.toml"),
        "name = \"test\"\nversion = \"0.1.0\"\n",
    );
    for template in [
        "base.html",
        "blog_post.html",
        "blog_index.html",
        "wiki_page.html",
        "wiki_index.html",
        "page.html",
        "tag.html",
        "tags_index.html",
        "404.html",
    ] {
        write_file(
            &theme_dir.join("templates").join(template),
            "{% block content %}{% endblock content %}",
        );
    }
    write_file(
        &theme_dir.join("templates/home.html"),
        r#"{% extends "base.html" %}
{% block content %}{{ home.content | safe }}{% endblock content %}
"#,
    );
    let mut config = std::fs::read_to_string(&config_path).unwrap();
    config.push_str(&format!("theme_dir = \"{}\"\n", theme_dir.display()));
    std::fs::write(&config_path, config).unwrap();

    let (port, _state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "broken link should not prevent serving");

    let body = resp.text().await.unwrap();
    assert!(
        body.contains("wikilink broken"),
        "home page HTML should contain broken wikilink span"
    );

    handle.abort();
}

#[tokio::test]
async fn serve_websocket_receives_reload() {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;

    let (_dir, config_path) = setup_fixture();
    let (port, state, handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test()
        .await
        .unwrap();

    let (mut ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .expect("WebSocket connect failed");

    // Simulate a rebuild notification
    let _ = state.reload_tx.send(());

    let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("timed out waiting for reload message")
        .expect("stream ended")
        .expect("ws error");

    assert_eq!(
        msg.to_text().unwrap(),
        "reload",
        "should receive reload message"
    );

    handle.abort();
}

#[tokio::test]
async fn watcher_triggers_reload_on_file_change() {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;

    // Need a writable source dir, so copy the seed content into a tempdir.
    let dir = TempDir::new().unwrap();
    let content_dir = dir.path().join("content");
    let config_path = common::write_fixture_config(dir.path(), &content_dir);

    let post_path = content_dir.join("blog/test.md");
    write_file(
        &post_path,
        "---\ntitle: Initial\nslug: test\nauthor: Test\ncreated: 2026-01-01\n---\nInitial body.\n",
    );

    let (port, _state, server_handle, watcher_handle) = aphid::serve::Server::new(&config_path)
        .unwrap()
        .spawn_test_with_watcher()
        .await
        .unwrap();

    let (mut ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .expect("WebSocket connect failed");

    // Small grace period so the WS handler has subscribed to the broadcast
    // before we trigger the file event.
    tokio::time::sleep(Duration::from_millis(100)).await;

    write_file(
        &post_path,
        "---\ntitle: Initial\nslug: test\nauthor: Test\ncreated: 2026-01-01\n---\nUpdated body.\n",
    );

    // Watcher debounces 200ms then rebuilds; allow generously for slow CI.
    let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("timed out waiting for reload after file change")
        .expect("stream ended")
        .expect("ws error");

    assert_eq!(msg.to_text().unwrap(), "reload");

    let body = reqwest::get(format!("http://127.0.0.1:{port}/blog/test/"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains("Updated body."),
        "rebuild should serve updated content; got body: {body}"
    );

    server_handle.abort();
    watcher_handle.abort();
}
