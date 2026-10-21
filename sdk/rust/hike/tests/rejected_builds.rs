//! Rejected constructions. Verify must not accept a leg produced by any of them.
//! The gate's thorough walk uses the same honest step, so a stored rejected leg mismatches.

use hike::rejected::{
    fill_all4, fill_pin_terrain, fill_ref, honest_terrain, leg_result_cached, leg_result_with,
    step_bare_finalizer, step_partial, step_s_plus_w, step_v2_0_fold, step_xor_acc, step_xor_w,
    terrain_word_xor_w,
};
use hike::{leg_result_sized, seed_preimage, Sizes};

fn sizes() -> Sizes {
    Sizes::conformance_small()
}

fn previous() -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&0x0123_4567_89ab_cdefu64.to_le_bytes());
    out
}

#[test]
fn each_rejected_build_differs_from_the_honest_leg() {
    let pk = [0x11u8; 32];
    let beacon = [0x22u8; 32];
    let sizes = sizes();
    let terrain = honest_terrain(&pk, &beacon, sizes);
    let previous = previous();
    let honest = leg_result_sized(&terrain, &previous, sizes);

    let bare = leg_result_with(&terrain, &previous, sizes, step_bare_finalizer);
    let xor = leg_result_with(&terrain, &previous, sizes, step_xor_acc);
    let v20 = leg_result_with(&terrain, &previous, sizes, step_v2_0_fold);
    let partial = leg_result_with(&terrain, &previous, sizes, step_partial);
    let xor_w = leg_result_with(&terrain, &previous, sizes, step_xor_w);
    let s_plus_w = leg_result_with(&terrain, &previous, sizes, step_s_plus_w);
    let hoisted = terrain_word_xor_w(&terrain, sizes);
    let cached = leg_result_cached(&terrain, &previous, sizes);
    assert_ne!(bare, honest, "mix64 without MIX_ADD");
    assert_ne!(xor, honest, "XOR accumulator");
    assert_ne!(v20, honest, "v2.0 fold");
    assert_ne!(partial, honest, "partial slot read");
    assert_ne!(xor_w, honest, "xor_w_fold");
    assert_ne!(s_plus_w, honest, "s_plus_w_fold");
    assert_ne!(leg_result_sized(&hoisted, &previous, sizes), honest, "prefold_hoist");
    assert_ne!(cached, honest, "cached leg");

    let preimage = seed_preimage(&pk, &beacon);
    let pin = fill_pin_terrain(&preimage, sizes);
    let reference = fill_ref(&hike::seed(&pk, &beacon), sizes);
    let all4 = fill_all4(&hike::seed(&pk, &beacon), sizes);
    assert_ne!(pin, terrain, "fill_pin terrain");
    assert_ne!(reference, terrain, "REF word map");
    assert_ne!(all4, terrain, "ALL4 word map");
    assert_ne!(leg_result_sized(&pin, &previous, sizes), honest);
    assert_ne!(leg_result_sized(&reference, &previous, sizes), honest);
    assert_ne!(leg_result_sized(&all4, &previous, sizes), honest);
}

#[test]
fn verify_rejects_a_leg_stored_from_each_rejected_fold() {
    let pk = [0x42u8; 32];
    let mut beacon = [0u8; 32];
    beacon[0] = 1;
    let sizes = sizes();
    let terrain = honest_terrain(&pk, &beacon, sizes);
    let seed = hike::seed(&pk, &beacon);
    let honest = leg_result_sized(&terrain, &seed, sizes);
    let rejected = [
        ("bare", leg_result_with(&terrain, &seed, sizes, step_bare_finalizer)),
        ("xor", leg_result_with(&terrain, &seed, sizes, step_xor_acc)),
        ("v2_0_fold", leg_result_with(&terrain, &seed, sizes, step_v2_0_fold)),
        ("partial", leg_result_with(&terrain, &seed, sizes, step_partial)),
        ("cached", leg_result_cached(&terrain, &seed, sizes)),
    ];
    for (name, bad) in rejected {
        assert_ne!(bad, honest, "{name}");
        let bytes = object_with_leg(&pk, beacon, seed, bad, sizes);
        let measurements = hike::verify_sized(
            &bytes,
            hike::AuditDraw::PinnedSeed([0xA5; 32]),
            |_| Err(()),
            |_| Ok(()),
            sizes,
        );
        let record = hike::verify_record_json("conformance-small", &measurements);
        let value: serde_json::Value = serde_json::from_str(record.trim()).unwrap();
        let results = value["six_leg"]["results"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} six_leg: {record}"));
        assert!(
            results.iter().any(|token| token == "mismatch"),
            "{name} leg was accepted: {results:?}"
        );
    }
}

fn object_with_leg(
    public_key: &[u8; 32],
    beacon: [u8; 32],
    seed: [u8; 32],
    leg: u64,
    sizes: Sizes,
) -> Vec<u8> {
    let terrain = honest_terrain(public_key, &beacon, sizes);
    let mut stages = Vec::new();
    for g in 0..6 {
        let mut stage_beacon = beacon;
        stage_beacon[0] = stage_beacon[0].wrapping_add(g as u8);
        let waypoint = if g == 0 { seed } else { [g as u8; 32] };
        let previous = waypoint;
        let stored = if g == 0 {
            leg
        } else {
            leg_result_sized(&terrain, &previous, sizes)
        };
        stages.push(hike::Stage {
            beacon: stage_beacon,
            beacon_height: 10 + g as u64,
            leg_count: 1,
            waypoint,
            path: Vec::new(),
            checkpoints: vec![[0xAB; 32]],
            leg_results: vec![stored.to_le_bytes()],
            checkpoint_rows_defined: true,
            leg_result_missing: Vec::new(),
        });
    }
    let mut last = [0u8; 32];
    last[0] = 9;
    let hike = hike::Hike {
        public_key: *public_key,
        seed_beacon: beacon,
        stages,
        last_interrupt: last,
        last_interrupt_height: 20,
        last_interrupt_absent: false,
        last_interrupt_hash_absent: false,
        last_interrupt_height_absent: false,
    };
    hike::encode_hike_sized(&hike, sizes)
}
