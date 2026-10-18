//! Verify the kat-small conformance object and reproduce the vector's stored
//! section 14 record byte for byte.
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn main() {
    let pack = std::env::args()
        .nth(1)
        .expect("usage: pcoc-quickstart <spec/conformance/v2.0>");
    let kat: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{pack}/kat-small.json")).unwrap())
            .unwrap();
    let object: Vec<u8> = unhex(kat["object_hex"].as_str().unwrap());

    // The index: height -> section 9 document body. A real verifier fetches
    // GET <index>/index/<height>; here the vector carries the replies.
    let docs: HashMap<u64, String> = kat["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let height = d["height"].as_u64().unwrap();
            (height, d["body"].as_str().unwrap().to_owned())
        })
        .collect();

    // The six-leg draw. kat-small's draw_bytes_hex is the pinned stream
    // SHA-256(seed || le64(0)) || SHA-256(seed || le64(1)) || ... for this seed,
    // so PinnedSeed(seed) reproduces the vector. Production uses AuditDraw::OsCsprng.
    let seed = [0x5A; 32];
    let stream: Vec<u8> = (0u64..8)
        .flat_map(|i| {
            Sha256::new()
                .chain_update(seed)
                .chain_update(i.to_le_bytes())
                .finalize()
        })
        .collect();
    println!(
        "draw = draw_bytes_hex: {}",
        stream == unhex(kat["draw_bytes_hex"].as_str().unwrap())
    );

    let measurements: hike::Measurements = hike::verify_sized(
        &object,
        hike::AuditDraw::PinnedSeed(seed),
        // beacon_at: the section 9 beacon at a height, Err(()) when there is no document.
        |height: u64| -> Result<hike::IndexBeacon, ()> {
            hike::parse_index_beacon(docs.get(&height).ok_or(())?).map_err(|_| ())
        },
        // path_chain: the plant check through the index (what the program does without --chain-rpc).
        |loc: &hike::PathLocators| match docs.get(&u64::from(loc.block_height)) {
            Some(doc) => hike::proxy_matches(doc, loc).into(),
            None => hike::ChainRead::NotRead,
        },
        hike::Sizes::conformance_small(), // Sizes::production() for real objects
    );
    // The record bytes: compact JSON in section 14 key order plus one newline.
    let record: String = hike::verify_record_json("conformance-small", &measurements);

    println!(
        "shape: {}",
        if measurements.shape {
            "match"
        } else {
            "mismatch"
        }
    );
    println!(
        "matches kat-small.json record: {}",
        record == kat["record"].as_str().unwrap()
    );
    println!(
        "record bytes sha256: {:x}",
        Sha256::digest(record.as_bytes())
    );
    let jcs: [u8; 32] = hike::verify_record_sha256(&record).unwrap();
    println!(
        "verify_record_sha256: {}",
        jcs.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
}
