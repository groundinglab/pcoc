// Conformance driver for the Go verifier.
// The verify CLI does not take an index (sdk/go/cmd/pcoc/main.go). This
// program calls the same pcoc.Verify, and reads GET http://127.0.0.1/index/{height}.
package main

import (
	"context"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"

	pcoc "github.com/groundinglab/pcoc/sdk/go/v2"
)

func main() {
	os.Exit(run(os.Args[1:]))
}

func run(args []string) int {
	var indexURL, profile, file, drawLabel string
	var sample, slot, terrain, steps int
	var gotSample, gotSlot, gotTerrain, gotSteps bool
	for i := 0; i < len(args); i++ {
		a := args[i]
		switch {
		case a == "--index-url" && i+1 < len(args):
			i++
			indexURL = args[i]
		case a == "--profile" && i+1 < len(args):
			i++
			profile = args[i]
		case a == "--draw-label" && i+1 < len(args):
			i++
			drawLabel = args[i]
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
		case strings.HasPrefix(a, "-"):
			usage()
			return 2
		default:
			if file != "" {
				usage()
				return 2
			}
			file = a
		}
	}
	if file == "" || indexURL == "" || !loopback(indexURL) {
		usage()
		return 2
	}
	p, code := resolve(profile, gotSample, sample, gotSlot, slot, gotTerrain, terrain, gotSteps, steps)
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
	idx := newHTTPIndex(indexURL)
	rec, decFail := pcoc.Verify(pcoc.VerifyInput{Profile: p, Object: body, Index: idx, Draw: draw})
	if decFail {
		fmt.Fprintf(os.Stderr, "section 7.3: no record\n")
		return 1
	}
	os.Stdout.Write(rec)
	os.Stdout.Write([]byte{'\n'})
	return 0
}

func loopback(raw string) bool {
	const p = "http://127.0.0.1:"
	if !strings.HasPrefix(raw, p) {
		return false
	}
	port := raw[len(p):]
	n, err := strconv.Atoi(port)
	return err == nil && n > 0 && n < 65536 && !strings.Contains(port, "/")
}

func usage() {
	fmt.Fprintf(os.Stderr, "usage: xverify-go --index-url http://127.0.0.1:PORT [--profile production|conformance-small] [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N] [--draw-label LABEL] OBJECT\n")
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

func resolve(name string, gotS bool, sample int, gotSl bool, slot int, gotT bool, terrain int, gotSt bool, steps int) (pcoc.Profile, int) {
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

type httpIndex struct {
	base   string
	client *http.Client
	hit    map[uint64][]byte
	miss   map[uint64]bool
}

func newHTTPIndex(base string) *httpIndex {
	dialer := &net.Dialer{Timeout: 5 * time.Second}
	tr := &http.Transport{
		DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
			host, _, err := net.SplitHostPort(addr)
			if err != nil || host != "127.0.0.1" {
				return nil, fmt.Errorf("index host")
			}
			return dialer.DialContext(ctx, network, addr)
		},
	}
	return &httpIndex{
		base: base,
		client: &http.Client{
			Transport: tr,
			Timeout:   10 * time.Second,
			CheckRedirect: func(req *http.Request, via []*http.Request) error {
				return http.ErrUseLastResponse
			},
		},
		hit:  map[uint64][]byte{},
		miss: map[uint64]bool{},
	}
}

func (h *httpIndex) Get(height uint64) ([]byte, bool) {
	if b, ok := h.hit[height]; ok {
		return append([]byte(nil), b...), true
	}
	if h.miss[height] {
		return nil, false
	}
	resp, err := h.client.Get(fmt.Sprintf("%s/index/%d", h.base, height))
	if err != nil {
		fmt.Fprintf(os.Stderr, "index read failed for height %d\n", height)
		h.miss[height] = true
		return nil, false
	}
	defer resp.Body.Close()
	if resp.StatusCode != 200 {
		io.Copy(io.Discard, resp.Body)
		h.miss[height] = true
		return nil, false
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if err != nil {
		h.miss[height] = true
		return nil, false
	}
	h.hit[height] = body
	return append([]byte(nil), body...), true
}
