//! Rules A1.3 and A2.2. The stored waypoint and the checkpoint digest.

#[path = "common/support.rs"]
mod support;

use std::sync::OnceLock;

fn fixed_terrain() -> &'static [u8] {
    static TERRAIN: OnceLock<Vec<u8>> = OnceLock::new();
    TERRAIN.get_or_init(|| {
        let mut seed = [0u8; 32];
        seed[0] = 1;
        support::terrain_input(&[0x11; 32], &seed)
    })
}

fn base_hike() -> hike::Hike {
    let mut seed = [0u8; 32];
    seed[0] = 1;
    let mut hike = support::quiet_hike();
    hike.public_key = [0x11; 32];
    hike.seed_beacon = seed;
    for (g, stage) in hike.stages.iter_mut().enumerate() {
        stage.beacon = [0u8; 32];
        stage.beacon[0] = (g as u8) + 1;
        stage.beacon_height = 1_000 + g as u64;
        stage.waypoint = [0u8; 32];
        stage.path.clear();
    }
    hike.stages[0].beacon = seed;
    hike.stages[0].waypoint = support::seed_value(&hike.public_key, &seed);
    hike
}

/// Rule A1.3. `W_{g+1} = SHA-256(I_g ‖ slot_sample(I_g) ‖ W_g ‖ fold_g)`.
/// `I_g` is `stages[g+1].beacon`.
#[test]
fn a1_3_waypoint_preimage() {
    let terrain = fixed_terrain();
    let mut hike = base_hike();
    let checkpoint = [0xCD; 32];
    hike.stages[0].leg_count = 1;
    hike.stages[0].checkpoints = vec![checkpoint];
    hike.stages[0].leg_results = vec![1u64.to_le_bytes()];
    let fold = support::fold_checkpoints(&[checkpoint]);
    let interrupt = hike.stages[1].beacon;
    let digest = support::waypoint_digest(
        &interrupt,
        &support::slot_sample(terrain, &interrupt),
        &hike.stages[0].waypoint,
        &fold,
    );
    hike.stages[1].waypoint = digest;
    let record = support::section14(&support::verify_offline(&hike::encode_hike(&hike)));
    let rows = record.waypoints.expect("waypoint rows");
    assert_eq!(rows[0].g, 1);
    assert_eq!(rows[0].result, "match");
}

/// Rule A2.2. Stage 0 leg 0 starts from SHA-256(public_key ‖ seed_beacon).
#[test]
fn a2_2_stage0_leg0_from_sha256_seed() {
    let terrain = fixed_terrain();
    let mut hike = base_hike();
    let previous = hike.stages[0].waypoint;
    let digest = support::checkpoint_digest(
        &previous,
        &previous,
        &support::terrain_sample(terrain, &previous),
        4,
    );
    hike.stages[0].leg_count = 1;
    hike.stages[0].checkpoints = vec![digest];
    hike.stages[0].leg_results = vec![4u64.to_le_bytes()];
    let record = support::section14(&support::verify_offline(&hike::encode_hike(&hike)));
    match &record.checkpoints.expect("checkpoint rows")[0].legs {
        support::Field::Value(legs) => assert_eq!(legs[0].result, "match"),
        other => panic!("leg rows {other:?}"),
    }
}

/// Rule A2.2. Leg 0 of stage g ≥ 1 starts from the stored waypoint.
#[test]
fn a2_2_later_stage_leg0_from_waypoint() {
    let terrain = fixed_terrain();
    let mut hike = base_hike();
    let waypoint = [0x33; 32];
    hike.stages[3].waypoint = waypoint;
    let digest = support::checkpoint_digest(
        &waypoint,
        &waypoint,
        &support::terrain_sample(terrain, &waypoint),
        4,
    );
    hike.stages[3].leg_count = 1;
    hike.stages[3].checkpoints = vec![digest];
    hike.stages[3].leg_results = vec![4u64.to_le_bytes()];
    let record = support::section14(&support::verify_offline(&hike::encode_hike(&hike)));
    match &record.checkpoints.expect("checkpoint rows")[3].legs {
        support::Field::Value(legs) => assert_eq!(legs[0].result, "match"),
        other => panic!("leg rows {other:?}"),
    }
}
