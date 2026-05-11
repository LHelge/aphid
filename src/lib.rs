//! `aphid` — a static site generator for a blog and a wiki with shared
//! `[[wiki-link]]` resolution across both.
//!
//! Public surface is intentionally narrow: [`build`] and [`serve`] drive the
//! two entry points of the CLI; internal modules own the data model and the
//! two-pass rendering pipeline.
//!
//! # Example
//!
//! Build a site from `./aphid.toml` into the configured output directory:
//!
//! ```no_run
//! use std::path::Path;
//!
//! # async fn example() -> Result<(), aphid::Error> {
//! aphid::build(Path::new("aphid.toml"), Path::new("dist")).await?;
//! # Ok(())
//! # }
//! ```
//!
//! Or run a development server with file watching and live reload:
//!
//! ```no_run
//! use std::path::Path;
//!
//! # async fn example() -> Result<(), aphid::Error> {
//! aphid::serve(Path::new("aphid.toml"), 3000).await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod content;
mod error;
pub mod generated;
mod html;
pub mod markdown;
pub mod output;
pub mod render;
pub mod scaffold;
pub mod serve;
#[doc(hidden)]
pub mod testutil;

pub use error::Error;

use std::path::Path;

use config::Config;
use render::{BuiltSite, Theme};

/// Build the site into the given output directory.
///
/// Broken `[[wiki-links]]` are fatal here: any diagnostics produced during
/// pass 1 are converted into [`Error::BrokenWikiLinks`] before any output
/// is written. The dev server applies a different policy — see
/// [`serve`].
pub async fn build(config_path: &Path, output_dir: &Path) -> Result<(), Error> {
    let config = Config::from_path(config_path)?;

    let theme = Theme::load(&config)?;
    let built = BuiltSite::build(&config, &theme)?;

    if !built.diagnostics.is_empty() {
        return Err(Error::BrokenWikiLinks(
            built.diagnostics.broken_wiki_links.clone(),
        ));
    }

    // ── Write output ────────────────────────────────────────────────────────
    tracing::info!(output = %output_dir.display(), "writing output");
    let writer = output::OutputWriter::new(output_dir)?;
    writer.write(&built, &theme, &config.static_dir)?;

    tracing::info!("build complete");
    Ok(())
}

/// Serve the site with file watching and live reload.
pub async fn serve(config_path: &Path, port: u16) -> Result<(), Error> {
    serve::Server::new(config_path)?.run(port).await
}

/// Create a new site in a new directory named `name`.
pub fn scaffold_new(name: &str) -> Result<(), Error> {
    scaffold::new(name)
}

/// Initialize a site in an existing directory at `path`.
pub fn scaffold_init(path: &Path) -> Result<(), Error> {
    scaffold::init(path)
}

/// Create a new blog post in the site at `config_path`.
pub fn new_blog_post(config_path: &Path, title: &str) -> Result<(), Error> {
    scaffold::new_blog_post(config_path, title)
}

/// Create a new wiki page in the site at `config_path`.
pub fn new_wiki_page(config_path: &Path, title: &str) -> Result<(), Error> {
    scaffold::new_wiki_page(config_path, title)
}

/// Create a new standalone page in the site at `config_path`.
pub fn new_page(config_path: &Path, title: &str) -> Result<(), Error> {
    scaffold::new_page(config_path, title)
}
