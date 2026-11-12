//! `aphid-server`: a small production static-file server for sites built
//! with aphid. Plain HTTP/1.1, intended to run behind a TLS-terminating
//! reverse proxy.

mod access_log;
mod config;
mod error;
mod not_found;
mod server;
mod telemetry;

pub use config::{Config, LogFormat};
pub use error::{Error, Result};
pub use server::Server;
pub use telemetry::Telemetry;
