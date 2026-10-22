//! SPEC §13.2 walk. Back-to-back legs. The tip read returns at once.
//! This file does not post, ask, sleep, or wait on a clock.

use super::header::{HeaderSource, Observed, TipRead};
use super::post::PostJob;
use super::state::Run;
use crate::checkpoint::checkpoint;
use crate::constants::{HEIGHT_MAX, STAGES};
use crate::object::Stage;
use crate::profile::Profile;
use crate::step::leg_result;
use crate::terrain::terrain_from_seed;
use crate::waypoint::{fold_checkpoints, waypoint};
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::time::Instant;

pub(crate) struct Walked {
    pub stages: Vec<Stage>,
    pub last_interrupt: [u8; 32],
    pub last_interrupt_height: u64,
    pub legs: u64,
    pub walk_ns: u64,
    pub leg_ns: u64,
}

pub(crate) fn walk(
    run: &Run,
    source: &dyn HeaderSource,
    profile: &Profile,
    public_key: &[u8; 32],
    post_tx: &Sender<PostJob>,
) -> Option<Walked> {
    let seed = wait_seed(run, source)?;
    run.note_block(seed.height, &seed.natural);
    let (terrain, mut limb, _) = terrain_from_seed(public_key, &seed.natural, profile);

    let walk_at = Instant::now();
    let mut walk_ns = 0u64;
    let mut leg_ns = 0u64;
    let mut legs = 0u64;
    let mut beacon = seed.natural;
    let mut beacon_height = seed.height;
    let mut stages: Vec<Stage> = Vec::with_capacity(STAGES);
    let mut last_interrupt = [0u8; 32];
    let mut last_interrupt_height = 0u64;
    let mut failed_episode = false;
    let mut stalled_episode = false;

    for g in 0..STAGES {
        if run.cancel.load(Ordering::SeqCst) {
            return None;
        }
        run.stage.store(g, Ordering::Relaxed);
        if g >= 1 {
            {
                let mut plants = run.plants.lock().unwrap();
                plants[g].waypoint = limb;
                plants[g].height = beacon_height;
                plants[g].phase = super::state::PHASE_POSTING;
                plants[g].asked = [false; 3];
            }
            if run.live_log() {
                run.emit(super::events::waypoint_line(g, &limb));
            }
            let _ = post_tx.send(PostJob {
                stage: g,
                waypoint: limb,
            });
        }

        let stage_limb = limb;
        let mut previous = stage_limb;
        let mut cps: Vec<[u8; 32]> = Vec::new();
        let mut lrs: Vec<u64> = Vec::new();
        let interrupt = loop {
            if run.cancel.load(Ordering::SeqCst) {
                return None;
            }
            let leg_at = Instant::now();
            let lr = leg_result(&terrain, profile, &previous);
            leg_ns = leg_ns.saturating_add(ns_since(leg_at));
            let cp = checkpoint(&previous, &stage_limb, &terrain, profile, lr);
            cps.push(cp);
            lrs.push(lr);
            legs = legs.saturating_add(1);
            run.legs.store(legs, Ordering::Relaxed);
            previous = cp;

            match source.tip() {
                TipRead::Ready(obs) => {
                    failed_episode = false;
                    stalled_episode = false;
                    run.note_block(obs.height, &obs.natural);
                    if obs.natural != beacon && obs.height > beacon_height {
                        if g + 1 == STAGES {
                            walk_ns = ns_since(walk_at);
                        }
                        break obs;
                    }
                }
                TipRead::Failed => {
                    if !failed_episode {
                        failed_episode = true;
                        stalled_episode = false;
                        run.emit(super::events::alert_line(
                            "clock_read_failed",
                            g,
                            None,
                        ));
                    }
                }
                TipRead::Empty => {
                    if !stalled_episode {
                        stalled_episode = true;
                        failed_episode = false;
                        run.emit(super::events::alert_line(
                            "header_source_stalled",
                            g,
                            None,
                        ));
                    }
                }
            }
        };

        stages.push(Stage {
            beacon,
            beacon_height,
            height_over: beacon_height > HEIGHT_MAX,
            leg_count: cps.len() as u64,
            waypoint: stage_limb,
            path: if g == 0 { None } else { Some(Vec::new()) },
            checkpoints: cps.iter().map(|c| c.to_vec()).collect(),
            leg_results: lrs.iter().map(|r| r.to_le_bytes().to_vec()).collect(),
            usable: true,
        });

        if g + 1 == STAGES {
            last_interrupt = interrupt.natural;
            last_interrupt_height = interrupt.height;
        } else {
            let fold = fold_checkpoints(&cps);
            limb = waypoint(&interrupt.natural, &terrain, profile, &stage_limb, &fold);
            beacon = interrupt.natural;
            beacon_height = interrupt.height;
        }
    }

    if walk_ns == 0 {
        walk_ns = ns_since(walk_at);
    }
    Some(Walked {
        stages,
        last_interrupt,
        last_interrupt_height,
        legs,
        walk_ns,
        leg_ns,
    })
}

/// Until the seed read succeeds the run has not started. No leg exists yet.
fn wait_seed(run: &Run, source: &dyn HeaderSource) -> Option<Observed> {
    let mut failed_episode = false;
    let mut stalled_episode = false;
    loop {
        if run.cancel.load(Ordering::SeqCst) {
            return None;
        }
        match source.tip() {
            TipRead::Ready(obs) => return Some(obs),
            TipRead::Failed => {
                if !failed_episode {
                    failed_episode = true;
                    stalled_episode = false;
                    run.emit(super::events::alert_line("clock_read_failed", 0, None));
                }
                source.wait_learned();
            }
            TipRead::Empty => {
                if !stalled_episode {
                    stalled_episode = true;
                    failed_episode = false;
                    run.emit(super::events::alert_line(
                        "header_source_stalled",
                        0,
                        None,
                    ));
                }
                source.wait_learned();
            }
        }
    }
}

fn ns_since(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}
