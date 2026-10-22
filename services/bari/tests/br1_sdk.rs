//! Every Rule BR1 vector, through Bari's HTTP decision and the Rust SDK.
//!
//! Accept is the 204 path (the waypoint is held). Refuse is 402. The vectors
//! are `sdk/rust/candidates/br1.txt`, the file the conformance gate checks.
//! The service is constructed at 21 bits, so the decision is `pcoc::br1_status`.

#![forbid(unsafe_code)]

use axum::body::Body;
use axum::http::Request;
use pcoc::br1::{br1_status, br1_window, Br1Plane, Br1Tip, Br1Window};
use pcoc::hexutil::hex_decode_exact;
use pcoc_bari::clock::{
    BariClock, ChainBackend, ChainError, ChainPostCheck, ChainTip, PlantReceipt,
};
use pcoc_bari::http::BariService;
use pcoc_bari::proof::CanonicalPathBody;
use pcoc_bari::storage::{FileStore, StoredInclusion};
use pcoc_bari::synth_commit::SYNTH_INDEX_NETWORK;
use pcoc_bari::Hash32;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

fn fields() -> BTreeMap<String, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/candidates/br1.txt");
    let text = std::fs::read_to_string(&path).expect("br1.txt");
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('\t').expect("tab");
        out.insert(key.to_owned(), value.to_owned());
    }
    out
}

fn ids(map: &BTreeMap<String, String>) -> Vec<String> {
    let mut ids = Vec::new();
    for key in map.keys() {
        if let Some(id) = key
            .strip_prefix("br1.")
            .and_then(|rest| rest.strip_suffix(".status"))
        {
            ids.push(id.to_owned());
        }
    }
    ids
}

fn tip(map: &BTreeMap<String, String>, id: &str) -> Br1Tip {
    let hash: [u8; 32] = hex_decode_exact(map[&format!("br1.{id}.tip_hash")].as_str()).unwrap();
    let previous: [u8; 32] =
        hex_decode_exact(map[&format!("br1.{id}.tip_previous")].as_str()).unwrap();
    Br1Tip {
        hash,
        previous,
        height: map[&format!("br1.{id}.tip_height")].parse().unwrap(),
        time: map[&format!("br1.{id}.tip_time")].parse().unwrap(),
        bits: u32::from_str_radix(&map[&format!("br1.{id}.tip_bits")], 16).unwrap(),
        period_first_time: map[&format!("br1.{id}.period_first_time")].parse().unwrap(),
    }
}

fn plane(name: &str) -> Br1Plane {
    match name {
        "test" => Br1Plane::Test,
        "live" => Br1Plane::Live,
        other => panic!("plane {other}"),
    }
}

fn window_name(window: Br1Window) -> &'static str {
    match window {
        Br1Window::Current => "current",
        Br1Window::Before => "before",
        Br1Window::After => "after",
        Br1Window::Outside => "outside",
    }
}

struct FixedChain {
    network: String,
}

impl ChainBackend for FixedChain {
    fn network(&self) -> Result<String, ChainError> {
        Ok(self.network.clone())
    }

    fn initial_block_download(&self) -> Result<bool, ChainError> {
        Ok(false)
    }

    fn best_tip(&self) -> Result<ChainTip, ChainError> {
        Err(ChainError(
            "this test supplies the tip and does not poll".to_owned(),
        ))
    }

    fn plant(&self, _root: &Hash32) -> Result<PlantReceipt, pcoc_bari::clock::PlantError> {
        Ok(PlantReceipt {
            txid: [0xA1; 32],
            vout: 0,
            fee_rate_sat_vb: 0.0,
            fee_source: "br1-vector".to_owned(),
            fee_cap_sat_vb: None,
            fee_sat: None,
            vsize: None,
        })
    }

    fn included_in_tip(
        &self,
        _txid: &Hash32,
        _tip: &ChainTip,
    ) -> Result<Option<StoredInclusion>, ChainError> {
        Ok(None)
    }

    fn block_is_selected(&self, _block_hash: &Hash32) -> Result<bool, ChainError> {
        Ok(true)
    }

    fn post_check(&self, body: &CanonicalPathBody) -> Result<ChainPostCheck, ChainError> {
        Ok(ChainPostCheck {
            transaction_in_block: false,
            output_exists: false,
            zero_value: false,
            exact_script: false,
            selected_chain: false,
            carried_height: body.block_height,
            authenticated_height: None,
            height_matches: None,
        })
    }
}

fn chain_tip(tip: &Br1Tip) -> ChainTip {
    ChainTip {
        hash: tip.hash,
        previous_hash: Some(tip.previous),
        height: tip.height,
        time: Some(u64::from(tip.time)),
        bits: tip.bits,
        period_first_time: tip.period_first_time,
    }
}

fn network_name(plane: Br1Plane) -> &'static str {
    match plane {
        Br1Plane::Test => SYNTH_INDEX_NETWORK,
        Br1Plane::Live => "main",
    }
}

fn open_service(tip: &Br1Tip, plane: Br1Plane, origin: &str) -> (BariService, tempfile::TempDir) {
    let temp = tempfile::tempdir().expect("temp dir");
    let chain = Arc::new(FixedChain {
        network: network_name(plane).to_owned(),
    });
    let clock = BariClock::open_at_supplied_tip(
        chain,
        FileStore::open(temp.path()).unwrap(),
        chain_tip(tip),
        network_name(plane),
        None,
    )
    .expect("open");
    let service = BariService::new(clock, origin).expect("origin");
    (service, temp)
}

#[tokio::test]
async fn every_br1_vector_matches_the_sdk_on_the_http_decision() {
    let map = fields();
    let ids = ids(&map);
    assert!(
        ids.iter().any(|id| id == "after_live"),
        "live-plane after vector is missing"
    );
    assert!(ids.len() >= 12, "br1.txt has {} vectors", ids.len());
    for id in &ids {
        let header_hex = map[&format!("br1.{id}.fee_header")].clone();
        let header: [u8; 80] = hex_decode_exact(header_hex.as_str()).unwrap();
        let waypoint: [u8; 32] =
            hex_decode_exact(map[&format!("br1.{id}.waypoint")].as_str()).unwrap();
        let origin = map[&format!("br1.{id}.origin")].clone();
        let nonce = map[&format!("br1.{id}.nonce")].clone();
        let tip = tip(&map, id);
        let plane = plane(map[&format!("br1.{id}.plane")].as_str());
        let sdk = br1_status(&waypoint, &nonce, &header_hex, &origin, &tip, plane);
        assert_eq!(
            sdk.to_string(),
            map[&format!("br1.{id}.status")],
            "{id} sdk"
        );
        assert_eq!(
            window_name(br1_window(&header, &tip, plane)),
            map[&format!("br1.{id}.window")],
            "{id} window"
        );

        let (service, _temp) = open_service(&tip, plane, &origin);
        let request = Request::builder()
            .method("POST")
            .uri("/waypoint")
            .header("content-type", "application/octet-stream")
            .header("x-bari-hashcash-nonce", &nonce)
            .header("x-bari-hashcash-header", &header_hex)
            .body(Body::from(waypoint.to_vec()))
            .unwrap();
        let response = service.router().oneshot(request).await.unwrap();
        assert_eq!(response.status().as_u16(), sdk, "{id} bari http");
        if sdk == 204 {
            assert_eq!(service.pool_len(), 1, "{id} held");
        } else {
            assert_eq!(sdk, 402, "{id}");
            assert_eq!(service.pool_len(), 0, "{id} not held");
        }
    }
}
