// SECTION 15 Rule B2 integer comparison (leg-rate floor) and the SECTION 16.1
// integer anchor comparison. Both are integer-only; no rounding.

public enum Comparisons {
    /// Rule B2: `leg_rate_floor` p/q meets when hike_span is an integer
    /// greater than 0 and q · hike_legs ≥ p · hike_span. Returns "meets" or
    /// "refuse" (section 4 Comparison result).
    public static func rateFloor(p: UInt64, q: UInt64, hikeLegs: UInt64,
                                 hikeSpan: Int64) -> String {
        if hikeSpan <= 0 { return "refuse" } // the rate is undefined
        let span = UInt64(hikeSpan)
        // q · hike_legs ≥ p · hike_span in 128-bit, or equivalently with
        // wrapping checks; use a wide multiply for exactness.
        let lhs = mul128Wide(q, hikeLegs)
        let rhs = mul128Wide(p, span)
        return lhs >= rhs ? "meets" : "refuse"
    }

    static func mul128Wide(_ a: UInt64, _ b: UInt64) -> (UInt64, UInt64) { // (hi, lo)
        let aLo = a & 0xffff_ffff, aHi = a >> 32
        let bLo = b & 0xffff_ffff, bHi = b >> 32
        let ll = aLo * bLo
        let lh = aLo * bHi
        let hl = aHi * bLo
        let hh = aHi * bHi
        let mid = lh &+ hl &+ (ll >> 32)
        let lo = (ll & 0xffff_ffff) | (mid << 32)
        let hi = hh &+ (mid >> 32)
        return (hi, lo)
    }

    /// SECTION 16.1 Anchor comparison: the answer is `anchored` when the
    /// inclusion height is H plus one of ANCHOR_WINDOW_OFFSETS (2, 3, 4),
    /// `not anchored` otherwise. The stored-stage-height and plant conditions
    /// are not tested here.
    public static func anchor(H: UInt64, inclusionHeight: UInt64) -> String {
        for off in Spec.anchorWindowOffsets {
            if inclusionHeight == H &+ off { return "anchored" }
        }
        return "not anchored"
    }
}
