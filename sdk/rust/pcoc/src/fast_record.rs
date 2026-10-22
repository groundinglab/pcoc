//! SPEC §14 `hike_legs` and `hike_span` for the fast phase.
//!
//! SPEC §15.3: the fast phase never recomputes a leg and omits `six_leg`.
//! This module does not fill a terrain, does not draw, and does not call
//! [`crate::step::leg_result`]. The median-time rule is the same one §14
//! uses (erratum E26: do not link previous-hash).

use crate::constants::HEIGHT_MAX;
use crate::object::HikeObject;

/// Checkpoint count over stages 0–17. `None` when the object is not 18 stages
/// (`not defined` on the §14 record).
pub fn hike_legs(obj: &HikeObject) -> Option<i128> {
    if obj.stages.len() != 18 {
        return None;
    }
    let n: usize = obj.stages.iter().map(|stage| stage.checkpoints.len()).sum();
    Some(n as i128)
}

/// Median header time at `height`. `read_time` returns the header's `time`
/// when that height was read. A missing time is `not read`.
pub fn mtp_at(height: u64, mut read_time: impl FnMut(u64) -> Option<i128>) -> Option<i128> {
    if height > HEIGHT_MAX {
        return None;
    }
    let n = std::cmp::min(11u64, height.saturating_add(1));
    let start = height + 1 - n;
    let mut times = Vec::with_capacity(n as usize);
    for h in start..=height {
        times.push(read_time(h)?);
    }
    times.sort();
    times.get((n as usize) / 2).copied()
}

/// `MTP(last) − MTP(first)`. `None` when either median was not read.
pub fn hike_span(
    first_height: u64,
    last_height: u64,
    mut read_time: impl FnMut(u64) -> Option<i128>,
) -> Option<i128> {
    let first = mtp_at(first_height, &mut read_time)?;
    let last = mtp_at(last_height, &mut read_time)?;
    Some(last - first)
}
