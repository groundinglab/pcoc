//! SPEC §12. A fixture hike: legs, checkpoints, and waypoints from one terrain.
//! No header source and no Bari client.

use crate::checkpoint::checkpoint;
use crate::constants::{FILL, STAGES};
use crate::object::{make_object, HikeObject, Stage};
use crate::profile::Profile;
use crate::step::leg_result;
use crate::terrain::seed as seed_digest;
use crate::waypoint::{fold_checkpoints, waypoint};

/// SPEC §12. One stage's stored bytes, in leg order.
pub struct StageOut {
    pub waypoint: [u8; 32],
    pub checkpoints: Vec<[u8; 32]>,
    pub leg_results: Vec<u64>,
}

/// SPEC §12. Walk `legs_per_stage` on an already filled terrain.
/// `interrupts` is `I_0` … `I_17` with heights. Stage 0 starts at `seed_beacon`.
/// Returns the object, the per-stage outputs, and the seed `D`.
pub fn produce_hike(
    profile: &Profile,
    public_key: &[u8; 32],
    seed_beacon: &[u8; 32],
    seed_height: u64,
    interrupts: &[([u8; 32], u64); 18],
    legs_per_stage: &[u32; 18],
    terrain: &[u8],
) -> (HikeObject, [u8; 32], Vec<StageOut>) {
    let d = seed_digest(public_key, seed_beacon);
    let mut current_wp = d;
    let mut stages = Vec::with_capacity(STAGES);
    let mut outs = Vec::with_capacity(STAGES);
    for g in 0..STAGES {
        let (beacon, beacon_height) = if g == 0 {
            (*seed_beacon, seed_height)
        } else {
            interrupts[g - 1]
        };
        let limb = current_wp;
        let n = legs_per_stage[g] as usize;
        let mut prev = limb;
        let mut cps: Vec<[u8; 32]> = Vec::with_capacity(n);
        let mut lrs: Vec<u64> = Vec::with_capacity(n);
        for _j in 0..n {
            let lr = leg_result(terrain, profile, &prev);
            let cp = checkpoint(&prev, &limb, terrain, profile, lr);
            cps.push(cp);
            lrs.push(lr);
            prev = cp;
        }
        let path = if g == 0 { None } else { Some(Vec::new()) };
        stages.push(Stage {
            beacon,
            beacon_height,
            height_over: false,
            leg_count: n as u64,
            waypoint: limb,
            path,
            checkpoints: cps.iter().map(|c| c.to_vec()).collect(),
            leg_results: lrs.iter().map(|r| r.to_le_bytes().to_vec()).collect(),
            usable: true,
        });
        outs.push(StageOut {
            waypoint: limb,
            checkpoints: cps.clone(),
            leg_results: lrs,
        });
        if g < 17 {
            let fold = fold_checkpoints(&cps);
            let (ig, _) = interrupts[g];
            current_wp = waypoint(&ig, terrain, profile, &limb, &fold);
        }
    }
    let (last_hash, last_h) = interrupts[17];
    let obj = make_object(
        FILL,
        profile.slot_bytes as u64,
        profile.n_slots,
        profile.sample_bytes as u64,
        profile.steps_per_leg,
        *public_key,
        *seed_beacon,
        stages,
        last_hash,
        last_h,
    );
    (obj, d, outs)
}
