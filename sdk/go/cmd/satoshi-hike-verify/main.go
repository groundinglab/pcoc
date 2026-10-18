// Command satoshi-hike-verify measures one hike object.
//
// Exit 0: the object decoded and a section 14 record was written.
// Exit 1: the object file cannot be read or decoded.
// Exit 2: usage.
// Standard streams do not carry a policy verdict.
package main

import (
	"crypto/rand"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"strings"
	"time"

	"github.com/groundinglab/pcoc/sdk/go/v2/hike"
)

func main() {
	os.Exit(run(os.Args[1:]))
}

func run(args []string) int {
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
	indexURL, ok := resolveIndex(parsed)
	if !ok {
		usage()
		return 2
	}
	if parsed.chainRPC != "" && parsed.profile != "live" {
		usage()
		return 2
	}
	if parsed.profile == "live" && parsed.chainRPC != "" && !httpURL(parsed.chainRPC) {
		usage()
		return 2
	}
	object, err := os.ReadFile(parsed.input)
	if err != nil {
		fmt.Fprintf(os.Stderr, "satoshi-hike-verify: %s\n", err.Error())
		return 1
	}
	var stream []byte
	if parsed.audit != nil {
		stream = hike.AuditStream(parsed.audit, 4096)
	} else {
		stream = make([]byte, 4096)
		if _, err := rand.Read(stream); err != nil {
			stream = nil
		}
	}
	index := hike.Index{Fetch: fetchIndex(indexURL)}
	record, decoded := hike.Verify(object, profile, index, stream, false)
	if !decoded {
		fmt.Fprintf(os.Stderr, "satoshi-hike-verify: undecodable\n")
		return 1
	}
	body := record.Marshal()
	if _, err := os.Stdout.Write(body); err != nil {
		fmt.Fprintf(os.Stderr, "satoshi-hike-verify: stdout\n")
		return 1
	}
	if parsed.log == "verbose" {
		_, _ = os.Stderr.Write(body)
	}
	if parsed.report != "" {
		if err := os.WriteFile(parsed.report, body, 0o644); err != nil {
			fmt.Fprintf(os.Stderr, "satoshi-hike-verify: report path\n")
			return 1
		}
	}
	return 0
}

type parsedArgs struct {
	input    string
	indexURL string
	profile  string
	audit    []byte
	report   string
	chainRPC string
	log      string
}

func parse(args []string) (parsedArgs, bool) {
	var out parsedArgs
	out.log = "quiet"
	out.profile = "test"
	for i := 0; i < len(args); i++ {
		arg := args[i]
		name, value, inline := splitFlag(arg)
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
		case "input":
			v, ok := need()
			if !ok {
				return parsedArgs{}, false
			}
			out.input = v
		case "index-url":
			v, ok := need()
			if !ok || !httpURL(v) {
				return parsedArgs{}, false
			}
			out.indexURL = v
		case "profile":
			v, ok := need()
			if !ok {
				return parsedArgs{}, false
			}
			out.profile = v
		case "audit-seed":
			v, ok := need()
			if !ok || len(v) != 64 {
				return parsedArgs{}, false
			}
			raw, err := decodeHex(v)
			if err != nil || len(raw) != 32 {
				return parsedArgs{}, false
			}
			out.audit = raw
		case "report":
			v, ok := need()
			if !ok {
				return parsedArgs{}, false
			}
			out.report = v
		case "chain-rpc":
			v, ok := need()
			if !ok {
				return parsedArgs{}, false
			}
			out.chainRPC = v
		case "log":
			v, ok := need()
			if !ok || (v != "quiet" && v != "verbose") {
				return parsedArgs{}, false
			}
			out.log = v
		default:
			return parsedArgs{}, false
		}
	}
	if out.input == "" {
		return parsedArgs{}, false
	}
	return out, true
}

func measureProfile(name string) (hike.Profile, bool) {
	switch name {
	case "", "test", "live", "production":
		return hike.Production, true
	case "conformance-small":
		return hike.ConformanceSmallV2, true
	default:
		return hike.Profile{}, false
	}
}

func resolveIndex(parsed parsedArgs) (string, bool) {
	if parsed.indexURL != "" {
		if parsed.profile != "live" && livePublicZone(parsed.indexURL) {
			return "", false
		}
		return parsed.indexURL, true
	}
	if parsed.profile == "live" {
		return "https://index.groundinglab.io", true
	}
	env := os.Getenv("PCOC_TEST_INDEX_URL")
	if env == "" || !httpURL(env) || livePublicZone(env) {
		return "", false
	}
	return env, true
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

func httpURL(raw string) bool {
	if strings.HasSuffix(raw, "/") {
		return false
	}
	parsed, err := url.Parse(raw)
	if err != nil || parsed.Host == "" {
		return false
	}
	scheme := strings.ToLower(parsed.Scheme)
	return scheme == "http" || scheme == "https"
}

func fetchIndex(base string) func(uint64) (string, bool) {
	client := &http.Client{Timeout: 2 * time.Second}
	return func(height uint64) (string, bool) {
		endpoint := fmt.Sprintf("%s/index/%d", base, height)
		resp, err := client.Get(endpoint)
		if err != nil {
			return "", false
		}
		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			return "", false
		}
		body, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
		if err != nil {
			return "", false
		}
		return string(body), true
	}
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

func decodeHex(s string) ([]byte, error) {
	if len(s)%2 != 0 {
		return nil, fmt.Errorf("hex")
	}
	out := make([]byte, len(s)/2)
	for i := 0; i < len(out); i++ {
		n, err := fmt.Sscanf(s[i*2:i*2+2], "%02x", &out[i])
		if err != nil || n != 1 {
			return nil, fmt.Errorf("hex")
		}
	}
	return out, nil
}

func usage() {
	fmt.Fprintf(os.Stderr, "usage: satoshi-hike-verify --input PATH --index-url URL [--audit-seed HEX] [--report PATH] [--profile production|conformance-small|test|live] [--chain-rpc URL] [--log quiet|verbose]\n       default profile is test. Omitted --index-url is PCOC_TEST_INDEX_URL. Unset is a usage error. This tree does not fill that host. It does not fill https://index.groundinglab.io.\n       --profile live fills that index. --chain-rpc is live-only.\n       --log defaults to quiet. verbose writes the measurement record to stderr. stdout is the measurement record when the object decodes.\n")
}
