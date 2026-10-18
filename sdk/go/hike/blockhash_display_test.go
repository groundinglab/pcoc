package hike

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

// SPEC §9. P_g, P_b, and claimed_block block_hash are Bitcoin display order.
// The pinned header is Bitcoin mainnet genesis. Inclusion height 5 puts the
// path in the quiet object's anchor window. That height is not the Bitcoin
// height. stored_hash stays byte 0 first.
func TestBlockHashDisplayOrder(t *testing.T) {
	header, naturalHex, displayHexWant := genesisPin(t)
	sum := sha256.Sum256(header)
	natural := sha256.Sum256(sum[:])
	if hex.EncodeToString(natural[:]) != naturalHex {
		t.Fatalf("natural %x", natural)
	}
	var reversed [32]byte
	for i := range natural {
		reversed[i] = natural[31-i]
	}
	if hex.EncodeToString(reversed[:]) != displayHexWant || naturalHex == displayHexWant {
		t.Fatalf("display pin %s", displayHexWant)
	}

	object := QuietV2()
	copy(object.Stages[4].Beacon[:], natural[:])
	object.Stages[1].Path = encodePathForBlock(object.Stages[1].Waypoint[:], natural[:], 5)
	object.Stages[2].Path = encodePathForBlock(object.Stages[2].Waypoint[:], natural[:], 5)
	got := PackRecord(object.Encode(), Production, bookendIndex(object), func(PathLocators) error { return nil }, 1, nil)

	var rec struct {
		Headers []struct {
			StoredHash string `json:"stored_hash"`
		} `json:"headers"`
		Paths []struct {
			Plant   string          `json:"plant"`
			PG      json.RawMessage `json:"P_g"`
			Claimed json.RawMessage `json:"claimed_block"`
		} `json:"paths"`
		Anchors []string        `json:"anchors"`
		PB      json.RawMessage `json:"P_b"`
		Cmp     string          `json:"index_comparison_P_b"`
	}
	if err := json.Unmarshal([]byte(got), &rec); err != nil {
		t.Fatal(err)
	}
	if rec.Headers[4].StoredHash != naturalHex {
		t.Fatalf("stored_hash %s", rec.Headers[4].StoredHash)
	}
	if rec.Cmp != "match" || rec.Anchors[0] != "anchored" || rec.Anchors[1] != "anchored" {
		t.Fatalf("comparison %s anchors %v", rec.Cmp, rec.Anchors[:2])
	}
	for _, index := range []int{0, 1} {
		if rec.Paths[index].Plant != "match" {
			t.Fatalf("plant %s", rec.Paths[index].Plant)
		}
		requireDisplay(t, rec.Paths[index].PG, displayHexWant, naturalHex)
		requireDisplay(t, rec.Paths[index].Claimed, displayHexWant, naturalHex)
	}
	requireDisplay(t, rec.PB, displayHexWant, naturalHex)
}

func requireDisplay(t *testing.T, raw json.RawMessage, display, natural string) {
	t.Helper()
	var ref struct {
		Hash   string `json:"block_hash"`
		Height uint64 `json:"block_height"`
	}
	if err := json.Unmarshal(raw, &ref); err != nil {
		t.Fatalf("block %s: %v", raw, err)
	}
	if ref.Hash != display || ref.Hash == natural || ref.Height != 5 {
		t.Fatalf("block_hash %s height %d", ref.Hash, ref.Height)
	}
}

func genesisPin(t *testing.T) (header []byte, natural, display string) {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("caller")
	}
	raw, err := os.ReadFile(filepath.Join(filepath.Dir(file), "../../../spec/conformance/blockhash-display.json"))
	if err != nil {
		t.Fatal(err)
	}
	var pin struct {
		Header  string `json:"header_hex"`
		Natural string `json:"natural_block_hash"`
		Display string `json:"display_block_hash"`
	}
	if err := json.Unmarshal(raw, &pin); err != nil {
		t.Fatal(err)
	}
	header, err = hex.DecodeString(pin.Header)
	if err != nil || len(header) != 80 || len(pin.Natural) != 64 || len(pin.Display) != 64 {
		t.Fatalf("pin header %d", len(header))
	}
	return header, pin.Natural, pin.Display
}
