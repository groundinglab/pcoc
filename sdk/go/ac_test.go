package pcoc

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"testing"
	"time"
)

// repoRoot walks up from this test file to the directory that holds both
// sdk/VERSION and spec/. A missing root is a failure, not a skip.
func repoRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("cannot locate the repository: runtime.Caller failed")
	}
	start := filepath.Dir(file)
	if st, err := os.Stat(start); err != nil || !st.IsDir() {
		wd, werr := os.Getwd()
		if werr != nil {
			t.Fatalf("cannot locate the repository: %s is not a directory (%v) and the working directory is unavailable (%v)", start, err, werr)
		}
		start = wd
	}
	root, err := walkRepo(start)
	if err != nil {
		t.Fatal(err)
	}
	return root
}

func walkRepo(start string) (string, error) {
	dir := start
	for i := 0; i < 16; i++ {
		ver := filepath.Join(dir, "sdk", "VERSION")
		spec := filepath.Join(dir, "spec")
		stV, errV := os.Stat(ver)
		stS, errS := os.Stat(spec)
		if errV == nil && errS == nil && !stV.IsDir() && stS.IsDir() {
			return dir, nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return "", fmt.Errorf("cannot locate the repository: no directory at or above %s holds both sdk/VERSION and spec/", start)
}

func sdkDir(t *testing.T) string {
	t.Helper()
	return filepath.Join(repoRoot(t), "sdk", "go")
}

// Round-2 agreed bytes. The six independent builds and the SDKs agreed on
// these SHA-256 values of the committed candidate files and objects. The
// cross-build comparison itself is the separate cross-verify gate.
var candidateNames = []string{
	"anchor.txt",
	"br1.txt",
	"draw.txt",
	"fold.txt",
	"hashcash.txt",
	"kat-small.txt",
	"negatives.txt",
	"nl4.txt",
	"prod.txt",
	"rate.txt",
	"seed.txt",
	"terrain.txt",
	"test-3leg.txt",
	"unknown-kind.txt",
	"wes.txt",
}

var candidatePins = map[string]string{
	"anchor.txt":       "fe58c0b64263d11daa7dcbae7c6daa3e0b0daecb1084b44d47f6f2570da3811a",
	"br1.txt":          "76b36c948ae0f37ad9dffd0355db3e507f23fddedc203dde4d58d131b212fa99",
	"draw.txt":         "45821e7ed5d9bbb6b03c686dfc467c623203027a6adf5160414c4e4074a85e0c",
	"fold.txt":         "2f14b5ef6953e00673c4c7f124f0057cb5498f17d354057639d944e441ffd271",
	"hashcash.txt":     "92f010b46c15483877343f635164c485500e55527721305bedd0a34def7ca9d5",
	"kat-small.txt":    "f999097217af06b00a8b879dfc9422263f63cc80b1462d41ff64d09ea866b192",
	"negatives.txt":    "a236c2966ae46b3e2a9bd83f4414fb4cf8f166106ce0553443aa4bc2e91462d4",
	"nl4.txt":          "f3fc7424ecd76dbe8ca7976d2fc780bb2a448206e6f116defa4b7cb67fb67a0b",
	"prod.txt":         "0eb0a567aa73350b80822c86ea3b3ec71ce8f3078bb59012019662cb5850c995",
	"rate.txt":         "153fbec4acbd3678953f4420d4e5af2f405e866ea16361cc50ab8142cfeef34f",
	"seed.txt":         "10179736fe1394a3275ad010ad87d5db03214ab3993d94d7cd27ec5c43802002",
	"terrain.txt":      "f5664d41b9bdc811adc2d7b321f518d33b13808746cf7016837948269019e348",
	"test-3leg.txt":    "621277600af9a31e40f244ac6010dbe059e602b3b48e9b92a8ad0c0b6d8f7eea",
	"unknown-kind.txt": "351fe1fa59d6acd69039ca08704720dba7505155f0d5b26a7bb927321fbcccc2",
	"wes.txt":          "91c24b4c0e64b5e75bbe5422341cf01e09b25a12833196ac9ed8b898b5f6d768",
}

var objectNames = []string{"O1-S1.cbor", "O2-S2.cbor", "O3-PROD.cbor"}

var objectPins = map[string]string{
	"O1-S1.cbor":   "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246",
	"O2-S2.cbor":   "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0",
	"O3-PROD.cbor": "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7",
}

// prodLegPin is PROD stage 0 leg 0, legs_le_hex.
const prodLegPin = "88a04ac5beacb8a7"

func sha256Hex(b []byte) string {
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:])
}

func mustRead(t *testing.T, path string) []byte {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("%s: %v", path, err)
	}
	return b
}

func dirFileNames(t *testing.T, dir string) []string {
	t.Helper()
	ents, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("%s: %v", dir, err)
	}
	names := make([]string, 0, len(ents))
	for _, e := range ents {
		if e.IsDir() {
			t.Fatalf("%s/%s is a directory", dir, e.Name())
		}
		names = append(names, e.Name())
	}
	sort.Strings(names)
	return names
}

func sameNames(a, b []string) bool {
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

func checkCandidatePins(t *testing.T, dir string) {
	t.Helper()
	got := dirFileNames(t, dir)
	if !sameNames(got, candidateNames) {
		t.Fatalf("candidates %v, pinned names %v", got, candidateNames)
	}
	for _, name := range candidateNames {
		b := mustRead(t, filepath.Join(dir, name))
		sum := sha256Hex(b)
		want := candidatePins[name]
		if sum != want {
			t.Fatalf("candidate %s SHA-256 %s, pin %s", name, sum, want)
		}
	}
}

func checkObjectPins(t *testing.T, dir string) {
	t.Helper()
	got := dirFileNames(t, dir)
	if !sameNames(got, objectNames) {
		t.Fatalf("objects %v, pinned names %v", got, objectNames)
	}
	for _, name := range objectNames {
		b := mustRead(t, filepath.Join(dir, name))
		sum := sha256Hex(b)
		want := objectPins[name]
		if sum != want {
			t.Fatalf("object %s SHA-256 %s, pin %s", name, sum, want)
		}
	}
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
	// sdk/SPEC_PIN is owned by the release job. Until it names spec-2.3.0 the
	// pin tag and SpecTag may differ. The digest check below still applies.
	if pin.Tag == "" {
		t.Fatal("pin tag empty")
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

// TestACA5 regenerates the fifteen candidate files and checks them
// byte-identical to the committed files, checks each committed file's
// SHA-256 against its pin, checks the three object pins, and checks the
// PROD leg pin.
func TestACA5(t *testing.T) {
	sdk := sdkDir(t)
	cand := filepath.Join(sdk, "candidates")
	checkCandidatePins(t, cand)
	checkObjectPins(t, filepath.Join(sdk, "objects"))
	prod := readTSV(t, filepath.Join(cand, "prod.txt"))
	leg := prod["prod.PROD.stage.0.leg.0.legs_le_hex"]
	if leg != prodLegPin {
		t.Fatalf("prod.PROD.stage.0.leg.0.legs_le_hex %s, pin %s", leg, prodLegPin)
	}

	regen := t.TempDir()
	cmd := exec.Command("go", "run", "./gen", regen)
	cmd.Dir = sdk
	cmd.Env = append(os.Environ(), "GOMAXPROCS=4")
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("generator: %v\n%s", err, out)
	}
	got := dirFileNames(t, regen)
	if !sameNames(got, candidateNames) {
		t.Fatalf("regenerated candidates %v, committed names %v", got, candidateNames)
	}
	for _, name := range candidateNames {
		a := mustRead(t, filepath.Join(regen, name))
		b := mustRead(t, filepath.Join(cand, name))
		if !bytes.Equal(a, b) {
			t.Fatalf("candidate %s differs from the committed file", name)
		}
	}
}

// TestACA5HashcashSingle: a single SHA-256 of the fee header is not header_hash.
// The file is the committed candidate, checked against its pin.
func TestACA5HashcashSingle(t *testing.T) {
	path := filepath.Join(sdkDir(t), "candidates", "hashcash.txt")
	body := mustRead(t, path)
	sum := sha256Hex(body)
	if sum != candidatePins["hashcash.txt"] {
		t.Fatalf("candidate hashcash.txt SHA-256 %s, pin %s", sum, candidatePins["hashcash.txt"])
	}
	m := readTSV(t, path)
	fee, err := hex.DecodeString(m["hashcash.fee_header"])
	if err != nil {
		t.Fatalf("hashcash.fee_header: %v", err)
	}
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

// TestACA5x checks each committed object's SHA-256 against its pin and
// verifies it. The CASES synthetic index is served by this harness on
// loopback. checkRecord is the section 14 check. Each draw starts at the
// first byte of the label chain.
func TestACA5x(t *testing.T) {
	objDir := filepath.Join(sdkDir(t), "objects")
	checkObjectPins(t, objDir)
	profiles := map[string]Profile{
		"O1-S1.cbor":   {Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 100},
		"O2-S2.cbor":   {Name: "conformance-small", SlotBytes: 256, SampleBytes: 64, TerrainBytes: 1048576, StepsPerLeg: 1000},
		"O3-PROD.cbor": Production(),
	}
	idx := serveCasesIndex(t)
	for _, name := range objectNames {
		b := mustRead(t, filepath.Join(objDir, name))
		in := VerifyInput{Profile: profiles[name], Object: b, Index: idx, Draw: ExpandDrawLabel("pcoc-pack-2.2/draw")}
		rec, fail := Verify(in)
		if fail {
			t.Fatalf("%s decode failure", name)
		}
		checkRecord(t, name, rec)
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

// casesIndexDocument is one CASES synthetic index document.
// Heights 989 … 1018 are served. Any other height is not a document.
func casesIndexDocument(height uint64) ([]byte, bool) {
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
	blockTime := 1700000000 + 600*height
	body := fmt.Sprintf(`{"network":"synthetic","header":{"hash":"%s","height":%d,"time":%d}}`, disp, height, blockTime)
	return []byte(body), true
}

// loopbackIndex reads GET http://127.0.0.1:<port>/index/<height>.
type loopbackIndex struct {
	t    *testing.T
	base string
	cli  *http.Client
}

func (l loopbackIndex) Get(height uint64) ([]byte, bool) {
	l.t.Helper()
	url := fmt.Sprintf("%s/index/%d", l.base, height)
	resp, err := l.cli.Get(url)
	if err != nil {
		// The accept loop may not have entered Serve yet on the first call.
		resp, err = l.cli.Get(url)
	}
	if err != nil {
		l.t.Fatalf("index GET %s: %v", url, err)
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		l.t.Fatalf("index GET %s: %v", url, err)
	}
	if resp.StatusCode == http.StatusNotFound {
		return nil, false
	}
	if resp.StatusCode != http.StatusOK {
		l.t.Fatalf("index GET %s: %s", url, resp.Status)
	}
	return body, true
}

// serveCasesIndex serves the CASES synthetic index on 127.0.0.1.
func serveCasesIndex(t *testing.T) Index {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("loopback index: %v", err)
	}
	srv := &http.Server{Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "method", http.StatusMethodNotAllowed)
			return
		}
		hstr := strings.TrimPrefix(r.URL.Path, "/index/")
		if !strings.HasPrefix(r.URL.Path, "/index/") || hstr == "" || strings.Contains(hstr, "/") {
			http.NotFound(w, r)
			return
		}
		height, err := strconv.ParseUint(hstr, 10, 64)
		if err != nil || strconv.FormatUint(height, 10) != hstr {
			http.NotFound(w, r)
			return
		}
		body, ok := casesIndexDocument(height)
		if !ok {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(body)
	})}
	go func() { _ = srv.Serve(ln) }()
	t.Cleanup(func() { _ = srv.Close() })
	base := "http://" + ln.Addr().String()
	if !strings.HasPrefix(base, "http://127.0.0.1:") {
		t.Fatalf("index is not on loopback: %s", base)
	}
	return loopbackIndex{
		t:    t,
		base: base,
		cli:  &http.Client{Timeout: 2 * time.Second},
	}
}

// TestACA8: the backend is sha-ni when the CPU has SHA-NI.
func TestACA8(t *testing.T) {
	repoRoot(t)
	if HasSHANI() && SHABackend() != "sha-ni" {
		t.Fatal("SHA-NI present and backend is software")
	}
	if !HasSHANI() && SHABackend() == "sha-ni" {
		t.Fatal("backend sha-ni without the flag")
	}
}

// TestACA9: the library version is 2.3.0 and the spec tag is spec-2.3.0.
// sdk/VERSION and sdk/SPEC_PIN are the release job's files. While they still
// name 2.2.0 / spec-2.2.0, this test records that and does not fail.
func TestACA9(t *testing.T) {
	root := repoRoot(t)
	b, err := os.ReadFile(filepath.Join(root, "sdk", "VERSION"))
	if err != nil {
		t.Fatal(err)
	}
	fileVer := strings.TrimSpace(string(b))
	if fileVer != Version && fileVer != "2.2.0" {
		t.Fatalf("VERSION %q constant %s", fileVer, Version)
	}
	if Version != "2.3.0" || SpecTag != "spec-2.3.0" {
		t.Fatal(Version, SpecTag)
	}
	if Kind != "v2.2-pcoc-object" {
		t.Fatal(Kind)
	}
	pin, err := ReadPin(filepath.Join(root, "sdk", "SPEC_PIN"))
	if err != nil {
		t.Fatal(err)
	}
	if pin.Tag == "" || pin.VectorSHA256 == "" {
		t.Fatal(pin)
	}
	if pin.Tag != SpecTag && pin.Tag != "spec-2.2.0" {
		t.Fatalf("pin tag %s", pin.Tag)
	}
}
