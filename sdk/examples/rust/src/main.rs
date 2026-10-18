//! Verify the kat-small conformance object against its recorded index replies.
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn main() {
    let pack = std::env::args()
        .nth(1)
        .expect("usage: pcoc-quickstart <spec/conformance/v2.0>");
    let kat: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{pack}/kat-small.json")).unwrap())
            .unwrap();
    let hex = kat["object_hex"].as_str().unwrap();
    let object: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();

    // The index: height -> document body. A real verifier fetches GET <index>/index/<height>.
    let docs: HashMap<u64, String> = kat["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["height"].as_u64().unwrap(),
                d["body"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let beacon_at = |height: u64| -> Result<hike::IndexBeacon, ()> {
        hike::parse_index_beacon(docs.get(&height).ok_or(())?).map_err(|_| ())
    };

    let measurements = hike::verify_sized(
        &object,
        hike::AuditDraw::PinnedSeed([0x11; 32]), // use AuditDraw::OsCsprng in production
        beacon_at,
        // Plant check through the index (what the CLI does without --chain-rpc).
        |loc: &hike::PathLocators| match docs.get(&u64::from(loc.block_height)) {
            Some(doc) => hike::proxy_matches(doc, loc).into(),
            None => hike::ChainRead::NotRead,
        },
        hike::Sizes::conformance_small(), // Sizes::production() for real objects
    );
    let record = hike::verify_record_json("conformance-small", &measurements);
    println!("shape ok: {}", measurements.shape);
    println!("record sha256: {:x}", Sha256::digest(record.as_bytes()));
}
