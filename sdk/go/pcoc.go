// Package pcoc is the Satoshi Hike 2.2 core (SPEC 2.2).
//
// SPEC §16.2 Rule I2. Each exported function names the section it implements.
// The version constant is 2.2.0 (Rule I5).
package pcoc

// Version is the SDK version. SPEC §16.2 Rule I5: MAJOR.MINOR equals this
// file's version, 2.2.
const Version = "2.2.0"

// Kind is SPEC §6 KIND, the only object kind this construction reads.
const Kind = "v2.2-pcoc-object"

// Fill is SPEC §6 FILL.
const Fill = "dag2"

// Production profile, SPEC §6. Under conformance-small the same formulas
// apply to that profile's SLOT_BYTES, TERRAIN_BYTES, and SAMPLE_BYTES (§6,
// §16.1). These names are the section 6 evaluation.
const (
	SlotBytes     = 16384
	WordsPerSlot  = 2048 // SLOT_BYTES / 8
	TerrainBytes  = 268435456
	NSlots        = 16384 // TERRAIN_BYTES / SLOT_BYTES
	SampleBytes   = 1024
	OffsetModulus = 268434433 // TERRAIN_BYTES - SAMPLE_BYTES + 1
	StepsPerLeg   = 2000000
	Stages        = 18
	HashBytes     = 32
	LegResultBytes = 8
	PublicKeyBytes = 32
	BeaconBytes    = 32
)

// SealDomain is the twelve ASCII bytes of SPEC §6 SEAL_DOMAIN.
var SealDomain = []byte("pcoc-walk-v1")

// Mix constants, SPEC §6.
const (
	MixAdd    uint64 = 0x9E3779B97F4A7C15
	MixMul1   uint64 = 0xBF58476D1CE4E5B9
	MixMul2   uint64 = 0x94D049BB133111EB
	WordXor   uint64 = 0xC0FFEE
	ParentMul uint64 = 0xD1B54A32D192ED03
	SlotMul   uint64 = 0xA24BAED4963EE407
)

// AnchorWindowOffsets is SPEC §13.1 ANCHOR_WINDOW_OFFSETS, written once.
var AnchorWindowOffsets = []int{2, 3, 4}

// SixLegDrawCount is SPEC §14.6. It is not a section 6 constant.
const SixLegDrawCount = 6

// HashcashDifficultyBits is SPEC §13.1, 21 leading zero bits (P13.5).
const HashcashDifficultyBits = 21

// HashcashPrefix is the ten ASCII bytes "bari-hc-v2" (SPEC §13.1).
var HashcashPrefix = []byte("bari-hc-v2")

// Profile is the profile in force (SPEC §14, §16.1).
type Profile struct {
	Name         string
	SlotBytes    int
	SampleBytes  int
	TerrainBytes int
	StepsPerLeg  int
}

// Production is the section 6 profile.
func Production() Profile {
	return Profile{
		Name:         "production",
		SlotBytes:    SlotBytes,
		SampleBytes:  SampleBytes,
		TerrainBytes: TerrainBytes,
		StepsPerLeg:  StepsPerLeg,
	}
}

// WordsPerSlotOf is SLOT_BYTES / 8 (P6.23). The caller has already checked
// that the division is exact.
func (p Profile) WordsPerSlot() int { return p.SlotBytes / 8 }

// NSlotsOf is TERRAIN_BYTES / SLOT_BYTES (P6.24).
func (p Profile) NSlots() int { return p.TerrainBytes / p.SlotBytes }

// OffsetModulusOf is TERRAIN_BYTES − SAMPLE_BYTES + 1 (P6.25).
func (p Profile) OffsetModulus() uint64 {
	return uint64(p.TerrainBytes - p.SampleBytes + 1)
}

// Check reports whether the four conformance-small integers meet §16.1.
// A production profile is always valid.
func (p Profile) Check() error {
	if p.SlotBytes < 8 || p.SlotBytes > SlotBytes || p.SlotBytes%8 != 0 {
		return errProfile("slot_bytes")
	}
	if p.StepsPerLeg < 1 || p.StepsPerLeg > StepsPerLeg {
		return errProfile("steps_per_leg")
	}
	if p.SampleBytes < 1 || p.SampleBytes > SampleBytes {
		return errProfile("sample_bytes")
	}
	if p.TerrainBytes > TerrainBytes || p.TerrainBytes%p.SlotBytes != 0 || p.TerrainBytes < p.SampleBytes {
		return errProfile("terrain_bytes")
	}
	if p.Name != "production" && p.Name != "conformance-small" {
		return errProfile("name")
	}
	if p.Name == "production" {
		if p.SlotBytes != SlotBytes || p.SampleBytes != SampleBytes || p.TerrainBytes != TerrainBytes || p.StepsPerLeg != StepsPerLeg {
			return errProfile("production")
		}
	}
	return nil
}

type profileError string

func (e profileError) Error() string { return "profile: " + string(e) }

func errProfile(what string) error { return profileError(what) }
