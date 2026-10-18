// Library-level record, for sdk/tools/crosscheck.py.
//
// Usage: librecord OBJECT DOCS_JSON SEED_HEX
// Calls hike.Verify at conformance-small sizes with the pinned draw stream
// SHA-256(seed || le64(i)), i = 0, 1, ..., and prints the record it returns
// (also when decoded is false), then one newline.
package main

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"os"
	"strconv"

	"github.com/groundinglab/pcoc/sdk/go/v2/hike"
)

func main() {
	object, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	raw, err := os.ReadFile(os.Args[2])
	if err != nil {
		panic(err)
	}
	var byHeight map[string]string
	if err := json.Unmarshal(raw, &byHeight); err != nil {
		panic(err)
	}
	docs := map[uint64]string{}
	for h, body := range byHeight {
		n, err := strconv.ParseUint(h, 10, 64)
		if err != nil {
			panic(err)
		}
		docs[n] = body
	}
	seed, err := hex.DecodeString(os.Args[3])
	if err != nil {
		panic(err)
	}
	var stream []byte
	for i := uint64(0); i < 64; i++ {
		var le [8]byte
		binary.LittleEndian.PutUint64(le[:], i)
		block := sha256.Sum256(append(append([]byte{}, seed...), le[:]...))
		stream = append(stream, block[:]...)
	}
	record, _ := hike.Verify(object, hike.ConformanceSmallV2, hike.Index{Documents: docs}, stream, false)
	os.Stdout.Write(record.Marshal())
	os.Stdout.Write([]byte("\n"))
}
