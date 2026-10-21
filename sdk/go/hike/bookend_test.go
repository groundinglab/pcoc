package hike

import (
	"bytes"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestBookendRecordMatchesReference(t *testing.T) {
	object := bookendObject()
	got := PackRecord(object.Encode(), Production, bookendIndex(object), func(PathLocators) error { return nil }, 1, nil)
	want := readProbe(t, "bookend.record")
	if got != string(want) {
		t.Fatalf("bookend record %d bytes, reference %d", len(got), len(want))
	}
	if !containsAll(got, []string{`"a":1`, `"b":2`, `"N":3`, `"S":3000`, `"delta_h":3`, `"legs_outside":0`}) {
		t.Fatal("bookend integers")
	}
}

func TestSeedMismatchMatchesReference(t *testing.T) {
	object := structuralV2()
	for i := range object.Stages[0].Waypoint {
		object.Stages[0].Waypoint[i] = 0x22
	}
	stream := AuditStream(make([]byte, 32), 4096)
	got := PackRecord(object.Encode(), Production, Index{}, func(PathLocators) error { return nil }, 1, stream)
	want := readProbe(t, "seed.record")
	if got != string(want) {
		t.Fatalf("seed record %d bytes, reference %d", len(got), len(want))
	}
}

func bookendObject() V2Object {
	object := QuietV2()
	for i := range object.Stages[4].Beacon {
		object.Stages[4].Beacon[i] = 0x55
	}
	object.Stages[1].LegCount = 3
	object.Stages[1].Checkpoints = [][32]byte{filled32(0xAB), filled32(0xAB), filled32(0xAB)}
	leg := [8]byte{1}
	object.Stages[1].LegResults = [][8]byte{leg, leg, leg}
	object.Stages[1].Path = EncodePathInBlock(object.Stages[1].Waypoint[:], 5)
	object.Stages[2].Path = EncodePathInBlock(object.Stages[2].Waypoint[:], 5)
	return object
}

func structuralV2() V2Object {
	object := QuietV2()
	for g := range object.Stages {
		object.Stages[g].LegCount = 1
		object.Stages[g].Checkpoints = [][32]byte{filled32(0xAB)}
		object.Stages[g].LegResults = [][8]byte{{1}}
		if g != 0 && g != Stages-1 {
			object.Stages[g].Path = EncodePathInBlock(object.Stages[g].Waypoint[:], 123456)
		}
	}
	return object
}

func bookendIndex(object V2Object) Index {
	docs := map[uint64]string{}
	add := func(natural []byte, height uint64) {
		docs[height] = indexDocument(natural, height, height*1000)
	}
	for _, stage := range object.Stages {
		add(stage.Beacon[:], stage.BeaconHeight)
	}
	if object.LastHeight != nil {
		add(object.Last, *object.LastHeight)
	}
	// Rule A4.10 reads the eleven headers ending at each beacon. These
	// fillers are not stored interrupts. Their time equals the beacon time,
	// so the median stays that time and hike_span stays the raw difference.
	fill := func(height, time uint64) {
		if _, ok := docs[height]; ok {
			return
		}
		// Same natural bytes as the beacon at this height when one is stored,
		// so the beacon read stays match. A height with no stored beacon is
		// only an MTP window read.
		natural := bytes.Repeat([]byte{0xee}, 32)
		for _, stage := range object.Stages {
			if stage.BeaconHeight == height {
				natural = append([]byte(nil), stage.Beacon[:]...)
			}
		}
		if object.LastHeight != nil && *object.LastHeight == height {
			natural = append([]byte(nil), object.Last...)
		}
		docs[height] = indexDocument(natural, height, time)
	}
	// Heights the stored headers do not name are the rest of each MTP window.
	// Their time is height*1000, the same rule as a stored header.
	for h := uint64(0); h <= 20; h++ {
		fill(h, h*1000)
	}
	return Index{Documents: docs}
}

func indexDocument(natural []byte, height, time uint64) string {
	display := make([]byte, len(natural))
	for i := range natural {
		display[i] = natural[len(natural)-1-i]
	}
	return fmt.Sprintf(`{"header":{"hash":"%s","height":%d,"time":%d}}`, hex.EncodeToString(display), height, time)
}

func readProbe(t *testing.T, name string) []byte {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("caller")
	}
	raw, err := os.ReadFile(filepath.Join(filepath.Dir(file), "testdata", name))
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func containsAll(body string, parts []string) bool {
	for _, part := range parts {
		if !bytesContains(body, part) {
			return false
		}
	}
	return true
}

func bytesContains(body, part string) bool {
	return len(body) >= len(part) && (body == part || len(part) == 0 || stringIndex(body, part) >= 0)
}

func stringIndex(body, part string) int {
	for i := 0; i+len(part) <= len(body); i++ {
		if body[i:i+len(part)] == part {
			return i
		}
	}
	return -1
}
