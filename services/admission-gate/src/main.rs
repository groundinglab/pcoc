//! `admission-service` — Satoshi Hike 2.2 admission gate.
//!
//! The process reads one config file. The path is `--config` or `GATE_CONFIG`.
//! `--version` prints the gate version, commit and Rust SDK version, then exits.
//! It does not read `PCOC_BIND_THREADS` or any `GL_VERIFY_*` variable.
#![forbid(unsafe_code)]

use pcoc_admission::{http, Config, Service};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("admission-service fatal: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if env::args()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("{}", http::version_line());
        return Ok(());
    }
    let path = config_path()?;
    let cfg = Config::load(&path)?;
    pcoc_admission::config::refuse_live_replacement(&cfg.plane, cfg.allow_2_2_live)?;
    let bind = cfg.bind;
    let service = Service::open(cfg)?;
    let cfg = service.config().clone();
    let listener = tokio::net::TcpListener::bind(bind).await?;

    let (spec_name, _) = pcoc_admission::verifier::health_spec();
    eprintln!(
        "admission-service v{} commit={} sdk={} spec={} bind={} index={} log={} config={}",
        http::VERSION,
        http::COMMIT,
        http::SDK_VERSION,
        spec_name,
        listener.local_addr()?,
        cfg.index_url,
        cfg.log_dir.display(),
        cfg.config_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_owned()),
    );

    let app = service
        .router()
        .into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    eprintln!("admission-service: shutting down; in-flight jobs are dropped");
}

/// `--config <path>` wins. Otherwise `GATE_CONFIG`. Nothing else is read.
fn config_path() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    let mut from_flag = None;
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            return Err(
                "usage: admission-service --config <file> | --version\nGATE_CONFIG may name that file instead"
                    .to_owned(),
            );
        } else if arg == "--config" {
            let path = args
                .next()
                .ok_or_else(|| "--config needs a path".to_owned())?;
            from_flag = Some(path);
        } else if let Some(path) = arg.strip_prefix("--config=") {
            if path.is_empty() {
                return Err("--config needs a path".to_owned());
            }
            from_flag = Some(path.to_owned());
        } else {
            return Err(format!("unknown argument {arg}"));
        }
    }
    if let Some(path) = from_flag {
        return Ok(PathBuf::from(path));
    }
    match env::var("GATE_CONFIG") {
        Ok(raw) if !raw.trim().is_empty() => Ok(PathBuf::from(raw.trim())),
        _ => Err(
            "admission-service needs --config <file>, or GATE_CONFIG naming that file".to_owned(),
        ),
    }
}
