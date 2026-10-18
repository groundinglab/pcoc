// Package hike is a clean-room Satoshi Hike produce and verify.
// The current profile is Satoshi Hike v2.0. An earlier kind string is
// recognized and is not measured. This package does not apply section 15.
package hike

import (
	"os"
	"strconv"
	"strings"
)

// Section 6 sizes, the version tokens, and the locked v2.0 policy numbers.
// conformance-small may change only the four free sizes; the three derived
// integers stay the section 6 formulas. KIND and the anchor offsets are
// SPEC v2.0 P6.15 and P13.2.
const (
	Stages         = 18
	HashBytes      = 32
	LegResultBytes = 8
	PublicKeyBytes = 32
	BeaconBytes    = 32
	// KindV135 is the earlier kind string Rule A5 names. It is not a profile.
	KindV135 = "satoshi-hike-v1.35-object"
	// Kind is SPEC v2.0 P6.15.
	Kind = "satoshi-hike-v2.0-object"
	Fill = "dag2"
	// EarlierVersionToken is Rule A5. The record key is version.
	// Gate reason earlier_version is section 15 and is not written here.
	EarlierVersionToken = "earlier version"
	// DefaultBindThreads is how many Rule A4.3 checkpoint comparisons run
	// at once when the caller does not set a count. It is not STEPS_PER_LEG.
	// One at a time is conforming. The verify record does not depend on it.
	DefaultBindThreads = 6
	JSONIntMax         = (1 << 53) - 1
	HeaderTimeMax      = uint64(1<<32 - 1)
	// No leg-rate floor here (rc.2): the spec defines measurements, a policy
	// sets the values. The section 15.5 example pairs are test fixtures and
	// spec/conformance/v2.0/constants.json; q·N ≥ p·S_eff is gate policy.
	// AnchorWindow is the locked count of blocks in the path-ask window.
	AnchorWindow = 3
)

var (
	SealDomain = []byte("pcoc-walk-v1")
	SeedDomain = []byte{'P', 'S', 'E', 'E', 'D', '6', '4', 0}
)

const (
	MixAdd    uint64 = 0x9E3779B97F4A7C15
	MixMul1   uint64 = 0xBF58476D1CE4E5B9
	MixMul2   uint64 = 0x94D049BB133111EB
	WordXOR   uint64 = 0xC0FFEE
	ParentMul uint64 = 0xD1B54A32D192ED03
	SlotMul   uint64 = 0xA24BAED4963EE407
)

// anchorOffsetsAfterPost is SPEC v2.0 P13.2, written once. Producer and
// verifier both use this list. A plant posted at H is asked at H+2, H+3,
// and H+4.
var anchorOffsetsAfterPost = []int{2, 3, 4}

// AnchorOffsetsAfterPost returns a copy of the §13 offset list.
func AnchorOffsetsAfterPost() []int {
	return append([]int(nil), anchorOffsetsAfterPost...)
}

// AnchorWindowOrigin is SPEC §13.1. The one height H is the stage's stored
// beacon height. The Hashcash tip on the POST is not H. The object does not
// store a second post height.
func AnchorWindowOrigin(stageBeaconHeight uint64) uint64 {
	return stageBeaconHeight
}

// IsEarlierKind reports the Rule A5 kind. A kind that is neither Kind nor
// this text stays a section 14.1 shape measurement.
func IsEarlierKind(kind string) bool {
	return kind == KindV135
}

// BindThreadsFrom reads PCOC_BIND_THREADS. Blank, unset, and non-positive
// text use DefaultBindThreads. The count is not STEPS_PER_LEG.
func BindThreadsFrom(text string) int {
	text = strings.TrimSpace(text)
	if text == "" {
		return DefaultBindThreads
	}
	n, err := strconv.Atoi(text)
	if err != nil || n < 1 {
		return DefaultBindThreads
	}
	return n
}

// ConfiguredBindThreads is BindThreadsFrom on the process environment.
func ConfiguredBindThreads() int {
	return BindThreadsFrom(os.Getenv("PCOC_BIND_THREADS"))
}

// Profile is the parameter set verify measures against.
type Profile struct {
	Name          string
	Kind          string
	SampleBytes   int
	SlotBytes     int
	TerrainBytes  int
	StepsPerLeg   int
	WordsPerSlot  int
	NSlots        int
	OffsetModulus uint64
}

// Production is the current section 6 evaluation: v2.0 sample and steps.
// Slot and terrain sizes are unchanged; #206 does not retune them.
var Production = mustProfile("production", Kind, 1024, 16384, 268435456, 2000000)

// ConformanceSmallV2 is SPEC §16.1 on the v2.0 kind. The four integers are
// sample 32, slot 64, terrain 256, and steps 32. KIND stays the v2.0 kind.
var ConformanceSmallV2 = mustProfile("conformance-small", Kind, 32, 64, 256, 32)

func mustProfile(name, kind string, sample, slot, terrain, steps int) Profile {
	p, err := NewProfile(name, kind, sample, slot, terrain, steps)
	if err != nil {
		panic(err)
	}
	return p
}

// NewProfile applies P6.23, P6.24, and P6.25.
func NewProfile(name, kind string, sample, slot, terrain, steps int) (Profile, error) {
	if slot < 8 || slot%8 != 0 {
		return Profile{}, errProfile
	}
	if terrain%slot != 0 {
		return Profile{}, errProfile
	}
	if sample < 1 || terrain < sample || steps < 1 {
		return Profile{}, errProfile
	}
	if kind == "" {
		return Profile{}, errProfile
	}
	return Profile{
		Name:          name,
		Kind:          kind,
		SampleBytes:   sample,
		SlotBytes:     slot,
		TerrainBytes:  terrain,
		StepsPerLeg:   steps,
		WordsPerSlot:  slot / 8,
		NSlots:        terrain / slot,
		OffsetModulus: uint64(terrain - sample + 1),
	}, nil
}

type profileError struct{}

func (profileError) Error() string { return "profile integers do not divide" }

var errProfile = profileError{}

// MeasureName selects section 6 unless the verifier names conformance-small.
// That name is the v2.0 §16.1 profile. test, live, and production measure
// with the section 6 constants.
func MeasureName(profile string) Profile {
	if profile == ConformanceSmallV2.Name {
		return ConformanceSmallV2
	}
	return Production
}
