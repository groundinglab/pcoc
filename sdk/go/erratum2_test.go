package pcoc

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

// malformedRecordSHA256 is the SHA-256 of the erratum 2 (E02) verify record
// for a malformed CBOR value: one JSON line and a trailing newline.
const malformedRecordSHA256 = "eb7bff4d55ddcc47e47c600276693db8a406a0083a47c2526462651ffa5bdb8c"

func erratum2Record(t *testing.T) []byte {
	t.Helper()
	b := mustRead(t, filepath.Join(sdkDir(t), "testdata", "erratum2-malformed-record.json"))
	if sha256Hex(b) != malformedRecordSHA256 {
		t.Fatalf("erratum 2 record sha256 %s, pin %s", sha256Hex(b), malformedRecordSHA256)
	}
	return b
}

func assertMalformedCLI(t *testing.T, body []byte) {
	t.Helper()
	want := erratum2Record(t)
	bin := pcocBin(t)
	path := filepath.Join(t.TempDir(), "in.cbor")
	if err := os.WriteFile(path, body, 0o644); err != nil {
		t.Fatal(err)
	}
	stdout, stderr, code := runCLI(t, bin, "verify", path)
	if code != 0 {
		t.Fatalf("exit %d stderr %s", code, stderr)
	}
	if stdout != string(want) {
		field, got, exp := firstJSONDiff(stdout, string(want))
		if field == "" {
			t.Fatal("record bytes differ from the erratum 2 record")
		}
		t.Fatalf("%s: got %s want %s", field, got, exp)
	}
	if sha256Hex([]byte(stdout)) != malformedRecordSHA256 {
		t.Fatalf("stdout sha256 %s, pin %s", sha256Hex([]byte(stdout)), malformedRecordSHA256)
	}
}

func TestErratum2EmptyInput(t *testing.T) {
	assertMalformedCLI(t, nil)
}

func TestErratum2TruncatedItem(t *testing.T) {
	assertMalformedCLI(t, []byte{0x58, 0x05, 0x01})
}

func TestErratum2KindNotUTF8(t *testing.T) {
	// Map of one pair: kind → one text byte 0xff, which is not UTF-8.
	assertMalformedCLI(t, []byte{0xa1, 0x64, 0x6b, 0x69, 0x6e, 0x64, 0x61, 0xff})
}

// TestErratum2DecodedEmptyCheckpointAndSkipPath keeps the malformed-record
// change off a decoded object. An empty checkpoint list is a count of 0
// (SPEC §12.2). An empty path byte string on a decoded stage is skip, and
// any other path bytes are present (Rule A4.5).
func TestErratum2DecodedEmptyCheckpointAndSkipPath(t *testing.T) {
	stages := make([]Stage, Stages)
	for g := range stages {
		stages[g] = Stage{
			Checkpoints: [][]byte{},
			LegResults:  [][]byte{},
		}
	}
	stages[2].Path = []byte{0x01}
	raw := EncodeObject(&Object{Kind: Kind, Stages: stages})
	dr := DecodeObject(raw)
	if dr.Class != ClassObject {
		t.Fatalf("decoded class %d, want object", dr.Class)
	}
	empty := dr.Object.Stages[1]
	if !empty.HasCP || len(empty.Checkpoints) != 0 {
		t.Fatal("stage 1 checkpoint list is not a decoded empty list")
	}
	if !empty.HasPath || len(empty.Path) != 0 {
		t.Fatal("stage 1 path is not a decoded empty byte string")
	}
	present := dr.Object.Stages[2]
	if !present.HasPath || len(present.Path) == 0 {
		t.Fatal("stage 2 path is empty")
	}

	rec, fail := Verify(VerifyInput{Profile: Production(), Object: raw})
	if fail {
		t.Fatal("decode failure")
	}
	var m map[string]any
	if err := json.Unmarshal(rec, &m); err != nil {
		t.Fatal(err)
	}
	paths := m["paths"].([]any)
	p1 := paths[0].(map[string]any)
	if p1["path_state"] != "skip" {
		t.Fatalf("path_state: got %#v, want skip", p1["path_state"])
	}
	if p1["plant"] != "n/a" {
		t.Fatalf("plant: got %#v, want n/a", p1["plant"])
	}
	p2 := paths[1].(map[string]any)
	if p2["path_state"] != "present" {
		t.Fatalf("path_state: got %#v, want present", p2["path_state"])
	}
	var saw1, saw2 bool
	for _, x := range m["skip_list"].([]any) {
		switch int(x.(float64)) {
		case 1:
			saw1 = true
		case 2:
			saw2 = true
		}
	}
	if !saw1 || saw2 {
		t.Fatalf("skip_list: got %#v", m["skip_list"])
	}
	an := m["analysis"].(map[string]any)
	for g, rawStage := range an["per_stage"].([]any) {
		item := rawStage.(map[string]any)
		n, ok := item["checkpoint_count"].(float64)
		if !ok || n != 0 {
			t.Fatalf("checkpoint_count: g %d got %#v, want 0", g, item["checkpoint_count"])
		}
	}
	if n, ok := an["whole_hike_legs"].(float64); !ok || n != 0 {
		t.Fatalf("whole_hike_legs: got %#v, want 0", an["whole_hike_legs"])
	}
	if n, ok := an["hike_legs"].(float64); !ok || n != 0 {
		t.Fatalf("hike_legs: got %#v, want 0", an["hike_legs"])
	}
}

func firstJSONDiff(got, want string) (field, gotVal, wantVal string) {
	var g, w any
	if err := json.Unmarshal([]byte(got), &g); err != nil {
		return "record", "not json", "erratum 2 record"
	}
	if err := json.Unmarshal([]byte(want), &w); err != nil {
		return "record", "cli record", "testdata is not json"
	}
	return walkJSON(g, w, "")
}

func walkJSON(got, want any, path string) (string, string, string) {
	switch w := want.(type) {
	case map[string]any:
		g, ok := got.(map[string]any)
		if !ok {
			return jsonPath(path), fmtJSON(got), fmtJSON(want)
		}
		keys := make([]string, 0, len(w))
		for k := range w {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			next := k
			if path != "" {
				next = path + "." + k
			}
			gv, exists := g[k]
			if !exists {
				return next, "absent", fmtJSON(w[k])
			}
			if f, a, b := walkJSON(gv, w[k], next); f != "" {
				return f, a, b
			}
		}
		return "", "", ""
	case []any:
		g, ok := got.([]any)
		if !ok || len(g) != len(w) {
			return jsonPath(path), fmtJSON(got), fmtJSON(want)
		}
		for i := range w {
			next := fmt.Sprintf("%s[%d]", path, i)
			if f, a, b := walkJSON(g[i], w[i], next); f != "" {
				return f, a, b
			}
		}
		return "", "", ""
	default:
		if !jsonLeafEqual(got, want) {
			return jsonPath(path), fmtJSON(got), fmtJSON(want)
		}
		return "", "", ""
	}
}

func jsonPath(path string) string {
	if path == "" {
		return "record"
	}
	return path
}

func jsonLeafEqual(a, b any) bool {
	ab, aerr := json.Marshal(a)
	bb, berr := json.Marshal(b)
	if aerr != nil || berr != nil {
		return false
	}
	return string(ab) == string(bb)
}

func fmtJSON(v any) string {
	b, err := json.Marshal(v)
	if err != nil {
		return fmt.Sprint(v)
	}
	return string(b)
}
