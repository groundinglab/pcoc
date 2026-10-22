// Package fold is the scalar slot fold (SPEC §11.1).
//
// Scalar is the only fold kernel. It loads every word of the slot it is
// given and mixes each word with the current state. Phase B may add other
// kernels beside this file; this file stays the oracle.
package fold

import "encoding/binary"

// Mix constants, SPEC §6. The public package re-exports them.
const (
	MixAdd  uint64 = 0x9E3779B97F4A7C15
	MixMul1 uint64 = 0xBF58476D1CE4E5B9
	MixMul2 uint64 = 0x94D049BB133111EB
)

// Mix64 is SPEC §10.1, including the initial addition of MIX_ADD.
//
//	y = (x + MIX_ADD) mod 2^64
//	y = ((y XOR (y >> 30)) × MIX_MUL1) mod 2^64
//	y = ((y XOR (y >> 27)) × MIX_MUL2) mod 2^64
//	return y XOR (y >> 31)
func Mix64(x uint64) uint64 {
	y := x + MixAdd
	y = (y ^ (y >> 30)) * MixMul1
	y = (y ^ (y >> 27)) * MixMul2
	return y ^ (y >> 31)
}

// Scalar is the SPEC §11.1 accumulator. slot is the full slot
// (SLOT_BYTES bytes, WORDS_PER_SLOT little-endian words). Every word is
// loaded and mixed with s. acc starts at 0. The sum is wrapping addition.
//
//	acc = 0
//	for w = 0 … WORDS_PER_SLOT − 1:
//	    acc = (acc + mix64(word(w) XOR S)) mod 2^64
func Scalar(slot []byte, s uint64) uint64 {
	words := len(slot) / 8
	var acc uint64
	for w := 0; w < words; w++ {
		word := binary.LittleEndian.Uint64(slot[8*w : 8*w+8])
		acc += Mix64(word ^ s)
	}
	return acc
}
