package hike

import (
	"bytes"
	"encoding/json"
	"testing"
)

func TestHarnessEncodeOmitsStage0Path(t *testing.T) {
	raw := QuietV2().Encode()
	assertStage0OmitsPath(t, raw)
	for g := 1; g < Stages; g++ {
		if !stageHasKey(t, raw, g, "path") {
			t.Fatalf("stage %d path key", g)
		}
	}
	forced := QuietV2()
	forced.Stages[0].PathKey = true
	forcedRaw := forced.Encode()
	if !stageHasKey(t, forcedRaw, 0, "path") {
		t.Fatal("stage 0 path key")
	}
	line := RecordLine(forcedRaw, Production, Index{}, nil, 1, nil)
	compact := mustCompact(t, line)
	if len(compact.Anchors) != 16 || compact.Shape != "mismatch" {
		t.Fatalf("forced path %s", line)
	}
	rec, ok := Verify(forcedRaw, Production, Index{}, nil, false)
	if !ok || !containsShape(rec.ShapeItem, "stage-0 path key") {
		t.Fatalf("shape items %v", rec.ShapeItem)
	}
}

func TestProduceCompactAgreesWithVerifier(t *testing.T) {
	if Production.SampleBytes != 1024 || Production.StepsPerLeg != 2_000_000 {
		t.Fatalf("production sample %d steps %d", Production.SampleBytes, Production.StepsPerLeg)
	}
	if Production.SampleBytes == Production.StepsPerLeg {
		t.Fatal("sample and steps collapsed")
	}
	raw, _ := driveSmall(t, ConformanceSmallV2)
	assertStage0OmitsPath(t, raw)
	if !stageHasKey(t, raw, 1, "path") {
		t.Fatal("produce stage 1 path")
	}
	line := RecordLine(raw, ConformanceSmallV2, Index{}, nil, 1, nil)
	compact := mustCompact(t, line)
	if len(compact.Anchors) != 16 || compact.Checkpoints == "not read" || compact.Waypoints == "not read" {
		t.Fatalf("parse failure %s", line)
	}
	rec, ok := Verify(raw, ConformanceSmallV2, Index{}, nil, false)
	if !ok {
		t.Fatal("verify")
	}
	if rec.Shape != "match" || len(rec.ShapeItem) != 0 {
		t.Fatalf("full shape %s %v", rec.Shape, rec.ShapeItem)
	}
	if compact.Profile != rec.Profile || compact.Shape != rec.Shape {
		t.Fatalf("agree profile %s/%s shape %s/%s line %s", compact.Profile, rec.Profile, compact.Shape, rec.Shape, line)
	}
	if compact.Waypoints != "match" {
		t.Fatalf("waypoints %s", line)
	}
	for _, row := range rec.Waypoints {
		if row.Result != "match" {
			t.Fatalf("waypoint %d %s", row.G, row.Result)
		}
	}
	assertSixAgree(t, compact.SixLeg, rec.SixLeg)

	mutated := withStage0Path(t, raw, []byte{})
	mutLine := RecordLine(mutated, ConformanceSmallV2, Index{}, nil, 1, nil)
	mutCompact := mustCompact(t, mutLine)
	if len(mutCompact.Anchors) != 16 || mutCompact.Shape != "mismatch" {
		t.Fatalf("empty stage-0 path %s", mutLine)
	}
	mutRec, ok := Verify(mutated, ConformanceSmallV2, Index{}, nil, false)
	if !ok || !containsShape(mutRec.ShapeItem, "stage-0 path key") {
		t.Fatalf("mutated items %v", mutRec.ShapeItem)
	}
}

func TestPackRecordIsTheSection14Record(t *testing.T) {
	raw, _ := driveSmall(t, ConformanceSmallV2)
	pack := PackRecord(raw, ConformanceSmallV2, Index{}, nil, 1, nil)
	line := RecordLine(raw, ConformanceSmallV2, Index{}, nil, 1, nil)
	rec, ok := Verify(raw, ConformanceSmallV2, Index{}, nil, false)
	if !ok {
		t.Fatal("verify")
	}
	detailed := rec.Marshal()
	if !bytes.Contains(detailed, []byte(`"shape_item"`)) || !bytes.Contains([]byte(pack), []byte(`"shape_item"`)) {
		t.Fatal("section 14 record")
	}
	if string(detailed) != pack {
		t.Fatal("pack line")
	}
	if bytes.Equal(bytes.TrimSpace(detailed), bytes.TrimSpace([]byte(line))) {
		t.Fatal("compact line is the section 14 record")
	}
}

func TestSixLegMeasuresAtLeastSix(t *testing.T) {
	if Production.StepsPerLeg != 2_000_000 || ConformanceSmallV2.StepsPerLeg == Production.StepsPerLeg {
		t.Fatalf("steps production %d small %d", Production.StepsPerLeg, ConformanceSmallV2.StepsPerLeg)
	}
	raw, _ := driveSmall(t, ConformanceSmallV2)
	seed := filled32(0x11)
	stream := AuditStream(seed[:], 4096)
	line := RecordLine(raw, ConformanceSmallV2, Index{}, nil, 1, stream)
	compact := mustCompact(t, line)
	rec, ok := Verify(raw, ConformanceSmallV2, Index{}, stream, false)
	if !ok || compact.Shape != "match" || rec.Shape != "match" || compact.SixLeg != "match" {
		t.Fatalf("six %s shape %s full %s items %v", compact.SixLeg, compact.Shape, rec.Shape, rec.ShapeItem)
	}
	assertSixAgree(t, compact.SixLeg, rec.SixLeg)
	shortLine := RecordLine(raw, ConformanceSmallV2, Index{}, nil, 1, []byte{1})
	short := mustCompact(t, shortLine)
	shortRec, ok := Verify(raw, ConformanceSmallV2, Index{}, []byte{1}, false)
	if !ok || short.SixLeg != "not read" {
		t.Fatalf("short %s", short.SixLeg)
	}
	assertSixAgree(t, short.SixLeg, shortRec.SixLeg)
}

func TestReferenceShapedLastInterrupt(t *testing.T) {
	raw := referenceShaped().encode(ConformanceSmallV2)
	if !stageHasKey(t, raw, 0, "beacon") || stageHasKey(t, raw, 0, "path") {
		t.Fatal("stage 0 keys")
	}
	top, err := MapKeys(raw)
	if err != nil {
		t.Fatal(err)
	}
	for _, key := range []string{"last_interrupt", "last_interrupt_height"} {
		if !containsShape(top, key) {
			t.Fatalf("missing %s in %v", key, top)
		}
	}
	line := RecordLine(raw, ConformanceSmallV2, Index{}, nil, 1, nil)
	compact := mustCompact(t, line)
	rec, ok := Verify(raw, ConformanceSmallV2, Index{}, nil, false)
	if !ok || compact.Shape != "match" || rec.Shape != "match" || len(rec.ShapeItem) != 0 {
		t.Fatalf("shaped %s full %s %v", line, rec.Shape, rec.ShapeItem)
	}
	stripped := dropKeys(t, raw, "last_interrupt", "last_interrupt_height")
	strippedLine := RecordLine(stripped, ConformanceSmallV2, Index{}, nil, 1, nil)
	strippedCompact := mustCompact(t, strippedLine)
	strippedRec, ok := Verify(stripped, ConformanceSmallV2, Index{}, nil, false)
	if !ok || len(strippedCompact.Anchors) != 16 || strippedCompact.Shape != "mismatch" || strippedRec.Shape != "mismatch" {
		t.Fatalf("stripped %s full %s %v", strippedLine, strippedRec.Shape, strippedRec.ShapeItem)
	}
	if !containsShape(strippedRec.ShapeItem, "last_interrupt") {
		t.Fatalf("items %v", strippedRec.ShapeItem)
	}
}

func referenceShaped() V2Object {
	object := QuietV2()
	copy(object.Stages[0].Waypoint[:], SeedValue(object.PublicKey[:], object.SeedBeacon[:]))
	checkpoint := filled32(0xAB)
	leg := [8]byte{1}
	for g := range object.Stages {
		object.Stages[g].LegCount = 1
		object.Stages[g].Checkpoints = [][32]byte{checkpoint}
		object.Stages[g].LegResults = [][8]byte{leg}
	}
	return object
}

func assertSixAgree(t *testing.T, compact string, raw json.RawMessage) {
	t.Helper()
	var text string
	if err := json.Unmarshal(raw, &text); err == nil {
		if text != compact {
			t.Fatalf("six %s full %s", compact, text)
		}
		return
	}
	var draw SixDraw
	if err := json.Unmarshal(raw, &draw); err != nil {
		t.Fatal(err)
	}
	want := "match"
	if len(draw.Results) == 0 {
		t.Fatal("empty draw")
	}
	for _, item := range draw.Results {
		if item != "match" {
			want = "mismatch"
		}
	}
	if compact != want {
		t.Fatalf("six %s full %v", compact, draw.Results)
	}
}

func dropKeys(t *testing.T, raw []byte, names ...string) []byte {
	t.Helper()
	item, err := decodeItem(raw)
	if err != nil {
		t.Fatal(err)
	}
	skip := map[string]bool{}
	for _, name := range names {
		skip[name] = true
	}
	pairs := make([][2][]byte, 0, len(item.keys))
	for i, key := range item.keys {
		if skip[key] {
			continue
		}
		pairs = append(pairs, textPair(key, encodeVal(item.vals[i])))
	}
	return encodeMap(pairs)
}

func assertStage0OmitsPath(t *testing.T, raw []byte) {
	t.Helper()
	if stageHasKey(t, raw, 0, "path") {
		t.Fatal("stage 0 path key")
	}
}

func stageHasKey(t *testing.T, raw []byte, stage int, key string) bool {
	t.Helper()
	keys, err := StageKeys(raw, stage)
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range keys {
		if item == key {
			return true
		}
	}
	return false
}

func containsShape(items []string, want string) bool {
	for _, item := range items {
		if item == want {
			return true
		}
	}
	return false
}

type compactLine struct {
	Profile     string   `json:"profile"`
	Shape       string   `json:"shape"`
	Checkpoints string   `json:"checkpoints"`
	Waypoints   string   `json:"waypoints"`
	LegRate     string   `json:"leg_rate"`
	SixLeg      string   `json:"six_leg"`
	Anchors     []string `json:"anchors"`
}

func mustCompact(t *testing.T, line string) compactLine {
	t.Helper()
	var compact compactLine
	if err := json.Unmarshal([]byte(line), &compact); err != nil {
		t.Fatal(err)
	}
	return compact
}

func withStage0Path(t *testing.T, raw []byte, path []byte) []byte {
	t.Helper()
	item, err := decodeItem(raw)
	if err != nil {
		t.Fatal(err)
	}
	stages, ok := item.field("stages")
	if !ok || stages.kind != 4 || len(stages.arr) == 0 {
		t.Fatal("stages")
	}
	stage := stages.arr[0]
	pairs := [][2][]byte{textPair("path", encodeBytes(path))}
	for i, key := range stage.keys {
		if key == "path" {
			continue
		}
		pairs = append(pairs, textPair(key, encodeVal(stage.vals[i])))
	}
	top := make([][2][]byte, 0, len(item.keys))
	for i, key := range item.keys {
		if key == "stages" {
			encoded := make([][]byte, len(stages.arr))
			for g := range stages.arr {
				if g == 0 {
					encoded[g] = encodeMap(pairs)
				} else {
					encoded[g] = encodeVal(stages.arr[g])
				}
			}
			top = append(top, textPair("stages", encodeArray(encoded)))
			continue
		}
		top = append(top, textPair(key, encodeVal(item.vals[i])))
	}
	return encodeMap(top)
}

func encodeVal(v cborVal) []byte {
	switch v.kind {
	case 0:
		return encodeUint(v.u)
	case 2:
		return encodeBytes(append([]byte(nil), v.b...))
	case 3:
		return encodeText(v.s)
	case 4:
		parts := make([][]byte, len(v.arr))
		for i := range v.arr {
			parts[i] = encodeVal(v.arr[i])
		}
		return encodeArray(parts)
	case 5:
		pairs := make([][2][]byte, len(v.keys))
		for i, key := range v.keys {
			pairs[i] = textPair(key, encodeVal(v.vals[i]))
		}
		return encodeMap(pairs)
	default:
		return encodeUint(0)
	}
}
