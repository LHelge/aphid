use std::process::ExitCode;

use aphid_server::{Config, Server, Telemetry};
use clap::Parser;

#[tokio::main]
async fn main() -> ExitCode {
    let config = Config::parse();
    let telemetry = match Telemetry::init(&config) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("aphid-server: {err}");
            return ExitCode::FAILURE;
        }
    };

    let result = match Server::bind(&config).await {
        Ok(server) => server.run(Server::shutdown_signal()).await,
        Err(err) => Err(err),
    };

    let code = match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "fatal");
            ExitCode::FAILURE
        }
    };
    telemetry.shutdown();
    code
}
