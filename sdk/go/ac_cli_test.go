package pcoc

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func pcocBin(t *testing.T) string {
	t.Helper()
	bin := filepath.Join(t.TempDir(), "pcoc")
	cmd := exec.Command("go", "build", "-o", bin, "./cmd/pcoc")
	cmd.Dir = sdkDir(t)
	cmd.Env = append(os.Environ(), "GOMAXPROCS=4")
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("build: %v\n%s", err, out)
	}
	return bin
}

func runCLI(t *testing.T, bin string, args ...string) (stdout, stderr string, code int) {
	t.Helper()
	cmd := exec.Command(bin, args...)
	var out, errb bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &errb
	err := cmd.Run()
	if err == nil {
		return out.String(), errb.String(), 0
	}
	if ee, ok := err.(*exec.ExitError); ok {
		return out.String(), errb.String(), ee.ExitCode()
	}
	t.Fatal(err)
	return "", "", -1
}

func TestACA7(t *testing.T) {
	bin := pcocBin(t)
	obj := filepath.Join(sdkDir(t), "objects", "O1-S1.cbor")
	body := mustRead(t, obj)
	sum := sha256Hex(body)
	if sum != objectPins["O1-S1.cbor"] {
		t.Fatalf("object O1-S1.cbor SHA-256 %s, pin %s", sum, objectPins["O1-S1.cbor"])
	}

	stdout, stderr, code := runCLI(t, bin, "verify", "--profile", "conformance-small", obj)
	if code != 2 {
		t.Fatalf("missing integers code %d", code)
	}
	if stdout != "" {
		t.Fatalf("stdout on usage %q", stdout)
	}
	_ = stderr

	empty := filepath.Join(t.TempDir(), "empty.cbor")
	os.WriteFile(empty, nil, 0o644)
	stdout, stderr, code = runCLI(t, bin, "verify", empty)
	if code != 0 {
		t.Fatalf("empty code %d stderr %s", code, stderr)
	}
	if !strings.Contains(stdout, `"shape_item":["malformed cbor"]`) {
		t.Fatalf("empty record %s", stdout)
	}
	if !strings.HasSuffix(stdout, "\n") {
		t.Fatal("no trailing newline")
	}
	forbidWords(t, stdout+stderr)

	// not a map: a well-formed unsigned integer. Decode failure, exit 1.
	notMap := filepath.Join(t.TempDir(), "int.cbor")
	os.WriteFile(notMap, []byte{0x01}, 0o644)
	stdout, _, code = runCLI(t, bin, "verify", notMap)
	if code != 1 || stdout != "" {
		t.Fatalf("non-map code %d stdout %q", code, stdout)
	}

	// truncated item: malformed record, exit 0.
	trunc := filepath.Join(t.TempDir(), "trunc.cbor")
	os.WriteFile(trunc, []byte{0x58, 0x05, 0x01}, 0o644)
	stdout, _, code = runCLI(t, bin, "verify", trunc)
	if code != 0 || !strings.Contains(stdout, `"malformed cbor"`) {
		t.Fatalf("trunc code %d %s", code, stdout)
	}

	_, _, code = runCLI(t, bin, "verify", filepath.Join(t.TempDir(), "missing.cbor"))
	if code != 1 {
		t.Fatalf("missing file %d", code)
	}

	_, _, code = runCLI(t, bin, "verify", "--profile", "nope", empty)
	if code != 2 {
		t.Fatalf("bad profile %d", code)
	}

	stdout, stderr, code = runCLI(t, bin, "verify", "--draw-label", "pcoc-pack-2.2/draw", "--profile", "conformance-small",
		"--sample-bytes", "32", "--slot-bytes", "64", "--terrain-bytes", "65536", "--steps-per-leg", "100", obj)
	if code != 0 {
		t.Fatalf("o1 code %d stderr %s", code, stderr)
	}
	if !strings.Contains(stderr, "draw: injected label (rig input)") {
		t.Fatalf("stderr %q", stderr)
	}
	forbidWords(t, stdout+stderr)
	var m map[string]any
	if err := json.Unmarshal([]byte(stdout), &m); err != nil {
		t.Fatal(err)
	}
	if m["shape"] != "match" {
		t.Fatalf("shape %v %v", m["shape"], m["shape_item"])
	}
	six := m["six_leg"].(map[string]any)
	for _, r := range six["results"].([]any) {
		if r != "match" {
			t.Fatalf("six %v", six["results"])
		}
	}
	// key order: profile first, analysis last.
	if !strings.HasPrefix(stdout, `{"profile":"conformance-small","sample_bytes":32,`) {
		t.Fatalf("prefix %s", stdout[:80])
	}
	if !strings.Contains(stdout, `"analysis":`) {
		t.Fatal("no analysis")
	}
}

func forbidWords(t *testing.T, s string) {
	t.Helper()
	low := strings.ToLower(s)
	for _, w := range []string{"pass", "fail", "accept", "refuse"} {
		if strings.Contains(low, w) {
			t.Fatalf("word %s in %s", w, s)
		}
	}
}
