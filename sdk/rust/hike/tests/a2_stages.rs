//! Rules A2.1, A2.2, and A2.3 where the running verifier already agrees.
//! main @ 546d24f L639, L651–L666.

#[path = "common/support.rs"]
mod support;

use std::sync::OnceLock;

fn terrain() -> &'static [u8] {
    static TERRAIN: OnceLock<Vec<u8>> = OnceLock::new();
    TERRAIN.get_or_init(|| {
        let mut seed = [0u8; 32];
        seed[0] = 0x22;
        support::terrain_input(&[0x11; 32], &seed)
    })
}

fn two_leg_hike(terrain: &[u8]) -> hike::Hike {
    let mut seed = [0u8; 32];
    seed[0] = 0x22;
    let mut hike = support::quiet_hike();
    hike.public_key = [0x11; 32];
    hike.seed_beacon = seed;
    let waypoint = [0x44; 32];
    // Rule A2.2. Leg 0 of stage g ≥ 1 starts from the stored waypoint.
    let previous = waypoint;
    let first = support::checkpoint_digest(
        &previous,
        &waypoint,
        &support::terrain_sample(terrain, &previous),
        7,
    );
    let second = support::checkpoint_digest(
        &first,
        &waypoint,
        &support::terrain_sample(terrain, &first),
        9,
    );
    let stage = &mut hike.stages[4];
    stage.waypoint = waypoint;
    stage.leg_count = 2;
    stage.checkpoints = vec![first, second];
    stage.leg_results = vec![7u64.to_le_bytes(), 9u64.to_le_bytes()];
    hike
}

/// Rule A2.1. `seed_beacon` plus the 18 interrupts gives 18 stages, numbered 0–17.
/// main @ 546d24f L639.
#[test]
fn a2_1_eighteen_stages() {
    assert_eq!(hike::structural_object().stages.len(), 18);
}

/// Rule A2.2. Leg `j` for `j ≥ 1` uses previous checkpoint `checkpoints[j − 1]`.
/// main @ 546d24f L658.
#[test]
fn a2_2_later_leg_from_previous_checkpoint() {
    let terrain = terrain();
    let hike = two_leg_hike(terrain);
    let record = support::section14(&support::verify_offline(&hike::encode_hike(&hike)));
    let legs = match &record.checkpoints.expect("checkpoint rows")[4].legs {
        support::Field::Value(legs) => legs.clone(),
        other => panic!("leg rows {other:?}"),
    };
    assert_eq!(legs[1].j, 1);
    assert_eq!(legs[1].result, "match");
}
