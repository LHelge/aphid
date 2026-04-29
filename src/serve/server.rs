use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use tokio::task::JoinHandle;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use super::livereload;
use super::state::AppState;
use super::watcher;
use crate::Error;
use crate::config::Config;
use crate::generated::FaviconSet;
use crate::render::{RenderedSite, Theme};

/// A configured-but-not-yet-bound HTTP server: the rendered site, the axum
/// router, and the inputs needed to run the file watcher.
pub struct Server {
    state: Arc<AppState>,
    router: Router,
    config: Config,
    config_path: PathBuf,
}

impl Server {
    /// Load the config, render the site, and build the router. Does not bind.
    pub fn new(config_path: &Path) -> Result<Self, Error> {
        let config = Config::from_path(config_path)?;
        let theme = Theme::load(&config)?;

        let favicon = config
            .favicon
            .as_ref()
            .map(|p| FaviconSet::generate(p, &config.title))
            .transpose()?;

        let rendered = RenderedSite::build_with_favicon(&config, &theme, false, favicon.clone())?;
        let state = Arc::new(AppState::new(rendered, favicon));
        let router = Self::build_router(Arc::clone(&state), &config);
        Ok(Self {
            state,
            router,
            config,
            config_path: config_path.to_path_buf(),
        })
    }

    /// Bind on `0.0.0.0:port`, start the file watcher, and serve until the
    /// process receives ctrl-c.
    pub async fn run(self, port: u16) -> Result<(), Error> {
        let watcher_handle = self.start_watcher()?;

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        tracing::info!("serving at http://localhost:{port}");
        let listener = tokio::net::TcpListener::bind(addr).await?;

        let state_for_shutdown = Arc::clone(&self.state);
        axum::serve(listener, self.router)
            .with_graceful_shutdown(async move {
                shutdown_signal().await;
                // Wakes both the file watcher and all WebSocket handlers so
                // axum can drain.
                state_for_shutdown.shutdown.notify_waiters();
            })
            .await?;

        tracing::info!("stopping file watcher");
        let _ = tokio::time::timeout(Duration::from_secs(5), watcher_handle).await;

        tracing::info!("server shut down");
        Ok(())
    }

    /// Bind on `127.0.0.1:0` and spawn the server in a background task. No
    /// file watcher and no signal handling — for integration tests.
    pub async fn spawn_test(self) -> Result<(u16, Arc<AppState>, JoinHandle<()>), Error> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let state = Arc::clone(&self.state);
        let router = self.router;

        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.ok();
        });

        Ok((port, state, handle))
    }

    /// Like `spawn_test` but also starts the file watcher. Returns
    /// `(port, state, server_task, watcher_task)` — both tasks should be
    /// aborted when the test ends.
    pub async fn spawn_test_with_watcher(
        self,
    ) -> Result<(u16, Arc<AppState>, JoinHandle<()>, JoinHandle<()>), Error> {
        let watcher_handle = self.start_watcher()?;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let state = Arc::clone(&self.state);
        let router = self.router;

        let server_handle = tokio::spawn(async move {
            axum::serve(listener, router).await.ok();
        });

        Ok((port, state, server_handle, watcher_handle))
    }

    /// Spawn the file watcher in a background task. Used by both
    /// production `run` and the watcher-enabled test variant.
    fn start_watcher(&self) -> Result<JoinHandle<()>, Error> {
        let watch_state = Arc::clone(&self.state);
        let watch_config_path = self.config_path.clone();
        let mut content_watcher = watcher::ContentWatcher::new(&self.config)?;
        Ok(tokio::task::spawn(async move {
            if let Err(e) = content_watcher.run(&watch_config_path, &watch_state).await {
                tracing::error!("file watcher failed: {e}");
            }
        }))
    }

    fn build_router(state: Arc<AppState>, config: &Config) -> Router {
        let mut static_router = Router::new();

        // Theme static files (lower precedence — registered as fallback)
        if let Some(theme_static) = config.theme_dir.as_ref().map(|d| d.join("static"))
            && theme_static.is_dir()
        {
            static_router = static_router.fallback_service(ServeDir::new(theme_static));
        }

        // User static files (higher precedence — nested as the primary service)
        if config.static_dir.is_dir() {
            static_router = Router::new()
                .fallback_service(ServeDir::new(&config.static_dir).fallback(static_router));
        }

        Router::new()
            .route("/ws", get(livereload::ws_handler))
            .nest_service("/static", static_router)
            .fallback(handle_page)
            .with_state(state)
            // Dev-mode: never let the browser cache responses, otherwise
            // edits to templates or static files won't show up without a
            // hard reload.
            .layer(SetResponseHeaderLayer::overriding(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-store"),
            ))
    }
}

/// Catch-all handler: serves root files (favicon, robots.txt, etc.),
/// then falls back to rendered pages with live-reload script injection.
async fn handle_page(State(state): State<Arc<AppState>>, req: axum::extract::Request) -> Response {
    let path = req.uri().path().to_string();
    let site = state.site.read().await;

    // Root files (favicon.ico, robots.txt, sitemap.xml, etc.)
    let filename = path.trim_start_matches('/');
    if let Some((_, content)) = site.root_files.iter().find(|(name, _)| name == filename) {
        let content_type = match std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
        {
            Some("ico") => "image/x-icon",
            Some("png") => "image/png",
            Some("xml") => "application/xml",
            Some("txt") => "text/plain; charset=utf-8",
            Some("webmanifest") => "application/manifest+json",
            _ => "application/octet-stream",
        };
        return ([(header::CONTENT_TYPE, content_type)], content.clone()).into_response();
    }

    match site.lookup(&path) {
        Some(html) => Html(livereload::inject_live_reload(html)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Html(livereload::inject_live_reload(&site.not_found_html)),
        )
            .into_response(),
    }
}

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("received ctrl-c, shutting down"),
        Err(e) => tracing::error!("failed to install ctrl-c handler: {e}"),
    }
}
