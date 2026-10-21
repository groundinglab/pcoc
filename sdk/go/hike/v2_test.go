package hike

import (
	"bytes"
	"strings"
	"testing"
)

func TestV20ProfileNumbers(t *testing.T) {
	if Production.SampleBytes != 1024 || Production.StepsPerLeg != 2_000_000 {
		t.Fatalf("production sizes %d %d", Production.SampleBytes, Production.StepsPerLeg)
	}
	if Production.SlotBytes != 16384 || Production.TerrainBytes != 268435456 {
		t.Fatal("slot or terrain changed")
	}
	if Production.OffsetModulus != uint64(Production.TerrainBytes-Production.SampleBytes+1) {
		t.Fatal("offset")
	}
	if Production.OffsetModulus != 268434433 {
		t.Fatalf("modulus %d", Production.OffsetModulus)
	}
	if Production.Kind != Kind || Kind == KindV135 {
		t.Fatal("kind")
	}
	if ConformanceSmallV2.Kind != Kind {
		t.Fatal("small kind")
	}
	if got := MeasureName("conformance-small"); got.Kind != Kind || got.SampleBytes != 32 || got.StepsPerLeg != 32 {
		t.Fatalf("name selector %+v", got)
	}
	if ConformanceSmallV2.SampleBytes != 32 || ConformanceSmallV2.StepsPerLeg != 32 {
		t.Fatal("small sizes")
	}
	// Rule B2. The ratified floor is TBD. The test placeholder is 1/5.
	// q*hike_legs >= p*hike_span and hike_span > 0. The withdrawn
	// examples 7/20, 1/4, and 1/8 are not this comparison.
	if !LegRateMeets(2, 9, PlaceholderLegRateP, PlaceholderLegRateQ) || LegRateMeets(2, 11, PlaceholderLegRateP, PlaceholderLegRateQ) {
		t.Fatal("placeholder floor")
	}
	if !LegRateMeets(1, 5, 1, 5) || LegRateMeets(1, 6, 1, 5) || LegRateMeets(1, 0, 1, 5) {
		t.Fatal("boundary")
	}
	// Rule B4. Six anywhere. Five does not meet. No first or last rule.
	if !AnchoredIndicesMeet([]int{2, 4, 6, 8, 10, 12}) || AnchoredIndicesMeet([]int{2, 4, 6, 8, 10}) {
		t.Fatal("anchors")
	}
	if !AnchoredIndicesMeet([]int{11, 12, 13, 14, 15, 16}) || AnchorsMeet(5) || !AnchorsMeet(6) {
		t.Fatal("anchor positions")
	}
	if AnchorWindowOrigin(17) != 17 {
		t.Fatal("anchor origin")
	}
	offs := AnchorOffsetsAfterPost()
	if AnchorWindow != 3 || len(offs) != 3 || offs[0] != 2 || offs[1] != 3 || offs[2] != 4 {
		t.Fatalf("window %v", offs)
	}
	if !InclusionInWindow(102, 100, offs, []uint64{102, 103, 104}) {
		t.Fatal("published window")
	}
	if DefaultBindThreads != 6 {
		t.Fatal("bind threads")
	}
	if BindThreadsFrom("") != 6 || BindThreadsFrom("0") != 6 || BindThreadsFrom("1") != 1 {
		t.Fatal("bind thread text")
	}
}

func TestInclusionWindowAndSingleRead(t *testing.T) {
	offsets := []int{2, 3, 4}
	interrupts := []uint64{102, 110}
	if !InclusionInWindow(102, 100, offsets, interrupts) {
		t.Fatal("H+2")
	}
	if InclusionInWindow(101, 100, offsets, interrupts) {
		t.Fatal("H+1 is not in this offset list")
	}
	if InclusionInWindow(102, 100, offsets, []uint64{110}) {
		t.Fatal("not an interrupt")
	}
	if !InclusionInWindow(101, 100, []int{1, 2, 3}, []uint64{101}) {
		t.Fatal("explicit H+1")
	}
	if InclusionInWindow(102, 100, nil, interrupts) || InclusionInWindow(102, 100, []int{2, 3}, interrupts) {
		t.Fatal("short window")
	}

	var got []uint64
	index := Index{Fetch: func(h uint64) (string, bool) {
		got = append(got, h)
		return "", false
	}}
	read, in := BindInclusion(index, 102, 100, offsets, interrupts)
	if !in || read != nil || len(got) != 1 || got[0] != 102 {
		t.Fatalf("read %+v in %v got %v", read, in, got)
	}
	read, in = BindInclusion(index, 101, 100, offsets, []uint64{101})
	if in || read != nil || len(got) != 1 {
		t.Fatal("scanned a second height")
	}
}

func TestPathInclusionHeightIsUnread(t *testing.T) {
	if _, ok := PathInclusionHeight([]byte{0x01, 0x02, 0x03, 0x04}); ok {
		t.Fatal("layout was invented")
	}
	var reads int
	chain := func(PathLocators) error {
		reads++
		return nil
	}
	measured := section14Path(1, []byte{0x01}, bytes.Repeat([]byte{0x02}, 32), 100, Header{}, cborVal{}, cborVal{}, chain, Index{})
	state, plant, anchor := measured.state, measured.plant, measured.anchor
	if state != "present" || plant != "mismatch" || anchor != "not anchored" || reads != 0 {
		t.Fatalf("%s %s %s reads %d", state, plant, anchor, reads)
	}
}

func TestDriveWritesProfileKindAndSizes(t *testing.T) {
	profile := mustProfile("conformance-small", Kind, 32, 64, 256, 32)
	object, stderr := driveSmall(t, profile)
	item, err := decodeItem(object)
	if err != nil {
		t.Fatal(err)
	}
	kind, ok := textOf(item, "kind")
	if !ok || kind != Kind {
		t.Fatalf("kind %q", kind)
	}
	sample, okS, _ := uintField(item, "sample_bytes")
	steps, okT, _ := uintField(item, "steps_per_leg")
	if !okS || !okT || sample != 32 || steps != 32 {
		t.Fatalf("sizes %d %d", sample, steps)
	}
	lines := strings.Split(strings.TrimSuffix(stderr, "\n"), "\n")
	if len(lines) != 17 || lines[0] != "path_skip 1" || lines[15] != "path_skip 16" || lines[16] != "not fetched by design 17" {
		t.Fatalf("stderr %q", stderr)
	}
	if strings.Contains(stderr, "path_skip 0") || strings.Contains(stderr, "not fetched by design 15") || strings.Contains(stderr, "not fetched by design 16") {
		t.Fatalf("stderr %q", stderr)
	}
}

func TestEarlierVersionOnCurrentProfile(t *testing.T) {
	raw := encodeMap([][2][]byte{textPair("kind", encodeText(KindV135))})
	want := `{"version":"earlier version","kind":"satoshi-hike-v1.35-object"}`
	rec, ok := Verify(raw, Production, Index{}, nil, true)
	if !ok || rec.EarlierKind != KindV135 || strings.TrimSpace(string(rec.Marshal())) != want {
		t.Fatalf("production %q", rec.Marshal())
	}
	v20 := encodeMap([][2][]byte{textPair("kind", encodeText(KindV20))})
	want20 := `{"version":"earlier version","kind":"satoshi-hike-v2.0-object"}`
	got20, ok := Verify(v20, Production, Index{Fetch: func(uint64) (string, bool) {
		t.Fatal("earlier kind filled or read")
		return "", false
	}}, nil, true)
	if !ok || strings.TrimSpace(string(got20.Marshal())) != want20 {
		t.Fatalf("v2.0 earlier %q", got20.Marshal())
	}
	small, ok := Verify(raw, ConformanceSmallV2, Index{}, nil, true)
	if !ok || strings.TrimSpace(string(small.Marshal())) != want {
		t.Fatal("small earlier record grew a field")
	}
}

func driveSmall(t *testing.T, profile Profile) ([]byte, string) {
	t.Helper()
	pub := bytes.Repeat([]byte{0x11}, 32)
	tips := make([]Tip, Stages+1)
	for i := range tips {
		tips[i] = Tip{Natural: bytes.Repeat([]byte{byte(i + 1)}, 32), Height: uint64(1000 + i)}
	}
	object, stderr, err := Drive(pub, profile, tips)
	if err != nil {
		t.Fatal(err)
	}
	return object, stderr
}
