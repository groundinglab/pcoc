package hike

import (
	"bytes"
	"testing"
)

func nestedArrays(prefix []byte, depth int) []byte {
	out := append([]byte(nil), prefix...)
	out = append(out, bytes.Repeat([]byte{0x81}, depth-1)...)
	return append(out, 0x80)
}

// MaxCBORNesting (64, the same in Rust and Java) is an implementation
// resource limit: 64 nested arrays or maps decode, 65 are malformed CBOR.
func TestNestingLimitBoundary(t *testing.T) {
	if MaxCBORNesting != 64 {
		t.Fatalf("limit %d", MaxCBORNesting)
	}
	if issue := CBORIssue(nestedArrays(nil, 64)); issue != "" {
		t.Fatalf("64 deep: %q", issue)
	}
	if issue := CBORIssue(nestedArrays(nil, 65)); issue != "nesting depth" {
		t.Fatalf("65 deep: %q", issue)
	}
	maps := bytes.Repeat([]byte{0xa1, 0x61, 'k'}, 63)
	maps = append(maps, 0xa0)
	if issue := CBORIssue(maps); issue != "" {
		t.Fatalf("64 maps: %q", issue)
	}
	if issue := CBORIssue(append(bytes.Repeat([]byte{0xa1, 0x61, 'k'}, 64), 0xa0)); issue != "nesting depth" {
		t.Fatalf("65 maps: %q", issue)
	}
}

// About 100 000 deep, alone, under a map key, and truncated: the undecoded
// record (decoded false), never a panic or a stack-bound recursion.
func TestDeepNestingIsTheUndecodedRecord(t *testing.T) {
	want, _ := Verify(nil, ConformanceSmallV2, Index{}, nil, false)
	key := []byte{0xa1, 0x63, 'z', 'z', 'z'}
	inputs := [][]byte{
		nestedArrays(nil, 100_000),
		nestedArrays(key, 100_000),
		nestedArrays(key, 8_000),
		nestedArrays(key, 100_000)[:50_000],
	}
	for n, in := range inputs {
		rec, decoded := Verify(in, ConformanceSmallV2, Index{}, nil, false)
		if decoded || !bytes.Equal(rec.Marshal(), want.Marshal()) {
			t.Fatalf("input %d: decoded=%v", n, decoded)
		}
	}
}

// Sibling containers do not add up: only the open ones count.
func TestSiblingContainersDoNotCount(t *testing.T) {
	arrays := []byte{0x98, 200}
	maps := []byte{0xb8, 200}
	for n := 0; n < 200; n++ {
		arrays = append(arrays, 0x81, 0x80)
		maps = append(maps, 0x62, byte('a'+n/26), byte('a'+n%26), 0xa1, 0x61, 'v', 0xa0)
	}
	if issue := CBORIssue(arrays); issue != "" {
		t.Fatalf("arrays: %q", issue)
	}
	if issue := CBORIssue(maps); issue != "" {
		t.Fatalf("maps: %q", issue)
	}
}
