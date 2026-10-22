package negatives

import (
	"bytes"
	"testing"

	"github.com/groundinglab/pcoc/sdk/go/v2"
	"github.com/groundinglab/pcoc/sdk/go/v2/internal/fold"
	"github.com/groundinglab/pcoc/sdk/go/v2/produce"
)

// ERRATUM-1 reference table for the S1 fixture (ERRATUM-1.md L720–L734).
// The test computes each value; these literals are only the check.
// Honest leg is L720. Honest terrain digest is L721. The thirteen
// negatives are L722–L734.
const (
	erratumHonestLeg     = "4c9a6a98574cbd31"                                                 // ERRATUM-1 L720
	erratumHonestTerrain = "366e6d1d27adb715d5c96c64e6b9d881e1701a0151e6e08d49df08d2996bd97f" // ERRATUM-1 L721
)

var erratumLegs = map[string]string{
	"slot_summary_fold":      "b87ce6fc485dd57c", // ERRATUM-1 L722
	"xor_accumulator_fold":   "346188d2d8cf98e8", // ERRATUM-1 L723
	"xor_w_fold":             "73a4c7fbd105aae1", // ERRATUM-1 L724
	"s_plus_w_fold":          "07262df2d90e8ecb", // ERRATUM-1 L725
	"prefold_hoist":          "a652562037b465fc", // ERRATUM-1 L726
	"final_stage_hoist":      "b1808c8d3927a7d1", // ERRATUM-1 L727
	"partial_slot_read":      "e1ae647ee5a5d7c5", // ERRATUM-1 L728
	"partial_slot_read_2047": "738755dabac824ab", // ERRATUM-1 L729
	"f3_shared_terrain":      "a73019b8a2c43cd1", // ERRATUM-1 L730
	"cached_leg":             "3b715e2a251a44d9", // ERRATUM-1 L731
}

var erratumTerrains = map[string]string{
	"one_lane_flip":         "5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608", // ERRATUM-1 L732
	"all4_kernel":           "2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e", // ERRATUM-1 L733
	"squeezed_seed_terrain": "ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22", // ERRATUM-1 L734
}

// TestACA6 computes the thirteen permanent negatives for the S1 fixture and
// checks them against the ERRATUM-1 reference table. The honest S1 object
// matches the six-leg check. An object built from each leg negative
// mismatches. A leg on each terrain negative, from the honest seed, differs
// from the honest leg.
func TestACA6(t *testing.T) {
	pkA := mustHex(t, "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
	pkB := mustHex(t, "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659")
	p := pcoc.Profile{Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 100}
	beacon := produce.BeaconAt(-1).Hash
	tA, err := pcoc.FillTerrain(p, pkA, beacon)
	if err != nil {
		t.Fatal(err)
	}
	tB, err := pcoc.FillTerrain(p, pkB, beacon)
	if err != nil {
		t.Fatal(err)
	}
	honest := pcoc.LegResult(tA, tA.Seed())
	want := erratumHonestLeg
	if hexOf(pcoc.Le64(honest)) != want {
		t.Fatalf("honest %s", hexOf(pcoc.Le64(honest)))
	}
	honestTerrain := hexOf(TerrainSHA256(tA.Bytes()))
	if honestTerrain != erratumHonestTerrain {
		t.Fatalf("honest terrain %s", honestTerrain)
	}
	// The terrain walk below is the same leg as pcoc.LegResult.
	if legOnTerrain(p, tA.Bytes(), tA.Seed()) != honest {
		t.Fatal("legOnTerrain disagrees with LegResult")
	}
	seen := map[string]struct{}{want: {}}

	idx := s1Index()
	draw := func() pcoc.ByteSource { return pcoc.ExpandDrawLabel("pcoc-pack-2.2/draw") }
	honestObj := mustDrive(t, p, pkA)
	rec, fail := pcoc.Verify(pcoc.VerifyInput{Profile: p, Object: honestObj, Index: idx, Draw: draw()})
	if fail {
		t.Fatal("honest decode")
	}
	if !bytes.Contains(rec, []byte(`"results":["match","match","match","match","match","match"]`)) {
		t.Fatalf("honest six %s", rec)
	}

	steps := map[string]func(uint64) uint64{
		"slot_summary_fold":      func(s uint64) uint64 { return SlotSummaryFold(tA, s) },
		"xor_accumulator_fold":   func(s uint64) uint64 { return XorAccumulatorFold(tA, s) },
		"xor_w_fold":             func(s uint64) uint64 { return XorWFold(tA, s) },
		"s_plus_w_fold":          func(s uint64) uint64 { return SPlusWFold(tA, s) },
		"prefold_hoist":          PrefoldHoist(tA),
		"final_stage_hoist":      func(s uint64) uint64 { return FinalStageHoist(tA, s) },
		"partial_slot_read":      func(s uint64) uint64 { return PartialSlotRead(tA, s) },
		"partial_slot_read_2047": func(s uint64) uint64 { return PartialSlotRead2047(tA, s) },
		"f3_shared_terrain":      func(s uint64) uint64 { return pcoc.Step(tB, s) },
		"cached_leg":             nil,
	}
	for _, name := range LegNames {
		var leg uint64
		if name == "cached_leg" {
			v, ok := CachedLeg(tA, tA.Seed(), p.StepsPerLeg)
			if !ok {
				t.Fatal("no revisit")
			}
			leg = v
		} else if name == "f3_shared_terrain" {
			leg = pcoc.LegResult(tB, pcoc.Seed(pkA, beacon))
		} else {
			leg = LegResult(tA, tA.Seed(), p.StepsPerLeg, steps[name])
		}
		hx := hexOf(pcoc.Le64(leg))
		if hx != erratumLegs[name] {
			t.Fatalf("%s legs_le_hex %s, ERRATUM-1 %s", name, hx, erratumLegs[name])
		}
		if hx == want {
			t.Fatalf("%s equals honest", name)
		}
		if _, dup := seen[hx]; dup {
			t.Fatalf("%s duplicate %s", name, hx)
		}
		seen[hx] = struct{}{}
		walk := tA
		if name == "f3_shared_terrain" {
			walk = tB
		}
		obj := objectWithLeg(t, p, pkA, walk, steps[name], name, beacon)
		rec, fail := pcoc.Verify(pcoc.VerifyInput{Profile: p, Object: obj, Index: idx, Draw: draw()})
		if fail {
			t.Fatalf("%s decode", name)
		}
		if bytes.Contains(rec, []byte(`"results":["match","match","match","match","match","match"]`)) {
			t.Fatalf("%s six matched", name)
		}
		if !bytes.Contains(rec, []byte(`"mismatch"`)) {
			t.Fatalf("%s no mismatch", name)
		}
	}
	if len(seen) != 11 {
		t.Fatalf("distinct %d", len(seen))
	}

	lanes := tA.Lanes()
	fills := map[string][]byte{
		"one_lane_flip":         FillOneLane(p, lanes),
		"all4_kernel":           FillAll4(p, lanes),
		"squeezed_seed_terrain": FillSqueezed(p, lanes),
	}
	seenTerrain := map[string]struct{}{}
	for _, name := range TerrainNames {
		hx := hexOf(TerrainSHA256(fills[name]))
		if hx != erratumTerrains[name] {
			t.Fatalf("%s terrain_sha256 %s, ERRATUM-1 %s", name, hx, erratumTerrains[name])
		}
		// A terrain negative cannot be the honest terrain.
		if hx == honestTerrain {
			t.Fatalf("%s terrain equals honest", name)
		}
		if _, dup := seenTerrain[hx]; dup {
			t.Fatalf("%s terrain duplicate %s", name, hx)
		}
		seenTerrain[hx] = struct{}{}
		// The honest seed, walked on this terrain, is not the honest leg.
		leg := legOnTerrain(p, fills[name], tA.Seed())
		if hexOf(pcoc.Le64(leg)) == want {
			t.Fatalf("%s leg on terrain equals honest", name)
		}
	}
	if len(seenTerrain) != 3 {
		t.Fatalf("terrain distinct %d", len(seenTerrain))
	}
}

// legOnTerrain is the section 11.1 leg on terrain bytes, from seed.
// FillOneLane, FillAll4, and FillSqueezed return the byte string; Terrain's
// fields are unexported, so the walk uses fold.Scalar and the seal directly.
// Every step reads the whole slot. Nothing is reused across terrains.
func legOnTerrain(p pcoc.Profile, terrain, seed []byte) uint64 {
	s := pcoc.U64le(seed[:8])
	nSlots := p.NSlots()
	slotBytes := p.SlotBytes
	for n := 0; n < p.StepsPerLeg; n++ {
		i := int(s % uint64(nSlots))
		base := i * slotBytes
		s = pcoc.SealState(fold.Scalar(terrain[base:base+slotBytes], s))
	}
	return s
}

// objectWithLeg walks the S1 schedule with step in place of the section 11.1
// step, so every stored leg result is the negative. Checkpoints use the
// honest checkpoint function over that leg result.
func objectWithLeg(t *testing.T, p pcoc.Profile, pk []byte, terrain *pcoc.Terrain, step func(uint64) uint64, name string, beacon []byte) []byte {
	t.Helper()
	beacons := produce.CASESBeacons()
	// The seed on the object is pk_A's seed. f3 walks pk_B's terrain from
	// that same start, so the limb is pk_A's seed, not pk_B's.
	seed := pcoc.Seed(pk, beacon)
	waypoint := append([]byte(nil), seed...)
	stages := make([]pcoc.Stage, 18)
	for g := 0; g < 18; g++ {
		prev := append([]byte(nil), waypoint...)
		limb := append([]byte(nil), waypoint...)
		var lr uint64
		switch {
		case name == "cached_leg":
			v, _ := CachedLeg(terrain, prev, p.StepsPerLeg)
			lr = v
		case name == "f3_shared_terrain":
			lr = pcoc.LegResult(terrain, prev)
		default:
			lr = LegResult(terrain, prev, p.StepsPerLeg, step)
		}
		cp := pcoc.Checkpoint(terrain, prev, limb, lr)
		st := pcoc.Stage{
			Beacon: append([]byte(nil), beacons[g].Hash...), BeaconHeight: beacons[g].Height,
			HasBeacon: true, HasHeight: true, LegCount: 1, HasLegCount: true,
			Waypoint: append([]byte(nil), waypoint...), HasWaypoint: true,
			Checkpoints: [][]byte{cp}, LegResults: [][]byte{pcoc.Le64(lr)},
			HasCP: true, HasLR: true, Present: true,
		}
		if g > 0 {
			st.HasPath = true
			st.Path = []byte{}
		}
		stages[g] = st
		if g < 17 {
			waypoint = pcoc.Waypoint(terrain, beacons[g+1].Hash, waypoint, st.Checkpoints)
		}
	}
	obj := &pcoc.Object{
		Kind: pcoc.Kind, Fill: pcoc.FillName,
		SlotBytes: uint64(p.SlotBytes), NSlots: uint64(p.NSlots()),
		SampleBytes: uint64(p.SampleBytes), StepsPerLeg: uint64(p.StepsPerLeg),
		PublicKey: append([]byte(nil), pk...), SeedBeacon: append([]byte(nil), beacon...),
		Stages: stages, HasLastInterrupt: true,
		LastInterrupt:          append([]byte(nil), beacons[18].Hash...),
		HasLastInterruptHeight: true, LastInterruptHeight: beacons[18].Height,
	}
	return pcoc.EncodeObject(obj)
}

func mustDrive(t *testing.T, p pcoc.Profile, pk []byte) []byte {
	t.Helper()
	legs := make([]int, 18)
	for i := range legs {
		legs[i] = 1
	}
	b, err := produce.Drive(p, pk, produce.CASESBeacons(), legs)
	if err != nil {
		t.Fatal(err)
	}
	return b
}

type s1Idx struct{}

func s1Index() pcoc.Index { return s1Idx{} }

func (s1Idx) Get(height uint64) ([]byte, bool) {
	// The six-leg check does not read the index. A nil body keeps headers
	// not read, which this test does not assert.
	return nil, false
}

func mustHex(t *testing.T, s string) []byte {
	t.Helper()
	b := make([]byte, len(s)/2)
	for i := 0; i < len(b); i++ {
		b[i] = hexNibble(s[2*i])<<4 | hexNibble(s[2*i+1])
	}
	return b
}

func hexNibble(c byte) byte {
	switch {
	case c >= '0' && c <= '9':
		return c - '0'
	case c >= 'a' && c <= 'f':
		return c - 'a' + 10
	default:
		return c - 'A' + 10
	}
}

func hexOf(b []byte) string {
	const d = "0123456789abcdef"
	out := make([]byte, len(b)*2)
	for i, v := range b {
		out[2*i] = d[v>>4]
		out[2*i+1] = d[v&0xf]
	}
	return string(out)
}
