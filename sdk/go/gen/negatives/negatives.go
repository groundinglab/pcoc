// Package negatives builds the thirteen permanent negatives (SPEC §16).
//
// This package is the only place those constructions exist. No release
// package imports it. Each function names the negative it builds. The
// generator compares each result with the honest value and stops when they
// are equal.
package negatives

import (
	"crypto/sha256"
	"encoding/binary"

	"github.com/groundinglab/pcoc/sdk/go/v2"
	"github.com/groundinglab/pcoc/sdk/go/v2/internal/fold"
)

// LegNames and TerrainNames are the thirteen names, in SPEC §16 order of
// the two groups.
var LegNames = []string{
	"slot_summary_fold",
	"xor_accumulator_fold",
	"xor_w_fold",
	"s_plus_w_fold",
	"prefold_hoist",
	"final_stage_hoist",
	"partial_slot_read",
	"partial_slot_read_2047",
	"f3_shared_terrain",
	"cached_leg",
}

var TerrainNames = []string{
	"one_lane_flip",
	"all4_kernel",
	"squeezed_seed_terrain",
}

// LegResult runs the first leg of stage 0 from the seed, with step in place
// of the section 11.1 step. steps is the profile's STEPS_PER_LEG.
func LegResult(t *pcoc.Terrain, previous []byte, steps int, step func(s uint64) uint64) uint64 {
	s := pcoc.U64le(previous[:8])
	for n := 0; n < steps; n++ {
		s = step(s)
	}
	return s
}

// SlotSummaryFold is the negative slot_summary_fold (SPEC §11.1, §16).
// S_folded = S XOR (XOR over w of (word(w) XOR w)), then the section 11.1 seal.
func SlotSummaryFold(t *pcoc.Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.Profile().NSlots())))
	var x uint64
	words := len(slot) / 8
	for w := 0; w < words; w++ {
		word := binary.LittleEndian.Uint64(slot[8*w : 8*w+8])
		x ^= word ^ uint64(w)
	}
	return pcoc.SealState(s ^ x)
}

// XorAccumulatorFold is the negative xor_accumulator_fold.
// acc = acc XOR mix64(word(w) XOR w XOR S), then the seal.
func XorAccumulatorFold(t *pcoc.Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.Profile().NSlots())))
	var acc uint64
	words := len(slot) / 8
	for w := 0; w < words; w++ {
		word := binary.LittleEndian.Uint64(slot[8*w : 8*w+8])
		acc ^= pcoc.Mix64(word ^ uint64(w) ^ s)
	}
	return pcoc.SealState(acc)
}

// XorWFold is the negative xor_w_fold.
// acc = (acc + mix64(word(w) XOR w XOR S)) mod 2^64, then the seal.
func XorWFold(t *pcoc.Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.Profile().NSlots())))
	var acc uint64
	words := len(slot) / 8
	for w := 0; w < words; w++ {
		word := binary.LittleEndian.Uint64(slot[8*w : 8*w+8])
		acc += pcoc.Mix64(word ^ uint64(w) ^ s)
	}
	return pcoc.SealState(acc)
}

// SPlusWFold is the negative s_plus_w_fold.
// acc = (acc + mix64(word(w) XOR ((S + w) mod 2^64))) mod 2^64, then the seal.
func SPlusWFold(t *pcoc.Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.Profile().NSlots())))
	var acc uint64
	words := len(slot) / 8
	for w := 0; w < words; w++ {
		word := binary.LittleEndian.Uint64(slot[8*w : 8*w+8])
		acc += pcoc.Mix64(word ^ (s + uint64(w)))
	}
	return pcoc.SealState(acc)
}

// PrefoldHoist is the negative prefold_hoist.
// Once per slot, pre(i) = (sum over w of mix64(word(w) XOR w)) mod 2^64.
// On a step that reads slot i, S_folded = (pre(i) + S) mod 2^64, then the seal.
func PrefoldHoist(t *pcoc.Terrain) func(uint64) uint64 {
	n := t.Profile().NSlots()
	words := t.Profile().WordsPerSlot()
	pre := make([]uint64, n)
	for i := 0; i < n; i++ {
		slot := t.Slot(i)
		var sum uint64
		for w := 0; w < words; w++ {
			word := binary.LittleEndian.Uint64(slot[8*w : 8*w+8])
			sum += pcoc.Mix64(word ^ uint64(w))
		}
		pre[i] = sum
	}
	return func(s uint64) uint64 {
		i := int(s % uint64(n))
		return pcoc.SealState(pre[i] + s)
	}
}

// FinalStageHoist is the negative final_stage_hoist.
// S_folded = x XOR (x >> 31), where x is the XOR over every word of mix64
// applied to (word(w) XOR S) without its last line.
func FinalStageHoist(t *pcoc.Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.Profile().NSlots())))
	var x uint64
	words := len(slot) / 8
	for w := 0; w < words; w++ {
		word := binary.LittleEndian.Uint64(slot[8*w : 8*w+8])
		x ^= mix64BeforeFinal(word ^ s)
	}
	return pcoc.SealState(x ^ (x >> 31))
}

// mix64BeforeFinal is mix64 without `return y XOR (y >> 31)` (SPEC §10.1).
func mix64BeforeFinal(x uint64) uint64 {
	y := x + fold.MixAdd
	y = (y ^ (y >> 30)) * fold.MixMul1
	y = (y ^ (y >> 27)) * fold.MixMul2
	return y
}

// PartialSlotRead is the section 11.1 fold over word 0 only, then the seal.
func PartialSlotRead(t *pcoc.Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.Profile().NSlots())))
	word := binary.LittleEndian.Uint64(slot[0:8])
	return pcoc.SealState(pcoc.Mix64(word ^ s))
}

// PartialSlotRead2047 is the section 11.1 fold over words 0 … WORDS_PER_SLOT−2.
func PartialSlotRead2047(t *pcoc.Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.Profile().NSlots())))
	words := len(slot)/8 - 1
	var acc uint64
	for w := 0; w < words; w++ {
		word := binary.LittleEndian.Uint64(slot[8*w : 8*w+8])
		acc += pcoc.Mix64(word ^ s)
	}
	return pcoc.SealState(acc)
}

// CachedLeg is the negative cached_leg.
// A step that reads a slot already read earlier in the same leg reuses that
// earlier read's S_folded. revisited reports whether the leg did so.
func CachedLeg(t *pcoc.Terrain, previous []byte, steps int) (leg uint64, revisited bool) {
	s := pcoc.U64le(previous[:8])
	type hit struct {
		folded uint64
		ok     bool
	}
	seen := map[int]hit{}
	nSlots := t.Profile().NSlots()
	for n := 0; n < steps; n++ {
		i := int(s % uint64(nSlots))
		var folded uint64
		if h, ok := seen[i]; ok {
			folded = h.folded
			revisited = true
		} else {
			folded = fold.Scalar(t.Slot(i), s)
			seen[i] = hit{folded: folded, ok: true}
		}
		s = pcoc.SealState(folded)
	}
	return s, revisited
}

// TerrainSHA256 is the SHA-256 of the whole terrain array.
func TerrainSHA256(bytes []byte) []byte {
	sum := sha256.Sum256(bytes)
	return sum[:]
}

// FillOneLane is the section 10.4 fill with the one-lane map in place of NL4.
// Parents and slot_mix are unchanged. The returned bytes are the terrain.
func FillOneLane(profile pcoc.Profile, lanes [4]uint64) []byte {
	return fillMapped(profile, lanes, func(p [4]uint64, i, j int) uint64 {
		c := (uint64(i) << 32) ^ uint64(j) ^ pcoc.WordXor
		return pcoc.Mix64(p[j%4] ^ c)
	})
}

// FillAll4 is the section 10.4 fill with the linear map in place of NL4.
func FillAll4(profile pcoc.Profile, lanes [4]uint64) []byte {
	return fillMapped(profile, lanes, func(p [4]uint64, i, j int) uint64 {
		c := (uint64(i) << 32) ^ uint64(j) ^ pcoc.WordXor
		x := p[j%4] ^ pcoc.Rotl(p[(j+1)%4], 17) ^ pcoc.Rotl(p[(j+2)%4], 31) ^ pcoc.Rotl(p[(j+3)%4], 47) ^ c
		return pcoc.Mix64(x)
	})
}

// FillSqueezed replaces each lane by P[0] XOR P[1] XOR P[2] XOR P[3], then
// fills by section 10.4.
func FillSqueezed(profile pcoc.Profile, lanes [4]uint64) []byte {
	s := lanes[0] ^ lanes[1] ^ lanes[2] ^ lanes[3]
	sq := [4]uint64{s, s, s, s}
	return fillMapped(profile, sq, nl4)
}

func nl4(lanes [4]uint64, i, j int) uint64 {
	c := (uint64(i) << 32) ^ uint64(j) ^ pcoc.WordXor
	t := pcoc.Mix64(lanes[j%4] ^ c)
	t = pcoc.Mix64(t ^ pcoc.Rotl(lanes[(j+1)%4], 17))
	t = pcoc.Mix64(t ^ pcoc.Rotl(lanes[(j+2)%4], 31))
	t = pcoc.Mix64(t ^ pcoc.Rotl(lanes[(j+3)%4], 47))
	return t
}

// fillMapped is the section 10.4 parent mix and slot mix over a base-word map.
func fillMapped(profile pcoc.Profile, lanes [4]uint64, base func([4]uint64, int, int) uint64) []byte {
	nSlots := profile.NSlots()
	words := profile.WordsPerSlot()
	slotBytes := profile.SlotBytes
	bytes := make([]byte, profile.TerrainBytes)
	w := make([]uint64, words)
	for i := 0; i < nSlots; i++ {
		for j := 0; j < words; j++ {
			w[j] = base(lanes, i, j)
		}
		if i > 0 {
			parents := [3]int{i - 1, pcoc.BitrevParent(i), pcoc.MulParent(i)}
			for k := 1; k <= 3; k++ {
				pidx := parents[k-1]
				off := pidx * slotBytes
				xorK := uint64(k) * pcoc.ParentMul
				for j := 0; j < words; j++ {
					pw := binary.LittleEndian.Uint64(bytes[off+8*j : off+8*j+8])
					rot := uint((j + k) % 64)
					w[j] = pcoc.Mix64((w[j] ^ pcoc.Rotl(pw, rot)) ^ xorK)
				}
			}
			slotMix := uint64(i) * pcoc.SlotMul
			for j := 0; j < words; j++ {
				w[j] = pcoc.Mix64(w[j] ^ slotMix)
			}
		}
		off := i * slotBytes
		for j := 0; j < words; j++ {
			binary.LittleEndian.PutUint64(bytes[off+8*j:off+8*j+8], w[j])
		}
	}
	return bytes
}
