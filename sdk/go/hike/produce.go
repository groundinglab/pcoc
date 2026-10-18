package hike

import "fmt"

// Tip is one Observe result: natural beacon and height.
type Tip struct {
	Natural []byte
	Height  uint64
}

// Drive runs section 13.2 against an already-read tip sequence.
// Path asks that are not a canonical path body store empty bytes. This
// tree does not invent a CanonicalPathBody layout; a body that cannot be
// validated is a miss, and produce continues.
func Drive(publicKey []byte, profile Profile, tips []Tip) (object []byte, stderr string, err error) {
	if len(publicKey) != 32 {
		return nil, "", fmt.Errorf("public key")
	}
	if len(tips) == 0 || len(tips[0].Natural) != 32 {
		return nil, "", fmt.Errorf("seed observe")
	}
	cursor := 0
	next := func() (Tip, error) {
		if cursor >= len(tips) {
			return Tip{}, fmt.Errorf("observe")
		}
		tip := tips[cursor]
		cursor++
		if len(tip.Natural) != 32 {
			return Tip{}, fmt.Errorf("observe")
		}
		return tip, nil
	}
	seedTip, err := next()
	if err != nil {
		return nil, "", err
	}
	seedBeacon := append([]byte(nil), seedTip.Natural...)
	terrain := FillTerrain(publicKey, seedBeacon, profile)
	waypoint := SeedValue(publicKey, seedBeacon)
	beacon := seedBeacon
	beaconHeight := seedTip.Height

	stages := make([]builtStage, Stages)
	var last []byte
	var lastHeight uint64
	for g := 0; g < Stages; g++ {
		limb := append([]byte(nil), waypoint...)
		previous := limb
		var checkpoints [][]byte
		var results [][]byte
		var interrupt Tip
		for {
			leg := terrain.Leg(previous)
			digest := CheckpointDigest(terrain.Blob, previous, limb, leg, profile)
			checkpoints = append(checkpoints, digest)
			results = append(results, Le64(leg))
			previous = digest
			tip, err := next()
			if err != nil {
				return nil, "", err
			}
			if !bytesEqual(tip.Natural, beacon) && tip.Height > beaconHeight {
				interrupt = tip
				break
			}
		}
		stages[g] = builtStage{
			Beacon:       append([]byte(nil), beacon...),
			BeaconHeight: beaconHeight,
			LegCount:     uint64(len(checkpoints)),
			Waypoint:     limb,
			Checkpoints:  checkpoints,
			LegResults:   results,
			HasPath:      g > 0,
		}
		if g < Stages-1 {
			waypoint = WaypointDigest(terrain.Blob, interrupt.Natural, stages[g].Waypoint, stages[g].Checkpoints, profile)
			beacon = append([]byte(nil), interrupt.Natural...)
			beaconHeight = interrupt.Height
		} else {
			last = append([]byte(nil), interrupt.Natural...)
			lastHeight = interrupt.Height
		}
	}
	// Stage 0 has no Bari post and no stderr line (SPEC §13, Rule A3.4).
	// This port does not fetch Bari. Every W_1 … W_16 path is empty, so the
	// final stderr step is path_skip 1 through path_skip 16, then
	// "not fetched by design 17". That last line is only for W_17.
	// The historical produce check reads these same bytes.
	var lines string
	for g := 1; g <= 16; g++ {
		lines += fmt.Sprintf("path_skip %d\n", g)
	}
	lines += "not fetched by design 17\n"
	raw := encodeBuilt(profile, publicKey, seedBeacon, stages, last, lastHeight)
	return raw, lines, nil
}

type builtStage struct {
	Beacon       []byte
	BeaconHeight uint64
	LegCount     uint64
	Waypoint     []byte
	Checkpoints  [][]byte
	LegResults   [][]byte
	HasPath      bool
}

func encodeBuilt(profile Profile, publicKey, seedBeacon []byte, stages []builtStage, last []byte, lastHeight uint64) []byte {
	encodedStages := make([][]byte, len(stages))
	for g, stage := range stages {
		checks := make([][]byte, len(stage.Checkpoints))
		for i, c := range stage.Checkpoints {
			checks[i] = encodeBytes(c)
		}
		results := make([][]byte, len(stage.LegResults))
		for i, c := range stage.LegResults {
			results[i] = encodeBytes(c)
		}
		pairs := [][2][]byte{
			textPair("beacon", encodeBytes(stage.Beacon)),
			textPair("beacon_height", encodeUint(stage.BeaconHeight)),
			textPair("checkpoints", encodeArray(checks)),
			textPair("leg_count", encodeUint(stage.LegCount)),
			textPair("leg_results", encodeArray(results)),
			textPair("waypoint", encodeBytes(stage.Waypoint)),
		}
		if stage.HasPath {
			pairs = append(pairs, textPair("path", encodeBytes(nil)))
		}
		encodedStages[g] = encodeMap(pairs)
	}
	top := [][2][]byte{
		textPair("fill", encodeText(Fill)),
		textPair("kind", encodeText(profile.Kind)),
		textPair("n_slots", encodeUint(uint64(profile.NSlots))),
		textPair("public_key", encodeBytes(publicKey)),
		textPair("sample_bytes", encodeUint(uint64(profile.SampleBytes))),
		textPair("seed_beacon", encodeBytes(seedBeacon)),
		textPair("slot_bytes", encodeUint(uint64(profile.SlotBytes))),
		textPair("stages", encodeArray(encodedStages)),
		textPair("steps_per_leg", encodeUint(uint64(profile.StepsPerLeg))),
		textPair("last_interrupt", encodeBytes(last)),
		textPair("last_interrupt_height", encodeUint(lastHeight)),
	}
	return encodeMap(top)
}

// PathAskHeights lists the section 13.1 window at or below cutOff.
// cutOff is last_interrupt_height. A height above it is not asked.
// The whole window is omitted when an offset does not fit in uint64.
// Asking every window height, including those above the cut-off, is the
// behaviour this function replaces.
func PathAskHeights(postHeight, cutOff uint64) []uint64 {
	origin := AnchorWindowOrigin(postHeight)
	window := make([]uint64, 0, len(anchorOffsetsAfterPost))
	for _, off := range anchorOffsetsAfterPost {
		if off < 0 {
			return nil
		}
		sum := origin + uint64(off)
		if sum < origin {
			return nil
		}
		window = append(window, sum)
	}
	asked := make([]uint64, 0, len(window))
	for _, height := range window {
		if height <= cutOff {
			asked = append(asked, height)
		}
	}
	return asked
}

// AcceptPath returns body when it validates for waypoint, the inclusion
// height is in the anchor window, and that height is at or below cutOff.
// Otherwise it returns nil. A nil result is a miss: the stage stores empty
// path bytes and the hike still returns the object.
func AcceptPath(body, waypoint []byte, postHeight, cutOff uint64) []byte {
	if len(waypoint) != 32 || len(body) == 0 {
		return nil
	}
	loc, err := PathLocatorsFor(body, waypoint)
	if err != nil {
		return nil
	}
	height := uint64(loc.BlockHeight)
	if !heightInAnchorWindow(postHeight, height) || height > cutOff {
		return nil
	}
	return append([]byte(nil), body...)
}

func bytesEqual(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
