use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::Error;
use crate::config::Config;
use crate::generated::FaviconSet;
use crate::render::{Mode, RenderedSite, Theme};

/// Owns the per-rebuild state of `aphid serve`: the path to `aphid.toml`
/// and the cached favicon set keyed by its source path and mtime.
///
/// Held by the file watcher across rebuilds; the watcher calls
/// [`Rebuilder::next_rendered`] on every change. The favicon-source check
/// is a fast `stat()`; the expensive regeneration only runs when the
/// source path or its mtime actually changed.
pub(crate) struct Rebuilder {
    config_path: PathBuf,
    favicon_cache: Option<FaviconCacheEntry>,
}

struct FaviconCacheEntry {
    set: FaviconSet,
    source_path: PathBuf,
    mtime: SystemTime,
}

impl Rebuilder {
    /// Construct with no cached favicon. The first call to
    /// [`next_rendered`](Self::next_rendered) generates one if config has
    /// a favicon source.
    #[cfg(test)]
    pub(crate) fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            favicon_cache: None,
        }
    }

    /// Construct with a pre-generated favicon and its source mtime, so the
    /// first rebuild after server startup doesn't redo work the initial
    /// render already did.
    pub(crate) fn with_initial_favicon(
        config_path: PathBuf,
        favicon: Option<(FaviconSet, PathBuf, SystemTime)>,
    ) -> Self {
        let favicon_cache = favicon.map(|(set, source_path, mtime)| FaviconCacheEntry {
            set,
            source_path,
            mtime,
        });
        Self {
            config_path,
            favicon_cache,
        }
    }

    /// The path to `aphid.toml`. The watcher reads this to register the
    /// config file itself for change events.
    pub(crate) fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Reload config + theme, refresh the favicon if its source changed,
    /// and re-render the site. Returns the freshly rendered site; the
    /// caller is responsible for swapping it into shared state.
    pub(crate) fn next_rendered(&mut self) -> Result<RenderedSite, Error> {
        let config = Config::from_path(&self.config_path)?;
        let theme = Theme::load(&config)?;
        let favicon = self.refresh_favicon(&config)?;
        RenderedSite::build_with_favicon(&config, &theme, Mode::Serve, favicon)
    }

    /// Return the cached favicon, regenerating it if and only if the
    /// configured source path or its mtime changed since the last cache
    /// entry. Drops the cache when the config no longer specifies a
    /// favicon.
    fn refresh_favicon(&mut self, config: &Config) -> Result<Option<FaviconSet>, Error> {
        let Some(path) = config.favicon.as_ref() else {
            self.favicon_cache = None;
            return Ok(None);
        };

        let mtime = favicon_mtime(path)?;

        if let Some(cache) = &self.favicon_cache
            && cache.source_path == *path
            && cache.mtime == mtime
        {
            return Ok(Some(cache.set.clone()));
        }

        let set = FaviconSet::generate(path, &config.title)?;
        self.favicon_cache = Some(FaviconCacheEntry {
            set: set.clone(),
            source_path: path.clone(),
            mtime,
        });
        Ok(Some(set))
    }

    #[cfg(test)]
    pub(crate) fn cached_favicon_mtime(&self) -> Option<SystemTime> {
        self.favicon_cache.as_ref().map(|c| c.mtime)
    }
}

/// Read the favicon source's modification time. Wraps the `stat()` call
/// with the `Io` error variant so the caller can surface a uniform
/// rebuild error.
pub(crate) fn favicon_mtime(path: &Path) -> Result<SystemTime, Error> {
    Ok(std::fs::metadata(path)?.modified()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::write_file;
    use std::fs;
    use tempfile::TempDir;

    fn write_minimal_site(dir: &Path, favicon_rel: &str) {
        write_file(
            &dir.join("aphid.toml"),
            &format!(
                "title = \"T\"\nbase_url = \"http://localhost\"\nfavicon = \"{favicon_rel}\"\n"
            ),
        );
        let img = image::RgbImage::from_pixel(1, 1, image::Rgb([255, 255, 255]));
        img.save(dir.join(favicon_rel)).unwrap();
        // Empty content dirs so Site::load succeeds.
        fs::create_dir_all(dir.join("content/blog")).unwrap();
        fs::create_dir_all(dir.join("content/wiki")).unwrap();
        fs::create_dir_all(dir.join("content/pages")).unwrap();
    }

    #[test]
    fn favicon_reused_when_source_unchanged() {
        let dir = TempDir::new().unwrap();
        write_minimal_site(dir.path(), "favicon.png");

        let mut rebuilder = Rebuilder::new(dir.path().join("aphid.toml"));

        // First call: generates and caches.
        rebuilder.next_rendered().unwrap();
        let mtime_after_first = rebuilder.cached_favicon_mtime().unwrap();

        // Second call without touching the file: cached mtime stays exactly
        // equal — no regeneration.
        rebuilder.next_rendered().unwrap();
        assert_eq!(rebuilder.cached_favicon_mtime().unwrap(), mtime_after_first);
    }

    #[test]
    fn favicon_regenerates_when_source_mtime_changes() {
        let dir = TempDir::new().unwrap();
        write_minimal_site(dir.path(), "favicon.png");

        let mut rebuilder = Rebuilder::new(dir.path().join("aphid.toml"));
        rebuilder.next_rendered().unwrap();
        let first_mtime = rebuilder.cached_favicon_mtime().unwrap();

        // Bump mtime forward — set explicitly so the test isn't sensitive
        // to filesystem timestamp granularity.
        let later = first_mtime + std::time::Duration::from_secs(2);
        let path = dir.path().join("favicon.png");
        let f = fs::File::options().write(true).open(&path).unwrap();
        f.set_modified(later).unwrap();
        drop(f);

        rebuilder.next_rendered().unwrap();
        assert_eq!(rebuilder.cached_favicon_mtime().unwrap(), later);
    }

    #[test]
    fn favicon_cache_dropped_when_config_removes_favicon() {
        let dir = TempDir::new().unwrap();
        write_minimal_site(dir.path(), "favicon.png");

        let mut rebuilder = Rebuilder::new(dir.path().join("aphid.toml"));
        rebuilder.next_rendered().unwrap();
        assert!(rebuilder.cached_favicon_mtime().is_some());

        // Rewrite config without favicon field.
        write_file(
            &dir.path().join("aphid.toml"),
            "title = \"T\"\nbase_url = \"http://localhost\"\n",
        );

        rebuilder.next_rendered().unwrap();
        assert!(rebuilder.cached_favicon_mtime().is_none());
    }
}
