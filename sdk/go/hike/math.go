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
	n := copy(buf[:], b)
	_ = n
	return binary.LittleEndian.Uint64(buf[:])
}

func Le64(v uint64) []byte {
	var buf [8]byte
	binary.LittleEndian.PutUint64(buf[:], v)
	return buf[:]
}

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

func bitrevParent(index int) int {
	bitsN := bits.Len(uint(index)) - 1
	if bitsN < 1 {
		bitsN = 1
	}
	return int(reverse32(uint32(index)) >> (32 - bitsN) % uint32(index))
}

func mulParent(index int) int {
	h := uint64(index) * MixAdd
	return int((h >> 32) % uint64(index))
}

// FillPin is section 10.2. seed is the 64-byte production_seed.
func FillPin(seed []byte) uint64 {
	state := U64LE(SeedDomain)
	for i := 0; i < 8; i++ {
		lane := U64LE(seed[8*i : 8*i+8])
		state = mix64(state ^ lane ^ (uint64(i) * MixAdd))
	}
	return state
}

// Terrain is the dag2 region for one public key and seed beacon.
type Terrain struct {
	Profile Profile
	Blob    []byte
	slotXOR []uint64
}

// FillTerrain builds the section 10.4 region once.
func FillTerrain(publicKey, seedBeacon []byte, profile Profile) *Terrain {
	seed := make([]byte, 0, 64)
	seed = append(seed, publicKey...)
	seed = append(seed, seedBeacon...)
	pin := FillPin(seed)
	blob := make([]byte, profile.TerrainBytes)
	slotXOR := make([]uint64, profile.NSlots)
	words := make([]uint64, profile.WordsPerSlot)
	for i := 0; i < profile.NSlots; i++ {
		base := pin ^ (uint64(i) << 32) ^ WordXOR
		for j := 0; j < profile.WordsPerSlot; j++ {
			words[j] = mix64(base ^ uint64(j))
		}
		if i > 0 {
			parents := [3]int{i - 1, bitrevParent(i), mulParent(i)}
			for k := 1; k <= 3; k++ {
				parent := parents[k-1]
				xk := uint64(k) * ParentMul
				for j := 0; j < profile.WordsPerSlot; j++ {
					p := loadWord(blob, profile, parent, j)
					rot := uint((j + k) & 63)
					p = rotl(p, rot)
					words[j] = mix64((words[j] ^ p) ^ xk)
				}
			}
			slotMix := uint64(i) * SlotMul
			for j := 0; j < profile.WordsPerSlot; j++ {
				words[j] = mix64(words[j] ^ slotMix)
			}
		}
		storeSlot(blob, slotXOR, profile, i, words)
	}
	return &Terrain{Profile: profile, Blob: blob, slotXOR: slotXOR}
}

func loadWord(blob []byte, profile Profile, slot, word int) uint64 {
	off := slot*profile.SlotBytes + word*8
	return binary.LittleEndian.Uint64(blob[off : off+8])
}

func storeSlot(blob []byte, slotXOR []uint64, profile Profile, index int, words []uint64) {
	start := index * profile.SlotBytes
	var folded uint64
	for j, w := range words {
		binary.LittleEndian.PutUint64(blob[start+j*8:start+j*8+8], w)
		folded ^= w ^ uint64(j)
	}
	slotXOR[index] = folded
}

// Leg runs STEPS_PER_LEG steps from the previous checkpoint (section 11.2).
func (t *Terrain) Leg(previous []byte) uint64 {
	state := U64LE(previous)
	var buf [20]byte
	copy(buf[:12], SealDomain)
	nslots := uint64(t.Profile.NSlots)
	for step := 0; step < t.Profile.StepsPerLeg; step++ {
		folded := state ^ t.slotXOR[state%nslots]
		binary.LittleEndian.PutUint64(buf[12:], folded)
		sum := sha256.Sum256(buf[:])
		state = binary.LittleEndian.Uint64(sum[:8])
	}
	return state
}

// SampleAt is the section 11.3 window.
func SampleAt(terrain []byte, previous []byte, profile Profile) (uint64, []byte) {
	offset := U64LE(previous) % profile.OffsetModulus
	return offset, terrain[offset : offset+uint64(profile.SampleBytes)]
}

// SlotAt is the waypoint slot sample (section 12.2).
func SlotAt(terrain []byte, interrupt []byte, profile Profile) (uint64, []byte) {
	index := U64LE(interrupt) % uint64(profile.NSlots)
	start := index * uint64(profile.SlotBytes)
	return index, terrain[start : start+uint64(profile.SlotBytes)]
}

// CheckpointDigest is section 11.4.
func CheckpointDigest(terrain, previous, limb []byte, leg uint64, profile Profile) []byte {
	_, sample := SampleAt(terrain, previous, profile)
	return SHA256(previous, limb, sample, Le64(leg))
}

// FoldCheckpoints is section 12.2. F0 is 32 zero bytes and is not itself hashed.
func FoldCheckpoints(checkpoints [][]byte) []byte {
	state := make([]byte, 32)
	for _, item := range checkpoints {
		state = SHA256(state, item)
	}
	return state
}

// WaypointDigest is W_{g+1} under section 12.2.
func WaypointDigest(terrain, interrupt, limb []byte, checkpoints [][]byte, profile Profile) []byte {
	folded := FoldCheckpoints(checkpoints)
	_, slot := SlotAt(terrain, interrupt, profile)
	return SHA256(interrupt, slot, limb, folded)
}

// SeedValue is SHA-256(public_key ‖ seed_beacon).
func SeedValue(publicKey, seedBeacon []byte) []byte {
	return SHA256(publicKey, seedBeacon)
}

// DrawSix is the unbiased distinct-index draw in section 14.6.
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
