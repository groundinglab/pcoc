package pcoc

import "crypto/sha256"

// Step is SPEC §11.1. S is the current u64 state. The next state is S'.
//
// The loop reads every word of the selected slot. acc starts at 0. Each word
// is mixed as word(w) XOR S, with mix64 including MIX_ADD, and added with
// wrapping addition. There is no w term, no cache, and no summary.
//
//	i = S mod N_SLOTS
//	acc = 0
//	for w = 0 … WORDS_PER_SLOT − 1:
//	    acc = (acc + mix64(word(w) XOR S)) mod 2^64
//	E = SHA-256(SEAL_DOMAIN ‖ le64(acc))
//	S' = u64_le(E[0:8])
func Step(t *Terrain, s uint64) uint64 {
	nSlots := t.p.NSlots()
	words := t.p.WordsPerSlot()
	i := int(s % uint64(nSlots))
	slot := t.Slot(i)

	var acc uint64
	for w := 0; w < words; w++ {
		word := u64le(slot[8*w : 8*w+8])
		acc += Mix64(word ^ s)
	}
	return SealState(acc)
}

// SealState is the SPEC §11.1 seal.
//
//	E = SHA-256(SEAL_DOMAIN ‖ le64(S_folded))
//	S' = u64_le(E[0:8])
func SealState(sFolded uint64) uint64 {
	var buf [20]byte
	copy(buf[0:12], SealDomain)
	putU64le(buf[12:20], sFolded)
	e := sha256.Sum256(buf[:])
	return u64le(e[0:8])
}

// LegResult is SPEC §11.2.
//
//	S_0 = u64_le(previousCheckpoint[0:8])
//	S_{t+1} = Step(S_t)    for t = 0 … STEPS_PER_LEG − 1
//	leg_result = S_{STEPS_PER_LEG}
func LegResult(t *Terrain, previousCheckpoint []byte) uint64 {
	s := u64le(previousCheckpoint[0:8])
	steps := t.p.StepsPerLeg
	for n := 0; n < steps; n++ {
		s = Step(t, s)
	}
	return s
}

// TerrainSample is SPEC §11.3. The window is SAMPLE_BYTES at
//
//	offset = u64_le(previousCheckpoint[0:8]) mod OFFSET_MODULUS
//
// It is not aligned to a slot boundary and it is not the waypoint slot sample.
func TerrainSample(t *Terrain, previousCheckpoint []byte) []byte {
	offset := u64le(previousCheckpoint[0:8]) % t.p.OffsetModulus()
	o := int(offset)
	out := make([]byte, t.p.SampleBytes)
	copy(out, t.bytes[o:o+t.p.SampleBytes])
	return out
}

// Checkpoint is SPEC §11.4 / Rule A2.3.
//
//	H(previous ‖ stageLimb ‖ terrainSample(at previous) ‖ le64(legResult))
//
// stageLimb is the seed on stage 0 and W_g on stage g ≥ 1. Each is 32 bytes.
func Checkpoint(t *Terrain, previous, stageLimb []byte, legResult uint64) []byte {
	sample := TerrainSample(t, previous)
	h := sha256.New()
	h.Write(previous)
	h.Write(stageLimb)
	h.Write(sample)
	var lr [8]byte
	putU64le(lr[:], legResult)
	h.Write(lr[:])
	return h.Sum(nil)
}

// Leg is one SPEC §11 leg: the leg result and the checkpoint built from it.
// previous and stageLimb are each 32 bytes. On leg 0 they are the same value
// (the seed, or W_g). On a later leg, previous is the preceding checkpoint
// and stageLimb stays the stage limb (Rule A2.3).
func Leg(t *Terrain, previous, stageLimb []byte) (legResult uint64, checkpoint []byte) {
	legResult = LegResult(t, previous)
	checkpoint = Checkpoint(t, previous, stageLimb, legResult)
	return legResult, checkpoint
}

// SlotSample is SPEC §12.2: the full slot indexed by
//
//	slot_index = u64_le(B[0:8]) mod N_SLOTS
func SlotSample(t *Terrain, b []byte) []byte {
	idx := int(u64le(b[0:8]) % uint64(t.p.NSlots()))
	out := make([]byte, t.p.SlotBytes)
	copy(out, t.Slot(idx))
	return out
}

// CheckpointFold is SPEC §12.2.
//
//	F_0 = 0^32
//	F_{j+1} = SHA-256(F_j ‖ C[j])    for j = 0 … n − 1
//	fold = F_n
//
// F_0 is not itself hashed. n = 0 leaves fold = 0^32 and performs no hash.
func CheckpointFold(checkpoints [][]byte) []byte {
	fold := make([]byte, 32)
	for _, c := range checkpoints {
		h := sha256.New()
		h.Write(fold)
		h.Write(c)
		fold = h.Sum(nil)
	}
	return fold
}

// Waypoint is SPEC §12.2 / Rule A1.3.
//
//	W_{g+1} = SHA-256(B ‖ slot_sample ‖ W ‖ fold)
//
// B is I_g (32 bytes). W is stages[g].waypoint (the seed when g = 0).
// checkpoints are stages[g].checkpoints in leg order.
func Waypoint(t *Terrain, interrupt, stageWaypoint []byte, checkpoints [][]byte) []byte {
	sample := SlotSample(t, interrupt)
	fold := CheckpointFold(checkpoints)
	h := sha256.New()
	h.Write(interrupt)
	h.Write(sample)
	h.Write(stageWaypoint)
	h.Write(fold)
	return h.Sum(nil)
}
