# Go reference

The Go implementation of Satoshi Hike v2.0. It is one of three independent references. It is a guide, not the specification. Where it disagrees with [Satoshi Hike v2.0](../../spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md), the specification wins and this tree has a bug.

This port does not generate the conformance vectors. It reads `spec/conformance/v2.0/` and compares its section 14 record with the bytes already there.

## Tag

The current tag is `reference/go/v2.0.3`. `go.mod` declares module `github.com/groundinglab/pcoc/reference/go/v2`, so the tag must have that prefix. There is no `ref-go-` tag.

## Toolchain

Go 1.22, from `go.mod`. No third-party dependencies.

## Test

From this directory:

```sh
go vet ./...
go test ./...
```

## Produce and verify

```sh
go run ./cmd/satoshi-hike-produce --pubkey-hex HEX --output PATH --gossip-peer HOST:PORT --bari-url URL
go run ./cmd/satoshi-hike-verify --input OBJECT --index-url URL
```

`satoshi-hike-verify` writes the section 14 measurement record to stdout when the object decodes. It does not apply admission policy.

## Spec pin

[../SPEC_PIN](../SPEC_PIN) names `spec-v2.0.2` and the vector hash. Check it with `python3 ../tools/check_spec_pin.py` from `reference/`.

## Demo keys

This port does not derive a production seed. A mnemonic on the command line is not `production_seed`.
