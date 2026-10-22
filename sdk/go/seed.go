package pcoc

import "crypto/sha256"

// SeedPreimage is SPEC §10: public_key ‖ seed_beacon, 64 bytes, no separator
// and no domain label.
func SeedPreimage(publicKey, seedBeacon []byte) []byte {
	out := make([]byte, 64)
	copy(out[0:32], publicKey)
	copy(out[32:64], seedBeacon)
	return out
}

// Seed is SPEC §10: D = SHA-256(public_key ‖ seed_beacon).
func Seed(publicKey, seedBeacon []byte) []byte {
	sum := sha256.Sum256(SeedPreimage(publicKey, seedBeacon))
	return sum[:]
}

// Lanes is SPEC §10: P[k] = u64_le(D[8k : 8k+8]) for k = 0 … 3.
// All 256 bits of D are the lanes. There is no fifth lane.
func Lanes(d []byte) [4]uint64 {
	var p [4]uint64
	for k := 0; k < 4; k++ {
		p[k] = U64le(d[8*k : 8*k+8])
	}
	return p
}

// U64le is SPEC §3 u64_le.
func U64le(b []byte) uint64 {
	return uint64(b[0]) |
		uint64(b[1])<<8 |
		uint64(b[2])<<16 |
		uint64(b[3])<<24 |
		uint64(b[4])<<32 |
		uint64(b[5])<<40 |
		uint64(b[6])<<48 |
		uint64(b[7])<<56
}

func putU64le(b []byte, x uint64) {
	b[0] = byte(x)
	b[1] = byte(x >> 8)
	b[2] = byte(x >> 16)
	b[3] = byte(x >> 24)
	b[4] = byte(x >> 32)
	b[5] = byte(x >> 40)
	b[6] = byte(x >> 48)
	b[7] = byte(x >> 56)
}

// Le64 is SPEC §3 le64.
func Le64(x uint64) []byte {
	b := make([]byte, 8)
	putU64le(b, x)
	return b
}
