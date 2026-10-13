// Command satoshi-hike-produce writes one hike object.
//
// The locked pack's produce is the deterministic tip sequence. Pass --tips
// with a drive document to run that clock. Without it, this port does not
// Observe gossip and does not write an object.
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"strings"

	"github.com/groundinglab/pcoc/reference/go/hike"
)

func main() {
	os.Exit(run(os.Args[1:]))
}

func run(args []string) int {
	if hasFlag(args, "demo-mnemonic") {
		fmt.Fprintf(os.Stderr, "satoshi-hike-produce: demo mnemonic is outside this port\n")
		usage()
		return 2
	}
	parsed, ok := parse(args)
	if !ok {
		usage()
		return 2
	}
	profile, ok := measureProfile(parsed.profile)
	if !ok {
		usage()
		return 2
	}
	if parsed.tips == "" {
		fmt.Fprintf(os.Stderr, "satoshi-hike-produce: observe unavailable\n")
		writeProduceReport(parsed.report, hike.ProduceReport(parsed.output, false))
		return 1
	}
	tips, err := readTips(parsed.tips)
	if err != nil {
		fmt.Fprintf(os.Stderr, "satoshi-hike-produce: observe unavailable\n")
		writeProduceReport(parsed.report, hike.ProduceReport(parsed.output, false))
		return 1
	}
	object, stderr, err := hike.Drive(parsed.publicKey, profile, tips)
	if err != nil {
		fmt.Fprintf(os.Stderr, "satoshi-hike-produce: observe unavailable\n")
		writeProduceReport(parsed.report, hike.ProduceReport(parsed.output, false))
		return 1
	}
	fmt.Fprint(os.Stderr, stderr)
	if err := os.WriteFile(parsed.output, object, 0o644); err != nil {
		fmt.Fprintf(os.Stderr, "satoshi-hike-produce: output path\n")
		writeProduceReport(parsed.report, hike.ProduceReport(parsed.output, false))
		return 1
	}
	report := hike.ProduceReport(parsed.output, true)
	fmt.Fprint(os.Stdout, report)
	if parsed.report != "" {
		if err := os.WriteFile(parsed.report, []byte(report), 0o644); err != nil {
			fmt.Fprintf(os.Stderr, "satoshi-hike-produce: report path\n")
			return 1
		}
	}
	return 0
}

func writeProduceReport(path, report string) {
	fmt.Fprint(os.Stdout, report)
	if path == "" {
		return
	}
	_ = os.WriteFile(path, []byte(report), 0o644)
}

type parsedArgs struct {
	publicKey []byte
	output    string
	profile   string
	tips      string
	peer      string
	bari      string
	report    string
}

func parse(args []string) (parsedArgs, bool) {
	var out parsedArgs
	out.profile = "test"
	for i := 0; i < len(args); i++ {
		name, value, inline := splitFlag(args[i])
		if name == "" {
			return parsedArgs{}, false
		}
		need := func() (string, bool) {
			if inline {
				return value, value != ""
			}
			i++
			if i >= len(args) || strings.HasPrefix(args[i], "-") {
				return "", false
			}
			return args[i], true
		}
		switch name {
		case "pubkey-hex":
			v, ok := need()
			if !ok || len(v) != 64 {
				return parsedArgs{}, false
			}
			raw, err := hex.DecodeString(v)
			if err != nil || len(raw) != 32 {
				return parsedArgs{}, false
			}
			out.publicKey = raw
		case "output":
			v, ok := need()
			if !ok {
				return parsedArgs{}, false
			}
			out.output = v
		case "profile":
			v, ok := need()
			if !ok {
				return parsedArgs{}, false
			}
			out.profile = v
		case "tips":
			v, ok := need()
			if !ok {
				return parsedArgs{}, false
			}
			out.tips = v
		case "gossip-peer", "bari-url", "report", "gossip-network", "clock-source":
			v, ok := need()
			if !ok {
				return parsedArgs{}, false
			}
			if name == "gossip-peer" {
				out.peer = v
			}
			if name == "report" {
				out.report = v
			}
			if name == "bari-url" {
				if !httpURL(v) {
					return parsedArgs{}, false
				}
				out.bari = v
			}
			if name == "clock-source" && v != "gossip" {
				return parsedArgs{}, false
			}
		case "tip-url":
			return parsedArgs{}, false
		default:
			return parsedArgs{}, false
		}
	}
	if out.publicKey == nil || out.output == "" {
		return parsedArgs{}, false
	}
	// conformance-small is a verifier input. A --tips rig does not select it.
	if out.profile == "conformance-small" {
		return parsedArgs{}, false
	}
	if out.tips != "" {
		return out, true
	}
	if out.profile == "live" || out.profile == "production" {
		return out, true
	}
	if out.profile != "test" && out.profile != "slow-lan" {
		return parsedArgs{}, false
	}
	if out.peer == "" {
		out.peer = os.Getenv("PCOC_TEST_GOSSIP_PEER")
	}
	if out.bari == "" {
		out.bari = os.Getenv("PCOC_TEST_BARI_URL")
	}
	if out.peer == "" || out.bari == "" {
		return parsedArgs{}, false
	}
	if out.profile == "test" && livePublicZone(out.bari) {
		return parsedArgs{}, false
	}
	return out, true
}

// livePublicZone reports the public zone. A test profile must not dial it.
func livePublicZone(raw string) bool {
	parsed, err := url.Parse(raw)
	if err != nil {
		return false
	}
	host := strings.ToLower(parsed.Hostname())
	if host == "" {
		return false
	}
	return host == "groundinglab.io" || strings.HasSuffix(host, ".groundinglab.io")
}

func measureProfile(name string) (hike.Profile, bool) {
	switch name {
	case "", "test", "live", "production", "slow-lan":
		return hike.Production, true
	default:
		// conformance-small is a verifier profile (SPEC §16.1).
		return hike.Profile{}, false
	}
}

func readTips(path string) ([]hike.Tip, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var doc struct {
		TipSequence []struct {
			Height     uint64 `json:"height"`
			NaturalHex string `json:"natural_hex"`
		} `json:"tip_sequence"`
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		return nil, err
	}
	tips := make([]hike.Tip, 0, len(doc.TipSequence))
	for _, row := range doc.TipSequence {
		natural, err := hex.DecodeString(row.NaturalHex)
		if err != nil || len(natural) != 32 {
			return nil, fmt.Errorf("tip")
		}
		tips = append(tips, hike.Tip{Natural: natural, Height: row.Height})
	}
	if len(tips) == 0 {
		return nil, fmt.Errorf("tip")
	}
	return tips, nil
}

func hasFlag(args []string, name string) bool {
	prefix := "--" + name
	for _, arg := range args {
		if arg == prefix || strings.HasPrefix(arg, prefix+"=") {
			return true
		}
	}
	return false
}

func splitFlag(arg string) (name, value string, inline bool) {
	if !strings.HasPrefix(arg, "--") || len(arg) < 3 {
		return "", "", false
	}
	body := arg[2:]
	if i := strings.IndexByte(body, '='); i >= 0 {
		return body[:i], body[i+1:], true
	}
	return body, "", false
}

func httpURL(raw string) bool {
	return strings.HasPrefix(raw, "http://") || strings.HasPrefix(raw, "https://")
}

func usage() {
	fmt.Fprintf(os.Stderr, "usage: satoshi-hike-produce --pubkey-hex HEX --output PATH --gossip-peer HOST:PORT --bari-url URL [--profile test|live|production] [--tips PATH]\n")
}
