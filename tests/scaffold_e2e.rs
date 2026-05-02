use std::fs;

use tempfile::TempDir;

fn scaffold_new(name: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let site_dir = dir.path().join(name);
    aphid::scaffold_new(site_dir.to_str().unwrap()).unwrap();
    (dir, site_dir)
}

// ── aphid new ───────────────────────────────────────────────────────────────

#[test]
fn new_creates_all_expected_files() {
    let (_dir, site) = scaffold_new("test-site");

    assert!(site.join("aphid.toml").exists());
    assert!(site.join(".gitignore").exists());
    assert!(site.join("content/home.md").exists());
    assert!(site.join("content/pages/about.md").exists());
    assert!(site.join("content/wiki/getting-started.md").exists());
    assert!(site.join("static").is_dir());

    let blog_entries: Vec<_> = fs::read_dir(site.join("content/blog"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert_eq!(blog_entries.len(), 1, "expected exactly one blog post");
}

#[test]
fn new_derives_title_from_name() {
    let (_dir, site) = scaffold_new("my-cool-blog");

    let config = fs::read_to_string(site.join("aphid.toml")).unwrap();
    assert!(
        config.contains(r#"title = "My Cool Blog""#),
        "expected title derived from directory name, got:\n{config}"
    );
}

#[test]
fn new_produces_buildable_site() {
    let (_dir, site) = scaffold_new("buildable");

    let config_path = site.join("aphid.toml");
    let output = site.join("dist");

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(aphid::build(&config_path, &output))
        .unwrap();

    assert!(output.join("index.html").exists(), "missing home page");
    assert!(
        output.join("blog/index.html").exists(),
        "missing blog index"
    );
    assert!(
        output.join("wiki/index.html").exists(),
        "missing wiki index"
    );
    assert!(
        output.join("about/index.html").exists(),
        "missing about page"
    );
}

#[test]
fn new_fails_when_directory_exists() {
    let dir = TempDir::new().unwrap();
    let site = dir.path().join("exists");
    fs::create_dir(&site).unwrap();

    let err = aphid::scaffold_new(site.to_str().unwrap()).unwrap_err();
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );
}

#[test]
fn new_gitignore_contains_dist() {
    let (_dir, site) = scaffold_new("gi-check");

    let gitignore = fs::read_to_string(site.join(".gitignore")).unwrap();
    assert!(gitignore.contains("/dist"), "gitignore should ignore /dist");
}

// ── aphid init ──────────────────────────────────────────────────────────────

#[test]
fn init_scaffolds_in_existing_directory() {
    let dir = TempDir::new().unwrap();

    aphid::scaffold_init(dir.path()).unwrap();

    assert!(dir.path().join("aphid.toml").exists());
    assert!(dir.path().join("content/blog").is_dir());
    assert!(dir.path().join("content/wiki/getting-started.md").exists());
}

#[test]
fn init_fails_when_config_exists() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("aphid.toml"), "").unwrap();

    let err = aphid::scaffold_init(dir.path()).unwrap_err();
    assert!(
        err.to_string().contains("already contains an aphid.toml"),
        "unexpected error: {err}"
    );
}

#[test]
fn init_creates_missing_directory() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("deep/nested/site");

    aphid::scaffold_init(&nested).unwrap();

    assert!(nested.join("aphid.toml").exists());
}

#[test]
fn init_produces_buildable_site() {
    let dir = TempDir::new().unwrap();

    aphid::scaffold_init(dir.path()).unwrap();

    let config_path = dir.path().join("aphid.toml");
    let output = dir.path().join("dist");

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(aphid::build(&config_path, &output))
        .unwrap();

    assert!(output.join("index.html").exists());
}

// ── aphid blog new ──────────────────────────────────────────────────────────

#[test]
fn blog_new_creates_post_with_todays_date() {
    let (_dir, site) = scaffold_new("blog-test");
    let config_path = site.join("aphid.toml");

    aphid::new_blog_post(&config_path, "My Second Post").unwrap();

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let expected = site
        .join("content/blog")
        .join(format!("{today}_my-second-post.md"));
    assert!(
        expected.exists(),
        "blog post file not found at {expected:?}"
    );

    let content = fs::read_to_string(&expected).unwrap();
    assert!(content.contains("title: My Second Post"));
    assert!(content.contains("slug: my-second-post"));
    assert!(content.contains(&format!("created: {today}")));
}

#[test]
fn blog_new_post_is_buildable() {
    let (_dir, site) = scaffold_new("blog-build");
    let config_path = site.join("aphid.toml");

    aphid::new_blog_post(&config_path, "Extra Post").unwrap();

    let output = site.join("dist");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(aphid::build(&config_path, &output))
        .unwrap();

    assert!(
        output.join("blog/extra-post/index.html").exists(),
        "new blog post should appear in build output"
    );
}

#[test]
fn blog_new_rejects_duplicate_slug() {
    let (_dir, site) = scaffold_new("blog-dup");
    let config_path = site.join("aphid.toml");

    aphid::new_blog_post(&config_path, "Unique Title").unwrap();
    let err = aphid::new_blog_post(&config_path, "Unique Title").unwrap_err();
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );
}

// ── aphid wiki new ──────────────────────────────────────────────────────────

#[test]
fn wiki_new_creates_page() {
    let (_dir, site) = scaffold_new("wiki-test");
    let config_path = site.join("aphid.toml");

    aphid::new_wiki_page(&config_path, "Architecture Overview").unwrap();

    let expected = site.join("content/wiki/architecture-overview.md");
    assert!(expected.exists());

    let content = fs::read_to_string(&expected).unwrap();
    assert!(content.contains("title: Architecture Overview"));
}

#[test]
fn wiki_new_page_is_buildable() {
    let (_dir, site) = scaffold_new("wiki-build");
    let config_path = site.join("aphid.toml");

    aphid::new_wiki_page(&config_path, "Deployment Guide").unwrap();

    let output = site.join("dist");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(aphid::build(&config_path, &output))
        .unwrap();

    assert!(
        output.join("wiki/deployment-guide/index.html").exists(),
        "new wiki page should appear in build output"
    );
}

#[test]
fn wiki_new_rejects_duplicate_slug() {
    let (_dir, site) = scaffold_new("wiki-dup");
    let config_path = site.join("aphid.toml");

    aphid::new_wiki_page(&config_path, "Some Topic").unwrap();
    let err = aphid::new_wiki_page(&config_path, "Some Topic").unwrap_err();
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );
}

// ── aphid page new ──────────────────────────────────────────────────────────

#[test]
fn page_new_creates_page() {
    let (_dir, site) = scaffold_new("page-test");
    let config_path = site.join("aphid.toml");

    aphid::new_page(&config_path, "Contact").unwrap();

    let expected = site.join("content/pages/contact.md");
    assert!(expected.exists());

    let content = fs::read_to_string(&expected).unwrap();
    assert!(content.contains("title: Contact"));
}

#[test]
fn page_new_is_buildable() {
    let (_dir, site) = scaffold_new("page-build");
    let config_path = site.join("aphid.toml");

    aphid::new_page(&config_path, "Contact").unwrap();

    let output = site.join("dist");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(aphid::build(&config_path, &output))
        .unwrap();

    assert!(
        output.join("contact/index.html").exists(),
        "new standalone page should appear in build output"
    );
}

#[test]
fn page_new_rejects_duplicate_slug() {
    let (_dir, site) = scaffold_new("page-dup");
    let config_path = site.join("aphid.toml");

    aphid::new_page(&config_path, "Contact").unwrap();
    let err = aphid::new_page(&config_path, "Contact").unwrap_err();
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );
}
