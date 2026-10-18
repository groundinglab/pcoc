// Verify a real hike object against the public index. Usage: live OBJECT_FILE
package main

import (
	"crypto/rand"
	"fmt"
	"io"
	"net/http"
	"os"
	"time"

	"github.com/groundinglab/pcoc/sdk/go/v2/hike"
)

func main() {
	object, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	client := &http.Client{Timeout: 5 * time.Second}
	index := hike.Index{Fetch: func(height uint64) (string, bool) {
		resp, err := client.Get(fmt.Sprintf("https://index.groundinglab.io/index/%d", height))
		if err != nil {
			return "", false
		}
		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			return "", false
		}
		b, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
		return string(b), err == nil
	}}
	stream := make([]byte, 4096)
	if _, err := rand.Read(stream); err != nil {
		stream = nil // the record then says the draw was not read
	}
	record, decoded := hike.Verify(object, hike.Production, index, stream, false)
	if !decoded {
		fmt.Fprintln(os.Stderr, "undecodable") // the record still says why
	}
	os.Stdout.Write(record.Marshal())
}
