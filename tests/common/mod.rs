//! Shared setup helpers for integration tests. Test-only code that needs
//! `CARGO_MANIFEST_DIR`-relative paths or other concerns specific to
//! integration tests (not the library) lives here.
//!
//! `mod common;` includes this whole file in each integration-test binary;
//! Cargo treats every `tests/*.rs` as a separate crate, so a binary that
//! only uses some of these helpers triggers `dead_code` on the rest. The
//! crate-wide allow keeps that quiet without per-fn attributes.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use aphid::testutil::write_file;

/// Path to the shared `tests/fixtures/content/` directory.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/content")
}

/// Write an `aphid.toml` into `tempdir` whose `source_dir` points at
/// `source` and whose `static_dir` is inside the tempdir.
/// Returns the config path.
pub fn write_fixture_config(tempdir: &Path, source: &Path) -> PathBuf {
    let config_path = tempdir.join("aphid.toml");
    let config = format!(
        r#"title = "Test Site"
base_url = "https://example.com"
source_dir = "{}"
static_dir = "{}"
"#,
        source.display(),
        tempdir.join("static").display(),
    );
    write_file(&config_path, &config);
    config_path
}

/// Set up a tempdir whose config points at the shared fixtures (read-only).
/// Use this when the test does not modify source content.
pub fn setup_with_shared_fixtures() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let config_path = write_fixture_config(dir.path(), &fixtures_dir());
    (dir, config_path)
}
