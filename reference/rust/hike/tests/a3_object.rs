//! Rule A3.2 stage fields the running object already stores.
//! main @ 546d24f L349–L376.

#[path = "common/support.rs"]
mod support;

/// Rule A3.2. Each stage stores beacon, height, leg_count, waypoint, checkpoints, and leg results.
/// main @ 546d24f L349–L376.
#[test]
fn a3_2_stage_record_fields() {
    let hike = hike::structural_object();
    assert_eq!(hike.stages.len(), 18);
    for stage in &hike.stages {
        assert_eq!(stage.waypoint.len(), 32);
        assert_eq!(stage.beacon.len(), 32);
        assert_eq!(stage.leg_count as usize, stage.checkpoints.len());
        assert_eq!(stage.checkpoints.len(), stage.leg_results.len());
        assert!(stage.leg_count >= 1);
        for checkpoint in &stage.checkpoints {
            assert_eq!(checkpoint.len(), 32);
        }
        for result in &stage.leg_results {
            assert_eq!(result.len(), 8);
        }
    }
}

/// Rule A3.3. Stages 1–16 store a path byte string, empty or present.
/// main @ 546d24f L386. This checks stored bytes, not the `path_state` token.
#[test]
fn a3_3_path_or_skip_on_w1_through_w16() {
    let mut hike = support::quiet_hike();
    let present = hike::encode_single_leaf(&hike.stages[3].waypoint);
    hike.stages[3].path = present.clone();
    let stored = support::read_stored_hike(&hike::encode_hike(&hike)).expect("stored object");
    for g in 1..=16 {
        if g == 3 {
            assert_eq!(stored.stages[g].path, present);
        } else {
            assert!(stored.stages[g].path.is_empty());
        }
    }
}
