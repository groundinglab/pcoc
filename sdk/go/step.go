package pcoc

import (
	"crypto/sha256"

	"github.com/groundinglab/pcoc/sdk/go/v2/internal/fold"
)

// Step is SPEC §11.1. One step loads the whole selected slot and folds every
// word with the current state, then seals.
//
//	i = S mod N_SLOTS
//	slot = terrain[i × SLOT_BYTES : (i+1) × SLOT_BYTES]
//	S_folded = Scalar(slot, S)          // every word, wrapping add
//	E = SHA-256(SEAL_DOMAIN ‖ le64(S_folded))
//	S' = u64_le(E[0:8])
func Step(t *Terrain, s uint64) uint64 {
	nSlots := t.p.NSlots()
	i := int(s % uint64(nSlots))
	slot := t.Slot(i)
	acc := fold.Scalar(slot, s)
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
	return U64le(e[0:8])
}

// LegResult is SPEC §11.2.
//
//	S_0 = u64_le(previousCheckpoint[0:8])
//	S_{t+1} = Step(S_t)    for t = 0 … STEPS_PER_LEG − 1
//	leg_result = S_{STEPS_PER_LEG}
func LegResult(t *Terrain, previousCheckpoint []byte) uint64 {
	s := U64le(previousCheckpoint[0:8])
	steps := t.p.StepsPerLeg
	for n := 0; n < steps; n++ {
		s = Step(t, s)
	}
	return s
}

// TerrainSample is SPEC §11.3.
//
//	offset = u64_le(previousCheckpoint[0:8]) mod OFFSET_MODULUS
//	terrain_sample = terrain[offset : offset + SAMPLE_BYTES]
func TerrainSample(t *Terrain, previousCheckpoint []byte) []byte {
	offset := U64le(previousCheckpoint[0:8]) % t.p.OffsetModulus()
	o := int(offset)
	out := make([]byte, t.p.SampleBytes)
	copy(out, t.bytes[o:o+t.p.SampleBytes])
	return out
}

// Checkpoint is SPEC §11.4 / Rule A2.3.
//
//	SHA-256(previous ‖ stageLimb ‖ terrain sample at previous ‖ le64(legResult))
//
// stageLimb is the seed on stage 0 and W_g on stage g ≥ 1.
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

// Leg is one SPEC §11 leg: the leg result and its checkpoint.
// On leg 0, previous and stageLimb are the same value (the seed, or W_g).
// On a later leg, previous is the preceding checkpoint and stageLimb stays
// the stage limb (Rule A2.3).
func Leg(t *Terrain, previous, stageLimb []byte) (uint64, []byte) {
	legResult := LegResult(t, previous)
	return legResult, Checkpoint(t, previous, stageLimb, legResult)
}

// SlotSample is SPEC §12.2.
//
//	slot_index = u64_le(B[0:8]) mod N_SLOTS
//	slot_sample = that whole slot
func SlotSample(t *Terrain, b []byte) []byte {
	idx := int(U64le(b[0:8]) % uint64(t.p.NSlots()))
	out := make([]byte, t.p.SlotBytes)
	copy(out, t.Slot(idx))
	return out
}

// CheckpointFold is SPEC §12.2.
//
//	F_0 = 0^32
//	F_{j+1} = SHA-256(F_j ‖ C[j])
//	fold = F_n
//
// F_0 is not itself hashed. n = 0 leaves fold = 0^32.
func CheckpointFold(checkpoints [][]byte) []byte {
	foldBytes := make([]byte, 32)
	for _, c := range checkpoints {
		h := sha256.New()
		h.Write(foldBytes)
		h.Write(c)
		foldBytes = h.Sum(nil)
	}
	return foldBytes
}

// Waypoint is SPEC §12.2 / Rule A1.3.
//
//	W_{g+1} = SHA-256(B ‖ slot_sample ‖ W ‖ fold)
//
// B is the interrupt I_g. W is stages[g].waypoint (the seed when g = 0).
func Waypoint(t *Terrain, interrupt, stageWaypoint []byte, checkpoints [][]byte) []byte {
	sample := SlotSample(t, interrupt)
	folded := CheckpointFold(checkpoints)
	h := sha256.New()
	h.Write(interrupt)
	h.Write(sample)
	h.Write(stageWaypoint)
	h.Write(folded)
	return h.Sum(nil)
}
