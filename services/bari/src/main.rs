use pcoc_bari::bitcoin_rpc::{BitcoinRpc, BitcoinRpcConfig, RpcAuth, UtxoPolicy};
use pcoc_bari::clock::{BariClock, ChainTip};
use pcoc_bari::header_source::{GossipNetwork, HeaderClock, TipTick};
use pcoc_bari::http::BariService;
use pcoc_bari::operator::VERSION as BARI_VERSION;
use pcoc_bari::storage::FileStore;
use pcoc_bari::synth_commit::{SynthCommit, SYNTH_INDEX_NETWORK};
use std::env;
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

/// Live Core plant settings. Set on the test process, startup refuses.
const CORE_PLANT_ENV: &[&str] = &[
    "BARI_BITCOIN_NETWORK",
    "BARI_BITCOIN_RPC_URL",
    "BARI_BITCOIN_WALLET",
    "BARI_BITCOIN_COOKIE_FILE",
    "BARI_BITCOIN_RPC_USER",
    "BARI_BITCOIN_RPC_PASSWORD",
    "BARI_FEE_RATE_SAT_VB",
    "BARI_FEE_DYNAMIC",
    "BARI_FEE_CAP_SAT_VB",
    "BARI_FEE_MULTIPLIER",
    "BARI_FEE_BID_SAT_VB",
    "BARI_DEFER_ABOVE_SAT_VB",
    "BARI_PLANT_UTXO_MIN_CONF",
    "BARI_PLANT_UTXO_RESERVE_MIN",
    "BARI_PLANT_UTXO_RESERVE_MAX",
    "BARI_PLANT_UTXO_CONSOLIDATE",
];

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("bari fatal: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).any(|arg| arg == "--self-check") {
        let arm = pcoc_bari::header_source::release_self_check()
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::Other, message))?;
        println!("live header source: {arm}");
        return Ok(());
    }
    if ops_fixture_requested() {
        return serve_ops_fixture().await;
    }
    let data_dir = required_path("BARI_DATA_DIR")?;
    let origin = public_origin()?;
    let bind_address: SocketAddr = env::var("BARI_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38080".to_owned())
        .parse()?;
    let soft_max = optional_u64("BARI_POOL_SOFT_MAX")?.map(|n| n as usize);
    let store = FileStore::open(&data_dir)?;
    match tip_mode()? {
        TipMode::Rpc { poll_interval } => {
            let network = required_string("BARI_BITCOIN_NETWORK")?;
            let backend = open_bitcoin_backend(network.clone())?;
            let fee_label = backend.fee_log_label();
            let clock = with_born_height(BariClock::open_for_network_with_soft_max(
                backend, store, &network, soft_max,
            )?);
            let initial_tip = clock.current_tip();
            let confirmation_window_tips = clock.confirmation_window_tips();
            let service = BariService::new(clock, &origin)?;
            let listener = tokio::net::TcpListener::bind(bind_address).await?;
            eprintln!(
                "bari v{BARI_VERSION} network={} bind={} data={} tip_height={} tip_hash={} poll_ms={} hashcash_bits={} soft_max={:?} confirmation_window_tips={confirmation_window_tips} tip_source=rpc {fee_label}",
                network,
                listener.local_addr()?,
                data_dir.display(),
                initial_tip.height,
                pcoc_bari::encode_bitcoin_id(&initial_tip.hash),
                poll_interval.as_millis(),
                pcoc_bari::hashcash::effective_bits(),
                soft_max.filter(|n| *n > 0),
            );
            let server = axum::serve(listener, service.router()).into_future();
            let poller = poll_tips(service.clone(), poll_interval);
            tokio::pin!(server);
            tokio::pin!(poller);
            tokio::select! {
                result = &mut server => result?,
                result = &mut poller => result?,
                result = tokio::signal::ctrl_c() => result?,
            }
        }
        TipMode::Synth { peer } => {
            let backend = Arc::new(SynthCommit::new());
            // Regtest here is Bitcoin header wire magic on the synth peer.
            // It is not a Bitcoin Core regtest node and it is not the plant.
            let mut headers = HeaderClock::start(&peer, GossipNetwork::Regtest);
            let (headers, first) = tokio::task::spawn_blocking(move || {
                headers
                    .wait_tips_after(None)
                    .map(|ticks| (headers, ticks))
                    .map_err(|err| err.message)
            })
            .await??;
            let opened = first.first().ok_or("synth returned no tip")?;
            let tip = chain_tip(opened)?;
            let opened_hash = opened.hash;
            let clock = with_born_height(BariClock::open_at_supplied_tip(
                backend,
                store,
                tip,
                SYNTH_INDEX_NETWORK,
                soft_max,
            )?);
            let initial_tip = clock.current_tip();
            let confirmation_window_tips = clock.confirmation_window_tips();
            let service = BariService::new(clock, &origin)?;
            let listener = tokio::net::TcpListener::bind(bind_address).await?;
            eprintln!(
                "bari v{BARI_VERSION} network={} bind={} data={} tip_height={} tip_hash={} hashcash_bits={} soft_max={:?} confirmation_window_tips={confirmation_window_tips} tip_source=synth plant=synth-commit peer={}",
                SYNTH_INDEX_NETWORK,
                listener.local_addr()?,
                data_dir.display(),
                initial_tip.height,
                pcoc_bari::encode_bitcoin_id(&initial_tip.hash),
                pcoc_bari::hashcash::effective_bits(),
                soft_max.filter(|n| *n > 0),
                peer,
            );
            let server = axum::serve(listener, service.router()).into_future();
            let follower = follow_headers(service.clone(), headers, opened_hash, "synth");
            tokio::pin!(server);
            tokio::pin!(follower);
            tokio::select! {
                result = &mut server => result?,
                result = &mut follower => result?,
                result = tokio::signal::ctrl_c() => result?,
            }
        }
        TipMode::BitcoinGossip { peer } => {
            let network = required_string("BARI_BITCOIN_NETWORK")?;
            validate_bitcoin_gossip_network(&network)?;
            refuse_synth_env_on_bitcoin_gossip(&synth_env_set())?;
            let backend = open_bitcoin_backend(network.clone())?;
            let fee_label = backend.fee_log_label();
            let mut headers = HeaderClock::start(&peer, GossipNetwork::Mainnet);
            let (headers, first) = tokio::task::spawn_blocking(move || {
                headers
                    .wait_tips_after(None)
                    .map(|ticks| (headers, ticks))
                    .map_err(|err| err.message)
            })
            .await??;
            let opened = first.first().ok_or("bitcoin gossip returned no tip")?;
            let tip = chain_tip(opened)?;
            let opened_hash = opened.hash;
            let clock = with_born_height(BariClock::open_at_supplied_tip(
                backend, store, tip, &network, soft_max,
            )?);
            let initial_tip = clock.current_tip();
            let confirmation_window_tips = clock.confirmation_window_tips();
            let service = BariService::new(clock, &origin)?;
            let listener = tokio::net::TcpListener::bind(bind_address).await?;
            eprintln!(
                "bari v{BARI_VERSION} network={} bind={} data={} tip_height={} tip_hash={} hashcash_bits={} soft_max={:?} confirmation_window_tips={confirmation_window_tips} tip_source=bitcoin-gossip plant=bitcoin-core peer={} {fee_label}",
                network,
                listener.local_addr()?,
                data_dir.display(),
                initial_tip.height,
                pcoc_bari::encode_bitcoin_id(&initial_tip.hash),
                pcoc_bari::hashcash::effective_bits(),
                soft_max.filter(|n| *n > 0),
                peer,
            );
            let server = axum::serve(listener, service.router()).into_future();
            let follower = follow_headers(service.clone(), headers, opened_hash, "bitcoin-gossip");
            tokio::pin!(server);
            tokio::pin!(follower);
            tokio::select! {
                result = &mut server => result?,
                result = &mut follower => result?,
                result = tokio::signal::ctrl_c() => result?,
            }
        }
    }
    Ok(())
}

/// SPEC §13.1. Bari's own public origin, serialized before it is stored.
fn public_origin() -> Result<String, Box<dyn std::error::Error>> {
    let raw = env::var("BARI_PUBLIC_ORIGIN").map_err(|_| {
        "BARI_PUBLIC_ORIGIN is required: the public http or https origin of this Bari"
    })?;
    Ok(pcoc_bari::hashcash::bari_origin(&raw)?.into())
}

/// Production serve floor. Tests build a clock and leave the floor at 0,
/// or set their own. This is not read from the environment.
fn with_born_height(mut clock: BariClock) -> BariClock {
    clock.set_born_height(pcoc_bari::BORN_HEIGHT);
    clock
}

fn ops_fixture_requested() -> bool {
    matches!(
        env::var("BARI_OPS_FIXTURE").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE")
    )
}

/// `BARI_OPS_FIXTURE=1` serves `GET /ops` from the built-in fixture.
/// No data directory, no Bitcoin node, no broadcast.
async fn serve_ops_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let bind_address: SocketAddr = env::var("BARI_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38080".to_owned())
        .parse()?;
    let instance = env::var("BARI_INSTANCE").unwrap_or_else(|_| "local".to_owned());
    let commit = env::var("BARI_COMMIT").unwrap_or_else(|_| "unknown".to_owned());
    let view = pcoc_bari::operator::fixture_view(&instance, &commit);
    let ops = pcoc_bari::operator::render(&view);
    let states = pcoc_bari::operator::render_states(&view);
    let health = pcoc_bari::http::health_json(view.tip);
    let app = axum::Router::new()
        .route(
            "/ops",
            axum::routing::get(move || {
                let body = ops.clone();
                async move {
                    (
                        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                        body,
                    )
                }
            }),
        )
        .route(
            "/states",
            axum::routing::get(move || {
                let body = states.clone();
                async move {
                    (
                        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                        body,
                    )
                }
            }),
        )
        .route(
            "/health",
            axum::routing::get(move || {
                let body = health.clone();
                async move {
                    (
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        body,
                    )
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind(bind_address).await?;
    eprintln!(
        "bari v{BARI_VERSION} ops fixture instance={instance} bind={} GET /ops",
        listener.local_addr()?,
    );
    axum::serve(listener, app).await?;
    Ok(())
}

enum TipMode {
    Rpc { poll_interval: Duration },
    Synth { peer: String },
    BitcoinGossip { peer: String },
}

fn tip_mode() -> Result<TipMode, Box<dyn std::error::Error>> {
    let raw = env::var("BARI_TIP_SOURCE").unwrap_or_else(|_| "rpc".to_owned());
    match classify_tip_source(&raw)? {
        TipKind::Rpc => {
            let poll_interval =
                Duration::from_millis(optional_u64("BARI_POLL_INTERVAL_MS")?.unwrap_or(5_000));
            if poll_interval.is_zero() {
                return Err("BARI_POLL_INTERVAL_MS must be positive".into());
            }
            Ok(TipMode::Rpc { poll_interval })
        }
        TipKind::Synth => {
            let peer = required_string("BARI_SYNTH_PEER")?;
            validate_synth_peer(&peer)?;
            let wire = env::var("BARI_SYNTH_NETWORK").ok();
            validate_synth_wire(wire.as_deref())?;
            refuse_core_plant_on_synth(&core_plant_env_set())?;
            Ok(TipMode::Synth { peer })
        }
        TipKind::BitcoinGossip => {
            let peer = live_gossip_peer_env()?;
            Ok(TipMode::BitcoinGossip { peer })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TipKind {
    Rpc,
    Synth,
    BitcoinGossip,
}

/// `rpc` is the unset poll. It is not the live plane tip.
/// `synth` is the test plane. `bitcoin-gossip` is the live plane tip.
fn classify_tip_source(raw: &str) -> Result<TipKind, String> {
    match raw {
        "" | "rpc" => Ok(TipKind::Rpc),
        "synth" => Ok(TipKind::Synth),
        "bitcoin-gossip" => Ok(TipKind::BitcoinGossip),
        other => Err(format!(
            "BARI_TIP_SOURCE {other:?} must be rpc, synth, or bitcoin-gossip"
        )),
    }
}

fn core_plant_env_set() -> Vec<String> {
    CORE_PLANT_ENV
        .iter()
        .filter(|name| env::var(name).is_ok())
        .map(|name| (*name).to_owned())
        .collect()
}

/// Testing-phase keys from #232. They are not the live decision path.
/// A set key is a stale unit file. Startup names the locked cap instead of
/// honouring 0.6 / previous-block p10.
fn testing_fee_env_error(set: &[String]) -> Result<(), String> {
    if set.is_empty() {
        return Ok(());
    }
    Err(format!(
        "testing-phase fee settings are set ({}); remove them before start. The bid is min(BARI_FEE_MULTIPLIER × next-block minimum, BARI_FEE_CAP_SAT_VB). BARI_FEE_CAP_SAT_VB is 1.1. BARI_FEE_MULTIPLIER is 1.5. A tree is broadcast only while the next-block minimum is below 1.1. BARI_FEE_BID_SAT_VB and BARI_DEFER_ABOVE_SAT_VB are not read",
        set.join(", ")
    ))
}

fn refuse_testing_fee_env() -> Result<(), String> {
    const TESTING: &[&str] = &["BARI_FEE_BID_SAT_VB", "BARI_DEFER_ABOVE_SAT_VB"];
    let set: Vec<String> = TESTING
        .iter()
        .copied()
        .filter(|name| env::var(name).is_ok())
        .map(str::to_owned)
        .collect();
    testing_fee_env_error(&set)
}

fn open_bitcoin_backend(network: String) -> Result<Arc<BitcoinRpc>, Box<dyn std::error::Error>> {
    refuse_testing_fee_env()?;
    let wallet_name = required_string("BARI_BITCOIN_WALLET")?;
    let rpc_url = Url::parse(
        &env::var("BARI_BITCOIN_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:38332".to_owned()),
    )?;
    let auth = rpc_auth()?;
    let fee_rate_sat_vb = optional_f64("BARI_FEE_RATE_SAT_VB")?;
    let fee_dynamic = optional_flag_01("BARI_FEE_DYNAMIC")?;
    let fee_cap_sat_vb = optional_f64("BARI_FEE_CAP_SAT_VB")?;
    let fee_multiplier = optional_f64("BARI_FEE_MULTIPLIER")?;
    let utxo_policy = UtxoPolicy::from_parts(
        optional_u64("BARI_PLANT_UTXO_MIN_CONF")?,
        optional_u64("BARI_PLANT_UTXO_RESERVE_MIN")?,
        optional_u64("BARI_PLANT_UTXO_RESERVE_MAX")?,
        optional_flag_01("BARI_PLANT_UTXO_CONSOLIDATE")?,
    )?;
    Ok(Arc::new(BitcoinRpc::new(BitcoinRpcConfig {
        rpc_url,
        wallet_name,
        expected_network: network,
        auth,
        fee_rate_sat_vb,
        fee_dynamic,
        fee_cap_sat_vb,
        fee_multiplier,
        utxo_policy,
        timeout: Duration::from_secs(30),
    })?))
}

/// Live tip clock is Bitcoin mainnet gossip. regtest, testnet4, and signet
/// are not that clock. The test tip stays `synth`.
fn validate_bitcoin_gossip_network(name: &str) -> Result<(), String> {
    if name == "main" {
        return Ok(());
    }
    Err(format!(
        "BARI_TIP_SOURCE=bitcoin-gossip requires BARI_BITCOIN_NETWORK=main (Bitcoin selected-chain gossip); got {name}. regtest and testnet4 are not this tip clock. The test tip is synth"
    ))
}

fn live_gossip_peer_from_env(raw: Option<&str>) -> Result<String, String> {
    let Some(raw) = raw else {
        return Err(
            "PCOC_LIVE_GOSSIP_PEER is required for BARI_TIP_SOURCE=bitcoin-gossip (host:port). This tree does not fill a live gossip peer"
                .to_owned(),
        );
    };
    let peer = raw.trim();
    if host_port(peer).is_err() {
        return Err("PCOC_LIVE_GOSSIP_PEER must be host:port and must not use port 0".to_owned());
    }
    Ok(peer.to_owned())
}

fn live_gossip_peer_env() -> Result<String, String> {
    live_gossip_peer_from_env(env::var("PCOC_LIVE_GOSSIP_PEER").ok().as_deref())
}

fn synth_env_set() -> Vec<String> {
    ["BARI_SYNTH_PEER", "BARI_SYNTH_NETWORK"]
        .into_iter()
        .filter(|name| env::var(name).is_ok())
        .map(str::to_owned)
        .collect()
}

fn refuse_synth_env_on_bitcoin_gossip(set: &[String]) -> Result<(), String> {
    if set.is_empty() {
        return Ok(());
    }
    Err(format!(
        "BARI_TIP_SOURCE=bitcoin-gossip refuses synth settings ({set}); the test tip is synth and is not the live tip",
        set = set.join(", ")
    ))
}

/// Test plant refuses every live Core setting. regtest and testnet4 are named
/// because those were the backends ops pointed at the synth tip.
fn refuse_core_plant_on_synth(set: &[String]) -> Result<(), String> {
    if set.is_empty() {
        return Ok(());
    }
    Err(format!(
        "BARI_TIP_SOURCE=synth refuses Bitcoin Core plant config ({set}); regtest and testnet4 are not test notaries; test plant is a simulated commit under the synth tip",
        set = set.join(", ")
    ))
}

fn validate_synth_peer(peer: &str) -> Result<(), String> {
    if host_port(peer).is_err() {
        return Err("BARI_SYNTH_PEER must be host:port and must not use port 0".to_owned());
    }
    Ok(())
}

/// `BARI_SYNTH_NETWORK` is Bitcoin header wire magic for the synth peer.
/// `regtest` here is that magic. It does not start or select Bitcoin Core.
fn validate_synth_wire(name: Option<&str>) -> Result<(), String> {
    match name {
        None | Some("regtest") => Ok(()),
        Some(other) => Err(format!(
            "BARI_SYNTH_NETWORK must be omitted or regtest (Bitcoin header wire magic, not a Core network); got {other}"
        )),
    }
}

async fn follow_headers(
    service: BariService,
    mut headers: HeaderClock,
    opened: [u8; 32],
    tip_source: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut after = Some(opened);
    loop {
        let mut current = headers;
        let waited = tokio::task::spawn_blocking(move || {
            current
                .wait_tips_after(after)
                .map(|ticks| (current, ticks))
                .map_err(|err| err.message)
        })
        .await??;
        headers = waited.0;
        for tick in waited.1 {
            let tip = chain_tip(&tick)?;
            let service = service.clone();
            if let Some(report) = tokio::task::spawn_blocking(move || service.on_tip(tip)).await?? {
                eprintln!(
                    "tip height={} hash={} continuous={} closed={:?} included={:?} missed={:?} archived={:?} tip_source={tip_source}",
                    report.tip.height,
                    pcoc_bari::encode_bitcoin_id(&report.tip.hash),
                    report.continuous,
                    report.closed_batch,
                    report.included,
                    report.missed,
                    report.archived,
                );
            }
            after = Some(tick.hash);
        }
    }
}

fn chain_tip(tick: &TipTick) -> Result<ChainTip, String> {
    ChainTip::from_gossip_tick(tick)
}

fn host_port(value: &str) -> Result<(), ()> {
    let port = if let Some(rest) = value.strip_prefix('[') {
        let (host, port) = rest.rsplit_once("]:").ok_or(())?;
        if host.is_empty() {
            return Err(());
        }
        port
    } else {
        let (host, port) = value.rsplit_once(':').ok_or(())?;
        if host.is_empty() || host.contains(':') {
            return Err(());
        }
        port
    };
    let parsed: u16 = port.parse().map_err(|_| ())?;
    if parsed == 0 {
        Err(())
    } else {
        Ok(())
    }
}

async fn poll_tips(
    service: BariService,
    poll_interval: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut interval = tokio::time::interval(poll_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let service = service.clone();
        let report = tokio::task::spawn_blocking(move || service.poll_once()).await??;
        if let Some(report) = report {
            eprintln!(
                "tip height={} hash={} continuous={} closed={:?} included={:?} missed={:?} archived={:?}",
                report.tip.height,
                pcoc_bari::encode_bitcoin_id(&report.tip.hash),
                report.continuous,
                report.closed_batch,
                report.included,
                report.missed,
                report.archived,
            );
        }
    }
}

fn rpc_auth() -> Result<RpcAuth, Box<dyn std::error::Error>> {
    if let Ok(path) = env::var("BARI_BITCOIN_COOKIE_FILE") {
        return Ok(RpcAuth::from_cookie(&std::fs::read_to_string(path)?)?);
    }
    let username = required_string("BARI_BITCOIN_RPC_USER")?;
    let password = required_string("BARI_BITCOIN_RPC_PASSWORD")?;
    Ok(RpcAuth { username, password })
}

fn required_path(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(required_string(name)?))
}

fn required_string(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = env::var(name).map_err(|_| format!("{name} is required"))?;
    if value.is_empty() {
        return Err(format!("{name} must not be empty").into());
    }
    Ok(value)
}

fn optional_f64(name: &str) -> Result<Option<f64>, Box<dyn std::error::Error>> {
    env::var(name)
        .ok()
        .map(|value| value.parse::<f64>().map_err(Into::into))
        .transpose()
}

fn optional_u64(name: &str) -> Result<Option<u64>, Box<dyn std::error::Error>> {
    env::var(name)
        .ok()
        .map(|value| value.parse::<u64>().map_err(Into::into))
        .transpose()
}

/// Unset is `None`. `0` and `1` are the only accepted values.
fn optional_flag_01(name: &str) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match env::var(name) {
        Err(_) => Ok(None),
        Ok(value) => match value.as_str() {
            "0" => Ok(Some(false)),
            "1" => Ok(Some(true)),
            _ => Err(format!("{name} must be 0 or 1").into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_refuses_core_regtest_and_testnet4_plant_config() {
        let regtest = refuse_core_plant_on_synth(&["BARI_BITCOIN_NETWORK".to_owned()]).unwrap_err();
        assert!(regtest.contains("BARI_BITCOIN_NETWORK"), "{regtest}");
        assert!(regtest.contains("regtest and testnet4"), "{regtest}");
        assert!(regtest.contains("simulated commit"), "{regtest}");

        let rpc = refuse_core_plant_on_synth(&[
            "BARI_BITCOIN_RPC_URL".to_owned(),
            "BARI_BITCOIN_WALLET".to_owned(),
        ])
        .unwrap_err();
        assert!(rpc.contains("BARI_BITCOIN_RPC_URL"), "{rpc}");
        assert!(rpc.contains("BARI_BITCOIN_WALLET"), "{rpc}");
        assert!(refuse_core_plant_on_synth(&[]).is_ok());
    }

    #[test]
    fn synth_wire_magic_is_not_a_core_network() {
        assert!(validate_synth_wire(None).is_ok());
        assert!(validate_synth_wire(Some("regtest")).is_ok());
        let testnet4 = validate_synth_wire(Some("testnet4")).unwrap_err();
        assert!(testnet4.contains("wire magic"), "{testnet4}");
        assert!(testnet4.contains("not a Core network"), "{testnet4}");
        assert!(validate_synth_wire(Some("main")).is_err());
        assert!(validate_synth_peer("127.0.0.1:38532").is_ok());
        assert!(validate_synth_peer("127.0.0.1:0").is_err());
        assert!(validate_synth_peer("no-port").is_err());
    }

    #[test]
    fn rpc_lab_mode_stays_the_poll_and_is_not_the_live_arm() {
        assert_eq!(classify_tip_source("").unwrap(), TipKind::Rpc);
        assert_eq!(classify_tip_source("rpc").unwrap(), TipKind::Rpc);
        let clock = HeaderClock::start("self-check.invalid:8333", GossipNetwork::Mainnet);
        assert!(clock.live_source_in_build());
        assert!(!clock.absent_live_source());
    }

    #[test]
    fn tip_source_names_are_rpc_synth_or_bitcoin_gossip() {
        assert_eq!(classify_tip_source("").unwrap(), TipKind::Rpc);
        assert_eq!(classify_tip_source("rpc").unwrap(), TipKind::Rpc);
        assert_eq!(classify_tip_source("synth").unwrap(), TipKind::Synth);
        assert_eq!(
            classify_tip_source("bitcoin-gossip").unwrap(),
            TipKind::BitcoinGossip
        );
        let http = classify_tip_source("http").unwrap_err();
        assert!(http.contains("bitcoin-gossip"), "{http}");
        assert!(classify_tip_source("http-tip").is_err());
    }

    #[test]
    fn bitcoin_gossip_peer_is_env_only_and_mainnet_only() {
        let missing = live_gossip_peer_from_env(None).unwrap_err();
        assert!(missing.contains("PCOC_LIVE_GOSSIP_PEER"), "{missing}");
        assert!(missing.contains("does not fill"), "{missing}");
        assert!(live_gossip_peer_from_env(Some("  ")).is_err());
        assert!(live_gossip_peer_from_env(Some("peer.invalid:0")).is_err());
        assert_eq!(
            live_gossip_peer_from_env(Some("  peer.invalid:9  ")).unwrap(),
            "peer.invalid:9"
        );
        assert!(validate_bitcoin_gossip_network("main").is_ok());
        for name in ["regtest", "testnet4", "signet", ""] {
            let err = validate_bitcoin_gossip_network(name).unwrap_err();
            assert!(err.contains("BARI_BITCOIN_NETWORK=main"), "{name}: {err}");
            assert!(err.contains("synth"), "{name}: {err}");
        }
        let mixed =
            refuse_synth_env_on_bitcoin_gossip(&["BARI_SYNTH_PEER".to_owned()]).unwrap_err();
        assert!(mixed.contains("BARI_SYNTH_PEER"), "{mixed}");
        assert!(mixed.contains("test tip is synth"), "{mixed}");
        assert!(refuse_synth_env_on_bitcoin_gossip(&[]).is_ok());
    }

    #[test]
    fn testing_phase_fee_keys_are_not_the_live_decision_path() {
        let err = testing_fee_env_error(&[
            "BARI_FEE_BID_SAT_VB".to_owned(),
            "BARI_DEFER_ABOVE_SAT_VB".to_owned(),
        ])
        .unwrap_err();
        assert!(err.contains("BARI_FEE_BID_SAT_VB"), "{err}");
        assert!(err.contains("BARI_DEFER_ABOVE_SAT_VB"), "{err}");
        assert!(err.contains("BARI_FEE_CAP_SAT_VB"), "{err}");
        assert!(err.contains("1.1"), "{err}");
        assert!(err.contains("BARI_FEE_MULTIPLIER"), "{err}");
        assert!(err.contains("1.5"), "{err}");
        assert!(err.contains("not read"), "{err}");
        assert!(testing_fee_env_error(&[]).is_ok());
    }

    #[test]
    fn core_plant_env_names_include_the_utxo_reserve_dials() {
        for name in [
            "BARI_PLANT_UTXO_MIN_CONF",
            "BARI_PLANT_UTXO_RESERVE_MIN",
            "BARI_PLANT_UTXO_RESERVE_MAX",
            "BARI_PLANT_UTXO_CONSOLIDATE",
            "BARI_FEE_DYNAMIC",
            "BARI_FEE_CAP_SAT_VB",
            "BARI_FEE_MULTIPLIER",
            "BARI_FEE_RATE_SAT_VB",
            "BARI_FEE_BID_SAT_VB",
            "BARI_DEFER_ABOVE_SAT_VB",
        ] {
            assert!(
                CORE_PLANT_ENV.contains(&name),
                "{name} must be refused on the synth tip"
            );
        }
    }
}
