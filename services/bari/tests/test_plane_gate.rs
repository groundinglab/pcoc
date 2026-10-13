//! Test-plane gate. Synth headers advance Bari through `on_tip`.
//! Bitcoin Core `best_tip` is not the narrator. This does not plant on mainnet.
//!
//! A full eighteen-stage produce is an operator run outside this test.
//! This test is the runnable notary
//! slice: one waypoint, path 200, test index, test policy. A skip sits beside it.
//!
//! `SynthCommit` is the test plant. It simulates the commit. No Bitcoin Core
//! node is opened. The counting backend below is only the policy fixture.

use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use hike::{live_plane_host, parse_index_document, serve_synth, GossipNetwork, HeaderClock, TipTick};
use hike_net::{judge_test_policy, GateJudgment, GateSkip, TestGateMeasurement};
use pcoc_bari::clock::{
    BariClock, ChainBackend, ChainError, ChainPostCheck, ChainTip, PlantError, PlantReceipt,
    SkipReason,
};
use pcoc_bari::hashcash::{self, HASHCASH_NONCE_HEADER};
use pcoc_bari::http::BariService;
use pcoc_bari::proof::CanonicalPathBody;
use pcoc_bari::storage::{FileStore, StoredInclusion};
use pcoc_bari::synth_commit::{SynthCommit, SYNTH_INDEX_NETWORK};
use pcoc_bari::Hash32;
use tower::ServiceExt;

/// Low difficulty for this in-process gate. Production `BariService::new` stays at 21.
const BITS: u32 = 8;

struct CountingChain {
    included: Mutex<bool>,
    plants: AtomicUsize,
    best_tip_calls: AtomicUsize,
}

impl CountingChain {
    fn new() -> Self {
        Self {
            included: Mutex::new(false),
            plants: AtomicUsize::new(0),
            best_tip_calls: AtomicUsize::new(0),
        }
    }
}

impl ChainBackend for CountingChain {
    fn network(&self) -> Result<String, ChainError> {
        Ok("regtest".to_owned())
    }

    fn initial_block_download(&self) -> Result<bool, ChainError> {
        Ok(false)
    }

    fn best_tip(&self) -> Result<ChainTip, ChainError> {
        self.best_tip_calls.fetch_add(1, Ordering::SeqCst);
        Err(ChainError(
            "test plane must not narrate the tip from best_tip".to_owned(),
        ))
    }

    fn plant(&self, _root: &Hash32) -> Result<PlantReceipt, PlantError> {
        let n = self.plants.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(PlantReceipt {
            txid: [0xA0 + n as u8; 32],
            vout: n as u32,
            fee_rate_sat_vb: 1.0,
            fee_source: "test-plane".to_owned(),
            fee_cap_sat_vb: None,
            fee_sat: None,
            vsize: None,
        })
    }

    fn included_in_tip(
        &self,
        _txid: &Hash32,
        tip: &ChainTip,
    ) -> Result<Option<StoredInclusion>, ChainError> {
        Ok(self.included.lock().unwrap().then_some(StoredInclusion {
            block_hash: tip.hash,
            block_height: tip.height,
        }))
    }

    fn block_is_selected(&self, _block_hash: &Hash32) -> Result<bool, ChainError> {
        Ok(true)
    }

    fn post_check(&self, body: &CanonicalPathBody) -> Result<ChainPostCheck, ChainError> {
        Ok(ChainPostCheck {
            transaction_in_block: true,
            output_exists: true,
            zero_value: true,
            exact_script: true,
            selected_chain: true,
            carried_height: body.block_height,
            authenticated_height: Some(body.block_height),
            height_matches: Some(true),
        })
    }
}

fn chain_tip(tick: &TipTick) -> ChainTip {
    ChainTip::from_gossip_tick(tick).expect("synth height")
}

fn spawn_synth() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let peer = format!("127.0.0.1:{port}");
    let bind = peer.clone();
    thread::spawn(move || {
        let _ = serve_synth(&bind, Duration::from_millis(80));
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if TcpStream::connect(&peer).is_ok() {
            break;
        }
        if Instant::now() > deadline {
            panic!("synth did not accept on {peer}");
        }
        thread::sleep(Duration::from_millis(20));
    }
    peer
}

async fn next_tips(headers: HeaderClock, after: Option<[u8; 32]>) -> (HeaderClock, Vec<TipTick>) {
    tokio::task::spawn_blocking(move || {
        headers
            .wait_tips_after(after)
            .map(|ticks| (headers, ticks))
            .expect("synth sequence")
    })
    .await
    .expect("wait thread")
}

fn paid(tip: [u8; 32], leaf: [u8; 32]) -> Request<Body> {
    let nonce = hashcash::solve(&hashcash::challenge_from_tip(&tip), &leaf, BITS);
    Request::builder()
        .method("POST")
        .uri("/waypoint")
        .header("content-type", "application/octet-stream")
        .header(HASHCASH_NONCE_HEADER, nonce.to_string())
        .body(Body::from(leaf.to_vec()))
        .unwrap()
}

async fn status_of(app: axum::Router, request: Request<Body>) -> StatusCode {
    app.oneshot(request).await.unwrap().status()
}

fn index_matches(service: &BariService) -> bool {
    let snapshot = service.index_snapshot();
    let stored = snapshot.to_stored();
    let body = serde_json::to_string(&stored).unwrap();
    let (natural, height) = parse_index_document(&body).expect("test index body");
    natural == snapshot.header.hash && height == u64::from(snapshot.header.height)
}

struct Opened {
    service: BariService,
    chain: std::sync::Arc<CountingChain>,
    headers: Option<HeaderClock>,
    tip_hash: [u8; 32],
    _temp: tempfile::TempDir,
}

async fn open_on_synth() -> Opened {
    let peer = spawn_synth();
    let headers = HeaderClock::start(&peer, GossipNetwork::Regtest);
    let (headers, ticks) = next_tips(headers, None).await;
    let tick = ticks.first().expect("open tip");
    let tip_hash = tick.hash;
    let chain = std::sync::Arc::new(CountingChain::new());
    let temp = tempfile::tempdir().unwrap();
    let clock = BariClock::open_at_supplied_tip(
        chain.clone(),
        FileStore::open(temp.path()).unwrap(),
        chain_tip(tick),
        "regtest",
        None,
    )
    .unwrap();
    Opened {
        service: BariService::with_hashcash_bits(clock, BITS),
        chain,
        headers: Some(headers),
        tip_hash,
        _temp: temp,
    }
}

async fn take_tips(opened: &mut Opened, after: Option<[u8; 32]>) -> Vec<TipTick> {
    let headers = opened.headers.take().expect("synth clock");
    let (headers, ticks) = next_tips(headers, after).await;
    opened.headers = Some(headers);
    ticks
}

#[tokio::test]
async fn one_object_on_the_synth_tip_passes_the_test_gate() {
    let mut opened = open_on_synth().await;
    let leaf = [0x71; 32];
    let posted = status_of(opened.service.router(), paid(opened.tip_hash, leaf)).await;
    assert_eq!(posted, StatusCode::NO_CONTENT);

    *opened.chain.included.lock().unwrap() = true;
    let mut after = Some(opened.tip_hash);
    let mut path = StatusCode::NOT_FOUND;
    let deadline = Instant::now() + Duration::from_secs(8);
    while path != StatusCode::OK {
        if Instant::now() > deadline {
            panic!("path did not return 200 inside the serve window");
        }
        let ticks = take_tips(&mut opened, after).await;
        for tick in ticks {
            opened.service.on_tip(chain_tip(&tick)).unwrap();
            after = Some(tick.hash);
            path = status_of(
                opened.service.router(),
                Request::builder()
                    .uri(format!("/path/{}", pcoc_bari::encode_hash32(&leaf)))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
            if path == StatusCode::OK {
                break;
            }
        }
    }

    assert_eq!(opened.chain.plants.load(Ordering::SeqCst), 1);
    assert_eq!(opened.chain.best_tip_calls.load(Ordering::SeqCst), 0);
    assert!(opened.service.index_snapshot().plant.is_some());
    let measurement = TestGateMeasurement {
        planted: true,
        path_status: 200,
        index_is_live: live_plane_host("http://127.0.0.1:38081"),
        index_matches_tip: index_matches(&opened.service),
    };
    assert_eq!(judge_test_policy(&measurement), GateJudgment::Pass);
    assert_eq!(opened.service.index_snapshot().network, "regtest");
}

#[tokio::test]
async fn synth_commit_serves_the_path_without_bitcoin_core() {
    let peer = spawn_synth();
    let headers = HeaderClock::start(&peer, GossipNetwork::Regtest);
    let (headers, ticks) = next_tips(headers, None).await;
    let opened = ticks.first().expect("open tip");
    let commit = std::sync::Arc::new(SynthCommit::new());
    let temp = tempfile::tempdir().unwrap();
    let clock = BariClock::open_at_supplied_tip(
        commit.clone(),
        FileStore::open(temp.path()).unwrap(),
        chain_tip(opened),
        SYNTH_INDEX_NETWORK,
        None,
    )
    .unwrap();
    let service = BariService::with_hashcash_bits(clock, BITS);
    let leaf = [0x74; 32];
    assert_eq!(
        status_of(service.router(), paid(opened.hash, leaf)).await,
        StatusCode::NO_CONTENT
    );

    let mut headers = Some(headers);
    let mut after = Some(opened.hash);
    let mut saw_pending = false;
    let mut saw_serve = false;
    let mut serve_tips = 0u32;
    let mut stayed = false;
    let deadline = Instant::now() + Duration::from_secs(8);
    while !stayed {
        if Instant::now() > deadline {
            panic!("synth commit did not plant and keep the confirmed path servable");
        }
        let clock = headers.take().expect("synth clock");
        let (clock, ticks) = next_tips(clock, after).await;
        headers = Some(clock);
        for tick in ticks {
            service.on_tip(chain_tip(&tick)).unwrap();
            after = Some(tick.hash);
            let path = status_of(
                service.router(),
                Request::builder()
                    .uri(format!("/path/{}", pcoc_bari::encode_hash32(&leaf)))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
            if !saw_pending {
                assert_eq!(path, StatusCode::NOT_FOUND);
                assert_eq!(commit.plant_count(), 1);
                assert!(service.index_snapshot().plant.is_none());
                assert_eq!(commit.best_tip_calls(), 0);
                saw_pending = true;
                continue;
            }
            if path == StatusCode::OK {
                if !saw_serve {
                    let snapshot = service.index_snapshot();
                    assert_eq!(snapshot.network, SYNTH_INDEX_NETWORK);
                    let plant = snapshot.plant.expect("inclusion tip publishes the anchor");
                    assert_eq!(plant.block_hash, tick.hash);
                    assert_eq!(plant.block_height, chain_tip(&tick).height);
                    let stored = service
                        .index_at_height(plant.block_height)
                        .unwrap()
                        .expect("verifier can read the inclusion height");
                    assert_eq!(stored.network, SYNTH_INDEX_NETWORK);
                    let stored_plant = stored.plant.expect("height file carries the anchor");
                    assert_eq!(stored_plant.block_height, plant.block_height);
                    assert_eq!(
                        stored_plant.block_hash,
                        pcoc_bari::encode_bitcoin_id(&tick.hash)
                    );
                    assert_eq!(commit.plant_count(), 1);
                    assert_eq!(commit.best_tip_calls(), 0);
                    saw_serve = true;
                }
                serve_tips += 1;
                if serve_tips > pcoc_bari::clock::DEFAULT_CONFIRMATION_WINDOW_TIPS {
                    stayed = true;
                }
                continue;
            }
            panic!("confirmed path left serving with status {path}");
        }
    }
    assert!(serve_tips > pcoc_bari::clock::DEFAULT_CONFIRMATION_WINDOW_TIPS);
    assert_eq!(commit.plant_count(), 1);
    assert_eq!(commit.best_tip_calls(), 0);
}

#[tokio::test]
async fn skip_is_recorded_beside_the_gate() {
    let mut opened = open_on_synth().await;
    let leaf = [0x72; 32];
    assert_eq!(
        status_of(opened.service.router(), paid(opened.tip_hash, leaf)).await,
        StatusCode::NO_CONTENT
    );

    let mut after = Some(opened.tip_hash);
    let mut skip: Option<GateSkip> = None;
    let deadline = Instant::now() + Duration::from_secs(8);
    while skip.is_none() {
        if Instant::now() > deadline {
            panic!("inclusion tip did not record a skip");
        }
        for tick in take_tips(&mut opened, after).await {
            if let Some(report) = opened.service.on_tip(chain_tip(&tick)).unwrap() {
                if let Some((_, reason)) = report.missed {
                    assert_eq!(reason, SkipReason::NotMined);
                    skip = Some(GateSkip {
                        reason: reason.as_str().to_owned(),
                    });
                }
            }
            after = Some(tick.hash);
        }
    }

    let path = status_of(
        opened.service.router(),
        Request::builder()
            .uri(format!("/path/{}", pcoc_bari::encode_hash32(&leaf)))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(path, StatusCode::NOT_FOUND);
    assert!(opened.service.index_snapshot().plant.is_none());
    assert_eq!(skip.unwrap().reason, "not-mined");
    let judgment = judge_test_policy(&TestGateMeasurement {
        planted: opened.chain.plants.load(Ordering::SeqCst) == 1,
        path_status: 404,
        index_is_live: false,
        index_matches_tip: index_matches(&opened.service),
    });
    assert_eq!(judgment, GateJudgment::Fail);
    assert_eq!(opened.chain.best_tip_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn hashcash_for_a_different_tip_is_400_and_not_the_gate() {
    let opened = open_on_synth().await;
    let leaf = [0x73; 32];
    let other = [0xEE; 32];
    assert_ne!(other, opened.tip_hash);
    let status = status_of(opened.service.router(), paid(other, leaf)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(opened.chain.plants.load(Ordering::SeqCst), 0);
    assert_eq!(opened.chain.best_tip_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        judge_test_policy(&TestGateMeasurement {
            planted: false,
            path_status: 400,
            index_is_live: false,
            index_matches_tip: false,
        }),
        GateJudgment::Fail
    );
}
