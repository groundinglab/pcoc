package pcoc

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"testing"
)

// repoRoot walks up from this file to the directory that holds SPEC_PIN.
func repoRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("caller")
	}
	dir := filepath.Dir(file)
	for i := 0; i < 6; i++ {
		if _, err := os.Stat(filepath.Join(dir, "sdk", "SPEC_PIN")); err == nil {
			return dir
		}
		dir = filepath.Dir(dir)
	}
	t.Fatal("SPEC_PIN not found")
	return ""
}

// AC-C2. The test reads sdk/SPEC_PIN and checks the SHA-256 of
// spec/conformance/pack/manifest.json against vector-sha256.
func TestPin(t *testing.T) {
	root := repoRoot(t)
	pin := readPin(t, filepath.Join(root, "sdk", "SPEC_PIN"))
	sum := sha256File(t, filepath.Join(root, "spec", "conformance", "pack", "manifest.json"))
	if sum != pin {
		t.Fatalf("manifest sha256 %s, pin %s", sum, pin)
	}
}

func readPin(t *testing.T, path string) string {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	for _, line := range bytes.Split(b, []byte("\n")) {
		if bytes.HasPrefix(line, []byte("vector-sha256=")) {
			return string(line[len("vector-sha256="):])
		}
	}
	t.Fatal("no vector-sha256")
	return ""
}

func sha256File(t *testing.T, path string) string {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:])
}

// AC-C3. The SDK reports 2.2.0. sdk/VERSION in the tree is the pre-cut
// file; this core's constant is the report.
func TestVersionReport(t *testing.T) {
	if Version != "2.2.0" {
		t.Fatalf("Version = %s", Version)
	}
}

// AC-C4. Re-run the generator and compare with the committed files.
// PROD is included. Set PCOC_SKIP_PROD=1 only for a workstation check that
// is not the acceptance run.
func TestCandidatesUnchanged(t *testing.T) {
	if os.Getenv("PCOC_SKIP_CANDIDATES") == "1" {
		t.Skip("PCOC_SKIP_CANDIDATES")
	}
	root := repoRoot(t)
	dir := filepath.Join(root, "sdk", "go", "candidates")
	tmp := t.TempDir()
	cmd := exec.Command("go", "run", "./cmd/gencandidates", tmp)
	cmd.Dir = filepath.Join(root, "sdk", "go")
	cmd.Env = append(os.Environ(), "GOMAXPROCS=4")
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("generator: %v\n%s", err, out)
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 14 {
		t.Fatalf("committed files: %d", len(entries))
	}
	for _, e := range entries {
		got, err := os.ReadFile(filepath.Join(tmp, e.Name()))
		if err != nil {
			t.Fatal(err)
		}
		want, err := os.ReadFile(filepath.Join(dir, e.Name()))
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(got, want) {
			t.Fatalf("%s differs", e.Name())
		}
	}
}

// AC-C5. Encode, decode, encode is byte-identical on an S1 object.
// Every non-KIND label from CASES.md yields the A5 record and nothing else.
func TestRoundTripAndUnknownKind(t *testing.T) {
	pk := bytes.Repeat([]byte{0x11}, 32)
	beacon := bytes.Repeat([]byte{0x22}, 32)
	profile := Profile{Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 1}
	var stages [18]StageInput
	for g := 0; g < 18; g++ {
		stages[g] = StageInput{
			Legs:            1,
			Beacon:          bytes.Repeat([]byte{byte(g + 1)}, 32),
			BeaconHeight:    uint64(10 + g),
			Interrupt:       bytes.Repeat([]byte{byte(g + 2)}, 32),
			InterruptHeight: uint64(11 + g),
		}
		if g == 0 {
			stages[g].Beacon = beacon
		}
		if g > 0 {
			stages[g].Path = []byte{}
		}
	}
	// Keep hashes distinct and heights increasing, including last_interrupt.
	stages[17].Interrupt = bytes.Repeat([]byte{0xab}, 32)
	stages[17].InterruptHeight = 100
	res, err := Drive(profile, pk, stages)
	if err != nil {
		t.Fatal(err)
	}
	dec := DecodeObject(res.Encoded)
	if dec.Malformed || dec.Unknown != nil || dec.Object == nil {
		t.Fatalf("decode %+v", dec.Unknown)
	}
	again := EncodeObject(dec.Object)
	if !bytes.Equal(again, res.Encoded) {
		t.Fatalf("round trip %d vs %d", len(again), len(res.Encoded))
	}
	if dec.Object.Kind != Kind {
		t.Fatalf("kind %s", dec.Object.Kind)
	}

	labels := []string{"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"}
	for _, lab := range labels {
		raw := encodeKindOnly(lab)
		d := DecodeObject(raw)
		if d.Unknown == nil || d.Object != nil || d.Malformed {
			t.Fatalf("label %q decoded as object/malformed", lab)
		}
		if d.Unknown.Kind != "unknown kind" || d.Unknown.KindText != lab {
			t.Fatalf("label %q record %+v", lab, d.Unknown)
		}
		rec, a5 := Verify(VerifyInput{Object: raw, Profile: profile})
		if !a5 {
			t.Fatal("expected A5")
		}
		if rec["kind"] != "unknown kind" || rec["kind_text"] != lab {
			t.Fatalf("record %+v", rec)
		}
		if _, ok := rec["shape"]; ok {
			t.Fatal("A5 record has shape")
		}
		got := UnknownKindRecord(lab)
		want := []byte(`{"kind":"unknown kind","kind_text":` + string(mustJSON(lab)) + `}`)
		if !bytes.Equal(got, want) {
			t.Fatalf("record bytes %s", got)
		}
	}
}

func encodeKindOnly(kind string) []byte {
	v := cborValue{typ: cborMap, pairs: []cborPair{{key: "kind", val: cborTextV(kind)}}}
	return encodeValue(v)
}

func mustJSON(s string) []byte {
	b, err := json.Marshal(s)
	if err != nil {
		panic(err)
	}
	return b
}

// AC-C6. An honest S1 object's six-leg check is match. Each leg-level
// negative, written into the leg results, is mismatch. Each terrain-level
// negative's digest differs from the honest terrain digest.
func TestSixLegHonestAndNegatives(t *testing.T) {
	pk := bytes.Repeat([]byte{0x11}, 32)
	// Distinct from the 0x11/0x22 §7.4 preimage so this is its own fixture.
	beacon := bytes.Repeat([]byte{0x33}, 32)
	profile := Profile{Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 4}
	terrain, err := FillTerrain(profile, pk, beacon)
	if err != nil {
		t.Fatal(err)
	}
	honest := driveSimple(t, profile, pk, beacon, terrain, nil)
	rec := verifyRecompute(t, honest, profile, nil)
	runs := rec["six_leg"].(map[string]JSON)
	results := runs["results"].([]JSON)
	if len(results) != 6 {
		t.Fatalf("indices %v", runs["indices"])
	}
	for i, r := range results {
		if r != "match" {
			t.Fatalf("honest leg %d %v", i, r)
		}
	}
	if rec["shape"] != "match" {
		t.Fatalf("shape %v items %v", rec["shape"], rec["shape_item"])
	}
	if rec["seed_value"] != "match" {
		t.Fatalf("seed_value %v", rec["seed_value"])
	}

	// Leg-level negatives. Each stored leg result is the shortcut run for
	// STEPS_PER_LEG steps from that leg's previous checkpoint. The verifier
	// recomputes section 11.2 and records mismatch.
	alts := []struct {
		name string
		step func(*Terrain, uint64) uint64
	}{
		{"slot_summary_fold", func(tr *Terrain, s uint64) uint64 { return SealState(slotSummary(tr, s)) }},
		{"xor_accumulator_fold", func(tr *Terrain, s uint64) uint64 { return SealState(xorAcc(tr, s)) }},
		{"xor_w_fold", func(tr *Terrain, s uint64) uint64 { return SealState(xorW(tr, s)) }},
		{"s_plus_w_fold", func(tr *Terrain, s uint64) uint64 { return SealState(sPlusW(tr, s)) }},
		{"partial_slot_read", func(tr *Terrain, s uint64) uint64 { return SealState(partialOne(tr, s)) }},
	}
	for _, a := range alts {
		obj := driveSimple(t, profile, pk, beacon, terrain, func(prev []byte) uint64 {
			s := u64le(prev[:8])
			for n := 0; n < profile.StepsPerLeg; n++ {
				s = a.step(terrain, s)
			}
			return s
		})
		r := verifyRecompute(t, obj, profile, nil)
		rs := r["six_leg"].(map[string]JSON)["results"].([]JSON)
		for i, one := range rs {
			if one != "mismatch" {
				t.Fatalf("%s result %d = %v", a.name, i, one)
			}
		}
	}

	honestDig := sha256.Sum256(terrain.Bytes())
	// Terrain-level: a squeezed-seed fill's digest differs.
	squeezed := squeezed(profile, pk, beacon)
	sd := sha256.Sum256(squeezed)
	if sd == honestDig {
		t.Fatal("squeezed matches honest")
	}
	one := oneLane(profile, pk, beacon)
	od := sha256.Sum256(one)
	if od == honestDig {
		t.Fatal("one lane matches honest")
	}
}

func slotSummary(t *Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.p.NSlots())))
	var x uint64
	for w := 0; w < t.p.WordsPerSlot(); w++ {
		x ^= u64le(slot[8*w:8*w+8]) ^ uint64(w)
	}
	return s ^ x
}

func xorAcc(t *Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.p.NSlots())))
	var acc uint64
	for w := 0; w < t.p.WordsPerSlot(); w++ {
		acc ^= Mix64(u64le(slot[8*w:8*w+8]) ^ uint64(w) ^ s)
	}
	return acc
}

func xorW(t *Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.p.NSlots())))
	var acc uint64
	for w := 0; w < t.p.WordsPerSlot(); w++ {
		acc += Mix64(u64le(slot[8*w:8*w+8]) ^ uint64(w) ^ s)
	}
	return acc
}

func sPlusW(t *Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.p.NSlots())))
	var acc uint64
	for w := 0; w < t.p.WordsPerSlot(); w++ {
		acc += Mix64(u64le(slot[8*w:8*w+8]) ^ (s + uint64(w)))
	}
	return acc
}

func partialOne(t *Terrain, s uint64) uint64 {
	slot := t.Slot(int(s % uint64(t.p.NSlots())))
	return Mix64(u64le(slot[0:8]) ^ s)
}

func squeezed(p Profile, pk, beacon []byte) []byte {
	d := Seed(pk, beacon)
	lanes := Lanes(d)
	lanes[1], lanes[2], lanes[3] = 0, 0, 0
	return fillNL4(p, lanes)
}

func oneLane(p Profile, pk, beacon []byte) []byte {
	d := Seed(pk, beacon)
	lanes := Lanes(d)
	lanes[0] = ^lanes[0]
	raw := make([]byte, p.TerrainBytes)
	for i := 0; i < p.NSlots(); i++ {
		base := i * p.SlotBytes
		for j := 0; j < p.WordsPerSlot(); j++ {
			c := (uint64(i) << 32) ^ uint64(j) ^ WordXor
			putU64le(raw[base+8*j:], Mix64(lanes[j%4]^c))
		}
	}
	return raw
}

func fillNL4(p Profile, lanes [4]uint64) []byte {
	raw := make([]byte, p.TerrainBytes)
	words := p.WordsPerSlot()
	w := make([]uint64, words)
	for i := 0; i < p.NSlots(); i++ {
		for j := 0; j < words; j++ {
			c := (uint64(i) << 32) ^ uint64(j) ^ WordXor
			x := Mix64(lanes[j%4] ^ c)
			x = Mix64(x ^ Rotl(lanes[(j+1)%4], 17))
			x = Mix64(x ^ Rotl(lanes[(j+2)%4], 31))
			x = Mix64(x ^ Rotl(lanes[(j+3)%4], 47))
			w[j] = x
		}
		if i > 0 {
			parents := [3]int{i - 1, BitrevParent(i), MulParent(i)}
			for k := 1; k <= 3; k++ {
				base := parents[k-1] * p.SlotBytes
				xorK := uint64(k) * ParentMul
				for j := 0; j < words; j++ {
					pw := u64le(raw[base+8*j : base+8*j+8])
					w[j] = Mix64((w[j] ^ Rotl(pw, uint((j+k)%64))) ^ xorK)
				}
			}
			sm := uint64(i) * SlotMul
			for j := 0; j < words; j++ {
				w[j] = Mix64(w[j] ^ sm)
			}
		}
		base := i * p.SlotBytes
		for j := 0; j < words; j++ {
			putU64le(raw[base+8*j:], w[j])
		}
	}
	return raw
}

func driveSimple(t *testing.T, profile Profile, pk, beacon []byte, terrain *Terrain, alt func([]byte) uint64) []byte {
	t.Helper()
	var stages [18]StageInput
	for g := 0; g < 18; g++ {
		stages[g] = StageInput{
			Legs:            1,
			Beacon:          bytes.Repeat([]byte{byte(0x40 + g)}, 32),
			BeaconHeight:    uint64(100 + g),
			Interrupt:       bytes.Repeat([]byte{byte(0x41 + g)}, 32),
			InterruptHeight: uint64(101 + g),
		}
		if g > 0 {
			stages[g].Path = []byte{}
		}
	}
	stages[0].Beacon = beacon
	if alt == nil {
		res, err := Drive(profile, pk, stages)
		if err != nil {
			t.Fatal(err)
		}
		return res.Encoded
	}
	// Same drive, with the leg result replaced.
	obj := &Object{
		Kind: Kind, Fill: Fill,
		SlotBytes: uint64(profile.SlotBytes), NSlots: uint64(profile.NSlots()),
		SampleBytes: uint64(profile.SampleBytes), StepsPerLeg: uint64(profile.StepsPerLeg),
		PublicKey: append([]byte(nil), pk...), SeedBeacon: append([]byte(nil), beacon...),
	}
	waypoint := append([]byte(nil), terrain.Seed()...)
	for g := 0; g < 18; g++ {
		prev := append([]byte(nil), waypoint...)
		limb := append([]byte(nil), waypoint...)
		lr := alt(prev)
		cp := Checkpoint(terrain, prev, limb, lr)
		st := Stage{
			Beacon: stages[g].Beacon, BeaconHeight: stages[g].BeaconHeight,
			LegCount: 1, Waypoint: limb,
			Checkpoints: [][]byte{cp}, LegResults: [][]byte{Le64(lr)},
		}
		if g > 0 {
			st.HasPath = true
			st.Path = []byte{}
		}
		obj.Stages = append(obj.Stages, st)
		if g < 17 {
			waypoint = Waypoint(terrain, stages[g].Interrupt, limb, st.Checkpoints)
		} else {
			obj.LastInterrupt = stages[g].Interrupt
			obj.LastInterruptHeight = stages[g].InterruptHeight
			obj.HasLastInterrupt = true
			obj.HasLastInterruptHeight = true
		}
	}
	return EncodeObject(obj)
}

func verifyRecompute(t *testing.T, obj []byte, profile Profile, draw []byte) map[string]JSON {
	t.Helper()
	if draw == nil {
		// Bytes that draw the first six indices in order for leg_total 18:
		// any u < limit with u % 18 = 0,1,2,3,4,5. Small values work when
		// they are below the limit, which they are.
		draw = make([]byte, 48)
		for i := 0; i < 6; i++ {
			putU64le(draw[8*i:], uint64(i))
		}
	}
	rec, a5 := Verify(VerifyInput{Object: obj, Profile: profile, Draw: draw, Runs: 1, Recompute: true})
	if a5 {
		t.Fatal("A5")
	}
	return rec
}
