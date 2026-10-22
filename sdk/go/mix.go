package pcoc

import "github.com/groundinglab/pcoc/sdk/go/v2/internal/fold"

// Mix64 is SPEC §10.1, including the initial + MIX_ADD.
func Mix64(x uint64) uint64 { return fold.Mix64(x) }

// Rotl is SPEC §10.3: w rotated left by r bits. r is in 0 … 63.
// rotl(w, 0) = w.
func Rotl(w uint64, r uint) uint64 {
	if r == 0 {
		return w
	}
	return (w << r) | (w >> (64 - r))
}

// floorLog2 is SPEC §10.3: the greatest integer b with 2^b ≤ n, for n ≥ 1.
func floorLog2(n int) int {
	b := 0
	for (1 << (b + 1)) <= n {
		b++
	}
	return b
}

// reverse32 is SPEC §10.3. Bit k of the result equals bit 31−k of v.
func reverse32(v uint32) uint32 {
	var out uint32
	for k := 0; k < 32; k++ {
		bit := (v >> uint(31-k)) & 1
		out |= bit << uint(k)
	}
	return out
}

// BitrevParent is SPEC §10.3 bitrev_parent, for 1 ≤ i < N_SLOTS.
func BitrevParent(i int) int {
	bits := floorLog2(i)
	if bits < 1 {
		bits = 1
	}
	return int((reverse32(uint32(i)) >> uint(32-bits)) % uint32(i))
}

// MulParent is SPEC §10.3.
//
//	h = (i × MIX_ADD) mod 2^64
//	mul_parent(i) = floor(h / 2^32) mod i
func MulParent(i int) int {
	h := uint64(i) * MixAdd
	return int((h >> 32) % uint64(i))
}
