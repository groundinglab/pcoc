//! SPEC §13 producer.
//!
//! `produce_on_rig` is the scripted offline rig. [`live`] is the live hike:
//! a gossip header source, an async Bari client, and a `v2.2-pcoc-object`.
//! The walk reads the tip through a call that returns at once (Rule I3).
//! It does not sleep.
#![deny(unsafe_code)]

pub mod bari;
pub mod live;
pub mod source;

pub use pcoc_gossip::MAINNET_DNS_SEEDS;

use pcoc::checkpoint::checkpoint;
use pcoc::constants::{FILL, STAGES};
use pcoc::hash::sha256;
use pcoc::object::{make_object, HikeObject, Stage};
use pcoc::profile::Profile;
use pcoc::step::leg_result;
use pcoc::terrain::terrain_from_seed;
use pcoc::waypoint::{fold_checkpoints, waypoint};
use std::sync::atomic::{AtomicBool, Ordering};

/// SPEC §13.3. One scripted header schedule. The caller supplies the key separately.
#[derive(Clone, Debug)]
pub struct ScriptedRig {
    pub seed_beacon: [u8; 32],
    pub seed_height: u64,
    /// `I_0` … `I_17` and their heights.
    pub interrupts: [([u8; 32], u64); 18],
    pub legs_per_stage: [u32; 18],
}

/// CASES.md fixture names. The schedule is the fixed beacons and heights in that file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    S1,
    S2,
    Prod,
}

/// SPEC §10. SHA-256 of an ASCII `pcoc-pack-2.2/` label. Fixture bytes, not a second construction.
pub fn label_digest(label: &str) -> [u8; 32] {
    sha256(format!("pcoc-pack-2.2/{label}").as_bytes())
}

/// CASES.md profiles. `Prod` is section 6.
pub fn cases_profile(case: Case) -> Profile {
    match case {
        Case::S1 => Profile::conformance_small(32, 64, 65_536, 100).expect("S1"),
        Case::S2 => Profile::conformance_small(64, 256, 1_048_576, 1_000).expect("S2"),
        Case::Prod => Profile::production(),
    }
}

/// CASES.md beacon schedule. Seed at height 1000. `I_k` at height `1001 + k`.
#[allow(clippy::needless_range_loop)]
pub fn cases_rig(case: Case) -> ScriptedRig {
    let legs = match case {
        Case::S1 | Case::Prod => [1u32; 18],
        Case::S2 => [3u32; 18],
    };
    let mut interrupts = [([0u8; 32], 0u64); 18];
    for k in 0..18 {
        interrupts[k] = (label_digest(&format!("beacon/{k}")), 1001 + k as u64);
    }
    ScriptedRig {
        seed_beacon: label_digest("beacon/seed"),
        seed_height: 1000,
        interrupts,
        legs_per_stage: legs,
    }
}

/// SPEC §13.5-shaped progress. The producer reports these; it does not admit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    TerrainFilled,
    Leg {
        stage: usize,
        leg: usize,
        leg_result: u64,
    },
    Interrupt {
        stage: usize,
        height: u64,
    },
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ProduceOutcome {
    Object(HikeObject),
    Cancelled,
}

/// SPEC §13.2 / §13.3. Walk `rig` on `profile`.
///
/// `public_key` is the caller's. This function does not read a key from disk.
/// `cancel` is checked between legs. A leg already started finishes.
/// The held tip starts at the seed. After `legs_per_stage[g]` checkpoints the
/// held tip advances, and the next tip read returns that header at once.
pub fn produce_on_rig(
    public_key: &[u8; 32],
    profile: &Profile,
    rig: &ScriptedRig,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> ProduceOutcome {
    if cancel.load(Ordering::SeqCst) {
        return ProduceOutcome::Cancelled;
    }
    let (terrain, mut limb, _) = terrain_from_seed(public_key, &rig.seed_beacon, profile);
    progress(Progress::TerrainFilled);
    if cancel.load(Ordering::SeqCst) {
        return ProduceOutcome::Cancelled;
    }

    let mut tips: Vec<([u8; 32], u64)> = Vec::with_capacity(19);
    tips.push((rig.seed_beacon, rig.seed_height));
    tips.extend_from_slice(&rig.interrupts);
    let mut held = 0usize;

    let mut beacon = rig.seed_beacon;
    let mut beacon_height = rig.seed_height;
    let mut stages: Vec<Stage> = Vec::with_capacity(STAGES);
    let mut last_interrupt = [0u8; 32];
    let mut last_interrupt_height = 0u64;

    for g in 0..STAGES {
        if cancel.load(Ordering::SeqCst) {
            return ProduceOutcome::Cancelled;
        }
        let stage_limb = limb;
        let mut previous = stage_limb;
        let mut cps: Vec<[u8; 32]> = Vec::new();
        let mut lrs: Vec<u64> = Vec::new();
        let limit = rig.legs_per_stage[g];
        assert!(limit >= 1, "a stage records at least one leg");
        let mut done = 0u32;
        let interrupt = loop {
            if cancel.load(Ordering::SeqCst) {
                return ProduceOutcome::Cancelled;
            }
            let lr = leg_result(&terrain, profile, &previous);
            let cp = checkpoint(&previous, &stage_limb, &terrain, profile, lr);
            cps.push(cp);
            lrs.push(lr);
            previous = cp;
            progress(Progress::Leg {
                stage: g,
                leg: done as usize,
                leg_result: lr,
            });
            done += 1;
            if done == limit && held + 1 < tips.len() {
                held += 1;
            }
            let (natural, height) = tips[held];
            if natural != beacon && height > beacon_height {
                break (natural, height);
            }
            if done == limit {
                break (natural, height);
            }
        };

        stages.push(Stage {
            beacon,
            beacon_height,
            height_over: false,
            leg_count: cps.len() as u64,
            waypoint: stage_limb,
            path: if g == 0 { None } else { Some(Vec::new()) },
            checkpoints: cps.iter().map(|c| c.to_vec()).collect(),
            leg_results: lrs.iter().map(|r| r.to_le_bytes().to_vec()).collect(),
            usable: true,
        });

        if g + 1 == STAGES {
            last_interrupt = interrupt.0;
            last_interrupt_height = interrupt.1;
            progress(Progress::Interrupt {
                stage: g,
                height: last_interrupt_height,
            });
        } else {
            let fold = fold_checkpoints(&cps);
            limb = waypoint(&interrupt.0, &terrain, profile, &stage_limb, &fold);
            beacon = interrupt.0;
            beacon_height = interrupt.1;
            progress(Progress::Interrupt {
                stage: g,
                height: beacon_height,
            });
        }
    }

    let obj = make_object(
        FILL,
        profile.slot_bytes as u64,
        profile.n_slots,
        profile.sample_bytes as u64,
        profile.steps_per_leg,
        *public_key,
        rig.seed_beacon,
        stages,
        last_interrupt,
        last_interrupt_height,
    );
    progress(Progress::Closed);
    ProduceOutcome::Object(obj)
}

/// SPEC §7.3. Canonical bytes of a produced object.
pub fn encode_produced(obj: &HikeObject) -> Vec<u8> {
    pcoc::encode_object(obj)
}

/// SPEC Rule A3.4. Stderr at close: `path_skip <g>` for `g` in 1…16, then `not fetched by design 17`.
pub fn path_skip_stderr() -> String {
    let mut out = String::new();
    for g in 1..=16 {
        out.push_str("path_skip ");
        out.push_str(&g.to_string());
        out.push('\n');
    }
    out.push_str("not fetched by design 17\n");
    out
}
