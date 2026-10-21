//! Runs the development HTTP server.
//!
//! Listens on `127.0.0.1:5174` unless `PICTURESQUE_DEVSERVER_ADDR` is set.
//! Only loopback addresses are accepted. The catalog is stored in
//! `PICTURESQUE_DATA_DIR` (default `target/devserver-data`).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use picturesque_app::sessions::Host;
use picturesque_devserver::EventBus;
use picturesque_infra::catalog::SqliteCatalog;
use picturesque_infra::sessions::StandardSessions;

const DEFAULT_ADDR: &str = "127.0.0.1:5174";
const DEFAULT_DATA_DIR: &str = "target/devserver-data";

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

    let data_dir = PathBuf::from(
        std::env::var("PICTURESQUE_DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.to_owned()),
    );
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("create {}: {e}", data_dir.display()))?;
    let catalog =
        Arc::new(SqliteCatalog::open(&data_dir.join("catalog.sqlite")).map_err(|e| e.to_string())?);
    let models = match std::env::var_os("PICTURESQUE_MODEL_MANIFEST") {
        // Tests serve their own models.
        Some(path) => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("read {}: {e}", path.to_string_lossy()))?;
            serde_json::from_str(&text).map_err(|e| format!("model manifest: {e}"))?
        }
        None => picturesque_app::models::catalog(),
    };
    let events = EventBus::default();
    let sessions = StandardSessions::new(
        data_dir.clone(),
        data_dir.join("cache"),
        catalog.clone(),
        Arc::new(events.clone()),
        models,
    );
    let host = Arc::new(Host::start(Arc::new(sessions), catalog).map_err(|e| e.message)?);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("bind {addr}: {e}"))?;
    println!(
        "picturesque-devserver listening on http://{addr} (data: {})",
        data_dir.display()
    );

    axum::serve(
        listener,
        picturesque_devserver::router(host.clone(), events),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|e| format!("server error: {e}"))?;
    host.shutdown();
    Ok(())
}
