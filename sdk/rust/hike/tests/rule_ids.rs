//! Per-rule checks on the SPEC §16.1 small profile. No production terrain.

use hike::{encode_hike_sized, verify_record_json, verify_sized, AuditDraw, Sizes};
use serde_json::Value;

fn beacon(tag: u8) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[0] = tag;
    out
}

fn record_of(stages: Vec<hike::Stage>, last_height: u64) -> Value {
    let sizes = Sizes::conformance_small();
    let object = hike::Hike {
        public_key: [0x42; 32],
        seed_beacon: stages[0].beacon,
        last_interrupt: beacon(0x63),
        last_interrupt_height: last_height,
        last_interrupt_absent: false,
        last_interrupt_hash_absent: false,
        last_interrupt_height_absent: false,
        stages,
    };
    let bytes = encode_hike_sized(&object, sizes);
    let measurements = verify_sized(
        &bytes,
        AuditDraw::PinnedSeed([0xA5; 32]),
        |_| Err(()),
        |_| Ok(()),
        sizes,
    );
    serde_json::from_str(verify_record_json("conformance-small", &measurements).trim_end()).unwrap()
}

fn stage(tag: u8, height: u64, waypoint: [u8; 32], checkpoints: Vec<[u8; 32]>, legs: Vec<[u8; 8]>) -> hike::Stage {
    hike::Stage {
        beacon: beacon(tag),
        beacon_height: height,
        leg_count: checkpoints.len() as u64,
        waypoint,
        path: Vec::new(),
        checkpoints,
        leg_results: legs,
        checkpoint_rows_defined: true,
        leg_result_missing: Vec::new(),
    }
}

/// Rule A4.1. Fewer than 18 stages names the absent stage-17 conditions.
/// Section 14.1: a condition that tests a field's value is violated when that
/// field is absent, so the missing stages' `leg_count` is in `shape_item`,
/// in the section 14 table order.
#[test]
fn a4_1_stages_length_on_a_short_object() {
    let seed = hike::production_seed(&[0x42; 32], &beacon(1));
    let seed_value = {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(seed);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    };
    let record = record_of(
        vec![
            stage(1, 10, seed_value, vec![[1; 32]], vec![1u64.to_le_bytes()]),
            stage(2, 20, [2; 32], vec![[2; 32]], vec![2u64.to_le_bytes()]),
        ],
        30,
    );
    assert_eq!(record["shape"], "mismatch");
    assert_eq!(
        record["shape_item"],
        serde_json::json!([
            "W_17 path",
            "stages length",
            "leg_count",
            "last_interrupt",
            "header height order",
            "header hash differ"
        ])
    );
    assert_eq!(record["paths"][0]["path_state"], "skip");
    assert_eq!(record["paths"][1]["path_state"], "not defined");
    assert_eq!(record["paths"][16]["w17_path"], "not defined");
}

/// Rule A4.1. A stored height above 2^53−1 is `height_range` on a short object.
#[test]
fn a4_1_height_range_on_a_short_object() {
    let record = record_of(
        vec![stage(1, 1 << 53, [3; 32], vec![[1; 32]], vec![1u64.to_le_bytes()])],
        30,
    );
    let items = record["shape_item"].as_array().unwrap();
    assert!(items.iter().any(|item| item == "height_range"));
    assert_eq!(record["headers"][0]["stored_height"], "not defined");
}

/// Rule A4.3. A stored checkpoint that is not the recomputed digest is `mismatch`.
#[test]
fn a4_3_checkpoint_digest_mismatch_on_a_small_leg() {
    let sizes = Sizes::conformance_small();
    let public_key = [0x42u8; 32];
    let seed_beacon = beacon(1);
    let seed = hike::production_seed(&public_key, &seed_beacon);
    let seed_value = {
        use sha2::{Digest, Sha256};
        let mut out = [0u8; 32];
        out.copy_from_slice(&Sha256::digest(seed));
        out
    };
    let terrain = hike::fill_terrain_sized(hike::fill_pin(&seed), sizes);
    let leg = hike::leg_result_sized(&terrain, &seed_value, sizes);
    let sample = hike::terrain_sample_sized(&terrain, &seed_value, sizes);
    let digest = hike::checkpoint_sized(&seed_value, &seed_value, &sample, leg, sizes);
    let matched = record_of(
        vec![stage(1, 10, seed_value, vec![digest], vec![leg.to_le_bytes()])],
        20,
    );
    assert_eq!(matched["checkpoints"][0]["legs"][0]["result"], "match");
    assert_eq!(matched["seed_value"], "match");
    let mut flipped = digest;
    flipped[0] ^= 0xff;
    let missed = record_of(
        vec![stage(1, 10, seed_value, vec![flipped], vec![leg.to_le_bytes()])],
        20,
    );
    assert_eq!(missed["checkpoints"][0]["legs"][0]["result"], "mismatch");
}

/// Section 12. A stored successor that is not the rehash is `mismatch`.
#[test]
fn a4_3_waypoint_rehash_mismatch_on_a_small_leg() {
    let sizes = Sizes::conformance_small();
    let public_key = [0x42u8; 32];
    let seed_beacon = beacon(1);
    let seed = hike::production_seed(&public_key, &seed_beacon);
    let seed_value = {
        use sha2::{Digest, Sha256};
        let mut out = [0u8; 32];
        out.copy_from_slice(&Sha256::digest(seed));
        out
    };
    let terrain = hike::fill_terrain_sized(hike::fill_pin(&seed), sizes);
    let leg = hike::leg_result_sized(&terrain, &seed_value, sizes);
    let sample = hike::terrain_sample_sized(&terrain, &seed_value, sizes);
    let digest = hike::checkpoint_sized(&seed_value, &seed_value, &sample, leg, sizes);
    let interrupt = beacon(2);
    let successor = hike::successor_waypoint_sized(&terrain, &interrupt, &seed_value, &[digest], sizes);
    let matched = record_of(
        vec![
            stage(1, 10, seed_value, vec![digest], vec![leg.to_le_bytes()]),
            stage(2, 20, successor, vec![[9; 32]], vec![3u64.to_le_bytes()]),
        ],
        30,
    );
    assert_eq!(matched["waypoints"][0]["result"], "match");
    let missed = record_of(
        vec![
            stage(1, 10, seed_value, vec![digest], vec![leg.to_le_bytes()]),
            stage(2, 20, [0x77; 32], vec![[9; 32]], vec![3u64.to_le_bytes()]),
        ],
        30,
    );
    assert_eq!(missed["waypoints"][0]["result"], "mismatch");
    assert_eq!(missed["seed_value"], "match");
}

/// Rule A4.4. Six stored legs that are not the recomputed leg results are `mismatch`.
#[test]
fn a4_4_six_leg_mismatch_on_a_small_stage() {
    let legs = vec![[0u8; 8]; 6];
    let checkpoints = vec![[0xABu8; 32]; 6];
    let record = record_of(vec![stage(1, 10, [4; 32], checkpoints, legs)], 20);
    let six = &record["six_leg"];
    assert!(six["indices"].as_array().unwrap().len() == 6);
    assert!(six["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|token| token == "mismatch"));
    assert_eq!(record["six_leg_runs"].as_array().unwrap().len(), 1);
}

/// Rule A4.4. Six recomputed legs on the small profile are `match`.
#[test]
fn a4_4_six_leg_match_on_a_small_stage() {
    let sizes = Sizes::conformance_small();
    let public_key = [0x42u8; 32];
    let seed_beacon = beacon(1);
    let seed = hike::production_seed(&public_key, &seed_beacon);
    let seed_value = {
        use sha2::{Digest, Sha256};
        let mut out = [0u8; 32];
        out.copy_from_slice(&Sha256::digest(seed));
        out
    };
    let terrain = hike::fill_terrain_sized(hike::fill_pin(&seed), sizes);
    let mut previous = seed_value;
    let mut checkpoints = Vec::new();
    let mut legs = Vec::new();
    for _ in 0..6 {
        let leg = hike::leg_result_sized(&terrain, &previous, sizes);
        let sample = hike::terrain_sample_sized(&terrain, &previous, sizes);
        let digest = hike::checkpoint_sized(&previous, &seed_value, &sample, leg, sizes);
        checkpoints.push(digest);
        legs.push(leg.to_le_bytes());
        previous = digest;
    }
    let record = record_of(vec![stage(1, 10, seed_value, checkpoints, legs)], 20);
    let results = record["six_leg"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 6);
    assert!(results.iter().all(|token| token == "match"));
}
