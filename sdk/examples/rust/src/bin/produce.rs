//! Produce a hike object offline from the stage0-start.json tip script at
//! conformance-small sizes, then verify it.
use sha2::{Digest, Sha256};

fn unhex32(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

fn main() {
    let pack = std::env::args()
        .nth(1)
        .expect("usage: produce <spec/conformance/v2.0>");
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{pack}/stage0-start.json")).unwrap(),
    )
    .unwrap();
    let public_key = unhex32(v["public_key_hex"].as_str().unwrap());
    // The tip sequence: (natural beacon, height). Live produce reads these from
    // Bitcoin header gossip; each new header ends a stage.
    let tips: Vec<([u8; 32], u64)> = v["tips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                unhex32(t["natural_hex"].as_str().unwrap()),
                t["height"].as_u64().unwrap(),
            )
        })
        .collect();
    let mut next = tips.into_iter();

    let object: Vec<u8> = hike::produce_with_settlement_sized(
        public_key,
        hike::Sizes::conformance_small(), // Sizes::production() for a real PCOC
        // observe: the current tip. Called after every checkpoint.
        || {
            next.next()
                .ok_or_else(|| hike::ProduceFailure::new("tip script ended"))
        },
        // open_stage: post the waypoint (the plant) and return its path body.
        // Offline there is no plant service, so every path is empty bytes.
        |_waypoint: &[u8; 32], _beacon_height: u64| Ok(Vec::new()),
        // settle: may replace stage paths after the last leg. Nothing to do here.
        |_stages: &mut [hike::Stage], _cut_off: u64| Ok(()),
    )
    .expect("produce");
    println!("object bytes: {}", object.len());
    println!("object sha256: {:x}", Sha256::digest(&object));

    // Verify what was produced. There is no index here, so header reads are "not read".
    let m = hike::verify_sized(
        &object,
        hike::AuditDraw::PinnedSeed([0x11; 32]),
        |_height| Err(()),
        |_loc: &hike::PathLocators| hike::ChainRead::NotRead,
        hike::Sizes::conformance_small(),
    );
    println!("shape: {}", if m.shape { "match" } else { "mismatch" });
}
