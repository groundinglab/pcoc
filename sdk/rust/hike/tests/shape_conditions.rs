//! Section 14.1: every violated shape condition is listed, and later
//! measurements are still recorded.
//!
//! Each case edits a few bytes of the kat-small object so it stays one CBOR
//! map, with one field absent, mistyped or out of order. Go and Java record
//! these the same way (sdk/tools/crosscheck.py compares the bytes); these
//! tests pin the section 14.1 facts in Rust.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn kat() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/conformance/v2.0/kat-small.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn object() -> Vec<u8> {
    hex(kat()["object_hex"].as_str().unwrap())
}

/// Offset of the `k`-th occurrence of `needle`.
fn nth(data: &[u8], needle: &[u8], k: usize) -> usize {
    data.windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .nth(k)
        .map(|(at, _)| at)
        .expect("needle in kat-small")
}

/// Rename the `k`-th occurrence of a key (last letter to `X`): that key is absent.
fn rename(obj: &[u8], needle: &[u8], k: usize) -> Vec<u8> {
    let mut m = obj.to_vec();
    m[nth(obj, needle, k) + needle.len() - 1] = b'X';
    m
}

fn beacon_at(obj: &[u8], k: usize) -> (usize, Vec<u8>) {
    let at = nth(obj, b"\x66beacon\x58\x20", k) + 9;
    (at, obj[at..at + 32].to_vec())
}

fn record(bytes: &[u8]) -> Value {
    let docs: HashMap<u64, String> = kat()["documents"]
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
    let measurements = hike::verify_sized(
        bytes,
        hike::AuditDraw::PinnedSeed([0x11; 32]),
        |height| hike::parse_index_beacon(docs.get(&height).ok_or(())?).map_err(|_| ()),
        |loc: &hike::PathLocators| match docs.get(&u64::from(loc.block_height)) {
            Some(doc) => hike::proxy_matches(doc, loc).into(),
            None => hike::ChainRead::NotRead,
        },
        hike::Sizes::conformance_small(),
    );
    let line = hike::verify_record_json("conformance-small", &measurements);
    serde_json::from_str(line.trim_end()).unwrap()
}

fn items(rec: &Value) -> Vec<&str> {
    rec["shape_item"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

/// The object was measured past shape: header 0 and the seed were read.
fn measured_on(rec: &Value) {
    assert_eq!(rec["headers"][0]["index_comparison"], "match", "{rec}");
    assert_eq!(rec["seed_value"], "match", "{rec}");
    assert!(rec["six_leg"].is_object(), "{rec}");
}

#[test]
fn absent_fill_is_field_set_and_constant_and_measures_on() {
    let rec = record(&rename(&object(), b"\x64fill", 0));
    assert_eq!(items(&rec), ["field set", "constant"]);
    measured_on(&rec);
}

#[test]
fn mistyped_constant_is_field_set_and_constant() {
    let obj = object();
    let mut m = obj.clone();
    m[nth(&obj, b"\x6aslot_bytes\x18", 0) + 11] = 0x61; // a 1-byte text, not a uint
    let rec = record(&m);
    assert_eq!(items(&rec), ["field set", "constant"]);
    measured_on(&rec);
}

#[test]
fn absent_stage_height_is_header_height_order_and_not_defined() {
    let rec = record(&rename(&object(), b"\x6dbeacon_height", 1));
    assert_eq!(items(&rec), ["field set", "header height order"]);
    assert_eq!(rec["headers"][1]["stored_height"], "not defined");
    assert_eq!(rec["headers"][1]["index_comparison"], "not read");
    assert_eq!(rec["headers"][2]["index_comparison"], "match");
    // No stored height: no window for W_1's path, so its plant is not read.
    assert_eq!(rec["paths"][0]["plant"], "not read");
    measured_on(&rec);
}

#[test]
fn absent_stage0_beacon_is_seed_beacon_and_header_hash_differ() {
    let rec = record(&rename(&object(), b"\x66beacon", 0));
    assert_eq!(
        items(&rec),
        ["field set", "seed beacon", "header hash differ"]
    );
    assert_eq!(rec["headers"][0]["stored_hash"], "not defined");
    assert_eq!(rec["headers"][0]["index_comparison"], "not read");
    assert_eq!(rec["headers"][1]["index_comparison"], "match");
}

#[test]
fn absent_stage_beacon_leaves_that_waypoint_recompute_not_defined() {
    let rec = record(&rename(&object(), b"\x66beacon", 5));
    assert_eq!(items(&rec), ["field set", "header hash differ"]);
    assert_eq!(rec["headers"][5]["stored_hash"], "not defined");
    // W_5 is recomputed from I_4 = stages[5].beacon.
    assert_eq!(rec["waypoints"][4]["result"], "not defined");
    assert_eq!(rec["waypoints"][5]["result"], "match");
    measured_on(&rec);
}

#[test]
fn absent_leg_count_is_leg_count_and_the_draw_counts_checkpoints() {
    let rec = record(&rename(&object(), b"\x69leg_count", 3));
    assert_eq!(items(&rec), ["field set", "leg_count"]);
    // Section 14.6 indexes legs by checkpoints, not leg_count: same draw as kat-small.
    let base = record(&object());
    assert_eq!(rec["six_leg"], base["six_leg"]);
}

#[test]
fn a_wrong_leg_count_does_not_move_the_draw() {
    let obj = object();
    let mut m = obj.clone();
    m[nth(&obj, b"\x69leg_count\x01", 3) + 10] = 0x02;
    let rec = record(&m);
    assert_eq!(items(&rec), ["leg_count"]);
    assert_eq!(rec["six_leg"], record(&obj)["six_leg"]);
}

#[test]
fn a_low_closing_height_is_last_interrupt_and_header_height_order() {
    let obj = object();
    let mut m = obj.clone();
    m[nth(&obj, b"\x75last_interrupt_height\x19", 0) + 23] ^= 0x01; // 1020 -> 764
    let rec = record(&m);
    assert_eq!(items(&rec), ["last_interrupt", "header height order"]);
    measured_on(&rec);
}

#[test]
fn a_closing_hash_equal_to_stage17_is_last_interrupt_and_header_hash_differ() {
    let obj = object();
    let mut m = obj.clone();
    let at = nth(&obj, b"\x6elast_interrupt\x58\x20", 0) + 17;
    m[at..at + 32].copy_from_slice(&beacon_at(&obj, 17).1);
    let rec = record(&m);
    assert_eq!(items(&rec), ["last_interrupt", "header hash differ"]);
}

#[test]
fn an_order_failure_does_not_hide_a_later_hash_failure() {
    let obj = object();
    let mut m = obj.clone();
    m[nth(&obj, b"\x6dbeacon_height\x19", 4) + 16] -= 1; // stage 4 height = stage 3
    let (at10, _) = beacon_at(&obj, 10);
    m[at10..at10 + 32].copy_from_slice(&beacon_at(&obj, 9).1); // stage 10 hash = stage 9
    let rec = record(&m);
    assert_eq!(items(&rec), ["header height order", "header hash differ"]);
}
