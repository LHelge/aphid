use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse error: {0}")]
    Config(#[from] toml::de::Error),
    #[error("missing YAML frontmatter")]
    MissingFrontmatter,
    #[error("frontmatter parse error: {0}")]
    FrontmatterParse(#[from] serde_yml::Error),
    #[error("failed to load {path}")]
    LoadPage {
        path: PathBuf,
        #[source]
        source: Box<Error>,
    },
    #[error("slug collision: '{slug}' claimed by {path1} and {path2}")]
    SlugCollision {
        slug: String,
        path1: PathBuf,
        path2: PathBuf,
    },
    #[error("failed to load theme from {path}")]
    ThemeLoad {
        path: PathBuf,
        #[source]
        source: Box<Error>,
    },
    #[error("theme is missing required templates: {}", missing.join(", "))]
    ThemeMissingTemplates { missing: Vec<String> },
    #[error("template render error: {0}")]
    Tera(#[from] tera::Error),
    #[error("broken wiki-links found:\n{}", format_broken_links(.0))]
    BrokenWikiLinks(Vec<(String, String)>),
    #[error("file watcher error: {0}")]
    Notify(#[from] notify::Error),
    #[error("refusing unsafe path '{}': {reason}", path.display())]
    UnsafeOutputPath { path: PathBuf, reason: &'static str },
}

fn format_broken_links(links: &[(String, String)]) -> String {
    links
        .iter()
        .map(|(page, target)| {
            format!("  - page \"{page}\" references missing wiki-link: \"{target}\"")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
