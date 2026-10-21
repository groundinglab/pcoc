package hike

import (
	"bytes"
	"encoding/hex"
	"testing"
)

// Section 6, rules P6.1–P6.25, and the §16.1 conformance-small sizes.
// P6.14 is retired. There is no SEED_DOMAIN constant.
func TestSpec6ConstantsArePinned(t *testing.T) {
	type pin struct {
		rule      string
		got, want uint64
	}
	p := Production
	pins := []pin{
		{"P6.1 SLOT_BYTES", uint64(p.SlotBytes), 16384},
		{"P6.2 WORDS_PER_SLOT", uint64(p.WordsPerSlot), 2048},
		{"P6.3 TERRAIN_BYTES", uint64(p.TerrainBytes), 268435456},
		{"P6.4 N_SLOTS", uint64(p.NSlots), 16384},
		{"P6.5 SAMPLE_BYTES", uint64(p.SampleBytes), 1024},
		{"P6.6 OFFSET_MODULUS", p.OffsetModulus, 268434433},
		{"P6.7 STEPS_PER_LEG", uint64(p.StepsPerLeg), 2000000},
		{"P6.8 STAGES", Stages, 18},
		{"P6.9 HASH_BYTES", HashBytes, 32},
		{"P6.10 LEG_RESULT_BYTES", LegResultBytes, 8},
		{"P6.11 PUBLIC_KEY_BYTES", PublicKeyBytes, 32},
		{"P6.12 BEACON_BYTES", BeaconBytes, 32},
		{"P6.17 MIX_ADD", MixAdd, 0x9E3779B97F4A7C15},
		{"P6.18 MIX_MUL1", MixMul1, 0xBF58476D1CE4E5B9},
		{"P6.19 MIX_MUL2", MixMul2, 0x94D049BB133111EB},
		{"P6.20 WORD_XOR", WordXOR, 0xC0FFEE},
		{"P6.21 PARENT_MUL", ParentMul, 0xD1B54A32D192ED03},
		{"P6.22 SLOT_MUL", SlotMul, 0xA24BAED4963EE407},
	}
	for _, x := range pins {
		if x.got != x.want {
			t.Errorf("%s = %d, want %d", x.rule, x.got, x.want)
		}
	}
	if string(SealDomain) != "pcoc-walk-v1" || len(SealDomain) != 12 {
		t.Errorf("P6.13 SEAL_DOMAIN %q", SealDomain)
	}
	// P6.14 is retired. The seed is SHA-256 of the 64-byte preimage, no label.
	pre := append(bytes.Repeat([]byte{0x11}, 32), bytes.Repeat([]byte{0x22}, 32)...)
	if hex.EncodeToString(SeedValue(pre[:32], pre[32:])) != "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c" {
		t.Error("P6.14 retired seed")
	}
	if Kind != "satoshi-hike-v2.0-object" {
		t.Errorf("P6.15 KIND %q", Kind)
	}
	if Fill != "dag2" {
		t.Errorf("P6.16 FILL %q", Fill)
	}
	s := ConformanceSmallV2
	if s.SampleBytes != 32 || s.SlotBytes != 64 || s.TerrainBytes != 256 || s.StepsPerLeg != 32 {
		t.Errorf("16.1 conformance-small sizes %+v", s)
	}
	// P6.23-P6.25 on the small sizes.
	if s.WordsPerSlot != 8 || s.NSlots != 4 || s.OffsetModulus != 225 || s.Kind != Kind {
		t.Errorf("16.1 conformance-small derived %+v", s)
	}
}
