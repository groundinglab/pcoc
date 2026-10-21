//! Byte check against `spec/conformance/v2.1`. The pack is the v2.1 target.

use hike::{
    below_floor, checkpoint_sized, fill_terrain_sized, judge_beacon_rate, leg_result_sized,
    seed, seed_preimage, step_sized, successor_waypoint_sized, BeaconRate, RateJudgment, Sizes,
    MIN_ANCHORED_WAYPOINTS, PLACEHOLDER_FLOOR_NOTE, PLACEHOLDER_FLOOR_P, PLACEHOLDER_FLOOR_Q,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn pack() -> PathBuf {
    let in_tree = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../spec/conformance/v2.1");
    if in_tree.join("seed.json").is_file() {
        return in_tree;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../spec-v2.1/pcoc/spec/conformance/v2.1")
}

fn load(name: &str) -> Value {
    let path = pack().join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn hex32(text: &str) -> [u8; 32] {
    let bytes = hex_bytes(text);
    bytes.try_into().unwrap()
}

fn hex_u64(text: &str) -> u64 {
    u64::from_str_radix(text, 16).unwrap()
}

fn encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sizes_of(slot: usize, terrain: usize) -> Sizes {
    Sizes::conformance_small_checked(32.min(slot), slot, terrain, 1).expect("sizes")
}

#[test]
fn seed_and_nl4_words_match_the_pack() {
    let seed_doc = load("seed.json");
    let pk = hex32(seed_doc["public_key_hex"].as_str().unwrap());
    let beacon = hex32(seed_doc["seed_beacon_hex"].as_str().unwrap());
    assert_eq!(encode(seed_preimage(&pk, &beacon)), seed_doc["seed_preimage_hex"]);
    let digest = seed(&pk, &beacon);
    assert_eq!(encode(digest), seed_doc["seed_hex"]);
    assert_eq!(seed_doc["waypoint_equals_seed"], 1);
    let words = load("nl4-words.json");
    let lanes = hike::lanes_of(&digest);
    for sample in words["samples"].as_array().unwrap() {
        let i = sample["i"].as_u64().unwrap();
        let j = sample["j"].as_u64().unwrap();
        assert_eq!(
            hike::base_word_nl4(&lanes, i, j),
            hex_u64(sample["u64_hex"].as_str().unwrap()),
            "i={i} j={j}"
        );
    }
    let slot = sizes_of(16384, 16384);
    let terrain = fill_terrain_sized(&digest, slot);
    assert_eq!(sha256(&terrain), words["slot0_sha256"].as_str().unwrap());
}

#[test]
fn c1_steps_on_slot0_match_the_pack() {
    let doc = load("step-c1.json");
    let pk = hex32(doc["public_key_hex"].as_str().unwrap());
    let beacon = hex32(doc["seed_beacon_hex"].as_str().unwrap());
    let sizes = sizes_of(16384, 16384);
    let terrain = fill_terrain_sized(&seed(&pk, &beacon), sizes);
    assert_eq!(
        hike::seal(0),
        hex_u64(doc["seal0_s_prime_u64_hex"].as_str().unwrap())
    );
    for step in doc["steps"].as_array().unwrap() {
        let state = hex_u64(step["S_u64_hex"].as_str().unwrap());
        let folded = hex_u64(step["S_folded_u64_hex"].as_str().unwrap());
        let prime_text = step
            .get("E_u64_hex")
            .or_else(|| step.get("S_prime_u64_hex"))
            .and_then(|item| item.as_str())
            .unwrap();
        let prime = hex_u64(prime_text);
        assert_eq!(hike::seal(folded), prime);
        assert_eq!(step_sized(&terrain, state, sizes), prime);
    }
}

#[test]
fn small_terrain_and_hikes_match_the_pack() {
    let terrain = load("terrain-small.json");
    let pk = hex32(terrain["public_key_hex"].as_str().unwrap());
    let beacon = hex32(terrain["seed_beacon_hex"].as_str().unwrap());
    let digest = seed(&pk, &beacon);
    for case in terrain["cases"].as_array().unwrap() {
        let slot = case["slot_bytes"].as_u64().unwrap() as usize;
        let bytes = case["terrain_bytes"].as_u64().unwrap() as usize;
        let filled = fill_terrain_sized(&digest, sizes_of(slot, bytes));
        assert_eq!(sha256(&filled), case["sha256"].as_str().unwrap());
    }
    check_hike(&load("test-3leg.json"));
    check_hike(&load("kat-small.json"));
}

fn check_hike(doc: &Value) {
    let profile = &doc["profile"];
    let sizes = Sizes::conformance_small_checked(
        profile["sample_bytes"].as_u64().unwrap() as usize,
        profile["slot_bytes"].as_u64().unwrap() as usize,
        profile["terrain_bytes"].as_u64().unwrap() as usize,
        profile["steps_per_leg"].as_u64().unwrap(),
    )
    .unwrap();
    let pk = hex32(doc["public_key_hex"].as_str().unwrap());
    let beacon = hex32(doc["seed_beacon_hex"].as_str().unwrap());
    let digest = seed(&pk, &beacon);
    assert_eq!(encode(digest), doc["seed_hex"].as_str().unwrap());
    assert_eq!(doc["waypoint_equals_seed"], 1);
    let terrain = fill_terrain_sized(&digest, sizes);
    assert_eq!(sha256(&terrain), doc["terrain_sha256"].as_str().unwrap());
    let mut limb = digest;
    for (index, stage) in doc["stages"].as_array().unwrap().iter().enumerate() {
        let mut previous = limb;
        let mut checkpoints = Vec::new();
        for (j, leg_hex) in stage["legs_le_hex"].as_array().unwrap().iter().enumerate() {
            let leg = leg_result_sized(&terrain, &previous, sizes);
            assert_eq!(encode(leg.to_le_bytes()), leg_hex.as_str().unwrap(), "stage {index} leg {j}");
            let checkpoint = checkpoint_sized(&previous, &limb, &hike::terrain_sample_sized(&terrain, &previous, sizes), leg, sizes);
            assert_eq!(
                encode(checkpoint),
                stage["checkpoints_hex"][j].as_str().unwrap(),
                "stage {index} checkpoint {j}"
            );
            checkpoints.push(checkpoint);
            previous = checkpoint;
        }
        match stage["next_beacon_hex"].as_str() {
            Some(text) => {
                let next = hex32(text);
                let waypoint = successor_waypoint_sized(&terrain, &next, &limb, &checkpoints, sizes);
                assert_eq!(
                    encode(waypoint),
                    stage["next_waypoint_hex"].as_str().unwrap(),
                    "stage {index} waypoint"
                );
                limb = waypoint;
            }
            None => assert!(stage["next_waypoint_hex"].is_null()),
        }
    }
}

#[test]
fn negatives_and_the_rate_floor_match_the_pack() {
    let doc = load("negatives.json");
    let pk = hex32(doc["public_key_hex"].as_str().unwrap());
    let beacon = hex32(doc["seed_beacon_hex"].as_str().unwrap());
    let digest = seed(&pk, &beacon);
    let lanes = hike::lanes_of(&digest);
    for case in doc["cases"].as_array().unwrap() {
        match case["name"].as_str().unwrap() {
            "fill_pin_terrain" => {
                let sizes = sizes_of(
                    case["slot_bytes"].as_u64().unwrap() as usize,
                    case["terrain_bytes"].as_u64().unwrap() as usize,
                );
                let bad = hike::rejected::fill_pin_terrain(&seed_preimage(&pk, &beacon), sizes);
                let good = fill_terrain_sized(&digest, sizes);
                assert_eq!(sha256(&bad), case["forbidden_sha256"].as_str().unwrap());
                assert_eq!(sha256(&good), case["conforming_sha256"].as_str().unwrap());
                assert_ne!(bad, good);
            }
            "ref_word" => {
                let bad = hike::rejected::fill_ref(&digest, sizes_of(64, 64));
                let word = u64::from_le_bytes(bad[..8].try_into().unwrap());
                assert_eq!(format!("{word:016x}"), case["forbidden_u64_hex"].as_str().unwrap());
                assert_eq!(
                    format!("{:016x}", hike::base_word_nl4(&lanes, 0, 0)),
                    case["conforming_u64_hex"].as_str().unwrap()
                );
            }
            "all4_word" => {
                let bad = hike::rejected::fill_all4(&digest, sizes_of(64, 64));
                let word = u64::from_le_bytes(bad[..8].try_into().unwrap());
                assert_eq!(format!("{word:016x}"), case["forbidden_u64_hex"].as_str().unwrap());
            }
            "mix64_without_mix_add"
            | "fatslot_fold"
            | "v2_0_fold"
            | "xor_accumulator"
            | "xor_accumulator_fold"
            | "partial_slot_read"
            | "partial_slot_read_2047" => {
                let sizes = sizes_of(16384, 16384);
                let terrain = fill_terrain_sized(&digest, sizes);
                let state = hex_u64(case["S_u64_hex"].as_str().unwrap());
                let forbidden = hex_u64(case["forbidden_S_folded_u64_hex"].as_str().unwrap());
                let conforming = hex_u64(case["conforming_S_folded_u64_hex"].as_str().unwrap());
                assert_ne!(forbidden, conforming);
                assert_eq!(step_sized(&terrain, state, sizes), hike::seal(conforming));
                let name = case["name"].as_str().unwrap();
                let rejected = match name {
                    "mix64_without_mix_add" => {
                        hike::rejected::step_bare_finalizer(&terrain, state, sizes)
                    }
                    "fatslot_fold" | "v2_0_fold" => {
                        hike::rejected::step_v2_0_fold(&terrain, state, sizes)
                    }
                    "xor_accumulator" | "xor_accumulator_fold" => {
                        hike::rejected::step_xor_acc(&terrain, state, sizes)
                    }
                    "partial_slot_read" | "partial_slot_read_2047" => {
                        let keep = case["words_read"].as_u64().unwrap() as usize;
                        hike::rejected::step_partial_words(&terrain, state, sizes, keep)
                    }
                    _ => unreachable!(),
                };
                assert_eq!(rejected, hike::seal(forbidden), "{name}");
                assert_ne!(rejected, step_sized(&terrain, state, sizes));
            }
            "one_lane_flip" => {
                let sizes = sizes_of(
                    case["slot_bytes"].as_u64().unwrap() as usize,
                    case["terrain_bytes"].as_u64().unwrap() as usize,
                );
                let good = fill_terrain_sized(&digest, sizes);
                assert_eq!(sha256(&good), case["conforming_sha256"].as_str().unwrap());
                let mut flipped = digest;
                let lane = case["lane"].as_u64().unwrap() as usize;
                let flip = hex_u64(case["flip_u64_hex"].as_str().unwrap()).to_le_bytes();
                for (byte, mask) in flipped[lane * 8..lane * 8 + 8].iter_mut().zip(flip) {
                    *byte ^= mask;
                }
                let bad = fill_terrain_sized(&flipped, sizes);
                assert_eq!(sha256(&bad), case["forbidden_sha256"].as_str().unwrap());
                assert_ne!(bad, good);
            }
            "forged_window" => {
                let slot = case["slot_bytes"].as_u64().unwrap() as usize;
                let bytes = case["terrain_bytes"].as_u64().unwrap() as usize;
                let sizes = hike::Sizes::conformance_small_checked(
                    case["sample_bytes"].as_u64().unwrap() as usize,
                    slot,
                    bytes,
                    1,
                )
                .unwrap();
                let terrain = fill_terrain_sized(&digest, sizes);
                let mut previous = digest;
                previous[..8].copy_from_slice(&hex_u64(case["leg_u64_hex"].as_str().unwrap()).to_le_bytes());
                let sample = hike::terrain_sample_sized(&terrain, &previous, sizes);
                let offset = (u64::from_le_bytes(previous[..8].try_into().unwrap())
                    % sizes.offset_modulus()) as usize;
                assert_eq!(case["sample_offset"].as_u64().unwrap() as usize, offset);
                let leg = hex_u64(case["leg_u64_hex"].as_str().unwrap());
                let good = hike::checkpoint_sized(&previous, &digest, &sample, leg, sizes);
                assert_eq!(encode(good), case["conforming_hex"].as_str().unwrap());
                let shifted = terrain[offset + 1..offset + 1 + sizes.sample_bytes].to_vec();
                let bad = hike::checkpoint_sized(&previous, &digest, &shifted, leg, sizes);
                assert_eq!(encode(bad), case["forbidden_hex"].as_str().unwrap());
                assert_ne!(good, bad);
                assert_eq!(case["fake_leg_ignore"], false);
            }
            "all4_kernel" => {
                let sizes = sizes_of(
                    case["slot_bytes"].as_u64().unwrap() as usize,
                    case["terrain_bytes"].as_u64().unwrap() as usize,
                );
                let good = fill_terrain_sized(&digest, sizes);
                assert_eq!(sha256(&good), case["conforming_sha256"].as_str().unwrap());
                let all4 = hike::rejected::fill_all4(&digest, sizes);
                assert_eq!(sha256(&all4), case["all4_sha256"].as_str().unwrap());
                assert_eq!(sha256(&all4), case["all4_sha256_flipped"].as_str().unwrap());
                assert_ne!(all4, good);
            }
            "f3_shared_terrain" => {
                let pk2 = hex32(case["pk2_hex"].as_str().unwrap());
                let sizes = sizes_of(
                    case["slot_bytes"].as_u64().unwrap() as usize,
                    case["terrain_bytes"].as_u64().unwrap() as usize,
                );
                let n1 = fill_terrain_sized(&digest, sizes);
                let n2 = fill_terrain_sized(&seed(&pk2, &beacon), sizes);
                assert_eq!(sha256(&n1), case["conforming_sha256_pk1"].as_str().unwrap());
                assert_eq!(sha256(&n2), case["conforming_sha256_pk2"].as_str().unwrap());
                assert_ne!(n1, n2);
                let shared = case["forbidden_sha256_pk1"].as_str().unwrap();
                assert_eq!(shared, case["forbidden_sha256_pk2"].as_str().unwrap());
                assert_ne!(sha256(&n1), shared);
            }
            other => panic!("unknown negative {other}"),
        }
    }

    let rate = load("rate.json");
    assert_eq!(rate["floor"]["p"], PLACEHOLDER_FLOOR_P);
    assert_eq!(rate["floor"]["q"], PLACEHOLDER_FLOOR_Q);
    assert_eq!(rate["floor"]["mark"], PLACEHOLDER_FLOOR_NOTE);
    let (p, q) = (PLACEHOLDER_FLOOR_P, PLACEHOLDER_FLOOR_Q);
    for case in rate["cases"].as_array().unwrap() {
        let legs = case["hike_legs"].as_u64().unwrap();
        let span = case["hike_span"].as_i64().unwrap() as i128;
        let meets = !below_floor(legs, span, p, q);
        assert_eq!(meets, case["meets"].as_bool().unwrap(), "{}", case["name"]);
    }
    assert!(below_floor(1, 0, p, q), "a zero span does not meet");

    let anchors = load("anchors.json");
    assert_eq!(anchors["minimum"], MIN_ANCHORED_WAYPOINTS);
    for case in anchors["cases"].as_array().unwrap() {
        let count = case["indices"].as_array().unwrap().len() as u64;
        let judgment = judge_beacon_rate(
            &BeaconRate {
                legs: Some(2),
                span_s: Some(5),
                first_height: None,
                last_height: None,
                first_time: Some(1),
                last_time: Some(6),
                anchored: Some(count),
                legs_not_defined: false,
                times_not_read: false,
            },
            p,
            q,
        );
        if case["meets"].as_bool().unwrap() {
            assert_eq!(judgment, RateJudgment::Clear, "{}", case["name"]);
        } else {
            assert_eq!(
                judgment,
                RateJudgment::Refuse("too_few_waypoints_anchored"),
                "{}",
                case["name"]
            );
        }
    }

    let kind = load("kind.json");
    assert_eq!(kind["kind"], hike::KIND);
    assert_eq!(kind["kind_bytes"], hike::KIND.len());
    let earlier: Vec<&str> = kind["earlier"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect();
    assert_eq!(earlier, hike::EARLIER_KINDS);

    let deadline = load("deadline.json");
    assert_eq!(deadline["K"], 3);
    assert_eq!(deadline["K_mark"], "placeholder, not ratified");
    assert_eq!(deadline["reason"], "submission_deadline");

    let mtp = load("mtp.json");
    for case in mtp["cases"].as_array().unwrap() {
        for side in ["first", "last"] {
            let Some(beacon) = case.get(side) else {
                continue;
            };
            let times = beacon["times"].as_array().unwrap();
            if times.iter().any(|item| !item.is_u64()) {
                assert!(beacon["mtp"].is_null() || beacon["mtp"] == "not read", "{}", case["name"]);
                continue;
            }
            let got = median_of(times);
            assert_eq!(got, beacon["mtp"].as_u64().unwrap(), "{} {side}", case["name"]);
            assert_eq!(
                beacon["reads"].as_array().unwrap().len() as u64,
                beacon["n"].as_u64().unwrap()
            );
        }
        if let Some(span) = case.get("hike_span").and_then(|item| item.as_i64()) {
            let first = case["first"]["mtp"].as_u64().unwrap() as i128;
            let last = case["last"]["mtp"].as_u64().unwrap() as i128;
            assert_eq!(last - first, span as i128, "{}", case["name"]);
            if span <= 0 {
                assert!(below_floor(1, span as i128, p, q));
            }
        }
    }

    let rounds = load("rounds.json");
    assert_eq!(rounds["thorough_rounds"], 3);
    assert_eq!(rounds["per_round"], 6);
    for case in rounds["cases"].as_array().unwrap() {
        let total = case["leg_total"].as_u64().unwrap();
        if case["draw"].as_str() == Some("not defined") {
            assert!(total < 6, "{}", case["name"]);
            continue;
        }
        let script: Vec<u8> = case["draws_u64_hex"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|item| hex_u64(item.as_str().unwrap()).to_le_bytes())
            .collect();
        let mut entropy = ScriptEntropy { bytes: script, pos: 0 };
        let started = case["rounds_started"].as_u64().unwrap() as usize;
        let drawn = hike::draw_thorough(total, started * 6, &mut entropy).unwrap();
        let mut got = Vec::new();
        for chunk in drawn.chunks(6) {
            got.push(chunk.to_vec());
        }
        let expected: Vec<Vec<u64>> = case["rounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|round| {
                round
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item.as_u64().unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(got, expected, "{}", case["name"]);
    }
}

struct ScriptEntropy {
    bytes: Vec<u8>,
    pos: usize,
}

impl hike::Entropy for ScriptEntropy {
    fn fill(&mut self, dest: &mut [u8]) -> Result<(), ()> {
        for byte in dest {
            *byte = *self.bytes.get(self.pos).ok_or(())?;
            self.pos += 1;
        }
        Ok(())
    }
}

fn median_of(times: &[Value]) -> u64 {
    let mut values: Vec<u64> = times.iter().map(|item| item.as_u64().unwrap()).collect();
    values.sort_unstable();
    values[values.len() / 2]
}
