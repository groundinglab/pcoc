package hike

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

// rc.2 break pass: byte edits of the kat-small object (Rust tests/break_cases.rs
// and sdk/tools/hikecases.py cover the same facts across the three SDKs).

type katPack struct {
	ObjectHex string `json:"object_hex"`
	Documents []struct {
		Height uint64  `json:"height"`
		Body   *string `json:"body"`
	} `json:"documents"`
}

func loadKat(t *testing.T) ([]byte, Index) {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join("..", "..", "..", "spec", "conformance", "v2.0", "kat-small.json"))
	if err != nil {
		t.Fatal(err)
	}
	var k katPack
	if err := json.Unmarshal(raw, &k); err != nil {
		t.Fatal(err)
	}
	obj, err := hex.DecodeString(k.ObjectHex)
	if err == nil {
		// The v2.0 object is an earlier kind under v2.1. These byte edits
		// measure a current object, so the kind text is the v2.1 kind.
		old := []byte("satoshi-hike-v2.0-object")
		next := []byte("satoshi-hike-v2.1-object")
		obj = bytes.Replace(obj, old, next, 1)
	}
	if err != nil {
		t.Fatal(err)
	}
	docs := map[uint64]string{}
	for _, d := range k.Documents {
		if d.Body != nil {
			docs[d.Height] = *d.Body
		}
	}
	return obj, Index{Documents: docs}
}

func nthIndex(t *testing.T, data, needle []byte, k int) int {
	t.Helper()
	at := -1
	for i := 0; i <= k; i++ {
		next := bytes.Index(data[at+1:], needle)
		if next < 0 {
			t.Fatalf("needle %q #%d", needle, k)
		}
		at += 1 + next
	}
	return at
}

// shorten makes the 32-byte string that follows the k-th needle 31 bytes long.
func shorten(t *testing.T, obj, needle []byte, k int) []byte {
	at := nthIndex(t, obj, needle, k) + len(needle) - 2 // the 0x58 0x20 head
	out := append([]byte{}, obj[:at]...)
	out = append(out, 0x58, 0x1f)
	out = append(out, obj[at+2:at+33]...)
	return append(out, obj[at+34:]...)
}

func katRecord(t *testing.T, data []byte, index Index) (map[string]any, bool) {
	t.Helper()
	rec, decoded := Verify(data, ConformanceSmallV2, index, AuditStream(bytes.Repeat([]byte{0x11}, 32), 4096), false)
	var out map[string]any
	if err := json.Unmarshal(rec.Marshal(), &out); err != nil {
		t.Fatal(err)
	}
	return out, decoded
}

func at(v any, path ...any) any {
	for _, p := range path {
		switch k := p.(type) {
		case string:
			v = v.(map[string]any)[k]
		case int:
			v = v.([]any)[k]
		}
	}
	return v
}

// A 31-byte checkpoint in stage 2: N, fixed_start.N and legs_outside still
// count it (section 14.4: "An integer when bookends exist"; section 14.6 L_g).
func TestRC2ShortCheckpointStillCounts(t *testing.T) {
	obj, index := loadKat(t)
	rec, _ := katRecord(t, shorten(t, obj, []byte("\x6bcheckpoints\x81\x58\x20"), 2), index)
	for _, c := range []struct {
		path []any
		want any
	}{
		{[]any{"N"}, 1.0},
		{[]any{"fixed_start", "N"}, 1.0},
		{[]any{"analysis", "legs_outside"}, 17.0},
		{[]any{"analysis", "whole_hike_legs"}, 17.0},
		{[]any{"analysis", "per_stage", 2, "checkpoint_count"}, 1.0},
		{[]any{"checkpoints", 2, "legs"}, "not defined"},
		{[]any{"waypoints", 2, "result"}, "not defined"},
	} {
		if got := at(rec, c.path...); got != c.want {
			t.Fatalf("%v = %v, want %v", c.path, got, c.want)
		}
	}
}

// A stored hash that is not 32 bytes is `not defined`, for a stage beacon and
// for last_interrupt.
func TestRC2ShortStoredHashIsNotDefined(t *testing.T) {
	obj, index := loadKat(t)
	rec, _ := katRecord(t, shorten(t, obj, []byte("\x66beacon\x58\x20"), 8), index)
	if got := at(rec, "headers", 8, "stored_hash"); got != "not defined" {
		t.Fatalf("stage 8 stored_hash %v", got)
	}
	rec, _ = katRecord(t, shorten(t, obj, []byte("\x6elast_interrupt\x58\x20"), 0), index)
	if got := at(rec, "headers", 18, "stored_hash"); got != "not defined" {
		t.Fatalf("last_interrupt stored_hash %v", got)
	}
}

// A checkpoints or leg_results value that is not an array is `field set`.
func TestRC2NonArrayCheckpointsIsFieldSet(t *testing.T) {
	obj, index := loadKat(t)
	for _, key := range []string{"\x6bcheckpoints", "\x6bleg_results"} {
		needle := []byte(key + "\x81")
		at0 := nthIndex(t, obj, needle, 5) + len(needle) - 1
		width := 34 // 0x58 0x20 + 32 bytes
		if key == "\x6bleg_results" {
			width = 9 // 0x48 + 8 bytes
		}
		edited := append(append(append([]byte{}, obj[:at0]...), 0x61, 'x'), obj[at0+1+width:]...)
		rec, decoded := katRecord(t, edited, index)
		items := at(rec, "shape_item").([]any)
		if !decoded || len(items) == 0 || items[0] != "field set" {
			t.Fatalf("%q: %v", key, items)
		}
	}
}

// rc.2: a top level that is not a map is the undecoded ["malformed cbor"]
// record, the same bytes as no input at all, and does not decode.
func TestRC2NonMapTopLevelIsUndecoded(t *testing.T) {
	none, decoded := Verify(nil, ConformanceSmallV2, Index{}, nil, false)
	if decoded {
		t.Fatal("empty input decoded")
	}
	for _, in := range [][]byte{{0x01}, {0x80}, {0x60}, {0x40}, {0x01, 0x00}} {
		rec, decoded := Verify(in, ConformanceSmallV2, Index{}, nil, false)
		if decoded || !bytes.Equal(rec.Marshal(), none.Marshal()) {
			t.Fatalf("%x decoded=%v", in, decoded)
		}
	}
	if _, decoded := Verify([]byte{0xa0, 0x00}, ConformanceSmallV2, Index{}, nil, false); !decoded {
		t.Fatal("a map with a trailing byte is a decoded top-level map")
	}
}

// F9: a checkpoint or leg-result element that is not bytes is field set
// as well as the width condition.
func TestF9NonByteElementIsFieldSet(t *testing.T) {
	obj, index := loadKat(t)
	// Stage 4's one checkpoint and one leg result, replaced by text.
	for _, c := range []struct {
		needle string
		width  int
		want   string
	}{
		{"\x6bcheckpoints\x81", 34, "checkpoint width"},
		{"\x6bleg_results\x81", 9, "leg result width"},
	} {
		pos := nthIndex(t, obj, []byte(c.needle), 4) + len(c.needle)
		edited := append(append(append([]byte{}, obj[:pos]...), 0x61, 'x'), obj[pos+c.width:]...)
		rec, decoded := katRecord(t, edited, index)
		if !decoded {
			t.Fatal(c.needle)
		}
		items := at(rec, "shape_item").([]any)
		has := map[string]bool{}
		for _, item := range items {
			has[item.(string)] = true
		}
		if !has["field set"] || !has[c.want] {
			t.Fatalf("%s %v", c.needle, items)
		}
	}
}

// F7: a text last_interrupt or last_interrupt_height is field set.
func TestF7TextLastInterruptIsFieldSet(t *testing.T) {
	obj, index := loadKat(t)
	check := func(needle string, textHead byte, width int) {
		t.Helper()
		pos := nthIndex(t, obj, []byte(needle), 0) + len(needle)
		edited := append(append(append([]byte{}, obj[:pos]...), textHead, 'x'), obj[pos+width:]...)
		rec, decoded := katRecord(t, edited, index)
		if !decoded {
			t.Fatal(needle)
		}
		items := at(rec, "shape_item").([]any)
		has := func(name string) bool {
			for _, item := range items {
				if item == name {
					return true
				}
			}
			return false
		}
		if !has("field set") || !has("last_interrupt") {
			t.Fatalf("%s pos=%d %v", needle, pos, items)
		}
	}
	check("\x6elast_interrupt", 0x61, 34)
	check("\x75last_interrupt_height", 0x61, 3)
}

// short-li: a 31-byte last_interrupt is the wrong width, not a wrong type.
// shape_item is last_interrupt and header hash differ, with no field set.
func TestF7ShortLastInterruptIsNotFieldSet(t *testing.T) {
	obj, index := loadKat(t)
	rec, decoded := katRecord(t, shorten(t, obj, []byte("\x6elast_interrupt\x58\x20"), 0), index)
	if !decoded {
		t.Fatal("short-li did not decode")
	}
	items := at(rec, "shape_item").([]any)
	if len(items) != 2 || items[0] != "last_interrupt" || items[1] != "header hash differ" {
		t.Fatalf("short-li %v", items)
	}
}

// F6: 19 stages with stage 3's header copied from stage 2 lists stages
// length, header height order and header hash differ. stages length does
// not drop the other conditions over the stages that are present.
func TestF6NineteenStagesKeepOtherShapeConditions(t *testing.T) {
	obj, index := loadKat(t)
	item, err := decodeItem(obj)
	if err != nil {
		t.Fatal(err)
	}
	stages, _ := item.field("stages")
	dup := stages.arr[2]
	beacon, _ := dup.field("beacon")
	height, _ := dup.field("beacon_height")
	target := &stages.arr[3]
	for i, key := range target.keys {
		if key == "beacon" {
			target.vals[i] = beacon
		}
		if key == "beacon_height" {
			target.vals[i] = height
		}
	}
	stages.arr = append(stages.arr, stages.arr[17])
	for i, key := range item.keys {
		if key == "stages" {
			item.vals[i] = stages
		}
	}
	rec, decoded := katRecord(t, encodeCBOR(item), index)
	if !decoded {
		t.Fatal("decoded")
	}
	got := fmt.Sprint(at(rec, "shape_item"))
	want := fmt.Sprint([]any{"stages length", "header height order", "header hash differ"})
	if got != want {
		t.Fatalf("shape %s", got)
	}
}

func encodeCBOR(v cborVal) []byte {
	switch v.kind {
	case 0:
		return encodeUint(v.u)
	case 2:
		return encodeBytes(v.b)
	case 3:
		return encodeText(v.s)
	case 4:
		items := make([][]byte, len(v.arr))
		for i, item := range v.arr {
			items[i] = encodeCBOR(item)
		}
		return encodeArray(items)
	case 5:
		pairs := make([][2][]byte, len(v.keys))
		for i, key := range v.keys {
			pairs[i] = textPair(key, encodeCBOR(v.vals[i]))
		}
		return encodeMap(pairs)
	default:
		return nil
	}
}

// Q-j: a non-height integer above 2^53−1 is not height_range.
// leg_count stays leg_count. n_slots stays constant.
func TestQJNonHeightIntegerIsNotHeightRange(t *testing.T) {
	obj, index := loadKat(t)
	replaceUint := func(needle []byte, n uint64) []byte {
		t.Helper()
		pos := nthIndex(t, obj, needle, 0) + len(needle)
		var be [8]byte
		binary.BigEndian.PutUint64(be[:], n)
		edited := append(append([]byte{}, obj[:pos-1]...), 0x1b)
		edited = append(edited, be[:]...)
		return append(edited, obj[pos:]...)
	}
	rec, decoded := katRecord(t, replaceUint([]byte("\x69leg_count\x01"), 1<<53), index)
	if !decoded {
		t.Fatal("leg_count")
	}
	if fmt.Sprint(at(rec, "shape_item")) != fmt.Sprint([]any{"leg_count"}) {
		t.Fatalf("leg_count shape %v", at(rec, "shape_item"))
	}
	rec, decoded = katRecord(t, replaceUint([]byte("\x67n_slots\x04"), ^uint64(0)), index)
	if !decoded {
		t.Fatal("n_slots")
	}
	if fmt.Sprint(at(rec, "shape_item")) != fmt.Sprint([]any{"constant"}) {
		t.Fatalf("n_slots shape %v", at(rec, "shape_item"))
	}
}

// putUint19 replaces the k-th 2-byte unsigned (head 0x19 plus two bytes) that
// the needle ends on with an 8-byte unsigned of n.
func putUint19(t *testing.T, obj, needle []byte, k int, n uint64) []byte {
	t.Helper()
	pos := nthIndex(t, obj, needle, k) + len(needle)
	var be [8]byte
	binary.BigEndian.PutUint64(be[:], n)
	edited := append(append([]byte{}, obj[:pos-1]...), 0x1b)
	edited = append(edited, be[:]...)
	return append(edited, obj[pos+2:]...)
}

func shapeIs(t *testing.T, rec map[string]any, want []any) {
	t.Helper()
	got := at(rec, "shape_item")
	if fmt.Sprint(got) != fmt.Sprint(want) {
		t.Fatalf("shape %v", got)
	}
}

// F5 / Q-a: last_interrupt_height of 2^53 and of 2^64−1 is height_range only.
// Header height order compares the exact integer, and both values are greater
// than stage 17's height, so header height order is not listed.
func TestF5OutOfRangeInterruptHeightKeepsExactOrder(t *testing.T) {
	obj, index := loadKat(t)
	needle := []byte("\x75last_interrupt_height\x19")
	for _, n := range []uint64{1 << 53, ^uint64(0)} {
		rec, decoded := katRecord(t, putUint19(t, obj, needle, 0, n), index)
		if !decoded {
			t.Fatal("decoded")
		}
		shapeIs(t, rec, []any{"height_range"})
		if at(rec, "headers", 18, "stored_height") != "not defined" {
			t.Fatalf("height %d stored %v", n, at(rec, "headers", 18, "stored_height"))
		}
	}
	// Stage 17 at 2^53 is still below last_interrupt_height 2^64−1.
	both := putUint19(t, obj, needle, 0, ^uint64(0))
	both = putUint19(t, both, []byte("\x6dbeacon_height\x19"), 17, 1<<53)
	rec, decoded := katRecord(t, both, index)
	if !decoded {
		t.Fatal("decoded")
	}
	shapeIs(t, rec, []any{"height_range"})
	// Stage 16 at 2^64−1 is followed by stage 17's smaller height.
	gap := putUint19(t, obj, []byte("\x6dbeacon_height\x19"), 16, ^uint64(0))
	rec, decoded = katRecord(t, gap, index)
	if !decoded {
		t.Fatal("decoded")
	}
	shapeIs(t, rec, []any{"header height order", "height_range"})
}

// A refused index read returns no document. plant on an issued path is not read.
func TestPlantNotReadWhenIndexReturnsNoDocument(t *testing.T) {
	obj, index := loadKat(t)
	got, decoded := katRecord(t, obj, index)
	if !decoded || !pathPlant(got, "match") {
		t.Fatal("pack index plant")
	}
	refused := Index{Fetch: func(uint64) (string, bool) { return "", false }}
	got, decoded = katRecord(t, obj, refused)
	if !decoded || pathPlant(got, "match") || !pathPlant(got, "not read") {
		t.Fatalf("refused plant %v", at(got, "paths"))
	}
}

// Q-b: stage 5 replaced by the unsigned integer 7 is a stage with every field
// absent, so its path is skip, not an absent stage.
func TestQBNonMapStageIsEveryFieldAbsent(t *testing.T) {
	obj, index := loadKat(t)
	item, err := decodeItem(obj)
	if err != nil {
		t.Fatal(err)
	}
	stages, ok := item.field("stages")
	if !ok || len(stages.arr) < 6 {
		t.Fatal("stages")
	}
	stages.arr[5] = cborVal{kind: 0, u: 7}
	rec, decoded := katRecord(t, encodeCBOR(item), index)
	if !decoded {
		t.Fatal("decoded")
	}
	paths, _ := rec["paths"].([]any)
	row, _ := paths[4].(map[string]any)
	if row["path_state"] != "skip" || row["plant"] != "n/a" {
		t.Fatalf("stage 5 %+v", row)
	}
}

func pathPlant(rec map[string]any, token string) bool {
	paths, _ := rec["paths"].([]any)
	for _, row := range paths {
		obj, _ := row.(map[string]any)
		if obj["plant"] == token {
			return true
		}
	}
	return false
}

// F4: seed 11*32 draws stage 5 leg 0. A waypoint that is not 32 bytes makes
// that leg mismatch. Section 14.6 reads the stored waypoint; section 11.2
// does not take the first 8 bytes of a value of another width.
func TestF4WrongWidthWaypointLeg0IsMismatch(t *testing.T) {
	obj, index := loadKat(t)
	stream := AuditStream(bytes.Repeat([]byte{0x11}, 32), 4096)
	// Stage 5's waypoint is the 6th 0x68 waypoint 0x58 0x20 (stages 0..5).
	needle := []byte("\x68waypoint\x58\x20")
	at := nthIndex(t, obj, needle, 5) + len(needle)
	check := func(edited []byte, width int) {
		t.Helper()
		rec, decoded := Verify(edited, ConformanceSmallV2, index, stream, false)
		if !decoded {
			t.Fatal("decoded")
		}
		var out map[string]any
		if err := json.Unmarshal(rec.Marshal(), &out); err != nil {
			t.Fatal(err)
		}
		six := atAny(out, "six_leg").(map[string]any)
		indices := six["indices"].([]any)
		results := six["results"].([]any)
		found := false
		for i, q := range indices {
			if q.(float64) == 5 {
				found = true
				if results[i] != "mismatch" {
					t.Fatalf("width %d leg result %v", width, results[i])
				}
			}
		}
		if !found {
			t.Fatalf("width %d draw %v did not include index 5", width, indices)
		}
		items := atAny(out, "shape_item").([]any)
		if len(items) == 0 || items[0] != "field set" {
			t.Fatalf("width %d shape %v", width, items)
		}
	}
	// 31 bytes, 33 bytes, 8 bytes.
	check(append(append(append([]byte{}, obj[:at-2]...), 0x58, 0x1f), append(append([]byte{}, obj[at:at+31]...), obj[at+32:]...)...), 31)
	check(append(append(append([]byte{}, obj[:at-1]...), 0x21), append(append([]byte{}, obj[at:at+32]...), append([]byte{0}, obj[at+32:]...)...)...), 33)
	check(append(append(append([]byte{}, obj[:at-2]...), 0x48), append([]byte{}, obj[at+24:]...)...), 8)
}

func atAny(v any, path ...any) any { return at(v, path...) }

// F2: a declared byte, text, array or map length past the bytes still in the
// buffer is malformed CBOR. Verify must return the undecoded record and must
// not panic. The length is compared as uint64 before any allocation.
func TestF2OversizedLengthIsMalformed(t *testing.T) {
	none, decoded := Verify(nil, ConformanceSmallV2, Index{}, nil, false)
	if decoded {
		t.Fatal("empty input decoded")
	}
	want := none.Marshal()
	// 5b 7fffffffffffffff — a byte string of length 2^63−1.
	// 7b 7fffffffffffffff — the same length as text.
	// 9b ffffffffffffffff — an array of 2^64−1 elements.
	// ba ffffffff — a map of 2^32 pairs.
	// 9a 3b9aca00 — an array of 1_000_000_000 elements and no elements.
	inputs := [][]byte{
		{0x5b, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff},
		{0x7b, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff},
		{0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff},
		{0xba, 0xff, 0xff, 0xff, 0xff},
		{0x9a, 0x3b, 0x9a, 0xca, 0x00},
	}
	for _, in := range inputs {
		rec, decoded := Verify(in, ConformanceSmallV2, Index{}, nil, false)
		if decoded || !bytes.Equal(rec.Marshal(), want) {
			t.Fatalf("%x decoded=%v record=%s", in, decoded, rec.Marshal())
		}
		if issue := CBORIssue(in); issue == "" {
			t.Fatalf("%x decoded as one item", in)
		}
	}
}
