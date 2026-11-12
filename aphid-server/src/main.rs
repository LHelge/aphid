use std::process::ExitCode;

use aphid_server::{Config, Server, Telemetry};
use clap::Parser;

fn main() -> ExitCode {
    let config = Config::parse();

    // Telemetry lives outside the runtime: the OTLP exporters run blocking
    // HTTP clients on their own threads, and the final flush blocks.
    let telemetry = match Telemetry::init(&config) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("aphid-server: {err}");
            return ExitCode::FAILURE;
        }
    };

    let result = tokio::runtime::Runtime::new()
        .map_err(aphid_server::Error::from)
        .and_then(|runtime| {
            runtime.block_on(async {
                Server::bind(&config)
                    .await?
                    .run(Server::shutdown_signal()?)
                    .await
            })
        });

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
