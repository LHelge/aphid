//! Site-derived root artifacts: `robots.txt`, `sitemap.xml`, `feed.xml`,
//! `rss.xml`. Each is a single file at the output root, derived purely
//! from the [`RenderedSite`] (Site state plus rendered markdown bodies),
//! and written by [`crate::output::OutputWriter`].
//!
//! Favicons land at the root too but are a different concept: they're
//! processed from an external image source, cached across rebuilds, and
//! produce multiple files plus an HTML fragment for templates. They live
//! in [`crate::favicon`], not here.

mod feed;
mod robots;
mod sitemap;

use rayon::prelude::*;

use crate::markdown::RenderedSite;

use feed::{AtomFeed, RssFeed};
use robots::Robots;
use sitemap::Sitemap;

/// A single file written at the site root, produced from the rendered
/// site. Implementations are zero-sized markers — the bytes are computed
/// on demand in [`render`](Self::render) rather than stored, so the
/// per-artifact work can run in parallel.
pub trait RootArtifact: Sync {
    fn filename(&self) -> &'static str;
    fn render(&self, rendered: &RenderedSite<'_>) -> Vec<u8>;
}

/// The full set of root artifacts produced by every build. Order is not
/// observable to the output (each artifact has a distinct filename), so
/// the slice can be parallel-iterated.
pub static ARTIFACTS: &[&dyn RootArtifact] = &[&Robots, &Sitemap, &AtomFeed, &RssFeed];

/// Render every artifact in [`ARTIFACTS`] and return them as
/// `(filename, bytes)` pairs ready to extend the renderer's `root_files`.
pub fn render_all(rendered: &RenderedSite<'_>) -> Vec<(String, Vec<u8>)> {
    ARTIFACTS
        .par_iter()
        .map(|a| (a.filename().to_string(), a.render(rendered)))
        .collect()
}
