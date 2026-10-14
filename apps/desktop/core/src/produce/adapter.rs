//! Progress from the library's own callbacks.
//!
//! `hike::produce_with_settlement_sized` stores a checkpoint and then calls
//! `observe`. The first call is the seed beacon, before terrain fill. Every
//! later call follows exactly one stored leg, and a header that is newer than
//! the stage beacon closes the stage, the same comparison the crate makes.
//! `open_stage` runs at each stage boundary for `g ≥ 1`, and `settle` runs
//! once after the last leg. Stop is checked at each of those boundaries, so
//! a stop takes effect after the leg in progress and never inside a step.

use super::events::ProduceEvent;
use hike::{ProduceFailure, Sizes, Stage};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub const STOPPED: &str = "stopped by the user";

struct Tracker<'a> {
    emit: &'a dyn Fn(ProduceEvent),
    calls: u64,
    stage: usize,
    legs_in_stage: u64,
    beacon: ([u8; 32], u64),
}

impl Tracker<'_> {
    fn observed(&mut self, tip: [u8; 32], height: u64, header_time: u32, at_ms: u64) {
        self.calls += 1;
        if self.calls == 1 {
            self.beacon = (tip, height);
            (self.emit)(ProduceEvent::SeedBeacon {
                height,
                natural_hash: tip,
                header_time_unix_s: header_time,
                at_ms,
            });
            (self.emit)(ProduceEvent::Tip {
                height,
                natural_hash: tip,
                peers: None,
                header_seen_unix_ms: at_ms,
            });
            return;
        }
        if self.calls == 2 {
            (self.emit)(ProduceEvent::TerrainFilled { at_ms });
        }
        let j = self.legs_in_stage;
        self.legs_in_stage += 1;
        (self.emit)(ProduceEvent::LegRecorded {
            stage: self.stage,
            j,
            stored_in_stage: j + 1,
            at_ms,
        });
        if tip != self.beacon.0 {
            (self.emit)(ProduceEvent::Tip {
                height,
                natural_hash: tip,
                peers: None,
                header_seen_unix_ms: at_ms,
            });
        }
        if tip != self.beacon.0 && height > self.beacon.1 {
            (self.emit)(ProduceEvent::StageClosed {
                stage: self.stage,
                legs: self.legs_in_stage,
                interrupt_height: height,
            });
            self.stage += 1;
            self.legs_in_stage = 0;
            self.beacon = (tip, height);
        }
    }
}

/// Run the crate's produce with progress events and a stop flag.
#[allow(clippy::too_many_arguments)]
pub fn drive<O, P, S>(
    public_key: [u8; 32],
    sizes: Sizes,
    stop: &AtomicBool,
    now_ms: &dyn Fn() -> u64,
    emit: &dyn Fn(ProduceEvent),
    mut observe: O,
    mut open_stage: P,
    mut settle: S,
) -> Result<Vec<u8>, ProduceFailure>
where
    O: FnMut() -> Result<([u8; 32], u64, u32), ProduceFailure>,
    P: FnMut(&[u8; 32], u64) -> Result<Vec<u8>, ProduceFailure>,
    S: FnMut(&mut [Stage], u64) -> Result<(), ProduceFailure>,
{
    let tracker = RefCell::new(Tracker {
        emit,
        calls: 0,
        stage: 0,
        legs_in_stage: 0,
        beacon: ([0; 32], 0),
    });
    let stopped = || stop.load(Ordering::SeqCst);
    hike::produce_with_settlement_sized(
        public_key,
        sizes,
        || {
            if stopped() {
                return Err(ProduceFailure::new(STOPPED));
            }
            let (tip, height, header_time) = observe()?;
            tracker
                .borrow_mut()
                .observed(tip, height, header_time, now_ms());
            Ok((tip, height))
        },
        |waypoint, beacon_height| {
            if stopped() {
                return Err(ProduceFailure::new(STOPPED));
            }
            open_stage(waypoint, beacon_height)
        },
        |stages, cut_off| {
            if stopped() {
                return Err(ProduceFailure::new(STOPPED));
            }
            emit(ProduceEvent::Settling);
            settle(stages, cut_off)
        },
    )
}

pub fn is_stop(err: &ProduceFailure) -> bool {
    err.message == STOPPED
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A finite tip script: stage `g` records `legs[g]` legs.
    pub(crate) fn script(legs: &[u64; 18], first_height: u64) -> Vec<([u8; 32], u64)> {
        let mut hash = [0u8; 32];
        hash[0] = 1;
        let mut height = first_height;
        let mut tips = vec![(hash, height)];
        for &n in legs {
            for _ in 1..n {
                tips.push((hash, height));
            }
            hash[0] = hash[0].wrapping_add(1);
            hash[1] = 0xee;
            height += 1;
            tips.push((hash, height));
        }
        tips
    }

    pub(crate) fn run_scripted(
        legs: &[u64; 18],
        stop_after: Option<u64>,
    ) -> (Result<Vec<u8>, ProduceFailure>, Vec<ProduceEvent>) {
        let tips = script(legs, 1_000);
        let events = Mutex::new(Vec::new());
        let stop = AtomicBool::new(false);
        let emit = |e: ProduceEvent| {
            if let (Some(n), ProduceEvent::LegRecorded { .. }) = (stop_after, &e) {
                let seen = events
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|e| matches!(e, ProduceEvent::LegRecorded { .. }))
                    .count() as u64;
                if seen + 1 >= n {
                    stop.store(true, Ordering::SeqCst);
                }
            }
            events.lock().unwrap().push(e);
        };
        let mut at = 0usize;
        let result = drive(
            [0x11; 32],
            Sizes::conformance_small(),
            &stop,
            &|| 0,
            &emit,
            || {
                let tip = tips[at];
                at += 1;
                Ok((tip.0, tip.1, 0))
            },
            |_, _| Ok(Vec::new()),
            |_, _| Ok(()),
        );
        (result, events.into_inner().unwrap())
    }

    const LEGS: [u64; 18] = [3, 1, 2, 1, 4, 1, 1, 2, 1, 1, 3, 1, 1, 1, 2, 1, 1, 2];

    #[test]
    fn events_follow_the_libraries_own_stage_boundaries() {
        let (result, events) = run_scripted(&LEGS, None);
        let bytes = result.expect("scripted produce");

        let closed: Vec<(usize, u64)> = events
            .iter()
            .filter_map(|e| match e {
                ProduceEvent::StageClosed { stage, legs, .. } => Some((*stage, *legs)),
                _ => None,
            })
            .collect();
        assert_eq!(
            closed,
            LEGS.iter()
                .enumerate()
                .map(|(g, &n)| (g, n))
                .collect::<Vec<_>>()
        );

        let legs: Vec<(usize, u64, u64)> = events
            .iter()
            .filter_map(|e| match e {
                ProduceEvent::LegRecorded {
                    stage,
                    j,
                    stored_in_stage,
                    ..
                } => Some((*stage, *j, *stored_in_stage)),
                _ => None,
            })
            .collect();
        assert_eq!(legs.len() as u64, LEGS.iter().sum::<u64>());
        assert!(legs.iter().all(|&(_, j, stored)| stored == j + 1));
        assert_eq!(legs[0], (0, 0, 1));
        assert_eq!(legs[2], (0, 2, 3));
        assert_eq!(legs[3], (1, 0, 1));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ProduceEvent::TerrainFilled { .. }))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ProduceEvent::Settling))
                .count(),
            1
        );
        assert!(matches!(
            events[0],
            ProduceEvent::SeedBeacon { height: 1_000, .. }
        ));

        // The pinned verifier reads the same counts out of the object.
        let m = hike::verify_sized(
            &bytes,
            hike::AuditDraw::PinnedSeed([0; 32]),
            |_| Err(()),
            |_| hike::ChainRead::NotRead,
            Sizes::conformance_small(),
        );
        let json = hike::verify_record_json("conformance-small", &m);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let per_stage: Vec<u64> = v["checkpoints"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["legs"].as_array().unwrap().len() as u64)
            .collect();
        assert_eq!(per_stage, LEGS.to_vec());
    }

    #[test]
    fn stop_ends_after_the_leg_in_progress_and_writes_nothing() {
        let (result, events) = run_scripted(&LEGS, Some(5));
        let err = result.expect_err("stopped");
        assert!(is_stop(&err));
        let legs = events
            .iter()
            .filter(|e| matches!(e, ProduceEvent::LegRecorded { .. }))
            .count();
        assert_eq!(legs, 5);
        assert!(!events.iter().any(|e| matches!(e, ProduceEvent::Settling)));
    }
}
