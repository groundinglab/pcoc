// Package negatives computes the SPEC §16 permanent negatives.
//
// These functions exist only so the candidate generator can show each
// shortcut as a non-match. They are not an SDK code path. Nothing in the
// parent package calls them.
package negatives

import (
	"crypto/sha256"
	"encoding/binary"

	"pcoc.dev/sdk/go"
)

// SlotSummaryFold is the negative slot_summary_fold (SPEC §11.1, F1):
//
//	S_folded = S XOR (XOR over w of (word(w) XOR w))
//
// then the section 11.1 seal. The parenthesised XOR does not depend on S.
func SlotSummaryFold(t *pcoc.Terrain, s uint64) uint64 {
	slot := slotOf(t, s)
	var x uint64
	for w := 0; w < len(slot)/8; w++ {
		x ^= wordAt(slot, w) ^ uint64(w)
	}
	return pcoc.SealState(s ^ x)
}

// XorAccumulatorFold is the negative xor_accumulator_fold (SPEC §11.1, F3):
//
//	acc = acc XOR mix64(word(w) XOR w XOR S)
func XorAccumulatorFold(t *pcoc.Terrain, s uint64) uint64 {
	slot := slotOf(t, s)
	var acc uint64
	for w := 0; w < len(slot)/8; w++ {
		acc ^= pcoc.Mix64(wordAt(slot, w) ^ uint64(w) ^ s)
	}
	return pcoc.SealState(acc)
}

// XorWFold is the negative xor_w_fold (SPEC §11.1, F2):
//
//	acc = (acc + mix64(word(w) XOR w XOR S)) mod 2^64
func XorWFold(t *pcoc.Terrain, s uint64) uint64 {
	slot := slotOf(t, s)
	var acc uint64
	for w := 0; w < len(slot)/8; w++ {
		acc += pcoc.Mix64(wordAt(slot, w) ^ uint64(w) ^ s)
	}
	return pcoc.SealState(acc)
}

// SPlusWFold is the negative s_plus_w_fold (SPEC §11.1, F2):
//
//	acc = (acc + mix64(word(w) XOR ((S + w) mod 2^64))) mod 2^64
func SPlusWFold(t *pcoc.Terrain, s uint64) uint64 {
	slot := slotOf(t, s)
	var acc uint64
	for w := 0; w < len(slot)/8; w++ {
		acc += pcoc.Mix64(wordAt(slot, w) ^ (s + uint64(w)))
	}
	return pcoc.SealState(acc)
}

// PrefoldHoist is the negative prefold_hoist (SPEC §11.1).
//
// The last stage of mix64 is linear over GF(2). This shortcut XORs the
// pre-final mix64 values across the slot once, then applies `y XOR (y >> 31)`
// once, and seals that. It is not the wrapping-add fold.
func PrefoldHoist(t *pcoc.Terrain, s uint64) uint64 {
	slot := slotOf(t, s)
	var acc uint64
	for w := 0; w < len(slot)/8; w++ {
		acc ^= preFinal(wordAt(slot, w) ^ s)
	}
	acc = acc ^ (acc >> 31)
	return pcoc.SealState(acc)
}

// preFinal is mix64 without the last `y XOR (y >> 31)` stage.
func preFinal(x uint64) uint64 {
	y := x + pcoc.MixAdd
	y = (y ^ (y >> 30)) * pcoc.MixMul1
	y = (y ^ (y >> 27)) * pcoc.MixMul2
	return y
}

// PartialSlotRead is the negative partial_slot_read: the §11.1 loop stops
// after the first word of the slot.
func PartialSlotRead(t *pcoc.Terrain, s uint64) uint64 {
	return partial(t, s, 1)
}

// PartialSlotRead2047 is the negative partial_slot_read_2047 at PROD
// WORDS_PER_SLOT semantics: the loop runs w = 0 … 2046 and drops the last
// word. On a profile whose slot is shorter than 2048 words, the loop still
// stops one word early, which is that same omission at the profile width.
func PartialSlotRead2047(t *pcoc.Terrain, s uint64) uint64 {
	words := len(slotOf(t, s)) / 8
	n := 2047
	if n > words-1 {
		n = words - 1
	}
	if n < 1 {
		n = 1
	}
	return partial(t, s, n)
}

func partial(t *pcoc.Terrain, s uint64, nWords int) uint64 {
	slot := slotOf(t, s)
	var acc uint64
	for w := 0; w < nWords && 8*w+8 <= len(slot); w++ {
		acc += pcoc.Mix64(wordAt(slot, w) ^ s)
	}
	return pcoc.SealState(acc)
}

// OneLaneFlip fills terrain from lanes whose lane 0 is bitwise-complemented,
// which is the one-lane map's failure to depend on every lane (SPEC §10.4,
// negative one_lane_flip). The digest is SHA-256 of that terrain. The fill
// itself is the one-lane base word `mix64(P[j mod 4] XOR c)`, stored with no
// parent mixing: a word map that uses one lane.
func OneLaneFlip(profile pcoc.Profile, publicKey, seedBeacon []byte) []byte {
	d := pcoc.Seed(publicKey, seedBeacon)
	lanes := pcoc.Lanes(d)
	lanes[0] = ^lanes[0]
	return sha256of(oneLaneTerrain(profile, lanes))
}

// All4Kernel is the negative all4_kernel: the linear four-lane map SPEC §10.4
// names, which is not NL4.
//
//	mix64(P[j mod 4] XOR rotl(P[(j+1) mod 4], 17) XOR rotl(P[(j+2) mod 4], 31)
//	      XOR rotl(P[(j+3) mod 4], 47) XOR c)
func All4Kernel(profile pcoc.Profile, publicKey, seedBeacon []byte) []byte {
	d := pcoc.Seed(publicKey, seedBeacon)
	lanes := pcoc.Lanes(d)
	nSlots := profile.NSlots()
	words := profile.WordsPerSlot()
	raw := make([]byte, profile.TerrainBytes)
	for i := 0; i < nSlots; i++ {
		base := i * profile.SlotBytes
		for j := 0; j < words; j++ {
			c := (uint64(i) << 32) ^ uint64(j) ^ pcoc.WordXor
			x := lanes[j%4] ^
				pcoc.Rotl(lanes[(j+1)%4], 17) ^
				pcoc.Rotl(lanes[(j+2)%4], 31) ^
				pcoc.Rotl(lanes[(j+3)%4], 47) ^ c
			put(raw[base+8*j:], pcoc.Mix64(x))
		}
	}
	return sha256of(raw)
}

// F3SharedTerrain is the negative f3_shared_terrain: the terrain digest of
// pk_A's seed reused as the terrain of a second seed. The digest is therefore
// pk_A's own terrain digest, which does not match the second seed's terrain.
func F3SharedTerrain(profile pcoc.Profile, publicKey, seedBeacon []byte) []byte {
	t, err := pcoc.FillTerrain(profile, publicKey, seedBeacon)
	if err != nil {
		return nil
	}
	return sha256of(t.Bytes())
}

// CachedLeg is the negative cached leg: the leg result of step 0's seal
// repeated, with no further step. One step's output stands in for the leg.
func CachedLeg(t *pcoc.Terrain, previous []byte) uint64 {
	s0 := wordAt(previous, 0)
	return pcoc.Step(t, s0)
}

// SqueezedSeedTerrain is the negative terrain digest from a squeezed seed
// (SPEC §10.2, §16). The seed is reduced to lane 0 and the other three lanes
// are zero. That is a squeeze to one 64-bit fill pin, which is not this
// construction. The fill then uses the NL4 chain on those lanes so the only
// change from the construction is the squeeze.
func SqueezedSeedTerrain(profile pcoc.Profile, publicKey, seedBeacon []byte) []byte {
	d := pcoc.Seed(publicKey, seedBeacon)
	lanes := pcoc.Lanes(d)
	lanes[1], lanes[2], lanes[3] = 0, 0, 0
	return sha256of(nl4Terrain(profile, lanes))
}

func oneLaneTerrain(profile pcoc.Profile, lanes [4]uint64) []byte {
	nSlots := profile.NSlots()
	words := profile.WordsPerSlot()
	raw := make([]byte, profile.TerrainBytes)
	for i := 0; i < nSlots; i++ {
		base := i * profile.SlotBytes
		for j := 0; j < words; j++ {
			c := (uint64(i) << 32) ^ uint64(j) ^ pcoc.WordXor
			put(raw[base+8*j:], pcoc.Mix64(lanes[j%4]^c))
		}
	}
	return raw
}

func nl4Terrain(profile pcoc.Profile, lanes [4]uint64) []byte {
	nSlots := profile.NSlots()
	words := profile.WordsPerSlot()
	raw := make([]byte, profile.TerrainBytes)
	w := make([]uint64, words)
	for i := 0; i < nSlots; i++ {
		for j := 0; j < words; j++ {
			c := (uint64(i) << 32) ^ uint64(j) ^ pcoc.WordXor
			t := pcoc.Mix64(lanes[j%4] ^ c)
			t = pcoc.Mix64(t ^ pcoc.Rotl(lanes[(j+1)%4], 17))
			t = pcoc.Mix64(t ^ pcoc.Rotl(lanes[(j+2)%4], 31))
			t = pcoc.Mix64(t ^ pcoc.Rotl(lanes[(j+3)%4], 47))
			w[j] = t
		}
		if i > 0 {
			parents := [3]int{i - 1, pcoc.BitrevParent(i), pcoc.MulParent(i)}
			for k := 1; k <= 3; k++ {
				pidx := parents[k-1]
				base := pidx * profile.SlotBytes
				xorK := uint64(k) * pcoc.ParentMul
				for j := 0; j < words; j++ {
					pw := get(raw[base+8*j:])
					rot := uint((j + k) % 64)
					w[j] = pcoc.Mix64((w[j] ^ pcoc.Rotl(pw, rot)) ^ xorK)
				}
			}
			slotMix := uint64(i) * pcoc.SlotMul
			for j := 0; j < words; j++ {
				w[j] = pcoc.Mix64(w[j] ^ slotMix)
			}
		}
		base := i * profile.SlotBytes
		for j := 0; j < words; j++ {
			put(raw[base+8*j:], w[j])
		}
	}
	return raw
}

func slotOf(t *pcoc.Terrain, s uint64) []byte {
	n := t.Profile().NSlots()
	return t.Slot(int(s % uint64(n)))
}

func wordAt(b []byte, w int) uint64 {
	return binary.LittleEndian.Uint64(b[8*w : 8*w+8])
}

func put(b []byte, x uint64) { binary.LittleEndian.PutUint64(b, x) }
func get(b []byte) uint64    { return binary.LittleEndian.Uint64(b[:8]) }

func sha256of(b []byte) []byte {
	sum := sha256.Sum256(b)
	return sum[:]
}
