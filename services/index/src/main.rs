//! pcoc-index — read-only portable-index HTTP (`index.groundinglab.io` → :38081).

use pcoc_index::IndexService;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("pcoc-index fatal: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = env::var("PCOC_INDEX_DATA_DIR")
        .map(PathBuf::from)
        .map_err(|_| "PCOC_INDEX_DATA_DIR must point at the index cache directory (…/index)")?;
    let bind: SocketAddr = env::var("PCOC_INDEX_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38081".to_owned())
        .parse()?;

    let sequencer_dir = env::var_os("PCOC_INDEX_SEQUENCER_DIR").map(PathBuf::from);
    let service = IndexService::open_with_sequencer(&data_dir, sequencer_dir.as_deref())?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!(
        "pcoc-index v0.1 bind={} data_dir={} sequencer_clocks={} (read-only height-*.json cache)",
        listener.local_addr()?,
        data_dir.display(),
        service.clock_count()
    );
    axum::serve(listener, service.router()).await?;
    Ok(())
}
