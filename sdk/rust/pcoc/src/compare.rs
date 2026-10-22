//! SPEC §13.1 anchor window and Rule B2 integer comparison.
//! These are the comparisons the candidate grid asks for. They are not a Gate.

use crate::constants::ANCHOR_WINDOW_OFFSETS;

/// SPEC Rule B2 (§15.4) and §4 Comparison result. Integer comparison only.
///
/// `hike_span ≤ 0` and a numeric shortfall are `refuse`.
/// The comparison holds as `meets` (erratum E04). `hike_span` not read is not
/// this function: the caller records `held`.
pub fn leg_rate_floor(hike_legs: i128, hike_span: i128, p: i128, q: i128) -> &'static str {
    if hike_span <= 0 {
        return "refuse";
    }
    if q * hike_legs >= p * hike_span {
        "meets"
    } else {
        "refuse"
    }
}

/// SPEC §16.1 Anchor comparison. A header height is in the window of stored
/// stage height `h` when it equals `h` plus one of 2, 3, 4.
///
/// The outcome words are `anchored` and `not anchored`. This function is only
/// the integer window test. Plant match and the stored-interrupt check are the
/// verifier's, and they are not inputs here (§16.1).
pub fn anchor_window(stage_height: u64, header_height: u64) -> &'static str {
    if header_height >= stage_height {
        let delta = header_height - stage_height;
        if ANCHOR_WINDOW_OFFSETS.contains(&delta) {
            return "anchored";
        }
    }
    "not anchored"
}
