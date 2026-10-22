package pcoc

// SPEC pin (SPEC §16.2 Rule I5).
//
// sdk/SPEC_PIN names the tag of this file and the SHA-256 of the pack
// manifest. The SDK reads that file. It does not embed a second pin.

import (
	"bufio"
	"os"
	"path/filepath"
	"strings"
)

// Pin is one reading of sdk/SPEC_PIN.
type Pin struct {
	Tag          string
	VectorSHA256 string
}

// ReadPin reads sdk/SPEC_PIN at path.
func ReadPin(path string) (Pin, error) {
	f, err := os.Open(path)
	if err != nil {
		return Pin{}, err
	}
	defer f.Close()
	var p Pin
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		k, v, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		switch strings.TrimSpace(k) {
		case "tag":
			p.Tag = strings.TrimSpace(v)
		case "vector-sha256":
			p.VectorSHA256 = strings.TrimSpace(v)
		}
	}
	return p, sc.Err()
}

// RepoRoot walks up from dir until it finds sdk/SPEC_PIN beside spec/.
func RepoRoot(dir string) (string, bool) {
	for i := 0; i < 8; i++ {
		if _, err := os.Stat(filepath.Join(dir, "sdk", "SPEC_PIN")); err == nil {
			return dir, true
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return "", false
}
