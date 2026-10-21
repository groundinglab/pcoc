//! Admission rate. Total legs over every stage, divided by the median-time-past
//! span from the first beacon to the last beacon. MTP is the BIP113 median of
//! portable-index `header.time` over the 11 blocks ending at that beacon.
//!
//! The floor is policy. `1/5` (0.2 legs/s) is a test placeholder and is not ratified.
//! There is no first-or-last planted rule and no leg-count minimum in this judgment.

use serde_json::Value;

/// `p/q = 1/5` = 0.2 legs/s. Testing only.
pub const PLACEHOLDER_FLOOR_P: u64 = 1;
pub const PLACEHOLDER_FLOOR_Q: u64 = 5;
pub const PLACEHOLDER_FLOOR_NOTE: &str = "placeholder, not ratified";

/// Anchored waypoints required anywhere in the proof. Not a first or last index.
pub const MIN_ANCHORED_WAYPOINTS: u64 = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeaconRate {
    pub legs: Option<u64>,
    pub span_s: Option<i128>,
    pub first_height: Option<u64>,
    pub last_height: Option<u64>,
    pub first_time: Option<u64>,
    pub last_time: Option<u64>,
    pub anchored: Option<u64>,
    /// Checkpoint counts were not all numbers. The object's bytes settled that.
    pub legs_not_defined: bool,
    /// A beacon time was not read from the index.
    pub times_not_read: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateJudgment {
    Clear,
    /// Below the floor, or fewer than [`MIN_ANCHORED_WAYPOINTS`] anchored, or legs not defined.
    Refuse(&'static str),
    /// A time was not read. Not a refuse.
    Held,
}

/// `q·legs ≥ p·span` and `span > 0`. A non-positive span does not meet the floor.
pub fn below_floor(legs: u64, span_s: i128, p: u64, q: u64) -> bool {
    if span_s <= 0 {
        return true;
    }
    let (Ok(legs), Ok(span)) = (u128::try_from(legs), u128::try_from(span_s)) else {
        return true;
    };
    match (u128::from(q).checked_mul(legs), u128::from(p).checked_mul(span)) {
        (Some(left), Some(right)) => left < right,
        _ => true,
    }
}

/// Read the rate from a section 14 record.
///
/// Legs are the sum of `analysis.per_stage[].checkpoint_count` over every stage.
/// The span is `analysis.hike_span` when the record has it: MTP of the last
/// beacon minus MTP of the first. Those beacons are the seed beacon and
/// `last_interrupt`. A record without that field falls back to raw `T`.
pub fn beacon_rate(record: &str) -> BeaconRate {
    let Ok(value) = serde_json::from_str::<Value>(record.trim()) else {
        return unread();
    };
    let (legs, legs_not_defined) = hike_legs_of(&value).unwrap_or_else(|| sum_legs(&value));
    let headers = value.get("headers").and_then(Value::as_array);
    let (first_time, last_time, header_times_missing) = beacon_times(headers);
    let (span_s, times_not_read) = match hike_span_of(&value) {
        Some(span) => span,
        None => {
            let span = match (first_time, last_time) {
                (Some(first), Some(last)) => Some(i128::from(last) - i128::from(first)),
                _ => None,
            };
            (span, header_times_missing)
        }
    };
    let first_height = header_height(headers, 0);
    let last_height = headers.and_then(|items| {
        if items.is_empty() {
            None
        } else {
            header_height(Some(items), items.len() - 1)
        }
    });
    let anchored = value.get("anchored_total").and_then(Value::as_u64);
    BeaconRate {
        legs,
        span_s,
        first_height,
        last_height,
        first_time,
        last_time,
        anchored,
        legs_not_defined,
        times_not_read,
    }
}

pub fn judge_beacon_rate(rate: &BeaconRate, floor_p: u64, floor_q: u64) -> RateJudgment {
    if rate.legs_not_defined {
        return RateJudgment::Refuse("legs_not_defined");
    }
    if rate.times_not_read || rate.legs.is_none() || rate.span_s.is_none() {
        return RateJudgment::Held;
    }
    let legs = rate.legs.expect("legs");
    let span = rate.span_s.expect("span");
    if below_floor(legs, span, floor_p, floor_q) {
        return RateJudgment::Refuse("leg_rate_floor");
    }
    match rate.anchored {
        Some(anchored) if anchored >= MIN_ANCHORED_WAYPOINTS => RateJudgment::Clear,
        Some(_) => RateJudgment::Refuse("too_few_waypoints_anchored"),
        None => RateJudgment::Held,
    }
}

fn unread() -> BeaconRate {
    BeaconRate {
        legs: None,
        span_s: None,
        first_height: None,
        last_height: None,
        first_time: None,
        last_time: None,
        anchored: None,
        legs_not_defined: false,
        times_not_read: true,
    }
}

/// `Some((legs, not_defined))` when the record carries `analysis.hike_legs`.
fn hike_legs_of(value: &Value) -> Option<(Option<u64>, bool)> {
    let field = value.pointer("/analysis/hike_legs")?;
    Some(match field {
        Value::Number(number) => (number.as_u64(), false),
        Value::String(text) if text == "not read" => (None, false),
        _ => (None, true),
    })
}

/// `Some((span, times_not_read))` when the record carries `analysis.hike_span`.
fn hike_span_of(value: &Value) -> Option<(Option<i128>, bool)> {
    let field = value.pointer("/analysis/hike_span")?;
    Some(match field {
        Value::Number(number) => (
            number
                .as_i64()
                .map(i128::from)
                .or_else(|| number.as_u64().map(i128::from)),
            false,
        ),
        _ => (None, true),
    })
}

fn sum_legs(value: &Value) -> (Option<u64>, bool) {
    let Some(stages) = value
        .pointer("/analysis/per_stage")
        .and_then(Value::as_array)
    else {
        return (None, false);
    };
    if stages.is_empty() {
        return (None, true);
    }
    let mut total = 0u64;
    let mut unread = false;
    for stage in stages {
        match stage.get("checkpoint_count") {
            Some(Value::Number(number)) => {
                let Some(count) = number.as_u64() else {
                    return (None, true);
                };
                total = total.saturating_add(count);
            }
            Some(Value::String(text)) if text == "not read" => unread = true,
            _ => return (None, true),
        }
    }
    if unread {
        return (None, false);
    }
    (Some(total), false)
}

fn beacon_times(headers: Option<&Vec<Value>>) -> (Option<u64>, Option<u64>, bool) {
    let Some(headers) = headers else {
        return (None, None, true);
    };
    if headers.len() < 2 {
        return (None, None, true);
    }
    let first = json_time(headers.first());
    let last = json_time(headers.last());
    let missing = first.is_none() || last.is_none();
    (first, last, missing)
}

fn json_time(header: Option<&Value>) -> Option<u64> {
    let time = header?.get("T")?;
    time.as_u64().filter(|time| *time <= u64::from(u32::MAX))
}

fn header_height(headers: Option<&Vec<Value>>, index: usize) -> Option<u64> {
    headers?
        .get(index)?
        .get("stored_height")
        .and_then(Value::as_u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(legs: u64, span: u64, anchored: u64) -> String {
        let end = 1_000 + span;
        format!(
            r#"{{"anchored_total":{anchored},"headers":[{{"stored_height":10,"T":1000}},{{"stored_height":20,"T":{end}}}],"analysis":{{"per_stage":[{{"g":0,"checkpoint_count":{legs}}}]}}}}"#
        )
    }

    #[test]
    fn placeholder_floor_is_one_fifth_and_labelled() {
        assert_eq!(PLACEHOLDER_FLOOR_P, 1);
        assert_eq!(PLACEHOLDER_FLOOR_Q, 5);
        assert_eq!(PLACEHOLDER_FLOOR_NOTE, "placeholder, not ratified");
        assert!(!below_floor(1, 5, 1, 5));
        assert!(below_floor(1, 6, 1, 5));
    }

    #[test]
    fn rate_uses_every_leg_and_the_beacon_span() {
        let rate = beacon_rate(&record(10, 20, 6));
        assert_eq!(rate.legs, Some(10));
        assert_eq!(rate.span_s, Some(20));
        assert_eq!(rate.first_time, Some(1000));
        assert_eq!(rate.last_time, Some(1020));
        assert_eq!(
            judge_beacon_rate(&rate, PLACEHOLDER_FLOOR_P, PLACEHOLDER_FLOOR_Q),
            RateJudgment::Clear
        );
    }

    /// Six anchored waypoints anywhere meet the count. One leg is not a minimum miss
    /// when the beacon span still clears the floor.
    #[test]
    fn six_anchored_anywhere_and_no_leg_minimum() {
        let rate = beacon_rate(&record(1, 1, 6));
        assert_eq!(
            judge_beacon_rate(&rate, 1, 5),
            RateJudgment::Clear
        );
        let short = beacon_rate(&record(100, 1, 5));
        assert_eq!(
            judge_beacon_rate(&short, 1, 5),
            RateJudgment::Refuse("too_few_waypoints_anchored")
        );
    }

    #[test]
    fn an_unread_beacon_time_is_held_and_undefined_legs_refuse() {
        let unread = beacon_rate(
            r#"{"anchored_total":6,"headers":[{"T":"not read"},{"T":5}],"analysis":{"per_stage":[{"checkpoint_count":3}]}}"#,
        );
        assert!(unread.times_not_read);
        assert_eq!(judge_beacon_rate(&unread, 1, 5), RateJudgment::Held);
        let undefined = beacon_rate(
            r#"{"anchored_total":6,"headers":[{"T":1},{"T":2}],"analysis":{"per_stage":[{"checkpoint_count":"not defined"}]}}"#,
        );
        assert!(undefined.legs_not_defined);
        assert_eq!(
            judge_beacon_rate(&undefined, 1, 5),
            RateJudgment::Refuse("legs_not_defined")
        );
    }
}
