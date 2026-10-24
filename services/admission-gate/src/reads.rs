//! SPEC §15.3 header and plant reads for the fast phase.
//!
//! The mirror is a cache of `{index-url}` bodies. A mismatch against a cached
//! body is `held` until a fresh read that bypasses the cache confirms it.
//! This module does not fill a terrain and does not recompute a leg.

use crate::policy::AdmissionPolicy;
use crate::verifier::IndexClient;
use pcoc::indexdoc::{self, HeaderDoc};
use pcoc::object::HikeObject;
use pcoc::path::{self, PathBody};

pub enum ReadStop {
    Held {
        record: String,
        tip: Option<u64>,
    },
    Refuse {
        reasons: Vec<String>,
        line: String,
        record: String,
    },
}

pub struct ReadView {
    pub record: String,
    /// Deciding tip: the first successful read with `tip >= last_beacon`.
    pub tip: Option<u64>,
}

enum DocFail {
    NotRead,
    Mismatch,
}

/// Header, plant, B2, and B4. `Ok` means every measurement was read and met.
/// Tip age past `last_interrupt_height` is not a refusal.
pub fn assess(
    object: &[u8],
    policy: &AdmissionPolicy,
    index: &IndexClient,
) -> Result<ReadView, ReadStop> {
    if let Some(lowest) = crate::policy::lowest_before_born(object, policy.born_height) {
        return Err(ReadStop::Refuse {
            reasons: vec![crate::policy::BEFORE_BORN.to_owned()],
            line: crate::policy::before_born_line(policy.born_height, lowest),
            record: crate::policy::before_born_record(policy.born_height, lowest),
        });
    }
    let obj = pcoc::decode_object(object).map_err(|_| ReadStop::Refuse {
        reasons: vec!["malformed".to_owned()],
        line: "The object is not the canonical encoding.".to_owned(),
        record: "{}".to_owned(),
    })?;
    let tip = deciding_tip(&obj, index)?;
    let mut times = std::collections::HashMap::<u64, i128>::new();
    compare_headers(&obj, index, &mut times).map_err(|stop| with_tip(stop, tip))?;
    let anchored = compare_plants(&obj, policy, index).map_err(|stop| with_tip(stop, tip))?;
    let (legs, span) = hike_pair(&obj, index, &mut times).map_err(|stop| with_tip(stop, tip))?;
    if pcoc::leg_rate_floor(
        legs,
        span,
        i128::from(policy.leg_rate_floor.p),
        i128::from(policy.leg_rate_floor.q),
    ) != "meets"
    {
        return Err(ReadStop::Refuse {
            reasons: vec![crate::policy::REFUSE_LEG_RATE.to_owned()],
            line: crate::policy::leg_rate_line(
                legs,
                span,
                policy.leg_rate_floor.p,
                policy.leg_rate_floor.q,
            ),
            record: record_json(legs, span, anchored),
        });
    }
    if anchored < policy.min_anchored_paths {
        return Err(ReadStop::Refuse {
            reasons: vec![crate::policy::REFUSE_TOO_FEW.to_owned()],
            line: crate::policy::too_few_line(anchored, policy.min_anchored_paths),
            record: record_json(legs, span, anchored),
        });
    }
    Ok(ReadView {
        record: record_json(legs, span, anchored),
        tip,
    })
}

/// Tip for the held-bound clock: the first successful read at or above
/// `last_interrupt_height`. A tip that is not read, or still below that
/// height, is `held`. SPEC 2.3 has no tip-versus-last-beacon deadline, so
/// a tip many blocks past that height is not a refusal.
fn deciding_tip(obj: &HikeObject, index: &IndexClient) -> Result<Option<u64>, ReadStop> {
    if !obj.has_last_height {
        return Err(held(None));
    }
    let last = obj.last_interrupt_height;
    let Some(body) = index.tip().ok() else {
        return Err(held(None));
    };
    let Some(height) = header_of(body.as_bytes()).map(|header| header.height) else {
        return Err(held(None));
    };
    if height < last {
        return Err(held(None));
    }
    Ok(Some(height))
}

fn compare_headers(
    obj: &HikeObject,
    index: &IndexClient,
    times: &mut std::collections::HashMap<u64, i128>,
) -> Result<(), ReadStop> {
    for stage in &obj.stages {
        let path = format!("/index/{}", stage.beacon_height);
        let body = match load_checked(index, &path, |text| {
            header_matches(text, stage.beacon, stage.beacon_height)
        }) {
            Ok(body) => body,
            Err(DocFail::NotRead) => return Err(held(None)),
            Err(DocFail::Mismatch) => {
                return Err(ReadStop::Refuse {
                    reasons: vec!["checkpoint mismatch".to_owned()],
                    line: "Invalid: the Bitcoin blocks it names do not match the public chain."
                        .to_owned(),
                    record: "{}".to_owned(),
                });
            }
        };
        if let Some(header) = header_of(body.as_bytes()) {
            if let Some(time) = header.time {
                times.insert(stage.beacon_height, i128::from(time));
            }
        }
    }
    if obj.has_last {
        let path = format!("/index/{}", obj.last_interrupt_height);
        let body = match load_checked(index, &path, |text| {
            header_matches(text, obj.last_interrupt, obj.last_interrupt_height)
        }) {
            Ok(body) => body,
            Err(DocFail::NotRead) => return Err(held(None)),
            Err(DocFail::Mismatch) => {
                return Err(ReadStop::Refuse {
                    reasons: vec!["checkpoint mismatch".to_owned()],
                    line: "Invalid: the Bitcoin blocks it names do not match the public chain."
                        .to_owned(),
                    record: "{}".to_owned(),
                });
            }
        };
        if let Some(header) = header_of(body.as_bytes()) {
            if let Some(time) = header.time {
                times.insert(obj.last_interrupt_height, i128::from(time));
            }
        }
    }
    Ok(())
}

fn compare_plants(
    obj: &HikeObject,
    policy: &AdmissionPolicy,
    index: &IndexClient,
) -> Result<u64, ReadStop> {
    let mut anchored = 0u64;
    let mut unread = 0u64;
    for stage in obj.stages.iter().skip(1).take(16) {
        let Some(bytes) = stage.path.as_ref() else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let Some(body) = path::decode_path(bytes) else {
            return Err(plant_refuse());
        };
        if !path::validate_for(&body, &stage.waypoint) {
            return Err(plant_refuse());
        }
        let block_h = u64::from(body.block_height);
        // SPEC §13.1 / P14.8. Outside the anchor window, or not a stored
        // interrupt, a re-read cannot anchor the path. It is not fetched.
        if pcoc::anchor_window(stage.beacon_height, block_h) != "anchored"
            || !stored_interrupt(obj, block_h, &body.block_hash)
        {
            continue;
        }
        let path = format!("/index/{block_h}");
        match load_checked(index, &path, |text| {
            plant_matches(text, &body, &stage.waypoint)
        }) {
            Ok(_) => anchored += 1,
            Err(DocFail::NotRead) => unread += 1,
            Err(DocFail::Mismatch) => return Err(plant_refuse()),
        }
    }
    // SPEC Rule B4. Hold only when a re-read of a `not read` plant could
    // still reach `min_anchored_paths`. A count already settled below the
    // lever refuses at once.
    if unread > 0 && anchored.saturating_add(unread) >= policy.min_anchored_paths {
        return Err(held(None));
    }
    Ok(anchored)
}

/// An interrupt header stored on the object at `height` with this natural hash.
/// Stage 0 is the seed beacon, not an interrupt header for a plant (verify's
/// `interrupt_match` starts at stage 1). `last_interrupt` is included.
fn stored_interrupt(obj: &HikeObject, height: u64, natural: &[u8; 32]) -> bool {
    let stage_hit = obj.stages.iter().skip(1).any(|stage| {
        !stage.height_over && stage.beacon_height == height && stage.beacon == *natural
    });
    stage_hit
        || (obj.has_last
            && obj.has_last_height
            && !obj.last_height_over
            && obj.last_interrupt_height == height
            && obj.last_interrupt == *natural)
}

fn hike_pair(
    obj: &HikeObject,
    index: &IndexClient,
    times: &mut std::collections::HashMap<u64, i128>,
) -> Result<(i128, i128), ReadStop> {
    let Some(legs) = pcoc::hike_legs(obj) else {
        return Err(held(None));
    };
    let Some(first) = obj.stages.first().map(|stage| stage.beacon_height) else {
        return Err(held(None));
    };
    if !obj.has_last_height {
        return Err(held(None));
    }
    let last = obj.last_interrupt_height;
    if !fill_times(index, first, times) || !fill_times(index, last, times) {
        return Err(held(None));
    }
    let Some(span) = pcoc::hike_span(first, last, |height| times.get(&height).copied()) else {
        return Err(held(None));
    };
    Ok((legs, span))
}

fn fill_times(
    index: &IndexClient,
    height: u64,
    times: &mut std::collections::HashMap<u64, i128>,
) -> bool {
    let n = std::cmp::min(pcoc::constants::MTP_WINDOW, height.saturating_add(1));
    let start = height + 1 - n;
    for h in start..=height {
        if times.contains_key(&h) {
            continue;
        }
        let path = format!("/index/{h}");
        let Ok(body) = index.get_cached(&path) else {
            return false;
        };
        let Some(header) = header_of(body.as_bytes()) else {
            return false;
        };
        let Some(time) = header.time else {
            return false;
        };
        times.insert(h, i128::from(time));
    }
    true
}

fn load_checked(
    index: &IndexClient,
    path: &str,
    check: impl Fn(&str) -> bool,
) -> Result<String, DocFail> {
    let body = index.get_cached(path).map_err(|_| DocFail::NotRead)?;
    if check(&body) {
        return Ok(body);
    }
    let fresh = index.confirm_fresh(path).map_err(|_| DocFail::NotRead)?;
    if check(&fresh) {
        Ok(fresh)
    } else {
        Err(DocFail::Mismatch)
    }
}

fn header_of(bytes: &[u8]) -> Option<HeaderDoc> {
    let doc = indexdoc::parse_document(bytes)?;
    indexdoc::read_header(&doc)
}

fn header_matches(text: &str, natural: [u8; 32], height: u64) -> bool {
    let Some(header) = header_of(text.as_bytes()) else {
        return false;
    };
    header.natural == natural && header.height == height
}

fn plant_matches(text: &str, body: &PathBody, waypoint: &[u8; 32]) -> bool {
    let Some(doc) = indexdoc::parse_document(text.as_bytes()) else {
        return false;
    };
    let Some(plants) = indexdoc::read_plants(&doc) else {
        return false;
    };
    plants.plants.iter().any(|plant| {
        plant.root == body.root
            && plant.txid == body.txid
            && plant.block_hash == body.block_hash
            && plant.block_height == body.block_height
            && path::validate_for(body, waypoint)
    })
}

fn with_tip(stop: ReadStop, tip: Option<u64>) -> ReadStop {
    match stop {
        ReadStop::Held { record, .. } => ReadStop::Held { record, tip },
        other => other,
    }
}

fn held(tip: Option<u64>) -> ReadStop {
    ReadStop::Held {
        record: "{}".to_owned(),
        tip,
    }
}

fn plant_refuse() -> ReadStop {
    ReadStop::Refuse {
        reasons: vec!["a present path does not match the plant".to_owned()],
        line: "Invalid: a present path does not match the plant.".to_owned(),
        record: "{}".to_owned(),
    }
}

fn record_json(legs: i128, span: i128, anchored: u64) -> String {
    serde_json::json!({
        "profile": "production",
        "shape": "match",
        "shape_item": [],
        "analysis": { "hike_legs": legs, "hike_span": span },
        "anchored_total": anchored,
    })
    .to_string()
}

#[cfg(test)]
mod stored_interrupt_tests {
    use super::{compare_plants, stored_interrupt};
    use crate::policy::AdmissionPolicy;
    use crate::verifier::IndexClient;
    use pcoc::object::{HikeObject, Stage};
    use std::time::Duration;

    fn stage(height: u64, beacon: [u8; 32], path: Option<Vec<u8>>) -> Stage {
        Stage {
            beacon,
            beacon_height: height,
            height_over: false,
            leg_count: 0,
            waypoint: [0xab; 32],
            path,
            checkpoints: Vec::new(),
            leg_results: Vec::new(),
            usable: true,
        }
    }

    fn object(stages: Vec<Stage>, last: [u8; 32], last_height: u64, last_over: bool) -> HikeObject {
        HikeObject {
            kind: pcoc::constants::KIND.to_owned(),
            fill: "dag2".to_owned(),
            slot_bytes: 0,
            n_slots: 0,
            sample_bytes: 0,
            steps_per_leg: 0,
            public_key: [0; 32],
            seed_beacon: [0; 32],
            stages,
            last_interrupt: last,
            last_interrupt_height: last_height,
            last_height_over: last_over,
            has_last: true,
            has_last_height: true,
            field_set: true,
            stage0_path: false,
            trailing: false,
            top_ok: true,
        }
    }

    /// One-leaf path. `block_hash` is the inclusion header in natural order.
    fn one_leaf(block_hash: &[u8; 32], block_height: u32) -> Vec<u8> {
        let waypoint = [0xabu8; 32];
        let root = pcoc::hash::sha256_parts(&[&[0x1au8], &waypoint]);
        let mut out = Vec::with_capacity(146);
        out.push(0x01);
        out.extend_from_slice(&waypoint);
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&1u32.to_be_bytes());
        out.push(0);
        out.extend_from_slice(&root);
        out.extend_from_slice(&[0x11; 32]);
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(block_hash);
        out.extend_from_slice(&block_height.to_be_bytes());
        out
    }

    #[test]
    fn stored_interrupt_matches_a_later_stage_or_the_close() {
        let beacon = [0x22u8; 32];
        let foreign = [0x09u8; 32];
        let mut stages = vec![
            stage(100, [0x01; 32], None),
            stage(100, [0x11; 32], None),
            stage(102, beacon, None),
        ];
        let obj = object(stages.clone(), [0x33; 32], 200, false);
        assert!(stored_interrupt(&obj, 102, &beacon));
        assert!(!stored_interrupt(&obj, 102, &foreign));
        // Stage 0 is the seed beacon, not an interrupt header.
        stages[0].beacon = beacon;
        stages[0].beacon_height = 102;
        stages[2].beacon = [0x44; 32];
        let seed_only = object(stages, [0x33; 32], 200, false);
        assert!(!stored_interrupt(&seed_only, 102, &beacon));
        let closed = object(
            vec![stage(100, [0x11; 32], None), stage(100, [0x11; 32], None)],
            beacon,
            102,
            false,
        );
        assert!(stored_interrupt(&closed, 102, &beacon));
        let over = object(
            vec![stage(100, [0x11; 32], None), stage(100, [0x11; 32], None)],
            beacon,
            102,
            true,
        );
        assert!(!stored_interrupt(&over, 102, &beacon));
    }

    #[test]
    fn a_path_whose_block_is_not_a_stored_interrupt_is_not_read() {
        let posted = 100u64;
        let foreign = [0x09u8; 32];
        let beacon = [0x22u8; 32];
        let path_stage = stage(
            posted,
            [0x11; 32],
            Some(one_leaf(&foreign, (posted + 2) as u32)),
        );
        let interrupt = stage(posted + 2, beacon, None);
        let obj = object(
            vec![stage(posted, [0x01; 32], None), path_stage, interrupt],
            [0x33; 32],
            200,
            false,
        );
        let mut policy = AdmissionPolicy::default();
        policy.min_anchored_paths = 1;
        // Closed port. A plant GET would fail inside the timeout. The skip
        // returns before that GET.
        let index = IndexClient::open("http://127.0.0.1:1", Duration::from_millis(200));
        match compare_plants(&obj, &policy, &index) {
            Ok(0) => {}
            Ok(n) => panic!("counted a path that is not a stored interrupt: {n}"),
            Err(_) => panic!("plant read issued for a path that is not a stored interrupt"),
        }

        let stored_path = stage(
            posted,
            [0x11; 32],
            Some(one_leaf(&beacon, (posted + 2) as u32)),
        );
        let stored = object(
            vec![
                stage(posted, [0x01; 32], None),
                stored_path,
                stage(posted + 2, beacon, None),
            ],
            [0x33; 32],
            200,
            false,
        );
        match compare_plants(&stored, &policy, &index) {
            Err(super::ReadStop::Held { .. }) => {}
            Ok(n) => panic!("stored interrupt did not reach the plant read, anchored {n}"),
            Err(super::ReadStop::Refuse { .. }) => {
                panic!("stored interrupt was refused before the unread plant read")
            }
        }
    }
}
