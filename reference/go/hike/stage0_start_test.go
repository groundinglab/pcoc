package hike

import (
	"encoding/hex"
	"encoding/json"
	"os"
	"testing"
)

// P9.14. The first tip is already known. The same header after the first leg
// does not replace the seed, and the next header is I_0.
func TestStage0StartsOnTheLatestKnownHeader(t *testing.T) {
	raw, err := os.ReadFile("../../../conformance/v2.0/stage0-start.json")
	if err != nil {
		t.Fatal(err)
	}
	var doc struct {
		Rule         string `json:"rule"`
		Profile      string `json:"profile"`
		PublicKeyHex string `json:"public_key_hex"`
		SeedHeight   uint64 `json:"seed_height"`
		Stage0Legs   uint64 `json:"stage0_legs"`
		Tips         []struct {
			Height     uint64 `json:"height"`
			NaturalHex string `json:"natural_hex"`
		} `json:"tips"`
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatal(err)
	}
	if doc.Rule != "P9.14" || doc.Profile != "conformance-small" {
		t.Fatalf("vector %s %s", doc.Rule, doc.Profile)
	}
	pub, err := hex.DecodeString(doc.PublicKeyHex)
	if err != nil || len(pub) != 32 {
		t.Fatal("public key")
	}
	tips := make([]Tip, len(doc.Tips))
	for i, tip := range doc.Tips {
		natural, err := hex.DecodeString(tip.NaturalHex)
		if err != nil || len(natural) != 32 {
			t.Fatalf("tip %d", i)
		}
		tips[i] = Tip{Natural: natural, Height: tip.Height}
	}
	if len(tips) < 3 || string(tips[0].Natural) != string(tips[1].Natural) || tips[0].Height != doc.SeedHeight {
		t.Fatal("vector tips")
	}
	object, _, err := Drive(pub, ConformanceSmallV2, tips)
	if err != nil {
		t.Fatal(err)
	}
	item, err := decodeItem(object)
	if err != nil {
		t.Fatal(err)
	}
	parsed, ok := parseV2(item, ConformanceSmallV2)
	if !ok || !parsed.shape {
		t.Fatal("shape")
	}
	if parsed.seedBeacon != *(*[32]byte)(tips[0].Natural) {
		t.Fatal("seed beacon")
	}
	if parsed.stages[0].beacon != parsed.seedBeacon || parsed.stages[0].beaconHeight != doc.SeedHeight {
		t.Fatalf("stage 0 beacon %d", parsed.stages[0].beaconHeight)
	}
	if parsed.stages[0].legCount != doc.Stage0Legs {
		t.Fatalf("stage 0 legs %d", parsed.stages[0].legCount)
	}
	if parsed.stages[1].beacon != *(*[32]byte)(tips[2].Natural) || parsed.stages[1].beaconHeight != tips[2].Height {
		t.Fatal("I_0")
	}
}
