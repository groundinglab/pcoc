package pcoc

// Integer comparisons (SPEC Rule B2 and §16.1 Anchor comparison).
//
// These functions name the comparison. They do not admit an object.

// RateCompare is the Rule B2 integer comparison.
// The comparison result is meets when hikeSpan > 0 and
// q·hikeLegs ≥ p·hikeSpan, and refuse otherwise (SPEC §4, Rule B2).
// hikeSpan ≤ 0 is refuse. The products use int64. The caller supplies
// values whose products fit in int64; the CASES grid does.
func RateCompare(hikeLegs, hikeSpan, p, q int64) string {
	if hikeSpan > 0 && q*hikeLegs >= p*hikeSpan {
		return "meets"
	}
	return "refuse"
}

// AnchorCompare is the SPEC §16.1 integer anchor comparison.
// The answer is anchored when inclusion is H plus one of
// ANCHOR_WINDOW_OFFSETS, and not anchored otherwise. The comparison does
// not test that the header is a stored interrupt or that the plant matches.
func AnchorCompare(h, inclusion uint64) string {
	if inclusion >= h {
		d := inclusion - h
		for _, off := range AnchorWindowOffsets {
			if d == uint64(off) {
				return "anchored"
			}
		}
	}
	return "not anchored"
}

// HashcashSolve is the SPEC §13.1 nonce search: the first u64 in the order
// 0, 1, 2, … whose digest has at least difficulty leading zero bits.
// The returned digest is that nonce's digest.
func HashcashSolve(challenge, waypoint []byte, difficulty int) (nonce uint64, digest []byte) {
	for nonce = 0; ; nonce++ {
		d := HashcashDigest(challenge, waypoint, nonce)
		if LeadingZeroBits(d) >= difficulty {
			return nonce, d
		}
	}
}
