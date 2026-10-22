//! SPEC §13.1 anchor window and Rule B2 integer comparison.
//! These are the comparisons the candidate grid asks for. They are not a Gate.

use crate::constants::ANCHOR_WINDOW_OFFSETS;

/// SPEC Rule B2 (§15, line 1973). Integer comparison only.
///
/// `hike_span ≤ 0` and a numeric shortfall are `refuse`.
/// `hike_span` not read is not this function: the caller records `held`.
/// The true branch has no single glossary token. The reading used here is the
/// SPEC phrase "meets the floor". See DONE.md.
pub fn leg_rate_floor(hike_legs: i128, hike_span: i128, p: i128, q: i128) -> &'static str {
    if hike_span <= 0 {
        return "refuse";
    }
    if q * hike_legs >= p * hike_span {
        "meets the floor"
    } else {
        "refuse"
    }
}

/// SPEC §6 / §14. A header height is in the window of stored stage height `h`
/// when it equals `h` plus one of 2, 3, 4.
///
/// The outcome words of that rule are `anchored` and `not anchored`.
/// This grid is only the integer window test. Plant match and the stored-interrupt
/// check are the verifier's, and they are not inputs here. See DONE.md.
pub fn anchor_window(stage_height: u64, header_height: u64) -> &'static str {
    if header_height >= stage_height {
        let delta = header_height - stage_height;
        if ANCHOR_WINDOW_OFFSETS.contains(&delta) {
            return "anchored";
        }
    }
    "not anchored"
}
