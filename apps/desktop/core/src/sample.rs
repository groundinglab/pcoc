//! The DESIGN-v2 section 2 sample hike, as events and as a §14 record.
//!
//! Built only for tests and for the `sample` feature, which the screenshot
//! run uses. It is not a mode of the shipping app, and nothing here is
//! sent anywhere.

use crate::produce::events::{AskOutcome, ProduceEvent};
use crate::record::{present, Presented};
use crate::verify_job::{IndexReads, Outcome, VerifyRun};
use serde_json::{json, Value};
use std::path::PathBuf;

pub const NPUB: &str = "npub16896f5x95hys6mssl9hz7jtffhm3hzrzcmwd0a0ayq2kjn0walfqasq744";
pub const LEGS: [u64; 18] = [
    177, 251, 134, 323, 92, 215, 167, 398, 117, 191, 230, 1, 280, 145, 204, 346, 108, 180,
];
pub const SEED_HEIGHT: u64 = 971_204;
pub const START_S: u64 = 1_791_529_471;
pub const TERRAIN_FILL_S: u64 = 38;
pub const NOW_R_S: u64 = 1_791_533_491;
pub const LEGS_AT_R: u64 = 1_228;
pub const WRITTEN_S: u64 = 1_791_541_080;
pub const NOW_F_S: u64 = 1_791_541_092;
pub const VERIFIED_S: u64 = 1_791_541_310;
pub const ADMISSION_NOW_S: u64 = 1_791_541_360;
pub const ADMISSION_EXP_S: u64 = 1_791_541_440;
pub const OBJECT_BYTES: u64 = 186_432;
pub const OBJECT_SHA256: &str = "e7b3a1c04d9f6e28c5a0b17d3f84e6c29a15d0b7e4c38f6a1d2b5c90e7a4f18b";
const TIP_R_DISPLAY: &str = "00000000000000000000ab34cd56ef901234567890abcdef0123456789547f22";
const TIP_F_DISPLAY: &str = "00000000000000000000ab34cd56ef901234567890abcdef01234567892f4e50";
const PEERS: u32 = 9;

pub fn object_path() -> PathBuf {
    crate::config::home().join("PCOC/hikes/hike-2026-10-09-0704.cbor")
}

pub fn sha256() -> [u8; 32] {
    crate::hex::decode32(OBJECT_SHA256).unwrap()
}

fn natural(display: &str) -> [u8; 32] {
    crate::hex::display_order(&crate::hex::decode32(display).unwrap())
}

fn fake_natural(height: u64) -> [u8; 32] {
    let mut h = [0u8; 32];
    h[..8].copy_from_slice(&height.to_le_bytes());
    h[8] = 0x5a;
    h
}

/// `I_g` height.
pub fn interrupt(g: usize) -> u64 {
    SEED_HEIGHT + 1 + g as u64
}

/// Section 2.7, every ask in the hike: `(g, height, outcome)`.
fn asks() -> Vec<(u64, u64, AskOutcome)> {
    use AskOutcome::{Miss, Served};
    let m = |s: &str| Miss(Some(s.to_owned()));
    vec![
        (1, 971_207, Served),
        (3, 971_209, Served),
        (4, 971_210, Served),
        (5, 971_211, Miss(None)),
        (5, 971_212, Served),
        (6, 971_212, Served),
        (7, 971_213, Served),
        (8, 971_214, Served),
        (9, 971_215, m("still in the pool")),
        (9, 971_216, m("sealed but not in a block yet")),
        (9, 971_217, m("not included")),
        (10, 971_216, Served),
        (11, 971_217, Served),
        (12, 971_218, Miss(None)),
        (12, 971_219, Served),
        (13, 971_219, Served),
        (14, 971_220, Served),
        (15, 971_221, Served),
        (16, 971_222, Served),
    ]
}

/// The sample hike's events up to `legs` stored legs, or to the object write.
fn events(until_legs: Option<u64>) -> Vec<ProduceEvent> {
    let ms = |s: u64| s * 1000;
    let total: u64 = LEGS.iter().sum();
    let first_leg_s = START_S + TERRAIN_FILL_S;
    // Legs are evenly spaced so that 1,228 legs span 3,982 s.
    let leg_ms = |i: u64| ms(first_leg_s) + i * 3_982_000 / (LEGS_AT_R - 1);
    let mut out = vec![
        ProduceEvent::Started {
            output: object_path(),
            public_key_hex: "d1cba4d0c5a5c90d6e10f96e2f49694df71b8862c6dcd7f5fd2015694deeefd2"
                .into(),
            at_ms: ms(START_S),
        },
        ProduceEvent::SeedBeacon {
            height: SEED_HEIGHT,
            natural_hash: fake_natural(SEED_HEIGHT),
            header_time_unix_s: START_S as u32,
            at_ms: ms(START_S),
        },
        ProduceEvent::Tip {
            height: SEED_HEIGHT,
            natural_hash: fake_natural(SEED_HEIGHT),
            peers: Some(PEERS),
            header_seen_unix_ms: ms(START_S),
        },
        ProduceEvent::Log("gossip from the mainnet DNS seeds".into()),
    ];
    let asks = asks();
    let mut global = 0u64;
    for (g, &n) in LEGS.iter().enumerate() {
        for j in 0..n {
            if until_legs == Some(global) {
                return out;
            }
            if global == 0 {
                out.push(ProduceEvent::TerrainFilled { at_ms: leg_ms(0) });
            }
            out.push(ProduceEvent::LegRecorded {
                stage: g,
                j,
                stored_in_stage: j + 1,
                at_ms: leg_ms(global),
            });
            global += 1;
        }
        let height = interrupt(g);
        let seen_ms = match height {
            971_210 => ms(NOW_R_S - 123),
            971_222 => ms(WRITTEN_S),
            _ => leg_ms(global - 1),
        };
        let hash = match height {
            971_210 => natural(TIP_R_DISPLAY),
            971_222 => natural(TIP_F_DISPLAY),
            h => fake_natural(h),
        };
        out.push(ProduceEvent::Tip {
            height,
            natural_hash: hash,
            peers: Some(PEERS),
            header_seen_unix_ms: seen_ms,
        });
        out.push(ProduceEvent::Log(format!(
            "block {height} tip {} (natural order)",
            crate::hex::short(&crate::hex::encode(&hash))
        )));
        out.push(ProduceEvent::StageClosed {
            stage: g,
            legs: n,
            interrupt_height: height,
        });
        let w = g as u64 + 1;
        if w <= 17 {
            let (appended, status) = if w == 2 {
                (false, None)
            } else {
                (true, Some(204))
            };
            out.push(ProduceEvent::WaypointPosted {
                g: w,
                appended,
                status,
            });
            out.push(ProduceEvent::Log(match status {
                Some(s) => format!("posted W_{w} status {s}"),
                None => format!("posted W_{w} did not connect"),
            }));
        }
        for (ag, at, outcome) in asks.iter().filter(|(_, at, _)| *at == height) {
            out.push(ProduceEvent::PathAsk {
                g: *ag,
                at_height: *at,
                outcome: outcome.clone(),
            });
        }
    }
    debug_assert_eq!(global, total);
    out.push(ProduceEvent::Settling);
    out.push(ProduceEvent::PathLines(vec![
        "path_skip 2".into(),
        "path_skip 9".into(),
        "not fetched by design 17".into(),
    ]));
    out.push(ProduceEvent::ObjectWritten {
        path: object_path(),
        bytes: OBJECT_BYTES,
        sha256: sha256(),
        at_ms: ms(WRITTEN_S),
    });
    out
}

/// Moment R: recording stage 6, 36 legs stored in it, 1,228 in all.
pub fn moment_r() -> Vec<ProduceEvent> {
    events(Some(LEGS_AT_R))
}

/// Moment F: the object is written.
pub fn moment_f() -> Vec<ProduceEvent> {
    events(None)
}

/// The section 14 record for the sample object (sections 2.2, 2.7 to 2.9).
pub fn record_json() -> String {
    let t = |height: u64| 1_791_528_600 + 600 * (height - SEED_HEIGHT);
    let block = |height: u64| json!({ "block_hash": crate::hex::encode(&fake_natural(height)), "block_height": height });
    let mut headers = vec![json!({
        "stored_hash": crate::hex::encode(&fake_natural(SEED_HEIGHT)),
        "stored_height": SEED_HEIGHT, "index_comparison": "match", "T": t(SEED_HEIGHT)
    })];
    for k in 1..=18u64 {
        let h = SEED_HEIGHT + k;
        headers.push(json!({ "stored_hash": crate::hex::encode(&fake_natural(h)), "stored_height": h, "index_comparison": "match", "T": t(h) }));
    }
    let checkpoints: Vec<Value> = LEGS
        .iter()
        .enumerate()
        .map(|(g, &n)| json!({ "g": g, "legs": (0..n).map(|j| json!({ "j": j, "result": "match" })).collect::<Vec<_>>() }))
        .collect();
    let waypoints: Vec<Value> = (1..=17)
        .map(|g| json!({ "g": g, "result": "match" }))
        .collect();
    let d = |g: u64| match g {
        5 | 12 => 3,
        _ => 2,
    };
    let mut paths: Vec<Value> = (1..=16u64)
        .map(|g| {
            if g == 2 || g == 9 {
                json!({ "g": g, "path_state": "skip", "plant": "n/a", "P_g": "not defined", "height_P_g": "not defined", "d_g": "not defined", "claimed_block": "not defined" })
            } else {
                let h = SEED_HEIGHT + g + d(g);
                json!({ "g": g, "path_state": "present", "plant": "match", "P_g": block(h), "height_P_g": h, "d_g": d(g), "claimed_block": block(h) })
            }
        })
        .collect();
    paths.push(
        json!({ "g": 17, "w17_path": "not fetched by design", "claimed_block": "not defined" }),
    );
    let anchors: Vec<&str> = (1..=16)
        .map(|g| {
            if g == 2 || g == 9 {
                "not anchored"
            } else {
                "anchored"
            }
        })
        .collect();
    let draw =
        json!({ "indices": [3499, 3343, 816, 121, 2111, 1411], "results": vec!["match"; 6] });
    let per_stage: Vec<Value> = LEGS
        .iter()
        .enumerate()
        .map(|(g, &n)| json!({ "g": g, "checkpoint_count": n, "span": 600 }))
        .collect();
    let record = json!({
        "profile": "production",
        "sample_bytes": hike::SAMPLE_BYTES,
        "slot_bytes": hike::SLOT_BYTES,
        "terrain_bytes": crate::produce::TERRAIN_BYTES,
        "steps_per_leg": hike::STEPS_PER_LEG,
        "shape": "match",
        "shape_item": [],
        "headers": headers,
        "seed_value": "match",
        "checkpoints": checkpoints,
        "waypoints": waypoints,
        "paths": paths,
        "anchors": anchors,
        "anchored_total": 14,
        "skip_list": [2, 9],
        "a": 1, "b": 16, "N": 3094, "S": 10200, "delta_h": 17,
        "fixed_start": { "N": 3094, "S": 10200, "delta_h": 17 },
        "P_b": block(971_222),
        "height_P_b": 971_222,
        "index_comparison_P_b": "match",
        "T_P_b": t(971_222),
        "height_I_a_minus_1": 971_205,
        "T_I_a_minus_1": t(971_205),
        "six_leg": draw.clone(),
        "six_leg_runs": [draw],
        "analysis": { "whole_hike_legs": 3382, "whole_hike_span": 10200, "per_stage": per_stage, "legs_outside": 465 }
    });
    let mut text = serde_json::to_string(&record).unwrap();
    text.push('\n');
    text
}

/// The verification in section 2.8: 46 s, 6 threads, 33 reads at 88 ms.
pub fn verify_run() -> VerifyRun {
    let json = record_json();
    let presented: Presented = present(&json).unwrap();
    VerifyRun {
        path: object_path(),
        outcome: Outcome::Record {
            json,
            presented,
            bytes: OBJECT_BYTES,
            sha256: sha256(),
        },
        reads: IndexReads {
            issued: 33,
            answered: 33,
            total_ms: 33 * 88,
        },
        elapsed_s: 46,
        threads: 6,
        finished_unix_s: VERIFIED_S,
    }
}

/// Section 2.11.
pub fn admission_link() -> String {
    format!(
        "https://groundinglab.io/sign-in/?v=1&nonce={}&exp={ADMISSION_EXP_S}",
        "0123456789abcdef".repeat(4)
    )
}

/// Section 2.10. `(file, second line)` at moment R or F.
pub fn library_rows(moment_f: bool) -> Vec<(String, String)> {
    vec![
        (
            "hike-2026-10-09-0704.cbor".into(),
            if moment_f {
                "3,559 legs".into()
            } else {
                "in progress · not written yet".into()
            },
        ),
        (
            "hike-2026-10-06-2131.cbor".into(),
            "6 Oct 21:31 UTC · 3,212 legs".into(),
        ),
        ("alice-hike-1004.cbor".into(), "opened · other npub".into()),
        ("run-b-stages18.cbor".into(), "earlier version".into()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{clock_hm, duration_hm, duration_hms, thousands};
    use crate::produce::cells::{Cell, Row};
    use crate::produce::model::{Phase, ProduceView};

    fn fold(events: Vec<ProduceEvent>) -> ProduceView {
        let mut v = ProduceView::default();
        for e in events {
            v.apply(e);
        }
        v
    }

    #[test]
    fn moment_r_figures() {
        let v = fold(moment_r());
        let now = NOW_R_S * 1000;
        assert_eq!(v.phase, Phase::Recording);
        assert_eq!(
            v.hero_stage(),
            (
                "Stage 6".into(),
                "leg 37 in progress".into(),
                "6 of 18 stages recorded".into()
            )
        );
        assert_eq!(thousands(v.legs_total), "1,228");
        assert_eq!(v.rate(), Some(("0.308".into(), 3_982)));
        assert_eq!(v.tip_height, Some(971_210));
        assert_eq!(v.tip_age_s(now), Some(123));
        assert_eq!(duration_hms(123), "2 m 03 s");
        let e = v.estimate(now).unwrap();
        assert_eq!(e.headers_to_come, 12);
        assert_eq!(duration_hm(e.point_s), "1 h 57 m");
        assert_eq!(
            (duration_hm(e.low_s), duration_hm(e.high_s)),
            ("1 h 18 m".into(), "2 h 46 m".into())
        );
        assert_eq!(clock_hm(e.finish_unix_s(NOW_R_S)), "10:08");
        assert_eq!(v.sidebar_badge().as_deref(), Some("6 of 18"));
        assert_eq!(v.post_summary(), "5 of 6 posts appended 204");
        assert_eq!(v.stages[6].legs, Some(36));
        assert_eq!(v.stages[7].legs, None);

        let (gossip, bari) = v.status_line(now, false);
        assert_eq!(gossip, "Bitcoin gossip · 9 peers · tip 971,210 · 00000000…547f22 display · last block 2 m 03 s ago");
        assert_eq!(bari, "Bari reachable · last answered POST 204");
        assert_eq!(v.status_word(now), v.stage_line());
        assert!(v.stage_line().starts_with("Stage 6 of 18"));
        assert!(v.stage_line().contains("971210"));

        assert_eq!(
            v.waypoint_row(1),
            Row::Cells([Cell::Served, Cell::NotAskedStored, Cell::NotAskedStored])
        );
        assert_eq!(v.waypoint_row(2), Row::Skip);
        assert_eq!(
            v.waypoint_row(4),
            Row::Cells([Cell::Served, Cell::NotAskedStored, Cell::NotAskedStored])
        );
        assert_eq!(
            v.waypoint_row(5),
            Row::Cells([Cell::StillDue, Cell::NotYetDue, Cell::NotYetDue])
        );
        assert_eq!(
            v.waypoint_row(6),
            Row::Cells([Cell::NotYetDue, Cell::NotYetDue, Cell::NotYetDue])
        );
        assert_eq!(v.waypoint_row(7), Row::NotPosted);
    }

    #[test]
    fn moment_f_figures() {
        let v = fold(moment_f());
        assert_eq!(v.phase, Phase::Written);
        assert_eq!(v.stages_recorded(), 18);
        assert_eq!(v.legs_total, 3_559);
        assert_eq!(v.rate(), None);
        assert!(v.estimate(NOW_F_S * 1000).is_none());
        assert_eq!(duration_hms(v.wall_s().unwrap()), "3 h 13 m 29 s");
        assert_eq!(
            v.post_summary(),
            "16 of 17 waypoints posted · W2 did not connect"
        );
        assert_eq!(
            v.path_lines,
            vec!["path_skip 2", "path_skip 9", "not fetched by design 17"]
        );
        assert_eq!(v.sidebar_badge(), None);
        assert_eq!(v.tip_age_s(NOW_F_S * 1000), Some(12));
        assert_eq!(
            v.waypoint_row(9),
            Row::Cells([Cell::Miss, Cell::Miss, Cell::Miss])
        );
        assert_eq!(
            v.waypoint_row(5),
            Row::Cells([Cell::Miss, Cell::Served, Cell::NotAskedStored])
        );
        assert_eq!(
            v.waypoint_row(15),
            Row::Cells([Cell::Served, Cell::NotAskedStored, Cell::AboveCutOff])
        );
        assert_eq!(
            v.waypoint_row(16),
            Row::Cells([Cell::Served, Cell::AboveCutOff, Cell::AboveCutOff])
        );
        assert_eq!(v.waypoints[&9].miss_phrases.len(), 3);
        assert_eq!(v.stages[11].legs, Some(1));
    }

    #[test]
    fn sample_record_adds_up() {
        let Presented::Record(r) = present(&record_json()).unwrap() else {
            panic!()
        };
        let ledger = r.ledger().unwrap();
        assert_eq!(ledger.total, 3_559);
        assert_eq!(
            ledger.outside_identity,
            "3,094 + 465 = 3,559 · 465 is stages 0, 16 and 17, outside the bookends"
        );
        assert_eq!(
            ledger.whole_identity,
            "3,094 + 108 + 180 = 3,382 · 3,382 is whole_hike_legs, stages 1–17"
        );
        assert_eq!(r.n_over_s().as_deref(), Some("0.303"));
        assert_eq!(r.headers.len(), 19);
        assert_eq!(r.header_sentence(), "19 stored headers, each index_comparison match, from seed 971,204 through I_17 at 971,222.");
        assert_eq!(r.plant_reads_measured(), 14);
        assert_eq!(
            r.anchor_sentence(),
            "anchored_total 14. skip_list [2, 9]. 14 anchored + 2 skips = 16 fetchable. The count is not a threshold. W_17 is not in it."
        );
        let six = r.six_leg.as_ref().unwrap();
        assert_eq!(
            (six[0].stage, six[0].j, six[0].in_stage),
            (Some(17), Some(120), Some(180))
        );
        assert_eq!((six[3].stage, six[3].j), (Some(0), Some(121)));
        assert!(crate::signin::parse_link(&admission_link()).is_ok());
    }
}
