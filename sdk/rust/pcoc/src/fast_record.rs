//! SPEC §14 `hike_legs` and `hike_span` for the fast phase.
//!
//! SPEC §15.3: the fast phase never recomputes a leg and omits `six_leg`.
//! This module does not fill a terrain, does not draw, and does not call
//! [`crate::step::leg_result`].
//!
//! `hike_span` is raw Bitcoin header `nTime`: `header.time` at
//! `last_interrupt` minus `header.time` at the first interrupt `I_0`
//! (`stages[1].beacon`). Median-time-past is not this span. Stage 0 is not
//! in `hike_legs`. The seed beacon is not an endpoint.

use crate::object::HikeObject;

/// Checkpoint count over stages 1–17. Stage 0 is before the first interrupt
/// and is not an admission leg. `None` when the object is not 18 stages
/// (`not defined` on the §14 record).
pub fn hike_legs(obj: &HikeObject) -> Option<i128> {
    if obj.stages.len() != 18 {
        return None;
    }
    let n: usize = obj.stages[1..]
        .iter()
        .map(|stage| stage.checkpoints.len())
        .sum();
    Some(n as i128)
}

/// `nTime(last) − nTime(first)`. `read_time` returns `header.time` at that
/// height. `None` when either endpoint time was not read. No other height
/// is read.
pub fn hike_span(
    first_height: u64,
    last_height: u64,
    mut read_time: impl FnMut(u64) -> Option<i128>,
) -> Option<i128> {
    let first = read_time(first_height)?;
    let last = read_time(last_height)?;
    Some(last - first)
}
