use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tera::Tera;

use crate::Error;
use crate::config::Config;

pub const REQUIRED_TEMPLATES: &[&str] = &[
    "base.html",
    "home.html",
    "blog_post.html",
    "blog_index.html",
    "wiki_page.html",
    "wiki_index.html",
    "page.html",
    "tag.html",
    "tags_index.html",
    "404.html",
];

// Embedded default theme templates
const DEFAULT_BASE: &str = include_str!("../../default-theme/templates/base.html");
const DEFAULT_HOME: &str = include_str!("../../default-theme/templates/home.html");
const DEFAULT_BLOG_POST: &str = include_str!("../../default-theme/templates/blog_post.html");
const DEFAULT_BLOG_INDEX: &str = include_str!("../../default-theme/templates/blog_index.html");
const DEFAULT_WIKI_PAGE: &str = include_str!("../../default-theme/templates/wiki_page.html");
const DEFAULT_WIKI_INDEX: &str = include_str!("../../default-theme/templates/wiki_index.html");
const DEFAULT_PAGE: &str = include_str!("../../default-theme/templates/page.html");
const DEFAULT_TAG: &str = include_str!("../../default-theme/templates/tag.html");
const DEFAULT_TAGS_INDEX: &str = include_str!("../../default-theme/templates/tags_index.html");
const DEFAULT_PAGINATION: &str = include_str!("../../default-theme/templates/pagination.html");
const DEFAULT_404: &str = include_str!("../../default-theme/templates/404.html");

// Embedded default theme static files (relative path → content).
// `mermaid.min.js` is the upstream UMD bundle for client-side diagram
// rendering, vendored here to avoid a CDN dependency. Yes, it's ~3 MB
// of baked-in JS — that's the cost of self-contained mermaid support.
const DEFAULT_STATIC_FILES: &[(&str, &str)] = &[
    (
        "css/theme.css",
        include_str!("../../default-theme/static/css/theme.css"),
    ),
    (
        "js/mermaid.min.js",
        include_str!("../../default-theme/static/js/mermaid.min.js"),
    ),
];

#[derive(Debug, Deserialize)]
pub struct ThemeMeta {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

pub struct Theme {
    pub meta: ThemeMeta,
    pub tera: Tera,
    /// On-disk static directory (for themes loaded from a directory).
    pub static_dir: Option<PathBuf>,
    /// Embedded static files (relative path → content), used by the default theme.
    pub(crate) embedded_static: Vec<(&'static str, &'static str)>,
}

impl std::fmt::Debug for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Theme")
            .field("meta", &self.meta)
            .field("static_dir", &self.static_dir)
            .finish_non_exhaustive()
    }
}

impl Default for Theme {
    fn default() -> Self {
        let mut tera = Tera::default();
        let templates = [
            ("base.html", DEFAULT_BASE),
            ("home.html", DEFAULT_HOME),
            ("blog_post.html", DEFAULT_BLOG_POST),
            ("blog_index.html", DEFAULT_BLOG_INDEX),
            ("wiki_page.html", DEFAULT_WIKI_PAGE),
            ("wiki_index.html", DEFAULT_WIKI_INDEX),
            ("page.html", DEFAULT_PAGE),
            ("tag.html", DEFAULT_TAG),
            ("tags_index.html", DEFAULT_TAGS_INDEX),
            ("pagination.html", DEFAULT_PAGINATION),
            ("404.html", DEFAULT_404),
        ];
        for (name, content) in templates {
            tera.add_raw_template(name, content)
                .unwrap_or_else(|e| panic!("embedded template {name} failed to parse: {e}"));
        }

        Self {
            meta: ThemeMeta {
                name: "default".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                description: Some("The built-in aphid theme.".into()),
            },
            tera,
            static_dir: None,
            embedded_static: DEFAULT_STATIC_FILES.to_vec(),
        }
    }
}

impl Theme {
    /// Pick the theme implied by a [`Config`]: load from `config.theme_dir`
    /// when set, otherwise fall back to the embedded default theme.
    pub fn load(config: &Config) -> Result<Self, Error> {
        match &config.theme_dir {
            Some(dir) => {
                tracing::info!(path = %dir.display(), "loading theme from directory");
                Self::from_dir(dir)
            }
            None => {
                tracing::info!("using embedded default theme");
                Ok(Self::default())
            }
        }
    }

    /// Load a theme from a directory containing `theme.toml`, `templates/`,
    /// and optionally `static/`.
    pub fn from_dir(path: &Path) -> Result<Self, Error> {
        let meta_path = path.join("theme.toml");
        let meta_text = fs::read_to_string(&meta_path).map_err(|e| Error::ThemeLoad {
            path: meta_path.clone(),
            source: Box::new(Error::Io(e)),
        })?;
        let meta: ThemeMeta = toml::from_str(&meta_text).map_err(|e| Error::ThemeLoad {
            path: meta_path,
            source: Box::new(Error::Config(e)),
        })?;

        let templates_glob = path.join("templates").join("**").join("*.html");
        let glob_str = templates_glob.to_str().ok_or_else(|| Error::ThemeLoad {
            path: path.to_path_buf(),
            source: Box::new(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "theme path contains non-UTF-8 characters",
            ))),
        })?;
        let tera = Tera::new(glob_str).map_err(|e| Error::ThemeLoad {
            path: path.to_path_buf(),
            source: Box::new(Error::Tera(e)),
        })?;

        let static_path = path.join("static");
        let static_dir = if static_path.is_dir() {
            Some(static_path)
        } else {
            None
        };

        let theme = Self {
            meta,
            tera,
            static_dir,
            embedded_static: vec![],
        };
        theme.validate()?;
        Ok(theme)
    }

    /// Write theme static files to `dest/`.
    ///
    /// For on-disk themes, recursively copies from `static_dir`.
    /// For the embedded default theme, writes from compiled-in strings.
    pub fn write_static(&self, dest: &Path) -> Result<(), Error> {
        if let Some(ref dir) = self.static_dir {
            crate::output::copy_dir_recursive(dir, dest)?;
        }
        for (rel_path, content) in &self.embedded_static {
            let file_path = dest.join(rel_path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&file_path, content)?;
        }
        Ok(())
    }

    /// Check that all required templates are present in the loaded Tera
    /// instance. Returns an error listing any missing templates.
    fn validate(&self) -> Result<(), Error> {
        let missing: Vec<String> = REQUIRED_TEMPLATES
            .iter()
            .filter(|name| self.tera.get_template(name).is_err())
            .map(|name| (*name).to_owned())
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(Error::ThemeMissingTemplates { missing })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::testutil::write_file;

    fn write_theme_toml(dir: &Path) {
        write_file(
            &dir.join("theme.toml"),
            r#"
name = "test"
version = "0.1.0"
description = "A test theme."
"#,
        );
    }

    fn write_all_templates(dir: &Path) {
        let templates_dir = dir.join("templates");
        for name in REQUIRED_TEMPLATES {
            write_file(
                &templates_dir.join(name),
                "{% block body %}{{ content }}{% endblock body %}",
            );
        }
    }

    #[test]
    fn valid_theme_loads_successfully() {
        let dir = TempDir::new().unwrap();
        write_theme_toml(dir.path());
        write_all_templates(dir.path());

        let theme = Theme::from_dir(dir.path()).unwrap();
        assert_eq!(theme.meta.name, "test");
        assert_eq!(theme.meta.version, "0.1.0");
        assert_eq!(theme.meta.description.as_deref(), Some("A test theme."));
        assert!(theme.static_dir.is_none());
    }

    #[test]
    fn theme_with_static_dir() {
        let dir = TempDir::new().unwrap();
        write_theme_toml(dir.path());
        write_all_templates(dir.path());
        let static_dir = dir.path().join("static").join("css");
        fs::create_dir_all(&static_dir).unwrap();
        write_file(&static_dir.join("theme.css"), "body {}");

        let theme = Theme::from_dir(dir.path()).unwrap();
        assert!(theme.static_dir.is_some());
    }

    #[test]
    fn missing_theme_toml_is_error() {
        let dir = TempDir::new().unwrap();
        write_all_templates(dir.path());

        let err = Theme::from_dir(dir.path()).unwrap_err();
        assert!(
            matches!(err, Error::ThemeLoad { .. }),
            "Expected ThemeLoad, got: {err:?}"
        );
    }

    #[test]
    fn missing_required_template_is_error() {
        let dir = TempDir::new().unwrap();
        write_theme_toml(dir.path());
        // Write all but one template
        let templates_dir = dir.path().join("templates");
        for name in REQUIRED_TEMPLATES.iter().skip(1) {
            write_file(
                &templates_dir.join(name),
                "{% block body %}{{ content }}{% endblock body %}",
            );
        }

        let err = Theme::from_dir(dir.path()).unwrap_err();
        match err {
            Error::ThemeMissingTemplates { missing } => {
                assert_eq!(missing, vec!["base.html"]);
            }
            other => panic!("Expected ThemeMissingTemplates, got: {other:?}"),
        }
    }

    #[test]
    fn no_static_dir_is_not_error() {
        let dir = TempDir::new().unwrap();
        write_theme_toml(dir.path());
        write_all_templates(dir.path());
        // Explicitly ensure no static/ dir exists
        assert!(!dir.path().join("static").exists());

        let theme = Theme::from_dir(dir.path()).unwrap();
        assert!(theme.static_dir.is_none());
    }

    #[test]
    fn default_theme_loads_and_validates() {
        let theme_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("default-theme");
        let theme = Theme::from_dir(&theme_path).unwrap();
        assert_eq!(theme.meta.name, "default");
        assert!(theme.static_dir.is_some());
    }

    #[test]
    fn embedded_default_theme_validates() {
        let theme = Theme::default();
        assert_eq!(theme.meta.name, "default");
        assert!(theme.static_dir.is_none());
        assert!(!theme.embedded_static.is_empty());
        // All required templates are present
        theme.validate().unwrap();
    }

    #[test]
    fn embedded_theme_writes_static_files() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("static");
        let theme = Theme::default();

        theme.write_static(&dest).unwrap();

        assert!(dest.join("css/theme.css").exists());
        let css = fs::read_to_string(dest.join("css/theme.css")).unwrap();
        assert!(css.contains("body"));
    }
}
