# Go SDK

The Go SDK for Satoshi Hike v2.0, the PCOC specification. Module `github.com/groundinglab/pcoc/sdk/go/v2`, package `hike`. It verifies a hike object and writes the section 14 measurement record, byte-identical to the Rust and Java SDKs. It also has a deterministic producer for an already-read tip sequence (`Drive`), and the two programs `satoshi-hike-verify` and `satoshi-hike-produce`.

It uses only the Go standard library. Go 1.22 or newer.

## Install / pin

```sh
go get github.com/groundinglab/pcoc/sdk/go/v2@v2.0.0-rc.1
```

The module lives under `sdk/go/`, so its tag is `sdk/go/v2.0.0-rc.1`. This is the same commit as the repository tag `sdk-v2.0.0-rc.1`.

> The tag has been prepared but not yet published. Until it is, use a checkout of this repository:
>
> ```
> require github.com/groundinglab/pcoc/sdk/go/v2 v2.0.0-rc.1
> replace github.com/groundinglab/pcoc/sdk/go/v2 => ../pcoc/sdk/go
> ```

```go
import "github.com/groundinglab/pcoc/sdk/go/v2/hike"
```

## Quickstart (run)

[`sdk/examples/go`](../examples/go/) verifies the `kat-small` conformance object against its recorded index replies. `sdk/test.sh` runs it on every change.

```sh
cd sdk/examples/go
go run . ../../../spec/conformance/v2.0
```

```text
shape: match
record sha256: 79561726b24aad6a9fcdfb6ae09f0eb0b54934a57bb08dab4bd11e4113d64241
```

The digest is the SHA-256 of the record bytes. The Rust and Java quickstarts print the same digest. The core of it:

```go
index := hike.Index{Documents: map[uint64]string{ /* height: section 9 document body */ }}
stream := hike.AuditStream(seed, 4096) // pinned draw; production uses crypto/rand
record, decoded := hike.Verify(object, hike.ConformanceSmallV2, index, stream, false)
if !decoded { /* not a hike object; record still says why */ }
body := record.Marshal() // the exact bytes the conformance pack stores
```

For a real object, use `hike.Production`, fetch the index over HTTP, and draw the six legs from `crypto/rand`:

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
os.Stdout.Write(record.Marshal())
```

## API overview

**Verify (section 14).**
- `Verify(data, profile, index, stream, omitSix) (Record, bool)` measures one object. It does not apply policy. `bool` is false when the object does not decode, and the record is then the "undecodable" or "malformed" record.
- `Verify` counts a stored plant path as bound. To check each path against the chain yourself, call `PackRecord(data, profile, index, pathChain, threads, stream) string`. `pathChain(PathLocators) error` returns nil for a match. `threads` is the Rule A4.3 pool size and does not change the record.
- `Record` is the section 14 record. `Record.Marshal()` writes its bytes, with keys in spec order and one trailing newline. `VerifyRecordSHA256(record)` is SHA-256 over the RFC 8785 form, excluding the newline.

**Inputs.**
- `Index{Documents, Fetch}` is the index, keyed by height. `ReadIndexDocument(body)` applies the section 9 document rules.
- `Profile`: `Production` (the section 6 constants), `ConformanceSmallV2` (the section 16.1 sizes), `MeasureName(name)`, and `NewProfile(...)` for custom sizes.
- `AuditStream(seed, n)` gives the pinned demo draw, SHA-256(seed ‖ le64(i)) blocks, the same as the programs' `--audit-seed`.

**Objects and paths.**
- `MapKeys` and `StageKeys` list CBOR keys. `CBORIssue(data)` says why the bytes are not one section 7.3 item, and returns empty when they are.
- `PathLocatorsFor(path, waypoint)` decodes a canonical path body. `EncodePathInBlock` and `PathInclusionHeight` also work on path bodies.
- `IsEarlierKind` checks for an earlier object kind.

**Produce (section 13.2).** `Drive(publicKey, profile, tips)` runs produce against an already-read `[]Tip` sequence and returns the object bytes. Network produce (gossip, Bari) is `cmd/satoshi-hike-produce`.

**Building blocks.**
- `FillTerrain`, `SeedValue`, `SampleAt`, `SlotAt`, `CheckpointDigest`, `WaypointDigest`, `FoldCheckpoints`, `DrawSix`, `FillPin`.
- The section 6 constants: `Stages`, `Kind`, `SealDomain`, the `Mix*` values, and others.
- The informative §15.5 examples `LegRateFloor*`.

**Programs.** `cmd/satoshi-hike-verify` and `cmd/satoshi-hike-produce`. Both have `--help`.

The Rust SDK has two things this package does not: direct node reads for the plant check (`hike-net`), and the section 15 `admit` helper.

## Errors

| Error (text) | Returned by | Cause |
|---|---|---|
| `malformed cbor` | `MapKeys`, `StageKeys` (reported by `CBORIssue`) | A text string that is not UTF-8, the top item is not a map, or `stages` is missing, is not an array, has no such index, or that stage is not a map |
| `indefinite` | same | Indefinite-length item (section 7.3 forbids it) |
| `float-or-simple` | same | Major type 7 (float, simple, true/false/null), or a reserved length code (28–30) |
| `tag` | same | A CBOR tag |
| `duplicate` | same | A map key repeated |
| `non-text` | same | A map key that is not a text string |
| `truncated` | same | The item runs past the end of the buffer |
| `negative` | same | A negative integer |
| `trailing` | same | Bytes after the first item |
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
| `profile integers do not divide` | `NewProfile` | The slot is not a multiple of 8 (at least 8), the terrain is not a multiple of the slot, sample < 1, terrain < sample, steps < 1, or the kind is empty |
| `public key` | `Drive` | `publicKey` is not 32 bytes |
| `seed observe` | `Drive` | No tips, or the first tip's `Natural` is not 32 bytes |
| `observe` | `Drive` | The tip sequence ran out, or a tip's `Natural` is not 32 bytes |
| JSON error | `VerifyRecordSHA256` | The record is not JSON |

`Verify` does not return an error. Malformed input gives `decoded == false` or a record whose tokens say `mismatch`, `not defined` or `not read`. A failed index read is `ok == false` from `Fetch` and appears in the record. The programs exit 0 when the record is written, 1 when the object does not decode or cannot be read, and 2 on a usage error.

## Conformance vectors

`spec/conformance/v2.0/` is the source of truth ([what each file is](../../spec/conformance/v2.0/README.md)). This SDK does not generate the vectors. `conformance/v2_test.go` and the package tests read them in place and compare the record byte for byte. [../SPEC_PIN](../SPEC_PIN) holds the spec tag and the vector hash.

## Test

```sh
bash sdk/test.sh                          # every SDK, plus the cross-check (from the repo root)
cd sdk/go && go vet ./... && go test ./...   # this module only
```

## Programs

```sh
go run ./cmd/satoshi-hike-verify --input OBJECT --profile live --index-url https://index.groundinglab.io
go run ./cmd/satoshi-hike-produce --pubkey-hex HEX --output PATH --gossip-peer HOST:PORT --bari-url URL
```

The default `--profile` is `test`. It will not dial `https://index.groundinglab.io`, and it reads `PCOC_TEST_INDEX_URL` when `--index-url` is omitted. `--profile conformance-small` uses the section 16.1 sizes, and `--audit-seed HEX` pins the draw. Verify writes the section 14 record to stdout. It does not apply admission policy. `--chain-rpc URL` is accepted with `--profile live` for flag compatibility, but this program does not read the node: it calls verify without a chain callback, so a stored path in the anchor window counts as bound. This SDK does not derive a production seed from a mnemonic.
