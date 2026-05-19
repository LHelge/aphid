use crate::Error;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use url::Url;

fn default_source_dir() -> PathBuf {
    PathBuf::from("content")
}

fn default_static_dir() -> PathBuf {
    PathBuf::from("static")
}

#[derive(Debug, Clone, Deserialize)]
pub struct Author {
    pub name: String,
    pub link: Option<String>,
    pub email: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Social {
    pub platform: String,
    pub url: String,
}

fn default_feed_limit() -> usize {
    20
}

fn default_posts_per_page() -> usize {
    10
}

fn default_wiki_default_category() -> String {
    "Other".to_string()
}

fn default_reading_wpm() -> u32 {
    200
}

/// A structured wiki category entry in `aphid.toml`. Defines ordering,
/// display metadata (description, icon) for the wiki index page.
#[derive(Debug, Clone, Deserialize)]
pub struct WikiCategoryConfig {
    pub name: String,
    /// One or two sentences describing this category.
    pub description: Option<String>,
    /// Root-relative URL path to an SVG icon
    /// (e.g. `"/static/category/getting-started.svg"`).
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub title: String,
    /// Site root URL. Validated as a real `http(s)://` URL at load time
    /// and normalized so its path component always ends with `/`, which
    /// makes [`Config::absolutize`] a trivial `Url::join`.
    pub base_url: Url,
    /// Short site description used as the Atom `<subtitle>` and RSS
    /// `<channel><description>`.
    pub description: Option<String>,
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
    /// last. Each entry can carry an optional description and icon for the
    /// wiki index.
    #[serde(default)]
    pub wiki_categories: Vec<WikiCategoryConfig>,
    /// Display name used for wiki pages without a `category` in frontmatter.
    /// Surfaces both as the page's own category label and as the heading
    /// for the catch-all group on the wiki index. Defaults to `"Other"`.
    #[serde(default = "default_wiki_default_category")]
    pub wiki_default_category: String,
    /// Path to a source image (PNG, JPEG, SVG, etc.) used to generate
    /// favicons at standard sizes.
    pub favicon: Option<PathBuf>,
    /// Root-relative path or absolute URL to an image used as the default
    /// OpenGraph / Twitter card image for pages without their own. Write
    /// it as `/static/social-card.png` or an absolute URL — matching the
    /// convention used for blog hero images and `favicon`.
    pub social_image: Option<String>,
    /// Maximum number of blog posts included in RSS/Atom feeds. Set to `0`
    /// to include all posts. Defaults to 20.
    #[serde(default = "default_feed_limit")]
    pub feed_limit: usize,
    /// Maximum number of posts shown per page on the blog index and tag
    /// pages. Page 1 stays at the canonical URL (`/blog/`,
    /// `/tags/{tag}/`); subsequent pages live under `page/N/`. Defaults to 10.
    #[serde(default = "default_posts_per_page")]
    pub posts_per_page: usize,
    /// Words-per-minute used for the blog post `reading_time_minutes`
    /// estimate. Defaults to 200 (a conservative pace that fits technical
    /// writing with code samples); raise it for prose-heavy sites.
    #[serde(default = "default_reading_wpm")]
    pub reading_wpm: u32,
}

impl std::str::FromStr for Config {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut config: Self = toml::from_str(s)?;
        config.validate()?;
        Ok(config)
    }
}

impl Config {
    /// Check the scheme and normalize `base_url` so its path always ends
    /// with `/`. Called once after deserialization so all downstream code
    /// can treat `base_url` as canonical.
    fn validate(&mut self) -> Result<(), Error> {
        let scheme = self.base_url.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(Error::InvalidConfig {
                field: "base_url",
                message: format!(
                    "must use http:// or https://, got {:?} (scheme {:?})",
                    self.base_url.as_str(),
                    scheme
                ),
            });
        }
        if !self.base_url.path().ends_with('/') {
            let with_slash = format!("{}/", self.base_url.path());
            self.base_url.set_path(&with_slash);
        }
        Ok(())
    }

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

    /// Build a fully-qualified URL from a root-relative path (e.g.
    /// `/wiki/foo/`) or an already-absolute URL. Supports subpath
    /// deployments: with `base_url = "https://example.com/sub"`,
    /// `absolute_url("/wiki/foo/")` correctly yields
    /// `https://example.com/sub/wiki/foo/`.
    pub fn absolute_url(&self, path: &str) -> Url {
        let rel = path.strip_prefix('/').unwrap_or(path);
        self.base_url
            .join(rel)
            .unwrap_or_else(|e| panic!("failed to join {path:?} to base_url: {e}"))
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
        assert_eq!(cfg.base_url.as_str(), "https://example.com/");
        assert_eq!(cfg.source_dir, PathBuf::from("content"));
        assert!(cfg.theme_dir.is_none());
        assert_eq!(cfg.static_dir, PathBuf::from("static"));
        assert!(cfg.authors.is_empty());
        assert!(cfg.socials.is_empty());
        assert!(cfg.wiki_categories.is_empty());
        assert_eq!(cfg.wiki_default_category, "Other");
        assert_eq!(cfg.feed_limit, 20);
        assert_eq!(cfg.posts_per_page, 10);
    }

    #[test]
    fn fully_specified_config() {
        let cfg: Config = r#"
            title = "Full Site"
            base_url = "https://full.example.com"
            source_dir = "src_content"
            theme_dir = "tmpl"
            static_dir = "assets"
            feed_limit = 5
            posts_per_page = 3

            [[authors]]
            name = "Alice"
            link = "mailto:alice@example.com"

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
        assert_eq!(
            cfg.authors[0].link.as_deref(),
            Some("mailto:alice@example.com")
        );
        assert_eq!(cfg.authors[1].name, "Bob");
        assert!(cfg.authors[1].link.is_none());
        assert_eq!(cfg.socials.len(), 1);
        assert_eq!(cfg.socials[0].platform, "github");
        assert_eq!(cfg.feed_limit, 5);
        assert_eq!(cfg.posts_per_page, 3);
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
    fn relative_base_url_is_error() {
        let result = r#"
            title = "My Site"
            base_url = "/"
            "#
        .parse::<Config>();
        let err = result.unwrap_err().to_string();
        assert!(err.contains("base_url"), "err was: {err}");
    }

    #[test]
    fn non_http_scheme_is_error() {
        // Reaches our `validate` (`Url::parse` accepts `file://`), so we
        // get the explicit "must use http://" diagnostic.
        let result = r#"
            title = "My Site"
            base_url = "file:///tmp/site"
            "#
        .parse::<Config>();
        let err = result.unwrap_err().to_string();
        assert!(err.contains("http"), "err was: {err}");
    }

    #[test]
    fn bare_path_base_url_is_error() {
        let result = r#"
            title = "My Site"
            base_url = "example.com"
            "#
        .parse::<Config>();
        assert!(result.is_err());
    }

    #[test]
    fn base_url_path_canonicalized_with_trailing_slash() {
        // Both forms should canonicalize the same way — `validate` adds a
        // trailing slash if missing so `Url::join` semantics work uniformly.
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

        assert_eq!(with_slash.base_url.as_str(), "https://example.com/");
        assert_eq!(without_slash.base_url.as_str(), "https://example.com/");
    }

    #[test]
    fn subpath_base_url_gets_trailing_slash() {
        let cfg: Config = r#"
            title = "My Site"
            base_url = "https://example.com/sub"
            "#
        .parse()
        .unwrap();
        assert_eq!(cfg.base_url.as_str(), "https://example.com/sub/");
    }

    fn config_with_base(base_url: &str) -> Config {
        format!("title = \"T\"\nbase_url = \"{base_url}\"")
            .parse()
            .unwrap()
    }

    #[test]
    fn absolute_url_joins_root_relative_path() {
        let cfg = config_with_base("https://example.com");
        assert_eq!(
            cfg.absolute_url("/wiki/foo/").as_str(),
            "https://example.com/wiki/foo/"
        );
    }

    #[test]
    fn absolute_url_handles_base_url_with_trailing_slash() {
        let cfg = config_with_base("https://example.com/");
        assert_eq!(
            cfg.absolute_url("/wiki/foo/").as_str(),
            "https://example.com/wiki/foo/"
        );
    }

    #[test]
    fn absolute_url_preserves_subpath_deployment() {
        // base_url with a subpath should propagate into resolved URLs —
        // the regression-prone case naive `format!("{base}{path}")` got
        // right by accident and `Url::join` gets wrong without help.
        let cfg = config_with_base("https://example.com/sub");
        assert_eq!(
            cfg.absolute_url("/wiki/foo/").as_str(),
            "https://example.com/sub/wiki/foo/"
        );
    }

    #[test]
    fn absolute_url_passes_through_absolute_urls() {
        let cfg = config_with_base("https://example.com");
        assert_eq!(
            cfg.absolute_url("https://other.example.com/x.png").as_str(),
            "https://other.example.com/x.png"
        );
    }
}
