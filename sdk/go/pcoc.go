// Package pcoc is the Satoshi Hike 2.3 construction (SPEC 2.3).
//
// SPEC §16.2 Rule I2. Each exported function names the section it implements.
// The version is 2.3.0 (Rule I5). The only object kind is v2.2-pcoc-object.
// The module tag is sdk/go/v2.3.0.
package pcoc

import "github.com/groundinglab/pcoc/sdk/go/v2/internal/fold"

// Version is the SDK version. SPEC §16.2 Rule I5: MAJOR.MINOR is 2.3.
const Version = "2.3.0"

// SpecTag is the tag of this specification (SPEC §16).
const SpecTag = "spec-2.3.0"

// Kind is SPEC §6 KIND.
const Kind = "v2.2-pcoc-object"

// FillName is SPEC §6 FILL.
const FillName = "dag2"

// Section 6 constants. Under conformance-small the same formulas apply to
// that profile's SLOT_BYTES, TERRAIN_BYTES, and SAMPLE_BYTES (SPEC §6, §16.1).
const (
	SlotBytes      = 16384
	WordsPerSlot   = 2048 // SLOT_BYTES / 8
	TerrainBytes   = 268435456
	NSlots         = 16384 // TERRAIN_BYTES / SLOT_BYTES
	SampleBytes    = 1024
	OffsetModulus  = 268434433 // TERRAIN_BYTES − SAMPLE_BYTES + 1
	StepsPerLeg    = 2000000
	Stages         = 18
	HashBytes      = 32
	LegResultBytes = 8
	PublicKeyBytes = 32
	BeaconBytes    = 32
)

// SealDomain is the twelve ASCII bytes of SPEC §6 SEAL_DOMAIN.
var SealDomain = []byte("pcoc-walk-v1")

// Mix constants, SPEC §6. Mix64 lives in the scalar fold.
const (
	MixAdd    uint64 = fold.MixAdd
	MixMul1   uint64 = fold.MixMul1
	MixMul2   uint64 = fold.MixMul2
	WordXor   uint64 = 0xC0FFEE
	ParentMul uint64 = 0xD1B54A32D192ED03
	SlotMul   uint64 = 0xA24BAED4963EE407
)

// AnchorWindowOffsets is SPEC §13.1 ANCHOR_WINDOW_OFFSETS.
var AnchorWindowOffsets = []int{2, 3, 4}

// SixLegDrawCount is SPEC §14.6. It is not a section 6 constant.
const SixLegDrawCount = 6

// HashcashDifficultyBits is the one Hashcash difficulty (SPEC §13.1).
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

// WordsPerSlot is SLOT_BYTES / 8 (SPEC §6, P6.23).
func (p Profile) WordsPerSlot() int { return p.SlotBytes / 8 }

// NSlots is TERRAIN_BYTES / SLOT_BYTES (SPEC §6, P6.24).
func (p Profile) NSlots() int { return p.TerrainBytes / p.SlotBytes }

// OffsetModulus is TERRAIN_BYTES − SAMPLE_BYTES + 1 (SPEC §6, P6.25).
func (p Profile) OffsetModulus() uint64 {
	return uint64(p.TerrainBytes - p.SampleBytes + 1)
}

// Check reports whether the four integers meet SPEC §16.1.
// A production profile must be the section 6 numbers.
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
