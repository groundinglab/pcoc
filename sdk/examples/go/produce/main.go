// Produce a hike object offline from the stage0-start.json tip script at
// conformance-small sizes, then verify it.
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/groundinglab/pcoc/sdk/go/v2/hike"
)

func main() {
	pack := os.Args[1] // spec/conformance/v2.0
	raw, err := os.ReadFile(filepath.Join(pack, "stage0-start.json"))
	if err != nil {
		panic(err)
	}
	var v struct {
		PublicKeyHex string `json:"public_key_hex"`
		Tips         []struct {
			Height     uint64 `json:"height"`
			NaturalHex string `json:"natural_hex"`
		} `json:"tips"`
	}
	if err := json.Unmarshal(raw, &v); err != nil {
		panic(err)
	}
	publicKey, _ := hex.DecodeString(v.PublicKeyHex)
	// The tip sequence. Live produce reads it from Bitcoin header gossip;
	// each new header ends a stage.
	var tips []hike.Tip
	for _, t := range v.Tips {
		natural, _ := hex.DecodeString(t.NaturalHex)
		tips = append(tips, hike.Tip{Natural: natural, Height: t.Height})
	}

	// Drive has no plant service: every path is empty bytes.
	object, _, err := hike.Drive(publicKey, hike.ConformanceSmallV2, tips) // hike.Production for a real PCOC
	if err != nil {
		panic(err) // "public key", "seed observe" or "observe"
	}
	fmt.Println("object bytes:", len(object))
	fmt.Printf("object sha256: %x\n", sha256.Sum256(object))

	// Verify what was produced. There is no index here, so header reads are "not read".
	record, _ := hike.Verify(object, hike.ConformanceSmallV2, hike.Index{}, hike.AuditStream(make([]byte, 32), 4096), false)
	fmt.Println("shape:", record.Shape)
}
