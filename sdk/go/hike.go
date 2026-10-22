package pcoc

// Drive is the section 13.2 construction with a fixture tip schedule.
// Networking stays out of this core (no header source, no Bari client).
// The schedule says how many legs each stage records before its interrupt.
// That is legs_per_stage, vector fixture metadata (SPEC §16.1), not a
// section 6 constant.

// StageInput is one stage of a fixture hike.
type StageInput struct {
	Legs            int
	Beacon          []byte // the header the stage starts from; stage 0 is the seed beacon
	BeaconHeight    uint64
	Interrupt       []byte // I_g, the header that ends the stage
	InterruptHeight uint64
	Path            []byte // stages 1–17; nil on stage 0; empty is a skip
}

// HikeResult is the object built by Drive.
type HikeResult struct {
	Object  *Object
	Terrain *Terrain
	Encoded []byte
}

// Drive runs the section 13.2 loop for the given schedule and returns the
// encoded object. Paths are the bytes the caller already holds (SPEC §13.3:
// produce does not invent path bytes). Stage 17's path is stored empty.
func Drive(profile Profile, publicKey []byte, stages [18]StageInput) (*HikeResult, error) {
	if err := profile.Check(); err != nil {
		return nil, err
	}
	seedBeacon := stages[0].Beacon
	terrain, err := FillTerrain(profile, publicKey, seedBeacon)
	if err != nil {
		return nil, err
	}
	obj := &Object{
		Kind:        Kind,
		Fill:        Fill,
		SlotBytes:   uint64(profile.SlotBytes),
		NSlots:      uint64(profile.NSlots()),
		SampleBytes: uint64(profile.SampleBytes),
		StepsPerLeg: uint64(profile.StepsPerLeg),
		PublicKey:   append([]byte(nil), publicKey...),
		SeedBeacon:  append([]byte(nil), seedBeacon...),
	}
	waypoint := append([]byte(nil), terrain.Seed()...)
	for g := 0; g < 18; g++ {
		stIn := stages[g]
		prev := append([]byte(nil), waypoint...)
		limb := append([]byte(nil), waypoint...)
		var cps [][]byte
		var lrs [][]byte
		for j := 0; j < stIn.Legs; j++ {
			lr, cp := Leg(terrain, prev, limb)
			cps = append(cps, cp)
			lrs = append(lrs, Le64(lr))
			prev = cp
		}
		st := Stage{
			Beacon:       append([]byte(nil), stIn.Beacon...),
			BeaconHeight: stIn.BeaconHeight,
			LegCount:     uint64(len(cps)),
			Waypoint:     append([]byte(nil), limb...),
			Checkpoints:  cps,
			LegResults:   lrs,
		}
		if g > 0 {
			st.HasPath = true
			st.Path = append([]byte(nil), stIn.Path...)
		}
		obj.Stages = append(obj.Stages, st)
		if g < 17 {
			waypoint = Waypoint(terrain, stIn.Interrupt, limb, cps)
		} else {
			obj.LastInterrupt = append([]byte(nil), stIn.Interrupt...)
			obj.LastInterruptHeight = stIn.InterruptHeight
			obj.HasLastInterrupt = true
			obj.HasLastInterruptHeight = true
		}
	}
	return &HikeResult{Object: obj, Terrain: terrain, Encoded: EncodeObject(obj)}, nil
}
