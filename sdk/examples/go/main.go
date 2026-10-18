// Verify the kat-small conformance object and reproduce the vector's stored
// section 14 record byte for byte.
package main

import (
	"bytes"
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
		ObjectHex    string `json:"object_hex"`
		DrawBytesHex string `json:"draw_bytes_hex"`
		Record       string `json:"record"`
		Documents    []struct {
			Height uint64 `json:"height"`
			Body   string `json:"body"`
		} `json:"documents"`
	}
	if err := json.Unmarshal(raw, &kat); err != nil {
		panic(err)
	}
	object, _ := hex.DecodeString(kat.ObjectHex)

	// The index: height -> section 9 document body. A real verifier sets
	// Index.Fetch to GET <index>/index/<height>; here the vector carries the replies.
	index := hike.Index{Documents: map[uint64]string{}}
	for _, d := range kat.Documents {
		index.Documents[d.Height] = d.Body
	}

	// The six-leg draw bytes. kat-small's draw_bytes_hex is AuditStream(seed, 256)
	// for this seed. Production passes 4096 bytes from crypto/rand.
	stream, _ := hex.DecodeString(kat.DrawBytesHex)
	seed := bytes.Repeat([]byte{0x5A}, 32)
	fmt.Println("draw = draw_bytes_hex:", bytes.Equal(hike.AuditStream(seed, len(stream)), stream))

	record, decoded := hike.Verify(object, hike.ConformanceSmallV2, index, stream, false) // hike.Production for real objects
	if !decoded {
		fmt.Println("undecodable")
		os.Exit(1)
	}
	body := record.Marshal() // compact JSON in section 14 key order plus one newline
	fmt.Println("shape:", record.Shape)
	fmt.Println("matches kat-small.json record:", string(body) == kat.Record)
	fmt.Printf("record bytes sha256: %x\n", sha256.Sum256(body))
	jcs, err := hike.VerifyRecordSHA256(string(body))
	if err != nil {
		panic(err)
	}
	fmt.Printf("verify_record_sha256: %x\n", jcs)
}
