use std::path::Path;
use std::time::Instant;

use tokio::sync::{Notify, RwLock, broadcast};

use crate::Error;
use crate::config::Config;
use crate::generated::FaviconSet;
use crate::render::{Mode, RenderedSite, Theme};

/// Shared state for the axum application.
#[doc(hidden)]
pub struct AppState {
    pub(crate) site: RwLock<RenderedSite>,
    /// Cached favicon set — generated once at startup, reused on every
    /// file-change rebuild so the expensive image-processing step is not
    /// repeated.
    pub(crate) favicon: Option<FaviconSet>,
    /// Broadcast channel for signalling browsers to reload.
    pub reload_tx: broadcast::Sender<()>,
    /// Notified on shutdown so WebSocket handlers and the file watcher can
    /// break out of their loops and let axum drain.
    pub(crate) shutdown: Notify,
}

impl AppState {
    /// Wrap a freshly rendered site with the broadcast and shutdown channels.
    pub(super) fn new(rendered: RenderedSite, favicon: Option<FaviconSet>) -> Self {
        let (reload_tx, _) = broadcast::channel(16);
        Self {
            site: RwLock::new(rendered),
            favicon,
            reload_tx,
            shutdown: Notify::new(),
        }
    }

    /// Re-render the site, swap it into shared state, and notify connected
    /// browsers to reload.
    pub(crate) async fn rebuild(&self, config_path: &Path) -> Result<(), Error> {
        let config = Config::from_path(config_path)?;
        let start = Instant::now();

        let theme = Theme::load(&config)?;
        let rendered =
            RenderedSite::build_with_favicon(&config, &theme, Mode::Serve, self.favicon.clone())?;
        tracing::info!("rebuild complete in {}ms", start.elapsed().as_millis());

        *self.site.write().await = rendered;
        let _ = self.reload_tx.send(());
        Ok(())
    }
}
