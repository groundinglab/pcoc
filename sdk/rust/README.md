# Rust SDK

The Rust SDK for Satoshi Hike v2.0, the PCOC specification. One Cargo workspace:

| Crate | What it is |
|---|---|
| `hike` | The library. It covers decoding, measuring and verifying a hike object (the section 14 record), producing one (section 13), the section 15 admission helpers, the index and plant clients, and the constants. |
| `hike-net` | Network code that is kept out of `hike`: a direct Bitcoin Core RPC read for the plant check (`ChainRpc`), and the test-plane policy. |
| `nip06` | DEMO keys only: BIP-39 English 12-word, empty passphrase, NIP-06 path, BIP-340 x-only. Not a hike `production_seed`. |
| `satoshi-hike-verify`, `satoshi-hike-produce` | Command-line programs built on `hike`. |

This is the engine the admission gate and the desktop app use, and the core the Android app links through JNI. It is also the program that generates the conformance vectors.

## Install / pin

Rust 1.99.0 is pinned in `rust-toolchain.toml`. The crates declare `rust-version = "1.83"`. Nothing is published to crates.io, so depend on a git tag or a path:

```toml
[dependencies]
# a tag (Rust and Java share the repository tag)
hike = { git = "https://github.com/groundinglab/pcoc", tag = "sdk-v2.0.0-rc.1" }
# or a checkout of this repository
hike = { path = "../pcoc/sdk/rust/hike" }
```

> `sdk-v2.0.0-rc.1` has been prepared but not yet published. Use the path form until it is on the remote.

Cargo finds `hike` inside the repository by its package name, so you do not need a `subdir` key. Add `nip06` or `hike-net` the same way if you need them.

## Quickstart (run)

[`sdk/examples/rust`](../examples/rust/) verifies the `kat-small` conformance object against its recorded index replies and prints the SHA-256 of the record bytes. `sdk/test.sh` runs it on every change.

```sh
cd sdk/examples/rust
cargo run --release -- ../../../spec/conformance/v2.0
```

```text
shape ok: true
record sha256: 79561726b24aad6a9fcdfb6ae09f0eb0b54934a57bb08dab4bd11e4113d64241
```

The Go and Java quickstarts print the same digest. The core of it:

```rust
let measurements = hike::verify_sized(
    &object,                                   // the hike object bytes (CBOR)
    hike::AuditDraw::PinnedSeed([0x11; 32]),   // AuditDraw::OsCsprng in production
    |height| hike::parse_index_beacon(docs.get(&height).ok_or(())?).map_err(|_| ()),
    |loc: &hike::PathLocators| match docs.get(&u64::from(loc.block_height)) {
        Some(doc) => hike::proxy_matches(doc, loc).into(),   // plant check through the index
        None => hike::ChainRead::NotRead,
    },
    hike::Sizes::conformance_small(),          // Sizes::production() for real objects
);
let record: String = hike::verify_record_json("conformance-small", &measurements);
```

For a real object, use `hike::verify(&object, AuditDraw::OsCsprng, beacon_at, path_chain)` (production sizes), and read the index over HTTP:

```rust
let agent = hike::http_agent();
let index = "https://index.groundinglab.io"; // or your own pcoc-index
let m = hike::verify(
    &object,
    hike::AuditDraw::OsCsprng,
    |h| hike::read_index_beacon(&agent, index, h).map_err(|_| ()),
    |loc| match hike::index_document(&agent, index, u64::from(loc.block_height)) {
        Ok(doc) => hike::proxy_matches(&doc, loc).into(),
        Err(_) => hike::ChainRead::NotRead,
    },
);
print!("{}", hike::verify_record_json("production", &m));
```

## API overview

Everything below is re-exported at the crate root (`hike::…`).

**Verify (section 14).** `verify` and `verify_sized` take the object, an `AuditDraw` (`OsCsprng` or `PinnedSeed`), a `beacon_at(height) -> Result<IndexBeacon, ()>` closure and a `path_chain(&PathLocators) -> ChainRead` closure, and return `Measurements`. `verify_with_bind_threads[_sized]` also sets the Rule A4.3 pool size, which does not change the record. `verify_fast[_sized]` and `screen_fast[_sized]` are the cheap phases a gate runs first. `verify_record_json(profile, &Measurements)` writes the record bytes that the vectors hold, and `verify_record_sha256(record)` hashes the JCS form. In the result types (`Recorded<T>`, `ChainRead`, `SixLegs`, `LegRate` …), a value that could not be measured is a variant such as `NotObtained` or `NotRead`. It is not an error.

**Objects.** `decoded_hike`, `object_public_key`, `yielded_public_key`, `canonical_object`, `earlier_object_kind`, `encode_hike[_sized]`, `structural_object`, and the types `Hike`, `Stage` and `YieldedKey`.

**Index and plant.** `parse_index_beacon` / `parse_index_document` (section 9 documents), `IndexBeacon`, `read_index_beacon`, `index_document`, `beacon_at_height`, `http_agent`; `path_locators`, `PathLocators`, `proxy_matches` (plant check via the index), `direct_matches` / `direct_facts` (via a node; see `hike_net::ChainRpc`).

**Produce (section 13).** `produce`, `produce_with_settlement[_sized]` and `produce_stage` take an `observe` tip closure and an `open_stage` plant closure, and return the object bytes or `ProduceFailure`. `HeaderClock`, `GossipNetwork`, `note_produce_headers` and `serve_synth` cover the Bitcoin header feed. `plant_stage`, `open_stage`, `LivePathBook` and `PathFetch` handle the plant (Bari) side.

**Building blocks.** `fill_terrain[_sized]`, `production_seed`, `leg_result[_sized]`, `checkpoint_sized`, `successor_waypoint[_sized]`, `terrain_sample_sized`, `draw_distinct`, `effective_bits`; `Sizes` (`production()`, `conformance_small()`); and the section 6 constants (`KIND`, `STAGES`, `STEPS_PER_LEG`, `SAMPLE_BYTES` …, `ANCHOR_WINDOW_OFFSETS`).

**Admission (section 15).** `admit`, `failure_facts`. Policy belongs to the caller. The SDK holds no admission threshold beyond the informative §15.5 example constants (`LEG_RATE_FLOOR_*`).

**Events and reports.** `verify_events` turns one observation into `VerifyEvent`s: the object, then the six checks in section 14 order. `publish_verify` hands a record to a `VerifySubscriber` without running section 14 again. `Report` and `emit_report` write the programs' `--report` JSON (`satoshi-hike-cli-report-v0.2`).

**Programs.** `parse_verify` / `parse_produce` and the profile types are the argument parsers of the two programs.

**nip06.** `from_mnemonic`, `create_demo_key`, `DemoKey` (`public_key`, `npub`, and the accessors `secret_key()`, `nsec()`, `mnemonic()`; `Debug` shows public parts only, and there is no `Display`), `sign_bip340_message`, `verify_bip340_message`.

## Errors

| Type | Returned by | Cause |
|---|---|---|
| `PathError::Truncated` | `path_locators`, path checks | The body is shorter than its fixed 146 bytes, or a field runs past the end |
| `PathError::UnsupportedVersion(v)` | same | The first byte is not `0x01` |
| `PathError::TooManySiblings(n)` | same | More than 32 siblings |
| `PathError::WrongLength { actual, expected }` | same | The length is not 146 + 32 × the sibling count |
| `PathError::EmptyTree` | same | `tree_size` is 0 |
| `PathError::LeafIndexOutOfRange { index, tree_size }` | same | `leaf_index` ≥ `tree_size` |
| `PathError::NonCanonicalSiblingCount { actual, expected }` | same | The sibling count is not the one `tree_size` implies |
| `PathError::WrongSelfSibling` | same | A lone last node is not paired with itself |
| `PathError::WrongLeaf` | same | The leaf is not the waypoint being checked |
| `PathError::WrongRoot` | same | The recomputed Merkle root differs from the stored root |
| `ClockError` | `parse_index_beacon`, `parse_index_document` | The body is not one JSON value with unique members, `header` is not an object, `header.hash` is not 64 hex digits, or `header.height` is not an unsigned integer |
| `ProduceFailure { message, http_status }` | `produce*`, `read_clock`, plant calls | A tip or plant call failed. `http_status` is set when there was an HTTP answer |
| `UsageError` | `parse_verify`, `parse_produce` | Bad command-line arguments (the programs exit 2) |
| `Err(())` | `decoded_hike`, `object_public_key`, `canonical_object` | The bytes are not one well-formed hike object: not CBOR, not a map, wrong field set or types, or trailing bytes |
| `Err(())` | `proxy_matches`, `direct_matches` | The plant document or node facts do not match the path |
| `None` / `YieldedKey::Undecodable` | `earlier_object_kind`, `yielded_public_key` | Not a CBOR map, or not an earlier kind |
| `Err(String)` | `verify_record_sha256` | The record is not JSON |
| `Err(String)` | `nip06::from_mnemonic`, `create_demo_key`, `sign_bip340_message`, `nip06::decode_hex32`, `hike_net::ChainRpc::facts`, `emit_report` | Not 12 English BIP-39 words or a bad checksum; OS CSPRNG failure; the secret is rejected by secp256k1; bad hex; node RPC failure; the report file cannot be written |
| `Err(String)` | `read_index_beacon`, `index_document` | No HTTP answer, a non-200 answer, or (beacon) a body that `parse_index_beacon` rejects |

Verify itself never fails. Malformed input gives a record that says `mismatch` or `not defined`. Untrusted bytes never cause a panic: `tests/malformed_input.rs` feeds thousands of deterministic mutants of the pack objects, path bodies and index replies through every public decoder.

Programs exit 0 when the record is written, 1 when the object does not decode or a file cannot be read, and 2 on a usage error.

## Conformance vectors

`spec/conformance/v2.0/` is the source of truth ([what each file is](../../spec/conformance/v2.0/README.md)). `hike/tests/conformance_v2.rs` reproduces every vector byte for byte. With `PCOC_REGEN_V2=1`, and only in a spec change, it rewrites them instead. [../SPEC_PIN](../SPEC_PIN) holds the spec tag and the vector hash.

## Test

```sh
bash sdk/test.sh                                # every SDK, plus the cross-check (from the repo root)
cd sdk/rust && cargo test --locked --release    # this workspace only
```

## Programs

```sh
cargo build --release --locked
./target/release/satoshi-hike-verify --input OBJECT --profile live --index-url https://index.groundinglab.io
./target/release/satoshi-hike-produce --help
```

The default `--profile` is `test`. It will not dial `https://index.groundinglab.io`, and it reads `PCOC_TEST_INDEX_URL` when `--index-url` is omitted. `--profile conformance-small` uses the section 16.1 sizes. `--audit-seed HEX` pins the six-leg draw. `satoshi-hike-verify` writes the section 14 record to stdout. It does not apply admission policy.
