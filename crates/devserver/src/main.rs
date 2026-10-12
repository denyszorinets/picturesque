//! Runs the development HTTP server.
//!
//! Listens on `127.0.0.1:5174` unless `PICTURESQUE_DEVSERVER_ADDR` is set.
//! Only loopback addresses are accepted.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use picturesque_app::api::Api;

const DEFAULT_ADDR: &str = "127.0.0.1:5174";

#[tokio::main]
async fn main() -> ExitCode {
    match serve().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("picturesque-devserver: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> Result<(), String> {
    let raw =
        std::env::var("PICTURESQUE_DEVSERVER_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_owned());
    let addr: SocketAddr = raw
        .parse()
        .map_err(|e| format!("invalid address `{raw}`: {e}"))?;
    if !addr.ip().is_loopback() {
        return Err(format!("refusing to listen on non-loopback address {addr}"));
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("bind {addr}: {e}"))?;
    println!("picturesque-devserver listening on http://{addr}");

    axum::serve(
        listener,
        picturesque_devserver::router(Arc::new(Api::new())),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|e| format!("server error: {e}"))
}
