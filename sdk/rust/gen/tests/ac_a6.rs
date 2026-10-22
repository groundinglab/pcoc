//! AC-A6. The 13 permanent negatives match erratum 1, and a stored leg negative
//! makes the six-leg check `mismatch` at the injected index 0.

use pcoc::hash::sha256;
use pcoc::hexutil::{hex_decode_exact, hex_encode};
use pcoc::profile::Profile;
use pcoc::terrain::{lanes, seed, terrain_from_seed};
use pcoc::verify::Verified;
use pcoc_gen::{all4_kernel, f3_shared_terrain, leg_negatives, one_lane_flip, squeezed_seed_terrain};
use pcoc_produce::{encode_produced, produce_on_rig, Case, ProduceOutcome};
use pcoc_testing::{verify_from_bytes, CasesIndex};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

const PK_A: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
const PK_B: &str = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";

fn le_hex(n: u64) -> String {
    hex_encode(&n.to_le_bytes())
}

fn verify_injected(bytes: &[u8]) -> String {
    let profile = pcoc_produce::cases_profile(Case::S1);
    let mut index = CasesIndex::new();
    let mut n = 0u64;
    let mut next = || {
        let raw = n.to_le_bytes();
        n += 1;
        Some(raw)
    };
    match verify_from_bytes(bytes, &profile, &mut index, &mut next) {
        Verified::Record(s) => s,
        other => panic!("expected a record, got {other:?}"),
    }
}

fn six_results(record: &str) -> Vec<String> {
    let marker = "\"results\":[";
    let i = record.find(marker).expect("results");
    let after = &record[i + marker.len()..];
    let end = after.find(']').unwrap();
    after[..end]
        .split(',')
        .map(|s| s.trim_matches('"').to_string())
        .collect()
}

#[test]
fn negatives_and_six_leg() {
    let pk_a: [u8; 32] = hex_decode_exact(PK_A).unwrap();
    let pk_b: [u8; 32] = hex_decode_exact(PK_B).unwrap();
    let beacon = sha256(b"pcoc-pack-2.2/beacon/seed");
    let profile = Profile::conformance_small(32, 64, 65_536, 100).unwrap();
    let (terrain, d, lane) = terrain_from_seed(&pk_a, &beacon, &profile);
    let honest = pcoc::leg_result(&terrain, &profile, &d);
    assert_eq!(le_hex(honest), "4c9a6a98574cbd31");
    assert_eq!(
        hex_encode(&sha256(&terrain)),
        "366e6d1d27adb715d5c96c64e6b9d881e1701a0151e6e08d49df08d2996bd97f"
    );

    let legs = leg_negatives(&terrain, &profile, &d);
    assert_eq!(legs.cached_revisit, Some((34, 101, 20)));
    let lane_b = lanes(&seed(&pk_b, &beacon));
    let f3 = f3_shared_terrain(&profile, &d, &lane_b);
    let leg_named = [
        ("slot_summary_fold", legs.slot_summary_fold, "b87ce6fc485dd57c"),
        ("xor_accumulator_fold", legs.xor_accumulator_fold, "346188d2d8cf98e8"),
        ("xor_w_fold", legs.xor_w_fold, "73a4c7fbd105aae1"),
        ("s_plus_w_fold", legs.s_plus_w_fold, "07262df2d90e8ecb"),
        ("prefold_hoist", legs.prefold_hoist, "a652562037b465fc"),
        ("final_stage_hoist", legs.final_stage_hoist, "b1808c8d3927a7d1"),
        ("partial_slot_read", legs.partial_slot_read, "e1ae647ee5a5d7c5"),
        ("partial_slot_read_2047", legs.partial_slot_read_2047, "738755dabac824ab"),
        ("f3_shared_terrain", f3, "a73019b8a2c43cd1"),
        ("cached_leg", legs.cached_leg, "3b715e2a251a44d9"),
    ];
    let mut seen = HashSet::new();
    for (name, value, expect) in leg_named {
        let hex = le_hex(value);
        assert_eq!(hex, expect, "{name}");
        assert_ne!(value, honest, "{name}");
        assert!(seen.insert(hex), "{name} not distinct");
    }
    let terrain_named = [
        ("one_lane_flip", one_lane_flip(&profile, &lane), "5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608"),
        ("all4_kernel", all4_kernel(&profile, &lane), "2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e"),
        ("squeezed_seed_terrain", squeezed_seed_terrain(&profile, &lane), "ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22"),
    ];
    let honest_terrain = sha256(&terrain);
    let mut seen_t = HashSet::new();
    for (name, dig, expect) in terrain_named {
        let hex = hex_encode(&dig);
        assert_eq!(hex, expect, "{name}");
        assert_ne!(dig, honest_terrain, "{name}");
        assert!(seen_t.insert(hex), "{name} not distinct");
    }

    let key = pk_a;
    let cancel = AtomicBool::new(false);
    let honest_obj = match produce_on_rig(
        &key,
        &pcoc_produce::cases_profile(Case::S1),
        &pcoc_produce::cases_rig(Case::S1),
        &cancel,
        |_| {},
    ) {
        ProduceOutcome::Object(obj) => obj,
        ProduceOutcome::Cancelled => panic!("cancelled"),
    };
    let honest_bytes = encode_produced(&honest_obj);
    let honest_rec = verify_injected(&honest_bytes);
    assert!(honest_rec.contains("\"shape\":\"match\""));
    assert_eq!(
        six_results(&honest_rec),
        vec!["match", "match", "match", "match", "match", "match"]
    );

    for (name, value, _) in leg_named {
        let mut obj = honest_obj.clone();
        obj.stages[0].leg_results[0] = value.to_le_bytes().to_vec();
        let rec = verify_injected(&encode_produced(&obj));
        let results = six_results(&rec);
        assert_eq!(results[0], "mismatch", "{name}");
        assert_eq!(results.len(), 6, "{name}");
    }
}
