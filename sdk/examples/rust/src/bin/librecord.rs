//! Library-level record, for sdk/tools/crosscheck.py.
//!
//! Usage: librecord OBJECT DOCS_JSON SEED_HEX
//! DOCS_JSON maps a height (string) to its section 9 index document body.
//! Calls `hike::verify_sized` at conformance-small sizes with
//! `AuditDraw::PinnedSeed(SEED)` and prints `hike::verify_record_json`. The
//! library always returns a record, so this never prints a "no record" line.
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let object = std::fs::read(&args[0]).expect("object file");
    let docs: HashMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(&args[1]).expect("docs file"))
            .expect("docs json");
    let mut seed = [0u8; 32];
    for (i, byte) in seed.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&args[2][2 * i..2 * i + 2], 16).expect("seed hex");
    }
    let doc = |height: u64| docs.get(&height.to_string());
    let measurements = hike::verify_sized(
        &object,
        hike::AuditDraw::PinnedSeed(seed),
        |height| hike::parse_index_beacon(doc(height).ok_or(())?).map_err(|_| ()),
        |loc: &hike::PathLocators| match doc(u64::from(loc.block_height)) {
            Some(body) => hike::proxy_matches(body, loc).into(),
            None => hike::ChainRead::NotRead,
        },
        hike::Sizes::conformance_small(),
    );
    print!(
        "{}",
        hike::verify_record_json("conformance-small", &measurements)
    );
}
