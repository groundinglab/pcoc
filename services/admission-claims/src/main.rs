//! `admission-claims` — badge issuer for groundinglab.
//!
//! The process reads one config file. The path is `--config` or `CLAIMS_CONFIG`.
//! `admission-claims backup --config <file>` seals an age-encrypted snapshot.
#![forbid(unsafe_code)]

use admission_claims::{backup, Config, Service};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Both ring and aws-lc-rs are in the tree; pick one explicitly or rustls panics at first TLS use.
    let _ = rustls::crypto::ring::default_provider().install_default();
    if let Err(error) = run().await {
        eprintln!("admission-claims fatal: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let command = command()?;
    let cfg = Config::load(&command.config)?;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_new(&cfg.log_filter)
                .map_err(|err| format!("log_filter is not usable: {err}"))?,
        )
        .with_target(false)
        .init();
    match command.action {
        Action::Serve => serve(cfg).await?,
        Action::Backup => {
            backup::run(&cfg).await?;
        }
    }
    Ok(())
}

async fn serve(cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    let bind = cfg.bind;
    let role = match cfg.role {
        admission_claims::config::Role::Writer => "writer",
        admission_claims::config::Role::Mirror => "mirror",
    };
    let dir_mode = cfg.dir_mode();
    let service = Service::open(cfg)?;
    service.prepare().await?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(
        %role,
        bind = %listener.local_addr()?,
        dir_mode,
        version = admission_claims::http::VERSION,
        commit = admission_claims::http::COMMIT,
        "admission-claims listening"
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
    tracing::info!("admission-claims shutting down");
}

struct Command {
    action: Action,
    config: PathBuf,
}

enum Action {
    Serve,
    Backup,
}

fn command() -> Result<Command, String> {
    let mut args = env::args().skip(1);
    let mut action = Action::Serve;
    let mut from_flag = None;
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            return Err("usage: admission-claims [--config <file>] | backup --config <file>\nCLAIMS_CONFIG may name that file instead".into());
        } else if arg == "backup" {
            action = Action::Backup;
        } else if arg == "--config" {
            let path = args
                .next()
                .ok_or_else(|| "--config needs a path".to_owned())?;
            from_flag = Some(path);
        } else if let Some(path) = arg.strip_prefix("--config=") {
            if path.is_empty() {
                return Err("--config needs a path".into());
            }
            from_flag = Some(path.to_owned());
        } else {
            return Err(format!("unknown argument {arg}"));
        }
    }
    let config = if let Some(path) = from_flag {
        PathBuf::from(path)
    } else {
        match env::var("CLAIMS_CONFIG") {
            Ok(raw) if !raw.trim().is_empty() => PathBuf::from(raw.trim()),
            _ => {
                return Err(
                    "admission-claims needs --config <file>, or CLAIMS_CONFIG naming that file"
                        .into(),
                );
            }
        }
    };
    Ok(Command { action, config })
}
