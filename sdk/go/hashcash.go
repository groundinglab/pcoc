package pcoc

import "crypto/sha256"

// HashcashChallenge is SPEC §13.1.
//
//	challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)
func HashcashChallenge(bariOrigin string, headerHash []byte) []byte {
	h := sha256.New()
	h.Write(HashcashPrefix)
	h.Write([]byte(bariOrigin))
	h.Write(headerHash)
	return h.Sum(nil)
}

// HashcashDigest is SPEC §13.1.
//
//	SHA-256(challenge ‖ waypoint ‖ le64(nonce))
func HashcashDigest(challenge, waypoint []byte, nonce uint64) []byte {
	h := sha256.New()
	h.Write(challenge)
	h.Write(waypoint)
	var n [8]byte
	putU64le(n[:], nonce)
	h.Write(n[:])
	return h.Sum(nil)
}

// LeadingZeroBits counts leading zero bits from the first digest byte.
// Inside a byte the high bit is first (SPEC §13.1).
func LeadingZeroBits(digest []byte) int {
	n := 0
	for _, b := range digest {
		if b == 0 {
			n += 8
			continue
		}
		for mask := byte(0x80); mask != 0; mask >>= 1 {
			if b&mask != 0 {
				return n
			}
			n++
		}
	}
	return n
}

// HeaderHash is SHA-256(SHA-256(header)), natural order (SPEC §13.1).
// It is not a single SHA-256 of the header.
func HeaderHash(header80 []byte) []byte {
	a := sha256.Sum256(header80)
	b := sha256.Sum256(a[:])
	out := make([]byte, 32)
	copy(out, b[:])
	return out
}
