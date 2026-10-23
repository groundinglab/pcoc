//! pcoc-index — read-only portable-index HTTP (`index.groundinglab.io` → :38081).
//!
//! No arguments serves the cache. `pcoc-index rebuild` writes that cache from
//! public chain data.

use pcoc_index::chain::{
    ChainSource, EsploraSource, LimitedGet, RpcAuth, RpcSource, UreqGet, DEFAULT_ESPLORA_BASE,
    DEFAULT_MAX_RETRIES, DEFAULT_MIN_INTERVAL_MS, DEFAULT_REORG_DEPTH,
};
use pcoc_index::rebuild::{rebuild, RebuildOptions};
use pcoc_index::IndexService;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() {
    if let Err(error) = dispatch().await {
        eprintln!("pcoc-index fatal: {error}");
        std::process::exit(1);
    }
}

async fn dispatch() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("rebuild") => {
            let rest: Vec<String> = args.collect();
            // ureq is blocking. Keep it off the async runtime.
            let result = tokio::task::spawn_blocking(move || rebuild_command(&rest)).await?;
            result.map_err(|error| -> Box<dyn std::error::Error> { error })?;
            Ok(())
        }
        Some("serve") | None => serve().await,
        Some(other) => Err(format!(
            "unknown command {other}. Usage: pcoc-index [serve] | pcoc-index rebuild [options]"
        )
        .into()),
    }
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = env::var("PCOC_INDEX_DATA_DIR")
        .map(PathBuf::from)
        .map_err(|_| "PCOC_INDEX_DATA_DIR must point at the index cache directory (…/index)")?;
    let bind: SocketAddr = env::var("PCOC_INDEX_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38081".to_owned())
        .parse()?;

    let sequencer_dir = env::var_os("PCOC_INDEX_SEQUENCER_DIR").map(PathBuf::from);
    let service = IndexService::open_with_sequencer(&data_dir, sequencer_dir.as_deref())?
        .with_born_height(pcoc_bari::BORN_HEIGHT);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!(
        "pcoc-index v0.1 bind={} data_dir={} sequencer_clocks={} born_height={} (read-only height-*.json cache; median-time headers born_height-{} through born_height-1 are served by height and are not production heights; files below that stay on disk and are not served)",
        listener.local_addr()?,
        data_dir.display(),
        service.clock_count(),
        pcoc_bari::BORN_HEIGHT,
        pcoc_bari::MTP_LOOKBACK
    );
    axum::serve(listener, service.router()).await?;
    Ok(())
}

struct RebuildArgs {
    source: String,
    esplora_base: String,
    from_height: Option<u32>,
    from_txid: Option<String>,
    to_height: Option<u32>,
    out_dir: PathBuf,
    reorg_depth: u32,
    min_interval_ms: u64,
    max_retries: u32,
}

fn rebuild_command(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parsed = parse_rebuild_args(args)?;
    let timeout = Duration::from_secs(60);
    match parsed.source.as_str() {
        "esplora" => {
            let http = LimitedGet::new(
                UreqGet::new(timeout),
                Duration::from_millis(parsed.min_interval_ms),
                parsed.max_retries,
            );
            let source = EsploraSource::new(&parsed.esplora_base, http);
            run_rebuild(&parsed, &source)
        }
        "rpc" => {
            let source = rpc_source(timeout)?;
            run_rebuild(&parsed, &source)
        }
        other => Err(format!("--source must be esplora or rpc, got {other}").into()),
    }
}

fn run_rebuild(
    parsed: &RebuildArgs,
    source: &dyn ChainSource,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let to_height = match parsed.to_height {
        Some(height) => height,
        None => source.tip_height()?,
    };
    let requested_from = match (parsed.from_height, parsed.from_txid.as_deref()) {
        (Some(height), None) => height,
        (None, Some(txid)) => height_of_txid(source, txid, to_height)?,
        (Some(_), Some(_)) => {
            return Err("pass one of --from-height or --from-txid".into());
        }
        (None, None) => {
            return Err("pass --from-height or --from-txid".into());
        }
    };
    let Some((from_height, to_height)) = apply_born_floor(requested_from, to_height) else {
        return Ok(());
    };
    let report = rebuild(
        source,
        &RebuildOptions {
            from_height,
            to_height,
            out_dir: parsed.out_dir.clone(),
            reorg_depth: parsed.reorg_depth,
        },
    )?;
    eprintln!(
        "pcoc-index rebuild wrote {} new or replaced, {} unchanged, {} reorg rewrites, heights {}..={} in {}",
        report.written,
        report.unchanged,
        report.rewritten,
        report.from_height,
        report.to_height,
        parsed.out_dir.display()
    );
    Ok(())
}

/// Do not fetch heights below the median-time header floor.
///
/// The floor is `born_height - MTP_LOOKBACK`. Headers in that window are
/// written so a timestamp read can see them. They are not production heights.
/// Files already in the output directory are left in place.
fn apply_born_floor(from_height: u32, to_height: u32) -> Option<(u32, u32)> {
    let born = pcoc_bari::BORN_HEIGHT;
    let floor = born.saturating_sub(pcoc_bari::MTP_LOOKBACK);
    if to_height < floor {
        eprintln!(
            "pcoc-index rebuild: to-height {to_height} is below the median-time header floor {floor} (born_height {born} minus {}); nothing written; files already stored are left in place",
            pcoc_bari::MTP_LOOKBACK
        );
        return None;
    }
    if from_height < floor {
        eprintln!(
            "pcoc-index rebuild: from-height {from_height} is below the median-time header floor {floor} (born_height {born} minus {}); starting at {floor}; files below that height are left in place",
            pcoc_bari::MTP_LOOKBACK
        );
        return Some((floor, to_height));
    }
    Some((from_height, to_height))
}

/// Genesis plant: the block that includes this txid and its 39-byte BARI tag.
///
/// The scan walks backward from `to_height`. A public API answers slowly, so
/// the walk is bounded: `PCOC_INDEX_REBUILD_TXID_LOOKBACK` blocks (default 2016).
fn height_of_txid(
    source: &dyn ChainSource,
    txid: &str,
    to_height: u32,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let lookback: u32 = env::var("PCOC_INDEX_REBUILD_TXID_LOOKBACK")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2_016);
    let start = to_height
        .saturating_sub(lookback)
        .max(pcoc_bari::BORN_HEIGHT);
    if start > to_height {
        return Err(format!(
            "txid {txid} is below born_height {} and is not required",
            pcoc_bari::BORN_HEIGHT
        )
        .into());
    }
    for height in (start..=to_height).rev() {
        let header = source.header_at_height(height)?;
        let plants = source.plants_in_block(&header)?;
        if plants
            .iter()
            .any(|plant| plant.txid.eq_ignore_ascii_case(txid))
        {
            return Ok(height);
        }
    }
    Err(format!(
        "txid {txid} is not a BARI plant in heights {start}..={to_height}. Raise PCOC_INDEX_REBUILD_TXID_LOOKBACK or pass --from-height"
    )
    .into())
}

fn rpc_source(timeout: Duration) -> Result<RpcSource, Box<dyn std::error::Error + Send + Sync>> {
    let url = env::var("PCOC_INDEX_RPC_URL")
        .or_else(|_| env::var("BARI_BITCOIN_RPC_URL"))
        .map_err(|_| "rpc source needs PCOC_INDEX_RPC_URL (or BARI_BITCOIN_RPC_URL)")?;
    let auth = rpc_auth()?;
    Ok(RpcSource::new(url, auth, timeout))
}

fn rpc_auth() -> Result<RpcAuth, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(path) =
        env::var_os("PCOC_INDEX_RPC_COOKIE").or_else(|| env::var_os("BARI_BITCOIN_COOKIE_FILE"))
    {
        let cookie = std::fs::read_to_string(path)?;
        return Ok(RpcAuth::from_cookie(&cookie)?);
    }
    let username = env::var("PCOC_INDEX_RPC_USER")
        .or_else(|_| env::var("BARI_BITCOIN_RPC_USER"))
        .map_err(|_| {
            "rpc source needs PCOC_INDEX_RPC_COOKIE or PCOC_INDEX_RPC_USER and PCOC_INDEX_RPC_PASSWORD"
        })?;
    let password = env::var("PCOC_INDEX_RPC_PASSWORD")
        .or_else(|_| env::var("BARI_BITCOIN_RPC_PASSWORD"))
        .map_err(|_| "rpc source needs PCOC_INDEX_RPC_PASSWORD (or BARI_BITCOIN_RPC_PASSWORD)")?;
    Ok(RpcAuth { username, password })
}

fn parse_rebuild_args(
    args: &[String],
) -> Result<RebuildArgs, Box<dyn std::error::Error + Send + Sync>> {
    let mut source = "esplora".to_owned();
    let mut esplora_base =
        env::var("PCOC_INDEX_ESPLORA_URL").unwrap_or_else(|_| DEFAULT_ESPLORA_BASE.to_owned());
    let mut from_height = None;
    let mut from_txid = None;
    let mut to_height = None;
    let mut out_dir = None;
    let mut reorg_depth = DEFAULT_REORG_DEPTH;
    let mut min_interval_ms = env::var("PCOC_INDEX_REBUILD_MIN_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_MIN_INTERVAL_MS);
    let mut max_retries = DEFAULT_MAX_RETRIES;
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        let value = || {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag {
            "--source" => {
                source = value()?;
                i += 2;
            }
            "--esplora-url" => {
                esplora_base = value()?;
                i += 2;
            }
            "--from-height" => {
                from_height = Some(value()?.parse()?);
                i += 2;
            }
            "--from-txid" => {
                from_txid = Some(value()?);
                i += 2;
            }
            "--to-height" => {
                to_height = Some(value()?.parse()?);
                i += 2;
            }
            "--out" => {
                out_dir = Some(PathBuf::from(value()?));
                i += 2;
            }
            "--reorg-depth" => {
                reorg_depth = value()?.parse()?;
                i += 2;
            }
            "--min-interval-ms" => {
                min_interval_ms = value()?.parse()?;
                i += 2;
            }
            "--max-retries" => {
                max_retries = value()?.parse()?;
                i += 2;
            }
            "--help" | "-h" => {
                eprintln!("{}", rebuild_usage());
                std::process::exit(0);
            }
            other => return Err(format!("unknown option {other}\n{}", rebuild_usage()).into()),
        }
    }
    let Some(out_dir) = out_dir else {
        return Err(format!("--out DIR is required\n{}", rebuild_usage()).into());
    };
    if from_height.is_some() && from_txid.is_some() {
        return Err("pass one of --from-height or --from-txid".into());
    }
    Ok(RebuildArgs {
        source,
        esplora_base,
        from_height,
        from_txid,
        to_height,
        out_dir,
        reorg_depth,
        min_interval_ms,
        max_retries,
    })
}

fn rebuild_usage() -> &'static str {
    "pcoc-index rebuild --out DIR [--source esplora|rpc] [--from-height N | --from-txid TXID] [--to-height N]\n\
     esplora (default) reads https://mempool.space/api. Override with --esplora-url or PCOC_INDEX_ESPLORA_URL.\n\
     rpc reads Bitcoin Core JSON-RPC (getblockhash, getblockheader, getblock) at PCOC_INDEX_RPC_URL.\n\
     Auth is PCOC_INDEX_RPC_COOKIE, or PCOC_INDEX_RPC_USER and PCOC_INDEX_RPC_PASSWORD.\n\
     --reorg-depth N (default 6) re-fetches the last N blocks and rewrites a file whose header changed.\n\
     --min-interval-ms (default 250) and --max-retries (default 6) pace the public API. HTTP 429 waits on Retry-After."
}
