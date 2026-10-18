package hike

import (
	"strings"
	"testing"
)

func TestProduceReportUsesSection4Tokens(t *testing.T) {
	present := ProduceReport("out.cbor", true)
	absent := ProduceReport("", false)
	if present != "{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"present\",\"rung\":null,\"output\":\"out.cbor\"}\n" {
		t.Fatalf("present %s", present)
	}
	if absent != "{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"not read\",\"rung\":null}\n" {
		t.Fatalf("absent %s", absent)
	}
	for _, word := range []string{`"result":"pass"`, `"result":"fail"`} {
		if strings.Contains(present, word) || strings.Contains(absent, word) {
			t.Fatalf("old report token %s", word)
		}
	}
}

func TestPathAsksCutOffAtLastInterrupt(t *testing.T) {
	got := PathAskHeights(100, 102)
	if len(got) != 1 || got[0] != 102 {
		t.Fatalf("asks %v", got)
	}
	window := PathAskHeights(100, ^uint64(0))
	if len(window) != 3 || window[0] != 102 || window[1] != 103 || window[2] != 104 {
		t.Fatalf("window %v", window)
	}
	if len(got) == len(window) {
		t.Fatal("cut-off kept every ask")
	}
	if PathAskHeights(^uint64(0)-1, ^uint64(0)) != nil {
		t.Fatal("an offset that does not fit was asked")
	}
}

func TestPathAboveCutOffIsNotStoredAndMissContinues(t *testing.T) {
	waypoint := bytesRepeat(0x11, 32)
	body := EncodePathInBlock(waypoint, 104)
	if AcceptPath(body, waypoint, 100, 102) != nil {
		t.Fatal("stored a body above last_interrupt")
	}
	kept := AcceptPath(body, waypoint, 100, 104)
	if len(kept) == 0 {
		t.Fatal("dropped a body at the cut-off")
	}
	if AcceptPath([]byte{1, 2, 3}, waypoint, 100, 200) != nil || AcceptPath(nil, waypoint, 100, 200) != nil {
		t.Fatal("stored a miss")
	}
	pub := bytesRepeat(0x11, 32)
	tips := make([]Tip, Stages+1)
	for i := range tips {
		tips[i] = Tip{Natural: bytesRepeat(byte(i+1), 32), Height: uint64(1000 + i)}
	}
	object, stderr, err := Drive(pub, ConformanceSmallV2, tips)
	if err != nil || len(object) == 0 {
		t.Fatal("a path miss withheld the object")
	}
	if !strings.Contains(stderr, "path_skip 1\n") || !strings.HasSuffix(stderr, "not fetched by design 17\n") {
		t.Fatalf("stderr %q", stderr)
	}
}
