use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};
use tokio::sync::mpsc;

use super::AppState;
use super::rebuilder::Rebuilder;
use crate::Error;
use crate::config::Config;

const DEBOUNCE: Duration = Duration::from_millis(200);

pub(crate) struct ContentWatcher {
    /// Held to keep the watcher alive; dropping it stops file events.
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<()>,
}

impl ContentWatcher {
    pub fn new(config: &Config, config_path: &Path) -> Result<Self, Error> {
        let (tx, rx) = mpsc::channel::<()>(1);

        let mut watcher = RecommendedWatcher::new(
            move |result: Result<notify::Event, notify::Error>| match result {
                Ok(event) if is_content_change(&event.kind) => {
                    let _ = tx.try_send(());
                }
                Ok(_) => {}
                Err(e) => tracing::error!("file watcher error: {e}"),
            },
            notify::Config::default(),
        )?;

        let dirs = [
            Some(&config.source_dir),
            Some(&config.static_dir),
            config.theme_dir.as_ref(),
        ];
        for dir in dirs.into_iter().flatten() {
            if dir.is_dir() {
                tracing::debug!(path = %dir.display(), "watching directory");
                watcher.watch(dir, RecursiveMode::Recursive)?;
            }
        }

        // Watch aphid.toml itself so config edits — favicon path swap,
        // title change, posts_per_page, wiki_default_category — also
        // trigger rebuilds. Rebuilder::next_rendered reloads from disk
        // every time, so the new values flow through naturally.
        if config_path.is_file() {
            tracing::debug!(path = %config_path.display(), "watching config file");
            watcher.watch(config_path, RecursiveMode::NonRecursive)?;
        }

        tracing::info!("file watcher started");
        Ok(Self {
            _watcher: watcher,
            rx,
        })
    }

    pub async fn run(
        &mut self,
        rebuilder: &mut Rebuilder,
        state: &Arc<AppState>,
    ) -> Result<(), Error> {
        // Pinned across iterations: once registered as a waiter, the future
        // stays in `Notify`'s waker list and reliably observes a later
        // `notify_waiters()` even if it fired while we were rebuilding.
        let shutdown = state.shutdown.notified();
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown => {
                    tracing::debug!("watcher received shutdown signal");
                    break;
                }
                result = self.rx.recv() => {
                    if result.is_none() {
                        break;
                    }
                    self.debounce().await;

                    tracing::info!("file change detected, rebuilding…");
                    let start = Instant::now();
                    match rebuilder.next_rendered() {
                        Ok(rendered) => {
                            state.swap(rendered).await;
                            tracing::info!(
                                "rebuild complete in {}ms",
                                start.elapsed().as_millis()
                            );
                        }
                        Err(e) => tracing::error!("rebuild failed: {e}"),
                    }
                }
            }
        }

        Ok(())
    }

    async fn debounce(&mut self) {
        let mut deadline = tokio::time::Instant::now() + DEBOUNCE;
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => break,
                result = self.rx.recv() => {
                    if result.is_none() { break; }
                    deadline = tokio::time::Instant::now() + DEBOUNCE;
                }
            }
        }
    }
}

fn is_content_change(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Name(_) | ModifyKind::Any)
    )
}
