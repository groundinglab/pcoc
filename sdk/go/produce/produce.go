// Package produce is the scripted-rig producer (SPEC §13.2).
//
// The rig supplies the beacon schedule. There is no network, no sleep, and
// no Bari exchange. Each stage runs the scheduled number of legs, then the
// next beacon is the interrupt. Paths on stages 1–17 are the empty bytes
// SPEC §13.3 stores for a skip, and stage 17 is not fetched by design.
package produce

import (
	"crypto/sha256"
	"fmt"

	"github.com/groundinglab/pcoc/sdk/go/v2"
)

// Beacon is one header the rig's tip schedule already holds.
type Beacon struct {
	Hash   []byte
	Height uint64
}

// Drive runs one scripted hike and returns the section 7.3 object bytes.
// beacons[0] is the seed beacon. beacons[1] through beacons[18] are the
// interrupts I_0 through I_17. legsPerStage has 18 integers, each at least 1.
func Drive(profile pcoc.Profile, publicKey []byte, beacons []Beacon, legsPerStage []int) ([]byte, error) {
	if err := profile.Check(); err != nil {
		return nil, err
	}
	if len(beacons) != pcoc.Stages+1 {
		return nil, fmt.Errorf("produce: beacon count")
	}
	if len(legsPerStage) != pcoc.Stages {
		return nil, fmt.Errorf("produce: legs")
	}
	for _, n := range legsPerStage {
		if n < 1 {
			return nil, fmt.Errorf("produce: legs")
		}
	}
	terrain, err := pcoc.FillTerrain(profile, publicKey, beacons[0].Hash)
	if err != nil {
		return nil, err
	}
	seed := terrain.Seed()
	waypoint := append([]byte(nil), seed...)
	stages := make([]pcoc.Stage, pcoc.Stages)
	for g := 0; g < pcoc.Stages; g++ {
		prev := append([]byte(nil), waypoint...)
		limb := append([]byte(nil), waypoint...)
		cps := make([][]byte, 0, legsPerStage[g])
		lrs := make([][]byte, 0, legsPerStage[g])
		for j := 0; j < legsPerStage[g]; j++ {
			lr := pcoc.LegResult(terrain, prev)
			cp := pcoc.Checkpoint(terrain, prev, limb, lr)
			cps = append(cps, cp)
			lrs = append(lrs, pcoc.Le64(lr))
			prev = cp
		}
		st := pcoc.Stage{
			Beacon:       append([]byte(nil), beacons[g].Hash...),
			BeaconHeight: beacons[g].Height,
			HasBeacon:    true,
			HasHeight:    true,
			LegCount:     uint64(legsPerStage[g]),
			HasLegCount:  true,
			Waypoint:     append([]byte(nil), waypoint...),
			HasWaypoint:  true,
			Checkpoints:  cps,
			LegResults:   lrs,
			HasCP:        true,
			HasLR:        true,
			Present:      true,
		}
		if g > 0 {
			st.HasPath = true
			st.Path = []byte{}
		}
		stages[g] = st
		if g < 17 {
			interrupt := beacons[g+1].Hash
			waypoint = pcoc.Waypoint(terrain, interrupt, waypoint, cps)
		}
	}
	obj := &pcoc.Object{
		Kind:                   pcoc.Kind,
		HasKind:                true,
		Fill:                   pcoc.FillName,
		HasFill:                true,
		SlotBytes:              uint64(profile.SlotBytes),
		HasSlotBytes:           true,
		NSlots:                 uint64(profile.NSlots()),
		HasNSlots:              true,
		SampleBytes:            uint64(profile.SampleBytes),
		HasSampleBytes:         true,
		StepsPerLeg:            uint64(profile.StepsPerLeg),
		HasSteps:               true,
		PublicKey:              append([]byte(nil), publicKey...),
		HasPublicKey:           true,
		SeedBeacon:             append([]byte(nil), beacons[0].Hash...),
		HasSeedBeacon:          true,
		Stages:                 stages,
		HasStages:              true,
		StagesLenOK:            true,
		LastInterrupt:          append([]byte(nil), beacons[18].Hash...),
		HasLastInterrupt:       true,
		LastInterruptHeight:    beacons[18].Height,
		HasLastInterruptHeight: true,
	}
	return pcoc.EncodeObject(obj), nil
}

// BeaconAt is the CASES synthetic beacon.
// k < 0 is the seed beacon. k in 0 … 17 is I_k.
func BeaconAt(k int) Beacon {
	var label string
	var height uint64
	if k < 0 {
		label = "pcoc-pack-2.2/beacon/seed"
		height = 1000
	} else {
		label = fmt.Sprintf("pcoc-pack-2.2/beacon/%d", k)
		height = uint64(1001 + k)
	}
	sum := sha256.Sum256([]byte(label))
	return Beacon{Hash: sum[:], Height: height}
}

// CASESBeacons is the seed beacon and I_0 … I_17.
func CASESBeacons() []Beacon {
	out := make([]Beacon, pcoc.Stages+1)
	out[0] = BeaconAt(-1)
	for k := 0; k < pcoc.Stages; k++ {
		out[k+1] = BeaconAt(k)
	}
	return out
}

// StderrClose is the SPEC §13.2 stderr step at the close: path_skip for
// g = 1 … 16, then not fetched by design 17. The caller writes it.
func StderrClose() string {
	var b []byte
	for g := 1; g <= 16; g++ {
		b = append(b, []byte(fmt.Sprintf("path_skip %d\n", g))...)
	}
	b = append(b, []byte("not fetched by design 17\n")...)
	return string(b)
}
