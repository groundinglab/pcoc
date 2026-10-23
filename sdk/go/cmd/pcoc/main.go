// Command pcoc is the SDK command (SPEC §18, Rule I2).
//
// Subcommands: verify, selftest, candidates, produce.
// verify writes the section 14 record and nothing else on stdout.
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"

	"github.com/groundinglab/pcoc/sdk/go/v2"
	"github.com/groundinglab/pcoc/sdk/go/v2/produce"
)

func main() {
	if len(os.Args) < 2 {
		usage()
		os.Exit(2)
	}
	switch os.Args[1] {
	case "--version", "-version", "version":
		pin := readPin()
		fmt.Printf("%s\n%s\n%s\n", pcoc.Version, pcoc.SpecTag, pin.VectorSHA256)
	case "verify":
		os.Exit(cmdVerify(os.Args[2:]))
	case "selftest":
		fmt.Print(pcoc.SelftestText(readPin()))
	case "quickstart":
		os.Exit(cmdQuickstart())
	case "candidates":
		cmdCandidates(os.Args[2:])
	case "produce":
		os.Exit(cmdProduce(os.Args[2:]))
	case "-h", "--help", "help":
		usage()
	default:
		usage()
		os.Exit(2)
	}
}

func usage() {
	fmt.Fprintf(os.Stderr, "usage: pcoc verify|selftest|quickstart|candidates|produce|--version\n")
}

func cmdQuickstart() int {
	root := repoFromCwd()
	if root == "" {
		fmt.Fprintln(os.Stderr, "SPEC_PIN not found")
		return 1
	}
	pinText, err := os.ReadFile(filepath.Join(root, "sdk", "SPEC_PIN"))
	if err != nil {
		fmt.Fprintln(os.Stderr, "SPEC_PIN not read")
		return 1
	}
	manifest, err := os.ReadFile(filepath.Join(root, "spec", "conformance", "pack", "manifest.json"))
	if err != nil {
		fmt.Fprintln(os.Stderr, "manifest not read")
		return 1
	}
	sum := sha256.Sum256(manifest)
	vector := pinField(string(pinText), "vector-sha256")
	if hex.EncodeToString(sum[:]) != vector || pinField(string(pinText), "erratum") != "2" || len(vector) < 16 {
		fmt.Fprintln(os.Stderr, "pin mismatch")
		return 1
	}
	object, err := os.ReadFile(filepath.Join(root, "sdk", "go", "objects", "O1-S1.cbor"))
	if err != nil {
		fmt.Fprintln(os.Stderr, "O1-S1 not read")
		return 1
	}
	const o1 = "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246"
	objSum := sha256.Sum256(object)
	if hex.EncodeToString(objSum[:]) != o1 {
		fmt.Fprintln(os.Stderr, "O1-S1 sha256")
		return 1
	}
	profile := pcoc.Profile{Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 100}
	rec, decFail := pcoc.Verify(pcoc.VerifyInput{
		Profile: profile,
		Object:  object,
		Draw:    pcoc.ExpandDrawLabel("pcoc-pack-2.2/draw"),
	})
	if decFail || !measuredOK(string(rec)) {
		fmt.Fprintln(os.Stderr, "O1-S1 six_leg, checkpoints, or waypoints differ")
		return 1
	}
	neg, err := os.ReadFile(filepath.Join(root, "sdk", "go", "candidates", "negatives.txt"))
	if err != nil || strings.Count(string(neg), "matches_honest\tfalse") != 13 {
		fmt.Fprintln(os.Stderr, "negatives")
		return 1
	}
	prod, err := os.ReadFile(filepath.Join(root, "sdk", "go", "candidates", "prod.txt"))
	if err != nil || !strings.Contains(string(prod), "prod.PROD.stage.0.leg.0.legs_le_hex\t88a04ac5beacb8a7\n") {
		fmt.Fprintln(os.Stderr, "prod leg")
		return 1
	}
	fmt.Printf("pcoc %s (go) %s erratum 2 pack %s: pin match\n", pcoc.Version, pcoc.SpecTag, vector[:16])
	fmt.Printf("kernel scalar, sha256 %s\n", pcoc.SHABackend())
	fmt.Printf("O1-S1 object sha256 %s\n", o1)
	fmt.Println("O1-S1 six_leg match, checkpoints match, waypoints match")
	fmt.Println("negatives 13 non-match")
	fmt.Println("PROD leg 0 legs_le_hex 88a04ac5beacb8a7")
	return 0
}

func repoFromCwd() string {
	dir, err := os.Getwd()
	if err != nil {
		return ""
	}
	for i := 0; i < 8; i++ {
		if _, err := os.Stat(filepath.Join(dir, "sdk", "SPEC_PIN")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return ""
}

func pinField(text, key string) string {
	prefix := key + "="
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, prefix) {
			return strings.TrimPrefix(line, prefix)
		}
	}
	return ""
}

func between(text, start, end string) (string, bool) {
	i := strings.Index(text, start)
	if i < 0 {
		return "", false
	}
	rest := text[i+len(start):]
	j := strings.Index(rest, end)
	if j < 0 {
		return "", false
	}
	return rest[:j], true
}

func resultsMatch(body string) bool {
	n := 0
	rest := body
	const mark = `"result":"`
	for {
		i := strings.Index(rest, mark)
		if i < 0 {
			break
		}
		v := rest[i+len(mark):]
		if !strings.HasPrefix(v, `match"`) {
			return false
		}
		n++
		rest = v[len(`match"`):]
	}
	return n > 0
}

func measuredOK(record string) bool {
	six, ok := between(record, `"six_leg":`, `,"six_leg_runs":`)
	if !ok || strings.Contains(six, "mismatch") || strings.Count(six, `"match"`) != 6 {
		return false
	}
	checkpoints, ok := between(record, `"checkpoints":`, `,"waypoints":`)
	if !ok || !resultsMatch(checkpoints) {
		return false
	}
	waypoints, ok := between(record, `"waypoints":`, `,"paths":`)
	return ok && resultsMatch(waypoints)
}

func cmdVerify(args []string) int {
	var profile string
	var file string
	var drawLabel string
	var sample, slot, terrain, steps int
	var gotSample, gotSlot, gotTerrain, gotSteps bool
	for i := 0; i < len(args); i++ {
		a := args[i]
		switch {
		case a == "--profile":
			if i+1 >= len(args) {
				return 2
			}
			i++
			profile = args[i]
		case strings.HasPrefix(a, "--profile="):
			profile = a[len("--profile="):]
		case a == "--draw-label":
			if i+1 >= len(args) {
				return 2
			}
			i++
			drawLabel = args[i]
		case strings.HasPrefix(a, "--draw-label="):
			drawLabel = a[len("--draw-label="):]
		case a == "--sample-bytes":
			if !takeInt(args, &i, &sample) {
				return 2
			}
			gotSample = true
		case a == "--slot-bytes":
			if !takeInt(args, &i, &slot) {
				return 2
			}
			gotSlot = true
		case a == "--terrain-bytes":
			if !takeInt(args, &i, &terrain) {
				return 2
			}
			gotTerrain = true
		case a == "--steps-per-leg":
			if !takeInt(args, &i, &steps) {
				return 2
			}
			gotSteps = true
		case a == "--":
			if i+1 >= len(args) || file != "" {
				return 2
			}
			file = args[i+1]
			i++
		case strings.HasPrefix(a, "-"):
			return 2
		default:
			if file != "" {
				return 2
			}
			file = a
		}
	}
	if file == "" {
		return 2
	}
	p, code := resolveProfile(profile, gotSample, sample, gotSlot, slot, gotTerrain, terrain, gotSteps, steps)
	if code != 0 {
		return code
	}
	body, err := os.ReadFile(file)
	if err != nil {
		return 1
	}
	var draw pcoc.ByteSource
	if drawLabel != "" {
		fmt.Fprintf(os.Stderr, "draw: injected label (rig input)\n")
		draw = pcoc.ExpandDrawLabel(drawLabel)
	}
	rec, decFail := pcoc.Verify(pcoc.VerifyInput{Profile: p, Object: body, Draw: draw})
	if decFail {
		return 1
	}
	os.Stdout.Write(rec)
	os.Stdout.Write([]byte{'\n'})
	return 0
}

func takeInt(args []string, i *int, dst *int) bool {
	if *i+1 >= len(args) {
		return false
	}
	*i++
	n, err := strconv.Atoi(args[*i])
	if err != nil || n < 0 {
		return false
	}
	*dst = n
	return true
}

func resolveProfile(name string, gotS bool, sample int, gotSl bool, slot int, gotT bool, terrain int, gotSt bool, steps int) (pcoc.Profile, int) {
	switch name {
	case "", "production", "test", "live":
		if gotS || gotSl || gotT || gotSt {
			return pcoc.Profile{}, 2
		}
		return pcoc.Production(), 0
	case "conformance-small":
		if !gotS || !gotSl || !gotT || !gotSt {
			return pcoc.Profile{}, 2
		}
		p := pcoc.Profile{Name: "conformance-small", SlotBytes: slot, SampleBytes: sample, TerrainBytes: terrain, StepsPerLeg: steps}
		if err := p.Check(); err != nil {
			return pcoc.Profile{}, 2
		}
		return p, 0
	default:
		return pcoc.Profile{}, 2
	}
}

func cmdProduce(args []string) int {
	var rig bool
	var profile string
	var pkHex string
	var out string
	var legs = 1
	for i := 0; i < len(args); i++ {
		a := args[i]
		switch {
		case a == "--rig":
			rig = true
		case a == "--profile":
			if i+1 >= len(args) {
				return 2
			}
			i++
			profile = args[i]
		case a == "--public-key":
			if i+1 >= len(args) {
				return 2
			}
			i++
			pkHex = args[i]
		case a == "--out":
			if i+1 >= len(args) {
				return 2
			}
			i++
			out = args[i]
		case a == "--legs":
			if !takeInt(args, &i, &legs) {
				return 2
			}
		default:
			return 2
		}
	}
	if !rig || pkHex == "" || out == "" {
		return 2
	}
	pk, err := decodeHex(pkHex)
	if err != nil || len(pk) != 32 {
		return 2
	}
	var p pcoc.Profile
	switch profile {
	case "", "production":
		p = pcoc.Production()
	case "S1":
		p = pcoc.Profile{Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 100}
	case "S2":
		p = pcoc.Profile{Name: "conformance-small", SlotBytes: 256, SampleBytes: 64, TerrainBytes: 1048576, StepsPerLeg: 1000}
	default:
		return 2
	}
	n := make([]int, 18)
	for i := range n {
		n[i] = legs
	}
	body, err := produce.Drive(p, pk, produce.CASESBeacons(), n)
	if err != nil {
		return 1
	}
	if err := os.WriteFile(out, body, 0o644); err != nil {
		return 1
	}
	io.WriteString(os.Stderr, produce.StderrClose())
	return 0
}

// cmdCandidates runs the generator as its own program. This binary does not
// import package negatives (SPEC §16, White Hat Q7).
func cmdCandidates(args []string) {
	dir := "candidates"
	if len(args) > 0 {
		dir = args[0]
	}
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		os.Exit(1)
	}
	sdk := filepath.Join(filepath.Dir(file), "..", "..")
	cmd := exec.Command("go", "run", "./gen", dir)
	cmd.Dir = sdk
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		os.Exit(1)
	}
}

func readPin() pcoc.Pin {
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		return pcoc.Pin{}
	}
	root, ok := pcoc.RepoRoot(filepath.Dir(file))
	if !ok {
		return pcoc.Pin{}
	}
	p, err := pcoc.ReadPin(filepath.Join(root, "sdk", "SPEC_PIN"))
	if err != nil {
		return pcoc.Pin{}
	}
	return p
}

func decodeHex(s string) ([]byte, error) {
	s = strings.TrimPrefix(s, "0x")
	if len(s)%2 != 0 {
		return nil, fmt.Errorf("hex")
	}
	out := make([]byte, len(s)/2)
	for i := 0; i < len(out); i++ {
		hi, ok1 := hexVal(s[2*i])
		lo, ok2 := hexVal(s[2*i+1])
		if !ok1 || !ok2 {
			return nil, fmt.Errorf("hex")
		}
		out[i] = hi<<4 | lo
	}
	return out, nil
}

func hexVal(c byte) (byte, bool) {
	switch {
	case c >= '0' && c <= '9':
		return c - '0', true
	case c >= 'a' && c <= 'f':
		return c - 'a' + 10, true
	case c >= 'A' && c <= 'F':
		return c - 'A' + 10, true
	default:
		return 0, false
	}
}
