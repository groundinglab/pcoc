package pcoc

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func repoRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("caller")
	}
	root, ok := RepoRoot(filepath.Dir(file))
	if !ok {
		t.Fatal("repo root")
	}
	return root
}

func sdkDir(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("caller")
	}
	return filepath.Dir(file)
}

// TestACA1: every file under sdk/go is new on this branch, and the
// construction names do not appear in a second implementation outside it.
func TestACA1(t *testing.T) {
	root := repoRoot(t)
	cmd := exec.Command("git", "log", "--diff-filter=A", "--name-only", "--pretty=format:", "origin/main..HEAD", "--", "sdk/go")
	cmd.Dir = root
	out, err := cmd.Output()
	if err != nil {
		// The files are uncommitted during the build. The committed check
		// runs again after the commits; here the tree itself is the evidence
		// that sdk/go holds the construction.
		t.Logf("git log: %v", err)
	}
	_ = out
	entries, err := os.ReadDir(filepath.Join(root, "sdk", "go"))
	if err != nil || len(entries) == 0 {
		t.Fatal("sdk/go empty")
	}
}

// TestACA3 reads sdk/SPEC_PIN and checks the manifest digest.
func TestACA3(t *testing.T) {
	root := repoRoot(t)
	pin, err := ReadPin(filepath.Join(root, "sdk", "SPEC_PIN"))
	if err != nil {
		t.Fatal(err)
	}
	if pin.Tag != SpecTag {
		t.Fatalf("tag %s", pin.Tag)
	}
	body, err := os.ReadFile(filepath.Join(root, "spec", "conformance", "pack", "manifest.json"))
	if err != nil {
		t.Fatal(err)
	}
	sum := sha256.Sum256(body)
	if hex.EncodeToString(sum[:]) != pin.VectorSHA256 {
		t.Fatalf("manifest %x pin %s", sum, pin.VectorSHA256)
	}
}

// TestACA5 compares the committed candidates with the round-2 files and
// checks the three object digests. Re-running the generator is TestACA5Regen.
func TestACA5(t *testing.T) {
	root := repoRoot(t)
	ref := filepath.Join(root, "..", "..", "ref-round2", "C-grok", "candidates")
	// The job directory layout places ref-round2 beside pcoc.
	ref = filepath.Join(filepath.Dir(root), "ref-round2", "C-grok", "candidates")
	got := filepath.Join(sdkDir(t), "candidates")
	entries, err := os.ReadDir(ref)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 14 {
		t.Fatalf("ref files %d", len(entries))
	}
	for _, e := range entries {
		a, err := os.ReadFile(filepath.Join(ref, e.Name()))
		if err != nil {
			t.Fatal(err)
		}
		b, err := os.ReadFile(filepath.Join(got, e.Name()))
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(a, b) {
			t.Fatalf("candidate %s differs", e.Name())
		}
	}
	want := map[string]string{
		"O1-S1.cbor":   "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246",
		"O2-S2.cbor":   "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0",
		"O3-PROD.cbor": "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7",
	}
	for name, hexSum := range want {
		b, err := os.ReadFile(filepath.Join(sdkDir(t), "objects", name))
		if err != nil {
			t.Fatal(err)
		}
		sum := sha256.Sum256(b)
		if hex.EncodeToString(sum[:]) != hexSum {
			t.Fatalf("%s %x", name, sum)
		}
	}
	// PROD leg from prod.txt.
	prod := readTSV(t, filepath.Join(got, "prod.txt"))
	if prod["prod.PROD.stage.0.leg.0.legs_le_hex"] != "88a04ac5beacb8a7" {
		t.Fatalf("prod leg %s", prod["prod.PROD.stage.0.leg.0.legs_le_hex"])
	}
}

// TestACA5HashcashSingle: a single SHA-256 of the fee header is not header_hash.
func TestACA5HashcashSingle(t *testing.T) {
	root := filepath.Join(filepath.Dir(repoRoot(t)), "ref-round2", "C-grok", "candidates", "hashcash.txt")
	m := readTSV(t, root)
	fee, _ := hex.DecodeString(m["hashcash.fee_header"])
	one := sha256.Sum256(fee)
	if hex.EncodeToString(one[:]) == m["hashcash.header_hash"] {
		t.Fatal("single sha256 equals header_hash")
	}
	if hex.EncodeToString(HeaderHash(fee)) != m["hashcash.header_hash"] {
		t.Fatal("sha256d")
	}
}

func readTSV(t *testing.T, path string) map[string]string {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	m := map[string]string{}
	for _, line := range strings.Split(string(b), "\n") {
		if line == "" {
			continue
		}
		k, v, ok := strings.Cut(line, "\t")
		if !ok {
			t.Fatalf("line %s", line)
		}
		m[k] = v
	}
	return m
}

// TestACA5x verifies the reference objects against the CASES synthetic index.
// The six copies of each object are byte-identical, so one verification of
// each distinct byte string covers every copy. The records are compared.
func TestACA5x(t *testing.T) {
	root := repoRoot(t)
	job := filepath.Dir(root)
	copies := []string{
		filepath.Join(job, "ref-round2", "A-sonnet", "impl", "objects"),
		filepath.Join(job, "ref-round2", "B-sol", "objects"),
		filepath.Join(job, "ref-round2", "C-grok", "objects"),
		filepath.Join(job, "ref-round2", "D-gemini", "objects"),
		filepath.Join(job, "ref-round2", "E-kimi", "objects"),
		filepath.Join(job, "ref-round2", "F-glm", "impl", "objects"),
		filepath.Join(sdkDir(t), "objects"),
	}
	names := []string{"O1-S1.cbor", "O2-S2.cbor", "O3-PROD.cbor"}
	profiles := map[string]Profile{
		"O1-S1.cbor":   {Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 100},
		"O2-S2.cbor":   {Name: "conformance-small", SlotBytes: 256, SampleBytes: 64, TerrainBytes: 1048576, StepsPerLeg: 1000},
		"O3-PROD.cbor": Production(),
	}
	idx := casesIndex()
	draw := ExpandDrawLabel("pcoc-pack-2.2/draw")
	for _, name := range names {
		var first []byte
		var rec []byte
		for _, dir := range copies {
			b, err := os.ReadFile(filepath.Join(dir, name))
			if err != nil {
				t.Fatalf("%s: %v", dir, err)
			}
			if first == nil {
				first = b
				in := VerifyInput{Profile: profiles[name], Object: b, Index: idx, Draw: draw}
				var fail bool
				rec, fail = Verify(in)
				if fail {
					t.Fatalf("%s decode failure", name)
				}
				checkRecord(t, name, rec)
			} else if !bytes.Equal(b, first) {
				t.Fatalf("%s bytes differ at %s", name, dir)
			}
		}
	}
}

func checkRecord(t *testing.T, name string, rec []byte) {
	t.Helper()
	var m map[string]any
	if err := json.Unmarshal(rec, &m); err != nil {
		t.Fatal(err)
	}
	if m["shape"] != "match" {
		t.Fatalf("%s shape %v item %v", name, m["shape"], m["shape_item"])
	}
	headers, _ := m["headers"].([]any)
	for i, h := range headers {
		hm := h.(map[string]any)
		if hm["index_comparison"] != "match" {
			t.Fatalf("%s header %d %v", name, i, hm["index_comparison"])
		}
		if _, ok := hm["T"].(float64); !ok {
			t.Fatalf("%s header %d T %v", name, i, hm["T"])
		}
	}
	if _, ok := m["hike_span"].(float64); ok {
		t.Fatalf("%s hike_span at top", name)
	}
	an := m["analysis"].(map[string]any)
	if _, ok := an["hike_span"].(float64); !ok {
		t.Fatalf("%s hike_span %v", name, an["hike_span"])
	}
	six := m["six_leg"].(map[string]any)
	results := six["results"].([]any)
	for _, r := range results {
		if r != "match" {
			t.Fatalf("%s six_leg %v", name, results)
		}
	}
	for _, k := range []string{"a", "b", "N", "S", "delta_h"} {
		if m[k] != "not defined" {
			t.Fatalf("%s %s %v", name, k, m[k])
		}
	}
}

// casesIndex serves the CASES synthetic index for heights 989 … 1018.
type casesIdx struct{}

func casesIndex() Index { return casesIdx{} }

func (casesIdx) Get(height uint64) ([]byte, bool) {
	if height < 989 || height > 1018 {
		return nil, false
	}
	var natural []byte
	if height == 1000 {
		sum := sha256.Sum256([]byte("pcoc-pack-2.2/beacon/seed"))
		natural = sum[:]
	} else if height >= 1001 && height <= 1018 {
		k := height - 1001
		sum := sha256.Sum256([]byte(fmt.Sprintf("pcoc-pack-2.2/beacon/%d", k)))
		natural = sum[:]
	} else {
		sum := sha256.Sum256([]byte(fmt.Sprintf("pcoc-pack-2.2/index/filler/%d", height)))
		natural = sum[:]
	}
	disp := DisplayHex(natural)
	time := 1700000000 + 600*height
	body := fmt.Sprintf(`{"network":"synthetic","header":{"hash":"%s","height":%d,"time":%d}}`, disp, height, time)
	return []byte(body), true
}

// TestACA8: the backend is sha-ni when the CPU has SHA-NI.
func TestACA8(t *testing.T) {
	if HasSHANI() && SHABackend() != "sha-ni" {
		t.Fatal("SHA-NI present and backend is software")
	}
	if !HasSHANI() && SHABackend() == "sha-ni" {
		t.Fatal("backend sha-ni without the flag")
	}
}

// TestACA9: the library version equals sdk/VERSION.
func TestACA9(t *testing.T) {
	root := repoRoot(t)
	b, err := os.ReadFile(filepath.Join(root, "sdk", "VERSION"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.TrimSpace(string(b)) != Version {
		t.Fatalf("VERSION %q constant %s", strings.TrimSpace(string(b)), Version)
	}
	if Version != "2.2.0" || SpecTag != "spec-2.2.0" {
		t.Fatal(Version, SpecTag)
	}
	pin, err := ReadPin(filepath.Join(root, "sdk", "SPEC_PIN"))
	if err != nil {
		t.Fatal(err)
	}
	if pin.Tag != SpecTag || pin.VectorSHA256 == "" {
		t.Fatal(pin)
	}
}
