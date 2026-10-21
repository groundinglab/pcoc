package hike

import (
	"crypto/sha256"
	"encoding/binary"
	"math/bits"
)

func SHA256(parts ...[]byte) []byte {
	h := sha256.New()
	for _, p := range parts {
		_, _ = h.Write(p)
	}
	return h.Sum(nil)
}

func U64LE(b []byte) uint64 {
	var buf [8]byte
	copy(buf[:], b)
	return binary.LittleEndian.Uint64(buf[:])
}

func Le64(v uint64) []byte {
	var buf [8]byte
	binary.LittleEndian.PutUint64(buf[:], v)
	return buf[:]
}

// mix64 is §10.1. The first step adds MIX_ADD. A finalizer that omits
// MIX_ADD is not this function.
func mix64(x uint64) uint64 {
	y := x + MixAdd
	y = (y ^ (y >> 30)) * MixMul1
	y = (y ^ (y >> 27)) * MixMul2
	return y ^ (y >> 31)
}

func rotl(w uint64, r uint) uint64 {
	if r == 0 || r == 64 {
		return w
	}
	r &= 63
	return (w << r) | (w >> (64 - r))
}

func reverse32(v uint32) uint32 {
	x := v
	x = ((x >> 1) & 0x55555555) | ((x & 0x55555555) << 1)
	x = ((x >> 2) & 0x33333333) | ((x & 0x33333333) << 2)
	x = ((x >> 4) & 0x0F0F0F0F) | ((x & 0x0F0F0F0F) << 4)
	x = ((x >> 8) & 0x00FF00FF) | ((x & 0x00FF00FF) << 8)
	x = (x >> 16) | (x << 16)
	return x
}

// floorLog2 is the greatest b with 2^b <= n, for n >= 1.
func floorLog2(n int) int {
	return bits.Len(uint(n)) - 1
}

// bitrevParent is §10.3. bits = max(floor_log2(i), 1).
func bitrevParent(index int) int {
	b := floorLog2(index)
	if b < 1 {
		b = 1
	}
	return int((reverse32(uint32(index)) >> uint(32-b)) % uint32(index))
}

func mulParent(index int) int {
	h := uint64(index) * MixAdd
	return int((h >> 32) % uint64(index))
}

// Terrain is the §10.4 byte region. It keeps the bytes and nothing else.
// There is no per-slot digest, no word table, and no fold cache.
type Terrain struct {
	Profile Profile
	Blob    []byte
}

// SeedLanes is D = SHA-256(public_key ‖ seed_beacon) and the four
// little-endian lanes P[0] … P[3]. There is no domain label and no fill pin.
func SeedLanes(publicKey, seedBeacon []byte) (digest []byte, lanes [4]uint64) {
	digest = SHA256(publicKey, seedBeacon)
	for k := 0; k < 4; k++ {
		lanes[k] = binary.LittleEndian.Uint64(digest[8*k : 8*k+8])
	}
	return digest, lanes
}

// NL4BaseWord is the §10.4 base word at slot i, word j, before parents
// and before slot_mix. Every word mixes all four lanes.
func NL4BaseWord(publicKey, seedBeacon []byte, slot, word int) uint64 {
	_, lanes := SeedLanes(publicKey, seedBeacon)
	return nl4Base(lanes, slot, word)
}

func nl4Base(lanes [4]uint64, slot, word int) uint64 {
	c := (uint64(slot) << 32) ^ uint64(word) ^ WordXOR
	t := mix64(lanes[word%4] ^ c)
	t = mix64(t ^ rotl(lanes[(word+1)%4], 17))
	t = mix64(t ^ rotl(lanes[(word+2)%4], 31))
	t = mix64(t ^ rotl(lanes[(word+3)%4], 47))
	return t
}

// NL4BaseSlot is slot `slot` as `words` little-endian base words.
// Parents and slot_mix are not applied. Slot 0 of a full fill is this
// byte string when `words` is WORDS_PER_SLOT.
func NL4BaseSlot(publicKey, seedBeacon []byte, slot, words int) []byte {
	_, lanes := SeedLanes(publicKey, seedBeacon)
	out := make([]byte, words*8)
	for j := 0; j < words; j++ {
		binary.LittleEndian.PutUint64(out[j*8:], nl4Base(lanes, slot, j))
	}
	return out
}

// FillTerrain builds the §10.4 region once. Each later step reads the
// stored bytes. The working word buffer is the slot being built. It is
// not kept.
func FillTerrain(publicKey, seedBeacon []byte, profile Profile) *Terrain {
	_, lanes := SeedLanes(publicKey, seedBeacon)
	blob := make([]byte, profile.TerrainBytes)
	words := make([]uint64, profile.WordsPerSlot)
	for i := 0; i < profile.NSlots; i++ {
		for j := 0; j < profile.WordsPerSlot; j++ {
			words[j] = nl4Base(lanes, i, j)
		}
		if i > 0 {
			parents := [3]int{i - 1, bitrevParent(i), mulParent(i)}
			for k := 1; k <= 3; k++ {
				parent := parents[k-1]
				xk := uint64(k) * ParentMul
				for j := 0; j < profile.WordsPerSlot; j++ {
					pw := loadWord(blob, profile, parent, j)
					rot := uint((j + k) % 64)
					words[j] = mix64((words[j] ^ rotl(pw, rot)) ^ xk)
				}
			}
			slotMix := uint64(i) * SlotMul
			for j := 0; j < profile.WordsPerSlot; j++ {
				words[j] = mix64(words[j] ^ slotMix)
			}
		}
		writeSlot(blob, profile, i, words)
	}
	return &Terrain{Profile: profile, Blob: blob}
}

func loadWord(blob []byte, profile Profile, slot, word int) uint64 {
	off := slot*profile.SlotBytes + word*8
	return binary.LittleEndian.Uint64(blob[off : off+8])
}

func writeSlot(blob []byte, profile Profile, index int, words []uint64) {
	start := index * profile.SlotBytes
	for j, w := range words {
		binary.LittleEndian.PutUint64(blob[start+j*8:start+j*8+8], w)
	}
}

// FoldSlot is the §11.1 accumulator. acc starts at 0. Every little-endian
// word of slot is mixed with its index and the state, then added with
// wrapping addition. An XOR of the mix64 outputs is not this function.
// A shorter read is not this function.
func FoldSlot(slot []byte, state uint64) uint64 {
	var acc uint64
	n := len(slot) / 8
	for w := 0; w < n; w++ {
		word := binary.LittleEndian.Uint64(slot[w*8 : w*8+8])
		acc += mix64(word ^ uint64(w) ^ state)
	}
	return acc
}

// SealState is E = SHA-256(SEAL_DOMAIN ‖ le64(S_folded)) and S' from the
// first eight digest bytes, little-endian. E is the seal digest. D is the seed.
func SealState(folded uint64) (digest []byte, next uint64) {
	sum := sealSum(folded)
	digest = append([]byte(nil), sum[:]...)
	next = binary.LittleEndian.Uint64(sum[:8])
	return digest, next
}

func sealSum(folded uint64) [32]byte {
	var buf [20]byte
	copy(buf[:12], SealDomain)
	binary.LittleEndian.PutUint64(buf[12:], folded)
	return sha256.Sum256(buf[:])
}

// StepOnce is one §11.1 step. It reads the whole selected slot from the
// terrain bytes. It does not write terrain.
func (t *Terrain) StepOnce(state uint64) (folded, next uint64) {
	nslots := uint64(t.Profile.NSlots)
	slot := int(state % nslots)
	start := slot * t.Profile.SlotBytes
	folded = FoldSlot(t.Blob[start:start+t.Profile.SlotBytes], state)
	_, next = SealState(folded)
	return folded, next
}

// Leg runs STEPS_PER_LEG steps from the previous checkpoint (§11.2).
// Each step reads the full slot mixed with that step's state.
func (t *Terrain) Leg(previous []byte) uint64 {
	state := U64LE(previous)
	nslots := uint64(t.Profile.NSlots)
	slotBytes := t.Profile.SlotBytes
	for step := 0; step < t.Profile.StepsPerLeg; step++ {
		slot := int(state % nslots)
		start := slot * slotBytes
		folded := FoldSlot(t.Blob[start:start+slotBytes], state)
		sum := sealSum(folded)
		state = binary.LittleEndian.Uint64(sum[:8])
	}
	return state
}

// SampleAt is the §11.3 window.
func SampleAt(terrain []byte, previous []byte, profile Profile) (uint64, []byte) {
	offset := U64LE(previous) % profile.OffsetModulus
	return offset, terrain[offset : offset+uint64(profile.SampleBytes)]
}

// SlotAt is the waypoint slot sample (§12.2).
func SlotAt(terrain []byte, interrupt []byte, profile Profile) (uint64, []byte) {
	index := U64LE(interrupt) % uint64(profile.NSlots)
	start := index * uint64(profile.SlotBytes)
	return index, terrain[start : start+uint64(profile.SlotBytes)]
}

// CheckpointDigest is §11.4.
func CheckpointDigest(terrain, previous, limb []byte, leg uint64, profile Profile) []byte {
	_, sample := SampleAt(terrain, previous, profile)
	return SHA256(previous, limb, sample, Le64(leg))
}

// FoldCheckpoints is §12.2. F0 is 32 zero bytes and is not itself hashed.
func FoldCheckpoints(checkpoints [][]byte) []byte {
	state := make([]byte, 32)
	for _, item := range checkpoints {
		state = SHA256(state, item)
	}
	return state
}

// WaypointDigest is W_{g+1} under §12.2.
func WaypointDigest(terrain, interrupt, limb []byte, checkpoints [][]byte, profile Profile) []byte {
	folded := FoldCheckpoints(checkpoints)
	_, slot := SlotAt(terrain, interrupt, profile)
	return SHA256(interrupt, slot, limb, folded)
}

// SeedValue is SHA-256(public_key ‖ seed_beacon). It is stages[0].waypoint.
func SeedValue(publicKey, seedBeacon []byte) []byte {
	return SHA256(publicKey, seedBeacon)
}

// DrawSix is the unbiased distinct-index draw in §14.6.
// A leg total below 6 is "not defined". A short byte source is "not read".
func DrawSix(stream []byte, legTotal uint64) (indices []uint64, token string) {
	if legTotal < 6 {
		return nil, "not defined"
	}
	chosen := make([]uint64, 0, 6)
	pos := 0
	for len(chosen) < 6 {
		if pos+8 > len(stream) {
			return nil, "not read"
		}
		draw := binary.LittleEndian.Uint64(stream[pos : pos+8])
		pos += 8
		if belowLimit(draw, legTotal) {
			index := draw % legTotal
			if !containsU64(chosen, index) {
				chosen = append(chosen, index)
			}
		}
	}
	return chosen, ""
}

func belowLimit(u, n uint64) bool {
	if n == 0 {
		return false
	}
	// 2^64 % n, as a uint64. Zero means every u is inside the limit.
	rem := uint64(-n) % n
	if rem == 0 {
		return true
	}
	limit := -rem
	return u < limit
}

func containsU64(xs []uint64, v uint64) bool {
	for _, x := range xs {
		if x == v {
			return true
		}
	}
	return false
}

// AuditStream is the CLI demo draw: SHA-256(seed ‖ le64(i)) blocks.
func AuditStream(seed []byte, nbytes int) []byte {
	out := make([]byte, 0, nbytes)
	var i uint64
	for len(out) < nbytes {
		block := SHA256(seed, Le64(i))
		out = append(out, block...)
		i++
	}
	return out[:nbytes]
}
