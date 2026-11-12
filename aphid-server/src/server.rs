use std::{
    future::Future,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use axum::{
    Router,
    extract::Request,
    handler::Handler,
    http::{StatusCode, Uri},
    middleware,
    routing::get,
};
use tokio::net::TcpListener;
use tower_http::{compression::CompressionLayer, services::ServeDir};

use crate::{Config, Error, Result, access_log::AccessLog, not_found::NotFoundPage};

pub const HEALTHZ_PATH: &str = "/healthz";

/// A bound, ready-to-run static-file server.
pub struct Server {
    root: PathBuf,
    listener: TcpListener,
}

impl Server {
    /// Validate the site root and bind `0.0.0.0:<port>`.
    pub async fn bind(config: &Config) -> Result<Self> {
        if !tokio::fs::metadata(&config.root).await?.is_dir() {
            return Err(Error::RootNotDirectory(config.root.clone()));
        }
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, config.port)).await?;
        Ok(Self {
            root: config.root.clone(),
            listener,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    /// The axum application: `/healthz`, then files from the root.
    ///
    /// Compression is the outermost layer, so the access log sees the
    /// response before it is encoded and still has its `Content-Length`.
    pub fn router(root: &std::path::Path) -> Router {
        let not_found = NotFoundPage::handler.with_state(NotFoundPage::new(root));
        let files = ServeDir::new(root)
            .ignore_multi_range_requests(true)
            .fallback(not_found);

        Router::new()
            .route(HEALTHZ_PATH, get(|| async { StatusCode::OK }))
            .fallback_service(files)
            .layer(middleware::map_request(Self::collapse_leading_slashes))
            .layer(middleware::from_fn(AccessLog::middleware))
            .layer(CompressionLayer::new())
    }

    /// Serve until `shutdown` resolves, then stop accepting and drain
    /// in-flight requests.
    pub async fn run(self, shutdown: impl Future<Output = ()> + Send + 'static) -> Result<()> {
        tracing::info!(addr = %self.local_addr()?, root = %self.root.display(), "listening");
        let app = Self::router(&self.root);
        axum::serve(
            self.listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown)
        .await?;
        tracing::info!("shut down");
        Ok(())
    }

    /// Resolves on SIGTERM or SIGINT.
    pub async fn shutdown_signal() {
        let interrupt = async {
            let _ = tokio::signal::ctrl_c().await;
        };
        #[cfg(unix)]
        let terminate = async {
            use tokio::signal::unix::{SignalKind, signal};
            match signal(SignalKind::terminate()) {
                Ok(mut sig) => {
                    sig.recv().await;
                }
                Err(_) => std::future::pending().await,
            }
        };
        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            () = interrupt => {},
            () = terminate => {},
        }
        tracing::info!("shutdown signal received, draining connections");
    }

    /// `//host/x` → `/host/x`, so a trailing-slash redirect can never produce a
    /// protocol-relative `Location` pointing at another host.
    async fn collapse_leading_slashes(mut request: Request) -> Request {
        let uri = request.uri();
        if uri.path().starts_with("//") {
            let path = format!("/{}", uri.path().trim_start_matches('/'));
            let path_and_query = match uri.query() {
                Some(q) => format!("{path}?{q}"),
                None => path,
            };
            if let Ok(new) = Uri::builder().path_and_query(path_and_query).build() {
                *request.uri_mut() = new;
            }
        }
        request
    }
}
