use tokio::sync::{Notify, RwLock, broadcast};

use crate::render::RenderedSite;

/// Shared state for the axum application: the current rendered site,
/// the live-reload broadcast channel, and the shutdown notifier.
///
/// Everything cross-rebuild is owned by `Rebuilder` (see `rebuilder.rs`);
/// `AppState` carries only what HTTP handlers and the WebSocket
/// live-reload need to read.
#[doc(hidden)]
pub struct AppState {
    pub(crate) site: RwLock<RenderedSite>,
    /// Broadcast channel for signalling browsers to reload.
    pub reload_tx: broadcast::Sender<()>,
    /// Notified on shutdown so WebSocket handlers and the file watcher can
    /// break out of their loops and let axum drain.
    pub(crate) shutdown: Notify,
}

impl AppState {
    /// Wrap a freshly rendered site with the broadcast and shutdown channels.
    pub(super) fn new(rendered: RenderedSite) -> Self {
        let (reload_tx, _) = broadcast::channel(16);
        Self {
            site: RwLock::new(rendered),
            reload_tx,
            shutdown: Notify::new(),
        }
    }

    /// Atomically replace the rendered site and notify connected browsers
    /// to reload. The render itself happens elsewhere — this is the
    /// state-mutation half of a rebuild.
    pub(crate) async fn swap(&self, rendered: RenderedSite) {
        *self.site.write().await = rendered;
        let _ = self.reload_tx.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_rendered() -> RenderedSite {
        RenderedSite {
            pages: HashMap::new(),
            not_found_html: String::new(),
            root_files: Vec::new(),
        }
    }

    fn rendered_with_page(url: &str, html: &str) -> RenderedSite {
        let mut pages = HashMap::new();
        pages.insert(url.to_string(), html.to_string());
        RenderedSite {
            pages,
            not_found_html: String::new(),
            root_files: Vec::new(),
        }
    }

    #[tokio::test]
    async fn swap_replaces_site() {
        let state = AppState::new(empty_rendered());
        assert!(state.site.read().await.lookup("/blog/post/").is_none());

        state
            .swap(rendered_with_page("/blog/post/", "<p>hi</p>"))
            .await;

        assert_eq!(
            state.site.read().await.lookup("/blog/post/"),
            Some("<p>hi</p>")
        );
    }

    #[tokio::test]
    async fn swap_broadcasts_reload() {
        let state = AppState::new(empty_rendered());
        let mut rx = state.reload_tx.subscribe();

        state.swap(empty_rendered()).await;

        // A subscriber registered before swap sees exactly one reload tick.
        assert!(rx.try_recv().is_ok());
    }
}
