// Verify the kat-small conformance object against its recorded index replies.
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
	raw, err := os.ReadFile(filepath.Join(pack, "kat-small.json"))
	if err != nil {
		panic(err)
	}
	var kat struct {
		ObjectHex string `json:"object_hex"`
		Documents []struct {
			Height uint64 `json:"height"`
			Body   string `json:"body"`
		} `json:"documents"`
	}
	if err := json.Unmarshal(raw, &kat); err != nil {
		panic(err)
	}
	object, _ := hex.DecodeString(kat.ObjectHex)

	// The index: height -> document body. A real verifier sets Index.Fetch to
	// GET <index>/index/<height>.
	index := hike.Index{Documents: map[uint64]string{}}
	for _, d := range kat.Documents {
		index.Documents[d.Height] = d.Body
	}

	seed := make([]byte, 32)
	for i := range seed {
		seed[i] = 0x11
	}
	stream := hike.AuditStream(seed, 4096) // use 4096 bytes from crypto/rand in production

	record, decoded := hike.Verify(object, hike.ConformanceSmallV2, index, stream, false) // hike.Production for real objects
	if !decoded {
		fmt.Println("undecodable")
		os.Exit(1)
	}
	body := record.Marshal()
	fmt.Println("shape:", record.Shape)
	fmt.Printf("record sha256: %x\n", sha256.Sum256(body))
}
