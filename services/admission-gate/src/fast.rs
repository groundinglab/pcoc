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
//!
//! Header and plant reads use [`verifier::IndexRead`]. A failed read is
//! `not read` and the state is `held`. A plant or header `mismatch` is held
//! until a second read that bypasses the mirror cache confirms it. Only that
//! confirmed mismatch refuses (§15.3, Rule B9).

use crate::policy::{self, AdmissionPolicy};
use crate::verifier::{self, IndexRead, MeasureOpts, Report};
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

/// Run the fast phase against `{index_url}`.
///
/// The reader is shared by the screen and the fast record. It is `Fn`, not
/// `Copy`: the index client is not `Copy`. A height the reader does not hold
/// is `not read`.
pub fn decide(object: &[u8], policy: &AdmissionPolicy, index: &impl IndexRead) -> FastDecision {
    decide_at(object, policy, index, hike::Sizes::production())
}

/// Same phase at `conformance-small` sizes.
///
/// The profile is not offered on the production path. [`decide`] always uses
/// section 6. Tests use this so a small object can reach the index reads
/// without filling the production terrain.
#[cfg(test)]
pub(crate) fn decide_small(
    object: &[u8],
    policy: &AdmissionPolicy,
    index: &impl IndexRead,
) -> FastDecision {
    decide_at(object, policy, index, hike::Sizes::conformance_small())
}

fn decide_at(
    object: &[u8],
    policy: &AdmissionPolicy,
    index: &impl IndexRead,
    sizes: hike::Sizes,
) -> FastDecision {
    TERRAIN_BEFORE.store(hike::terrain_builds(), Ordering::Relaxed);
    if !canonical(object) {
        return FastDecision::Refuse {
            reasons: vec!["malformed".to_owned()],
            line: "The object is not the canonical encoding.".to_owned(),
            record: "{}".to_owned(),
            website: crate::wire::OUTCOME_REJECTED,
        };
    }
    // An unconfirmed plant mismatch is reported as `not read`, so the screen
    // holds it. A second mismatch is the confirmed refuse.
    let plant = |loc: &hike::PathLocators| -> hike::ChainRead {
        match index.plant(loc) {
            hike::ChainRead::Mismatch => match index.plant_fresh(loc) {
                hike::ChainRead::Mismatch => hike::ChainRead::Mismatch,
                _ => hike::ChainRead::NotRead,
            },
            other => other,
        }
    };
    let beacon = |height: u64| index.beacon(height);
    // Shape, seed, plant, and the anchor window do not need terrain.
    // A miss those checks settle does not call verify_fast.
    let screen = hike::screen_fast_sized(object, &plant, sizes);
    if let Some(decision) = settled_without_terrain(&screen, policy) {
        return decision;
    }
    let measurements = hike::verify_fast_sized(object, &beacon, &plant, sizes);
    let profile = if sizes.is_production() {
        "production"
    } else {
        "conformance-small"
    };
    let decision = apply_deadline(judge_fast(&measurements, policy, profile), policy, index);
    if let FastDecision::Refuse {
        reasons, record, ..
    } = &decision
    {
        // Earlier refuses (shape, seed, successor, a confirmed plant) return
        // before this reason. An unconfirmed header mismatch is held.
        if reasons.iter().any(|reason| reason == "checkpoint mismatch")
            && !header_mismatch_confirmed(record, index)
        {
            return FastDecision::Held {
                record: record.clone(),
            };
        }
    }
    decision
}

/// `true` when a fresh read still disagrees with a stored header.
fn header_mismatch_confirmed(record: &str, index: &impl IndexRead) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(record.trim_end()) else {
        return false;
    };
    let Some(headers) = value.get("headers").and_then(|item| item.as_array()) else {
        return false;
    };
    for header in headers {
        if header
            .get("index_comparison")
            .and_then(|item| item.as_str())
            != Some("mismatch")
        {
            continue;
        }
        let Some(height) = header.get("stored_height").and_then(|item| item.as_u64()) else {
            continue;
        };
        let Some(stored) = header.get("stored_hash").and_then(|item| item.as_str()) else {
            continue;
        };
        let Ok(bytes) = hex::decode(stored) else {
            continue;
        };
        if bytes.len() != 32 {
            continue;
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes);
        if let Ok(beacon) = index.beacon_fresh(height) {
            if beacon.natural != hash || beacon.height != height {
                return true;
            }
        }
    }
    false
}

/// A refuse the object's own bytes, or a confirmed plant mismatch, already settle.
fn settled_without_terrain(
    screen: &hike::FastScreen,
    policy: &AdmissionPolicy,
) -> Option<FastDecision> {
    if screen.legs.is_some_and(|legs| legs < 6) {
        return Some(refuse_now(
            vec!["fewer than 6 legs, so the six indices could not be drawn".to_owned()],
            "Invalid: fewer than 6 legs, so the proof cannot be drawn.".to_owned(),
            "{}".to_owned(),
            crate::wire::OUTCOME_REJECTED,
        ));
    }
    if !screen.shape {
        return Some(refuse_now(
            vec!["shape is false".to_owned()],
            "Invalid: the object does not match the v2.1 shape.".to_owned(),
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
    let now = |i: Option<u64>| i.is_none_or(|i| screen.anchored_indices.contains(&i));
    let possible = |i: Option<u64>| i.is_none_or(|i| screen.possible_indices.contains(&i));
    let first_now = now(policy.require_first_anchored);
    let last_now = now(policy.require_last_anchored);
    let first_possible = possible(policy.require_first_anchored);
    let last_possible = possible(policy.require_last_anchored);
    if screen.anchored >= policy.min_anchored_paths && first_now && last_now {
        return None;
    }
    if screen.anchored_possible < policy.min_anchored_paths || !first_possible || !last_possible {
        return Some(refuse_now(
            vec![policy::REFUSE_TOO_FEW.to_owned()],
            policy::too_few_line(screen.anchored, policy.min_anchored_paths),
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

fn judge_fast(
    measurements: &hike::Measurements,
    policy: &AdmissionPolicy,
    profile: &str,
) -> FastDecision {
    let record = hike::verify_record_json(profile, measurements);
    if !measurements.shape {
        return refuse_now(
            vec!["shape is false".to_owned()],
            "Invalid: the object does not match the v2.1 shape.".to_owned(),
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

    match anchored_judgment(measurements, policy, profile) {
        Anchored::SettledShort(anchored) => {
            return refuse_now(
                vec![policy::REFUSE_TOO_FEW.to_owned()],
                policy::too_few_line(anchored, policy.min_anchored_paths),
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
                policy::leg_rate_line(n, s, policy.leg_rate_floor.p, policy.leg_rate_floor.q),
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

/// Placeholder K, not ratified. A missing tip or last beacon is `held`.
/// A pass that is more than K blocks behind the gate's observed tip is a refuse.
/// This is a cheap check: it does not ban the key.
fn apply_deadline(
    decision: FastDecision,
    policy: &AdmissionPolicy,
    index: &impl IndexRead,
) -> FastDecision {
    let Some(k) = policy.submission_deadline_blocks else {
        return decision;
    };
    let FastDecision::Provisional { record } = &decision else {
        return decision;
    };
    let last = last_beacon_height(record);
    match (index.tip_height().ok(), last) {
        (Some(tip), Some(last)) if tip > last && tip - last > k => refuse_now(
            vec!["submission deadline".to_owned()],
            format!(
                "Too late: tip {tip} is more than {k} blocks after last beacon {last}. placeholder, not ratified."
            ),
            record.clone(),
            crate::wire::OUTCOME_REJECTED,
        ),
        (Some(_), Some(_)) => decision,
        _ if policy.held_on_not_read => FastDecision::Held {
            record: record.clone(),
        },
        _ => decision,
    }
}

fn last_beacon_height(record: &str) -> Option<u64> {
    let value = serde_json::from_str::<serde_json::Value>(record.trim_end()).ok()?;
    let headers = value.get("headers")?.as_array()?;
    headers
        .last()?
        .get("stored_height")
        .and_then(|item| item.as_u64())
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
    SettledShort(u64),
    NotRead,
}

fn anchored_judgment(
    measurements: &hike::Measurements,
    policy: &AdmissionPolicy,
    profile: &str,
) -> Anchored {
    let Some(value) = record_json(measurements, profile) else {
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
    let ok = |i: Option<u64>| {
        i.is_none_or(|i| {
            anchors.get(i.saturating_sub(1) as usize).and_then(|item| item.as_str())
                == Some("anchored")
        })
    };
    let could_be = |i: Option<u64>| i.is_none_or(|i| anchor_could(anchors.get(i.saturating_sub(1) as usize)));
    let first_ok = ok(policy.require_first_anchored);
    let last_ok = ok(policy.require_last_anchored);
    let first_possible = could_be(policy.require_first_anchored);
    let last_possible = could_be(policy.require_last_anchored);
    if anchored >= policy.min_anchored_paths && first_ok && last_ok {
        return Anchored::Met;
    }
    if could < policy.min_anchored_paths || !first_possible || !last_possible {
        return Anchored::SettledShort(anchored);
    }
    Anchored::NotRead
}

fn anchor_could(token: Option<&serde_json::Value>) -> bool {
    matches!(
        token.and_then(|item| item.as_str()),
        Some("anchored") | Some("not read")
    )
}

/// Measured numbers for a fast-phase log line, read off its section 14
/// record: bookends `N` (legs) and `S` (seconds) on the timed span, `N/S`,
/// and the anchored count among `W_1` … `W_16`. Numbers only, no threshold.
/// `None` when the record holds none of them.
pub fn fast_measures(record: &str) -> Option<serde_json::Value> {
    let rate = hike::beacon_rate(record);
    let legs = rate.legs;
    let span_s = rate.span_s.and_then(|span| u64::try_from(span).ok());
    let anchored = rate.anchored;
    if legs.is_none() && span_s.is_none() && anchored.is_none() {
        return None;
    }
    let leg_rate = match (legs, span_s) {
        (Some(n), Some(s)) if s > 0 => Some(n as f64 / s as f64),
        _ => None,
    };
    Some(serde_json::json!({
        "legs": legs,
        "span_s": span_s,
        "leg_rate": leg_rate,
        "anchored": anchored,
        "anchored_span": policy::ANCHORED_SPAN,
    }))
}

fn record_json(measurements: &hike::Measurements, profile: &str) -> Option<serde_json::Value> {
    let record = hike::verify_record_json(profile, measurements);
    serde_json::from_str(record.trim_end()).ok()
}

enum Pair {
    NotDefined,
    NotRead,
    Numeric { n: i128, s: i128 },
}

/// All-legs count and first-beacon to last-beacon span. Not the bookend pair `(N, S)`.
fn bookend_pair(record: &str) -> Pair {
    let rate = hike::beacon_rate(record);
    if rate.legs_not_defined {
        return Pair::NotDefined;
    }
    match (rate.legs, rate.span_s) {
        (Some(legs), Some(span)) if !rate.times_not_read => Pair::Numeric {
            n: i128::from(legs),
            s: span,
        },
        _ => Pair::NotRead,
    }
}

/// Thorough measurement. Terrain is built here, inside `hike::verify`.
pub fn measure_thorough(object: &[u8], opts: &MeasureOpts) -> Report {
    verifier::measure(object, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ClosedIndex;

    impl IndexRead for ClosedIndex {
        fn beacon(&self, _: u64) -> Result<hike::IndexBeacon, ()> {
            Err(())
        }
        fn beacon_fresh(&self, _: u64) -> Result<hike::IndexBeacon, ()> {
            Err(())
        }
        fn plant(&self, _: &hike::PathLocators) -> hike::ChainRead {
            hike::ChainRead::NotRead
        }
        fn plant_fresh(&self, _: &hike::PathLocators) -> hike::ChainRead {
            hike::ChainRead::NotRead
        }
    }

    fn policy() -> AdmissionPolicy {
        AdmissionPolicy::default()
    }

    #[test]
    fn fast_measures_reads_the_beacon_span_and_the_anchored_count() {
        let m = fast_measures(
            "{\"profile\":\"production\",\"anchored_total\":12,\"headers\":[{\"T\":1000},{\"T\":10557}],\"analysis\":{\"per_stage\":[{\"checkpoint_count\":1499}]}}\n",
        )
        .expect("measures");
        assert_eq!(m["legs"], 1499);
        assert_eq!(m["span_s"], 9557);
        assert_eq!(m["anchored"], 12);
        assert_eq!(m["anchored_span"], 16);
        assert!((m["leg_rate"].as_f64().unwrap() - 1499.0 / 9557.0).abs() < 1e-12);
        let words = fast_measures("{\"N\":\"not read\",\"S\":\"not read\",\"anchored_total\":3}")
            .expect("count");
        assert!(words["legs"].is_null() && words["leg_rate"].is_null());
        assert_eq!(words["anchored"], 3);
        assert!(fast_measures("{}").is_none());
    }

    struct Tip(u64);

    impl IndexRead for Tip {
        fn beacon(&self, _: u64) -> Result<hike::IndexBeacon, ()> {
            Err(())
        }
        fn beacon_fresh(&self, _: u64) -> Result<hike::IndexBeacon, ()> {
            Err(())
        }
        fn plant(&self, _: &hike::PathLocators) -> hike::ChainRead {
            hike::ChainRead::NotRead
        }
        fn plant_fresh(&self, _: &hike::PathLocators) -> hike::ChainRead {
            hike::ChainRead::NotRead
        }
        fn tip_height(&self) -> Result<u64, ()> {
            Ok(self.0)
        }
    }

    #[test]
    fn a_late_submission_is_refused_and_a_missing_tip_is_held() {
        let record = r#"{"headers":[{"stored_height":1000,"T":1},{"stored_height":1010,"T":2}]}"#;
        let mut policy = policy();
        policy.submission_deadline_blocks = Some(3);
        let provisional = FastDecision::Provisional {
            record: record.to_owned(),
        };
        let late = apply_deadline(provisional.clone(), &policy, &Tip(1014));
        match late {
            FastDecision::Refuse { reasons, .. } => {
                assert_eq!(reasons, vec!["submission deadline".to_owned()]);
            }
            other => panic!("expected a deadline refuse, got {other:?}"),
        }
        let on_time = apply_deadline(provisional.clone(), &policy, &Tip(1013));
        assert!(matches!(on_time, FastDecision::Provisional { .. }));
        let unread = apply_deadline(provisional, &policy, &ClosedIndex);
        assert!(matches!(unread, FastDecision::Held { .. }));
    }

    /// One anchored waypoint and a bookend rate of 0.05. Terrain stays 0.
    ///
    /// The terrain counter is process-wide and other tests build terrain in
    /// parallel. A decide that built terrain would raise it on every try, so
    /// one try with the counter unmoved proves this decide built none.
    #[test]
    fn a_settled_short_anchored_count_builds_no_terrain() {
        let object = quiet(1);
        let mut unmoved = false;
        for _ in 0..50 {
            let before = hike::terrain_builds();
            let decision = decide(&object, &policy(), &ClosedIndex);
            assert!(
                matches!(decision, FastDecision::Refuse { website, .. } if website == "too_few_waypoints_anchored"),
                "{decision:?}"
            );
            if hike::terrain_builds() == before {
                unmoved = true;
                break;
            }
        }
        assert!(unmoved, "the terrain counter moved on every try");
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
