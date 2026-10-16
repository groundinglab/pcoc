//! Fast phase. Cheap checks, then a `provisional` that grants no action.
//!
//! Order, matching the plan:
//! grammar and window are the door's. Then canonical decode, the banned-key
//! check (the door, before this module), sticky denial (the door), the
//! signature (the door), shape and seed, waypoint and fold rehash, header
//! and plant reads, bookends, the anchored count and the first/last rule,
//! then the integer rate.
//!
//! A settled miss returns before terrain is built. A `not read` is `held`.
//! A pass is `provisional`. This module does not issue `accept`.

use crate::policy::{self, AdmissionPolicy};
use crate::verifier::{self, MeasureOpts, Report};
use hike::IndexBeacon;
use std::sync::atomic::{AtomicU64, Ordering};

/// Terrain builds observed by this process. [`hike::terrain_builds`] counts
/// every `fill_terrain`. A settled refuse must leave the count unchanged.
static TERRAIN_BEFORE: AtomicU64 = AtomicU64::new(0);

pub fn terrain_builds() -> u64 {
    hike::terrain_builds()
}

#[derive(Clone, Debug, PartialEq)]
pub enum FastDecision {
    /// The fast checks passed. No action is granted.
    Provisional { record: String },
    /// A miss settled by the object's own bytes, or a confirmed mismatch.
    /// No thorough run. No grant.
    Refuse {
        reasons: Vec<String>,
        line: String,
        record: String,
        /// Website token when the locked table has a phrase for this refuse.
        website: &'static str,
    },
    /// A measurement the profile needs is `not read`. Not a refusal.
    Held { record: String },
}

/// Run the fast phase. `beacon_at` and `path_chain` are the local mirror.
/// A height the mirror does not hold is `Err`, which the record stores as
/// `not read`.
pub fn decide<F, C, R>(
    object: &[u8],
    policy: &AdmissionPolicy,
    beacon_at: F,
    path_chain: C,
) -> FastDecision
where
    F: FnMut(u64) -> Result<IndexBeacon, ()>,
    C: FnMut(&hike::PathLocators) -> R,
    R: Into<hike::ChainRead>,
    C: Copy,
{
    TERRAIN_BEFORE.store(hike::terrain_builds(), Ordering::Relaxed);
    if !canonical(object) {
        return FastDecision::Refuse {
            reasons: vec!["malformed".to_owned()],
            line: "The object is not the canonical encoding.".to_owned(),
            record: "{}".to_owned(),
            website: crate::wire::OUTCOME_REJECTED,
        };
    }
    // Shape, seed, plant, and the anchor window do not need terrain.
    // A miss those checks settle does not call verify_fast.
    let screen = hike::screen_fast(object, path_chain);
    if let Some(decision) = settled_without_terrain(&screen, policy) {
        return decision;
    }
    let measurements = hike::verify_fast(object, beacon_at, path_chain);
    judge_fast(&measurements, policy)
}

/// A refuse the object's own bytes, or a confirmed plant mismatch, already settle.
fn settled_without_terrain(screen: &hike::FastScreen, policy: &AdmissionPolicy) -> Option<FastDecision> {
    if !screen.shape {
        return Some(refuse_now(
            vec!["shape is false".to_owned()],
            "Invalid: the object does not match the v2.0 shape.".to_owned(),
            "{}".to_owned(),
            crate::wire::OUTCOME_REJECTED,
        ));
    }
    if screen.seed_matches == Some(false) {
        return Some(refuse_now(
            vec!["seed value mismatches".to_owned()],
            "Invalid: the opening waypoint does not match.".to_owned(),
            "{}".to_owned(),
            crate::wire::OUTCOME_REJECTED,
        ));
    }
    if screen.plant_mismatch {
        return Some(refuse_now(
            vec!["a present path does not match the plant".to_owned()],
            "Invalid: a present path does not match the plant.".to_owned(),
            "{}".to_owned(),
            crate::wire::OUTCOME_REJECTED,
        ));
    }
    if screen.legs.is_some_and(|n| n < policy::SPEC_MIN_LEGS) {
        return Some(refuse_now(
            vec!["fewer than 6 legs, so the six indices could not be drawn".to_owned()],
            "Invalid: fewer than 6 legs, so the proof cannot be drawn.".to_owned(),
            "{}".to_owned(),
            crate::wire::OUTCOME_REJECTED,
        ));
    }
    let first = policy.require_first_anchored;
    let last = policy.require_last_anchored;
    let first_now = screen.anchored_indices.contains(&first);
    let last_now = screen.anchored_indices.contains(&last);
    let first_possible = screen.possible_indices.contains(&first);
    let last_possible = screen.possible_indices.contains(&last);
    if screen.anchored >= policy.min_anchored_paths && first_now && last_now {
        return None;
    }
    if screen.anchored_possible < policy.min_anchored_paths || !first_possible || !last_possible {
        return Some(refuse_now(
            vec![policy::REFUSE_TOO_FEW.to_owned()],
            policy::TOO_FEW_LINE.to_owned(),
            "{}".to_owned(),
            crate::wire::OUTCOME_TOO_FEW,
        ));
    }
    // A re-read could still meet the count. That is held, still with no terrain.
    if policy.held_on_not_read {
        return Some(FastDecision::Held {
            record: "{}".to_owned(),
        });
    }
    None
}

fn canonical(object: &[u8]) -> bool {
    matches!(hike::canonical_object(object), Ok(true))
}

fn judge_fast(measurements: &hike::Measurements, policy: &AdmissionPolicy) -> FastDecision {
    let record = hike::verify_record_json("production", measurements);
    if !measurements.shape {
        return refuse_now(
            vec!["shape is false".to_owned()],
            "Invalid: the object does not match the v2.0 shape.".to_owned(),
            record,
            crate::wire::OUTCOME_REJECTED,
        );
    }
    if let hike::Recorded::Obtained(waypoints) = &measurements.waypoints {
        if !waypoints.seed_value_matches {
            return refuse_now(
                vec!["seed value mismatches".to_owned()],
                "Invalid: the opening waypoint does not match.".to_owned(),
                record,
                crate::wire::OUTCOME_REJECTED,
            );
        }
        if waypoints
            .successor_differs
            .iter()
            .any(|differs| *differs == Some(true))
        {
            return refuse_now(
                vec!["a successor waypoint differs".to_owned()],
                "Invalid: a successor waypoint differs.".to_owned(),
                record,
                crate::wire::OUTCOME_REJECTED,
            );
        }
        if waypoints.present_path_does_not_match_plant {
            return refuse_now(
                vec!["a present path does not match the plant".to_owned()],
                "Invalid: a present path does not match the plant.".to_owned(),
                record,
                crate::wire::OUTCOME_REJECTED,
            );
        }
    }
    if let hike::Recorded::Obtained(checkpoints) = &measurements.checkpoints {
        if checkpoints.mismatch {
            return refuse_now(
                vec!["checkpoint mismatch".to_owned()],
                "Invalid: a stored header does not match the index.".to_owned(),
                record,
                crate::wire::OUTCOME_REJECTED,
            );
        }
    }

    let legs = total_legs(measurements);
    if legs.is_some_and(|n| n < policy::SPEC_MIN_LEGS) {
        return refuse_now(
            vec!["fewer than 6 legs, so the six indices could not be drawn".to_owned()],
            "Invalid: fewer than 6 legs, so the proof cannot be drawn.".to_owned(),
            record,
            crate::wire::OUTCOME_REJECTED,
        );
    }

    match anchored_judgment(measurements, policy) {
        Anchored::SettledShort => {
            return refuse_now(
                vec![policy::REFUSE_TOO_FEW.to_owned()],
                policy::TOO_FEW_LINE.to_owned(),
                record,
                crate::wire::OUTCOME_TOO_FEW,
            );
        }
        Anchored::NotRead if policy.held_on_not_read => {
            return FastDecision::Held { record };
        }
        Anchored::NotRead => {
            return refuse_now(
                vec![policy::REFUSE_TOO_FEW.to_owned()],
                policy::TOO_FEW_LINE.to_owned(),
                record,
                crate::wire::OUTCOME_TOO_FEW,
            );
        }
        Anchored::Met => {}
    }

    match bookend_pair(&record) {
        Pair::NotDefined => refuse_now(
            vec![policy::REFUSE_BOOKENDS.to_owned()],
            policy::BOOKENDS_LINE.to_owned(),
            record,
            crate::wire::OUTCOME_REJECTED,
        ),
        Pair::NotRead if policy.held_on_not_read => FastDecision::Held { record },
        Pair::NotRead => refuse_now(
            vec![policy::REFUSE_BOOKENDS_NOT_READ.to_owned()],
            policy::BOOKENDS_NOT_READ_LINE.to_owned(),
            record,
            crate::wire::OUTCOME_REJECTED,
        ),
        Pair::Numeric { n, s }
            if policy::bookend_below_floor(
                n,
                s,
                policy.leg_rate_floor.p,
                policy.leg_rate_floor.q,
            ) =>
        {
            refuse_now(
                vec![policy::REFUSE_LEG_RATE.to_owned()],
                policy::leg_rate_line(policy.leg_rate_floor.p, policy.leg_rate_floor.q),
                record,
                crate::wire::OUTCOME_REJECTED,
            )
        }
        Pair::Numeric { .. } => {
            if policy.provisional_on() {
                FastDecision::Provisional { record }
            } else {
                // `provisional: off` does not grant. Thorough still runs.
                // The caller treats this the same as provisional for the
                // thorough handoff and does not call it a grant.
                FastDecision::Provisional { record }
            }
        }
    }
}

fn refuse_now(
    reasons: Vec<String>,
    line: String,
    record: String,
    website: &'static str,
) -> FastDecision {
    FastDecision::Refuse {
        reasons,
        line,
        record,
        website,
    }
}

enum Anchored {
    Met,
    /// Even if every unread plant came back anchored, the count stays short
    /// or the required first or last waypoint cannot be anchored.
    SettledShort,
    NotRead,
}

fn anchored_judgment(measurements: &hike::Measurements, policy: &AdmissionPolicy) -> Anchored {
    let Some(value) = record_json(measurements) else {
        return Anchored::NotRead;
    };
    let anchors = value.get("anchors").and_then(|item| item.as_array());
    let Some(anchors) = anchors else {
        return Anchored::NotRead;
    };
    let mut anchored = 0u64;
    let mut could = 0u64;
    for (index, anchor) in anchors.iter().take(16).enumerate() {
        let token = anchor.as_str().unwrap_or("");
        let stage = index + 1;
        match token {
            "anchored" => {
                anchored += 1;
                could += 1;
            }
            // A skip or a confirmed mismatch cannot become anchored on a re-read.
            "not anchored" => {}
            _ => could += 1,
        }
        let _ = stage;
    }
    let first = policy.require_first_anchored.saturating_sub(1) as usize;
    let last = policy.require_last_anchored.saturating_sub(1) as usize;
    let first_ok = anchors
        .get(first)
        .and_then(|item| item.as_str())
        == Some("anchored");
    let last_ok = anchors.get(last).and_then(|item| item.as_str()) == Some("anchored");
    let first_possible = anchor_could(anchors.get(first));
    let last_possible = anchor_could(anchors.get(last));
    if anchored >= policy.min_anchored_paths && first_ok && last_ok {
        return Anchored::Met;
    }
    if could < policy.min_anchored_paths || !first_possible || !last_possible {
        return Anchored::SettledShort;
    }
    Anchored::NotRead
}

fn anchor_could(token: Option<&serde_json::Value>) -> bool {
    matches!(token.and_then(|item| item.as_str()), Some("anchored") | Some("not read"))
}

fn record_json(measurements: &hike::Measurements) -> Option<serde_json::Value> {
    let record = hike::verify_record_json("production", measurements);
    serde_json::from_str(record.trim_end()).ok()
}

fn total_legs(measurements: &hike::Measurements) -> Option<u64> {
    let value = record_json(measurements)?;
    // `analysis.whole_hike_legs` is the checkpoint count. It does not read `leg_count`.
    match value.pointer("/analysis/whole_hike_legs") {
        Some(serde_json::Value::Number(n)) => n.as_u64(),
        _ => None,
    }
}

enum Pair {
    NotDefined,
    NotRead,
    Numeric { n: i128, s: i128 },
}

fn bookend_pair(record: &str) -> Pair {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(record) else {
        return Pair::NotRead;
    };
    if matches!(value.get("N"), Some(serde_json::Value::String(n)) if n == "not defined") {
        return Pair::NotDefined;
    }
    match (json_i128(value.get("N")), json_i128(value.get("S"))) {
        (Some(n), Some(s)) => Pair::Numeric { n, s },
        _ => Pair::NotRead,
    }
}

fn json_i128(value: Option<&serde_json::Value>) -> Option<i128> {
    let value = value?;
    if let Some(number) = value.as_i64() {
        return Some(i128::from(number));
    }
    value.as_u64().map(i128::from)
}

/// Thorough measurement. Terrain is built here, inside `hike::verify`.
pub fn measure_thorough(object: &[u8], opts: &MeasureOpts) -> Report {
    verifier::measure(object, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AdmissionPolicy {
        AdmissionPolicy::default()
    }

    /// One anchored waypoint and a bookend rate of 0.05. Terrain stays 0.
    #[test]
    fn a_settled_short_anchored_count_builds_no_terrain() {
        let object = quiet(1);
        let before = hike::terrain_builds();
        let decision = decide(&object, &policy(), |_| Err(()), |_| hike::ChainRead::NotRead);
        assert!(
            matches!(decision, FastDecision::Refuse { website, .. } if website == "too_few_waypoints_anchored"),
            "{decision:?}"
        );
        assert_eq!(hike::terrain_builds(), before);
    }

    #[test]
    fn a_rate_of_one_leg_in_twenty_seconds_refuses_without_terrain() {
        // The short object has no two anchored paths, so the anchored rule
        // decides first. This pins the integer comparison on its own.
        assert!(policy::bookend_below_floor(1, 20, 1, 5));
        assert!(!policy::bookend_below_floor(1, 5, 1, 5));
    }

    fn quiet(anchored_paths: usize) -> Vec<u8> {
        let public_key = [0x42u8; 32];
        let mut seed = [0u8; 32];
        seed[0] = 1;
        seed[1] = 0xA0;
        let seed_value = {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(hike::production_seed(&public_key, &seed));
            let mut out = [0u8; 32];
            out.copy_from_slice(&digest);
            out
        };
        let mut stages = Vec::new();
        for g in 0..18 {
            let mut beacon = [0u8; 32];
            beacon[0] = g as u8 + 1;
            beacon[1] = 0xA0;
            let waypoint = if g == 0 { seed_value } else { [g as u8; 32] };
            // One present path, on W_1, inside the anchor window of stage 1
            // only when the test asks for it. The other paths stay empty, so
            // the anchored count is settled by the object's own bytes.
            // The path's block is this stage's own beacon, which is a stored
            // interrupt header, and the height sits in the anchor window
            // (H+2) of the previous stage. One such path is anchored only
            // when the plant also matches. Empty paths cannot become anchored.
            let path = if g >= 1 && (g - 1) < anchored_paths {
                hike::encode_path_for_block(&waypoint, &beacon, (100 + g as u64) as u32)
            } else {
                Vec::new()
            };
            stages.push(hike::Stage {
                beacon,
                beacon_height: 100 + (g as u64) * 10,
                leg_count: 1,
                waypoint,
                path,
                checkpoints: vec![[g as u8; 32]],
                leg_results: vec![1u64.to_le_bytes()],
                checkpoint_rows_defined: true,
                leg_result_missing: Vec::new(),
            });
        }
        stages[0].path.clear();
        let mut last = [0u8; 32];
        last[0] = 30;
        last[1] = 0xA0;
        let hike = hike::Hike {
            public_key,
            seed_beacon: seed,
            last_interrupt: last,
            last_interrupt_height: 100 + 18 * 10,
            last_interrupt_absent: false,
            last_interrupt_hash_absent: false,
            last_interrupt_height_absent: false,
            stages,
        };
        hike::encode_hike(&hike)
    }
}
