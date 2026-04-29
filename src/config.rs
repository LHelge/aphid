use crate::Error;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn default_source_dir() -> PathBuf {
    PathBuf::from("content")
}

fn default_static_dir() -> PathBuf {
    PathBuf::from("static")
}

#[derive(Debug, Clone, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Social {
    pub platform: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub title: String,
    pub base_url: String,
    #[serde(default = "default_source_dir")]
    pub source_dir: PathBuf,
    pub theme_dir: Option<PathBuf>,
    #[serde(default = "default_static_dir")]
    pub static_dir: PathBuf,
    #[serde(default)]
    pub authors: Vec<Author>,
    #[serde(default)]
    pub socials: Vec<Social>,
    /// Explicit ordering for wiki category headings. Categories listed here
    /// appear in this order; any wiki category not listed falls through to
    /// alphabetical placement after the listed ones, with uncategorised pages
    /// last.
    #[serde(default)]
    pub wiki_categories: Vec<String>,
    /// Path to a source image (PNG, JPEG, SVG, etc.) used to generate
    /// favicons at standard sizes.
    pub favicon: Option<PathBuf>,
}

impl std::str::FromStr for Config {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let config = toml::from_str(s)?;
        Ok(config)
    }
}

impl Config {
    pub fn from_path(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)?;
        let mut config: Self = text.parse()?;

        // Resolve relative paths against the config file's parent directory
        // so that `aphid serve --config sub/aphid.toml` works from any CWD.
        let base = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        config.resolve_paths(base);

        Ok(config)
    }

    pub fn normalized_base_url(&self) -> &str {
        Self::normalize_base_url(&self.base_url)
    }

    pub fn normalize_base_url(base_url: &str) -> &str {
        base_url.trim_end_matches('/')
    }

    fn resolve_path(path: &mut PathBuf, base: &Path) {
        if path.is_relative() {
            *path = base.join(&*path);
        }
    }

    fn resolve_optional_path(path: &mut Option<PathBuf>, base: &Path) {
        if let Some(path) = path {
            Self::resolve_path(path, base);
        }
    }

    /// Make all relative directory fields absolute by joining them with `base`.
    fn resolve_paths(&mut self, base: &Path) {
        Self::resolve_path(&mut self.source_dir, base);
        Self::resolve_path(&mut self.static_dir, base);
        Self::resolve_optional_path(&mut self.theme_dir, base);
        Self::resolve_optional_path(&mut self.favicon, base);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_config_applies_defaults() {
        let cfg: Config = r#"
            title = "My Site"
            base_url = "https://example.com"
            "#
        .parse()
        .unwrap();
        assert_eq!(cfg.title, "My Site");
        assert_eq!(cfg.base_url, "https://example.com");
        assert_eq!(cfg.source_dir, PathBuf::from("content"));
        assert!(cfg.theme_dir.is_none());
        assert_eq!(cfg.static_dir, PathBuf::from("static"));
        assert!(cfg.authors.is_empty());
        assert!(cfg.socials.is_empty());
        assert!(cfg.wiki_categories.is_empty());
    }

    #[test]
    fn fully_specified_config() {
        let cfg: Config = r#"
            title = "Full Site"
            base_url = "https://full.example.com"
            source_dir = "src_content"
            theme_dir = "tmpl"
            static_dir = "assets"

            [[authors]]
            name = "Alice"
            email = "alice@example.com"

            [[authors]]
            name = "Bob"

            [[socials]]
            platform = "github"
            url = "https://github.com/example"
            "#
        .parse()
        .unwrap();
        assert_eq!(cfg.source_dir, PathBuf::from("src_content"));
        assert_eq!(cfg.theme_dir, Some(PathBuf::from("tmpl")));
        assert_eq!(cfg.static_dir, PathBuf::from("assets"));
        assert_eq!(cfg.authors.len(), 2);
        assert_eq!(cfg.authors[0].name, "Alice");
        assert_eq!(cfg.authors[0].email.as_deref(), Some("alice@example.com"));
        assert_eq!(cfg.authors[1].name, "Bob");
        assert!(cfg.authors[1].email.is_none());
        assert_eq!(cfg.socials.len(), 1);
        assert_eq!(cfg.socials[0].platform, "github");
    }

    #[test]
    fn missing_title_is_error() {
        assert!(r#"base_url = "https://example.com""#.parse::<Config>().is_err());
    }

    #[test]
    fn missing_base_url_is_error() {
        assert!(r#"title = "My Site""#.parse::<Config>().is_err());
    }

    #[test]
    fn normalized_base_url_strips_trailing_slash() {
        let with_slash: Config = r#"
            title = "My Site"
            base_url = "https://example.com/"
            "#
        .parse()
        .unwrap();
        let without_slash: Config = r#"
            title = "My Site"
            base_url = "https://example.com"
            "#
        .parse()
        .unwrap();

        assert_eq!(with_slash.normalized_base_url(), "https://example.com");
        assert_eq!(without_slash.normalized_base_url(), "https://example.com");
    }
}
