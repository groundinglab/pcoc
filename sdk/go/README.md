# Go SDK

The Go SDK for Satoshi Hike v2.0, the PCOC specification: module `github.com/groundinglab/pcoc/sdk/go/v2`, package `hike`. It verifies a hike object and writes the section 14 measurement record, byte-identical to the Rust and Java SDKs. It also has a deterministic producer for an already-read tip sequence (`Drive`), and the programs `satoshi-hike-verify` and `satoshi-hike-produce`.

It uses only the Go standard library, and needs Go 1.22 or newer.

## Install / pin

```sh
go get github.com/groundinglab/pcoc/sdk/go/v2@v2.0.0-rc.3
```

The module lives under `sdk/go/`, so its tag is `sdk/go/v2.0.0-rc.3`. That is the same commit as the repository tag `sdk-v2.0.0-rc.3`.

> When that tag is published, `go get` resolves it. To work from a checkout instead, add a `replace`. Its path is relative to your `go.mod` and points at `<clone>/sdk/go`:
>
> ```
> require github.com/groundinglab/pcoc/sdk/go/v2 v2.0.0-rc.3
> replace github.com/groundinglab/pcoc/sdk/go/v2 => ../pcoc/sdk/go
> ```

```go
import "github.com/groundinglab/pcoc/sdk/go/v2/hike"
```

## Quickstart (run by `sdk/test.sh`)

[`sdk/examples/go`](../examples/go/) verifies the `kat-small` conformance object against the index replies recorded in the vector, and reproduces the vector's stored record byte for byte:

<!-- from: sdk/examples/go/main.go -->
```go
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
```

```sh
cd sdk/examples/go
go run . ../../../spec/conformance/v2.0
```

<!-- output: quickstart -->
```text
draw = draw_bytes_hex: true
shape: match
matches kat-small.json record: true
record bytes sha256: f539d89340a4e7885638412a0e1fbdf85212498c87ac5bb237a645ddc987ffac
verify_record_sha256: 084e60600907af444632fc2bb09bf8846d78384f4f23eb2a06f6f50491161c29
```

What the output lines mean:
- **The draw.** Passing the vector's `draw_bytes_hex` as `stream` is what reproduces the stored `record`. Those bytes are `AuditStream(seed, 256)` with `seed` = 32 bytes of `0x5a`. Any other stream draws other legs and writes another record.
- **`record bytes sha256`** is the SHA-256 of the record bytes, including the trailing newline.
- **`verify_record_sha256`** (`VerifyRecordSHA256`) is a different hash: SHA-256 over the RFC 8785 (JCS) form, excluding the newline. Use it as the record identifier. Neither is the vector's `sha256` field, which is the hash of the object bytes.

The Rust and Java quickstarts print the same lines.

## Verify a real object

[`sdk/examples/go/live`](../examples/go/live/main.go) uses `hike.Production`, reads the public index over HTTP, and draws from `crypto/rand`. `sdk/test.sh` compiles it with `go vet` but doesn't run it, because it needs the network.

<!-- from: sdk/examples/go/live/main.go -->
```go
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
```

**The public index.** `https://index.groundinglab.io` serves `GET /index/<height>` only for the mainnet heights it retains; any other height gets 404. The `conformance-small` vectors use small, non-mainnet heights, so on a vector object every read is `not read`. Use a real production object.

**Gating.** This SDK has no section 15 helper; only Rust has one (`hike::admit` / `hike::failure_facts`). A Go gate reads the record's fields and applies the section 15 invariant refusals itself: `Shape`, the `checkpoints` comparison, `seed_value`, plant, successor, and six legs (spec section 15). Then it applies its own thresholds. groundinglab's argued default is [POLICY.md](../../services/admission-gate/POLICY.md), a recommendation and not part of the spec. Writing that is new code; the SDK does not provide it.

## Produce

[`sdk/examples/go/produce`](../examples/go/produce/main.go) produces an object offline. It uses the `stage0-start.json` tip script at `conformance-small` sizes, and then verifies the result:

<!-- from: sdk/examples/go/produce/main.go -->
```go
var tips []hike.Tip
for _, t := range v.Tips {
	natural, _ := hex.DecodeString(t.NaturalHex)
	tips = append(tips, hike.Tip{Natural: natural, Height: t.Height})
}
// ...
object, _, err := hike.Drive(publicKey, hike.ConformanceSmallV2, tips) // hike.Production for a real PCOC
if err != nil {
	panic(err) // "public key", "seed observe" or "observe"
}
```

```sh
go run ./produce ../../../spec/conformance/v2.0
```

<!-- output: produce -->
```text
object bytes: 3670
object sha256: da3acd20d063b7492e192b557fdf711fbe5761a49e446a6b2f664af17f72f765
shape: match
```

`Drive` writes the same object bytes as the Rust and Java produce examples. It has no plant service, so every path is empty bytes. Network produce (Bitcoin header gossip, and the Bari plant service) is `cmd/satoshi-hike-produce`; it is not a library API in this SDK. For what a real hike costs (18 tip waits × the mean header interval, with 256 MiB held), see [What producing costs](../README.md#what-producing-costs).

## API overview

Generated docs: `go doc -all ./hike` in `sdk/go`.

```go
func Verify(data []byte, profile Profile, index Index, stream []byte, omitSix bool) (Record, bool)
func PackRecord(data []byte, profile Profile, index Index, pathChain func(PathLocators) error, threads int, stream []byte) string
func (r Record) Marshal() []byte                       // record bytes: section 14 key order, one trailing newline
func VerifyRecordSHA256(record string) ([32]byte, error) // JCS hash, newline excluded
func AuditStream(seed []byte, nbytes int) []byte         // SHA-256(seed ‖ le64(i)) blocks
type Index struct {
	Documents map[uint64]string
	Fetch     func(height uint64) (body string, ok bool) // used for heights not in Documents
}
func Drive(publicKey []byte, profile Profile, tips []Tip) (object []byte, stderr string, err error)
type Tip struct { Natural []byte; Height uint64 }
func PathLocatorsFor(path, waypoint []byte) (PathLocators, error)
func MapKeys(data []byte) ([]string, error)
func StageKeys(data []byte, index int) ([]string, error)
func CBORIssue(data []byte) string                      // "" when data is one well-formed item
func NewProfile(name, kind string, sample, slot, terrain, steps int) (Profile, error)
```

**Verify.**
- **`Verify`** measures one object. It does not apply policy. The `bool` is false when the object does not decode, and the record is then the "undecodable" or "malformed" record. `omitSix` leaves `six_leg` and `six_leg_runs` off the record.
- **`Verify` and the plant check.** `Verify` counts a stored plant path as bound. To check each path against the chain yourself, call `PackRecord` with a `pathChain` that returns nil for a match. Its `threads` argument does not change the record.
- **`Record` fields** (exported, JSON-tagged as the record keys):
  - shape: `Profile`, `Shape` (`match` or `mismatch`), `ShapeItem []string`;
  - stored data: `Headers`, `SeedValue`, `Checkpoints`, `Waypoints`, `Paths`, `Anchors`, `AnchoredTotal`, `SkipList`;
  - bookends: `A`, `B`, `N`, `S`, `DeltaH`, `FixedStart`, `PB`, `HeightPB`, `IndexComparisonPB`, `TPB`, `HeightIAMinus1`, `TIAMinus1`;
  - draw and analysis: `SixLeg`, `SixLegRuns`, `Analysis`;
  - for `conformance-small`: `SampleBytes`, `SlotBytes`, `TerrainBytes`, `StepsPerLeg`.

**Inputs.**
- `Production` (section 6) and `ConformanceSmallV2` (section 16.1) are the built-in profiles. `MeasureName(name)` selects one by program name.
- `ReadIndexDocument(body)` applies the section 9 document rules.

**Objects and paths.** `EncodePathInBlock`, `PathInclusionHeight` and `IsEarlierKind`.

**Building blocks.**
- Functions: `FillTerrain`, `SeedValue`, `SampleAt`, `SlotAt`, `CheckpointDigest`, `WaypointDigest`, `FoldCheckpoints`, `DrawSix` and `FillPin`.
- The section 6 constants: `Stages`, `Kind`, `SealDomain`, `Mix*`, ….

**Programs.** `cmd/satoshi-hike-verify` and `cmd/satoshi-hike-produce`.

The Rust SDK has two things this package does not: direct node reads for the plant check (`hike-net`), and the section 15 helpers.

## Errors

| Error text | Returned by | Cause |
|---|---|---|
| `malformed cbor` | `MapKeys`, `StageKeys` (`CBORIssue` reports the same text) | A text string that is not UTF-8; the top item is not a map; or `stages` is missing, is not an array, has no such index, or that stage is not a map |
| `indefinite` | same | An indefinite-length item, or a lone `0xff` break byte (section 7.3 forbids both) |
| `float-or-simple` | same | Major type 7 (float, simple, true/false/null), or a reserved length code (28–30) |
| `tag` | same | A CBOR tag |
| `duplicate` | same | A map key repeated |
| `non-text` | same | A map key that is not a text string |
| `truncated` | same | The item runs past the end of the buffer |
| `negative` | same | A negative integer |
| `trailing` | same | Bytes after the first item |
| `nesting depth` | same | More than `MaxCBORNesting` (64) nested arrays and maps (rc.3) |
| `truncated` | `PathLocatorsFor` | The path body is shorter than its fixed part |
| `version` | same | The first byte is not `0x01` |
| `siblings` | same | More than 32 siblings |
| `length` | same | The length is not the fixed part plus 32 × the sibling count |
| `empty tree` | same | `tree_size` is 0 |
| `leaf index` | same | `leaf_index` ≥ `tree_size` |
| `sibling count` | same | The sibling count is not the one `tree_size` implies |
| `self sibling` | same | A lone last node is not paired with itself |
| `leaf` | same | The leaf is not this waypoint |
| `root` | same | The recomputed Merkle root differs from the stored root |
| `profile integers do not divide` | `NewProfile` | The slot is not a multiple of 8 (or is under 8); the terrain is not a multiple of the slot; sample < 1; terrain < sample; steps < 1; or the kind is empty |
| `public key` | `Drive` | `publicKey` is not 32 bytes |
| `seed observe` | `Drive` | No tips, or the first tip's `Natural` is not 32 bytes |
| `observe` | `Drive` | The tip sequence ran out, or a tip's `Natural` is not 32 bytes |
| JSON error | `VerifyRecordSHA256` | The record is not JSON |

**Nesting limit (rc.3, implementation resource limit).** The decoder accepts at most `hike.MaxCBORNesting` = 64 nested arrays and maps (an item counts the containers around it, itself included). A deeper item is malformed CBOR: the record is `["malformed cbor"]` and the programs exit 1. A valid section 8 hike object nests 4 deep (top-level map, `stages` array, stage map, `checkpoints`/`leg_results` array), so no valid object is refused. The value is the same in the Rust, Go and Java SDKs; see [KNOWN-DIVERGENCES.md](../KNOWN-DIVERGENCES.md#implementation-resource-limits).

The diagnostic texts are not part of the record, and they differ between SDKs. Only the record bytes are conformance-tested. The [Rust README](../rust/README.md#error-names-across-the-sdks) has a table that maps them across Rust, Go and Java.

`Verify` does not return an error. Malformed input gives `decoded == false`, or a record whose tokens say `mismatch`, `not defined` or `not read`. A failed index read is `ok == false` from `Fetch`, and shows in the record.

The programs exit 1 only when no top-level CBOR map decodes: the file cannot be read, the first item does not decode under section 7.3, or it is well-formed but not a map. In every other case, trailing bytes after a map included, they write the record and exit 0. Usage errors exit 2. `Verify` returns the `["malformed cbor"]` record (with `decoded` false) for every input the program refuses. Rust and Java behave the same (rc.2).

## Conformance vectors

`spec/conformance/v2.0/` is the source of truth ([what each file is](../../spec/conformance/v2.0/README.md)). This SDK does not generate the vectors. `conformance/v2_test.go` and the package tests read them in place, and compare the record byte for byte. [../SPEC_PIN](../SPEC_PIN) holds the spec tag and the vector hash.

## Test

```sh
bash sdk/test.sh                              # every SDK, the cross-check, examples and docs (from the repo root)
cd sdk/go && go vet ./... && go test ./...    # this module only; four "ok" package lines
```

## Programs

```sh
go run ./cmd/satoshi-hike-verify --input OBJECT --profile live --index-url https://index.groundinglab.io
go run ./cmd/satoshi-hike-produce --pubkey-hex HEX --output PATH --gossip-peer HOST:PORT --bari-url URL
```

**Verify** takes the same flags as the Rust and Java programs; see [the verify programs](../README.md#the-verify-programs).
- The default `--profile` is `test`. It does not dial `https://index.groundinglab.io`; it reads `PCOC_TEST_INDEX_URL` when `--index-url` is omitted.
- `--profile conformance-small` requires `--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N`. The record states those integers. Any of the four with another profile is a usage error, exit 2.
- `--chain-rpc URL` is accepted with `--profile live` for flag compatibility, but this program does not read the node. It calls `Verify` without a chain callback, so a stored path in the anchor window counts as bound.

**Produce** has its own flags: `--tips PATH` and the profiles `test|live|production`. See `--help`. This SDK does not derive a production seed from a mnemonic.
