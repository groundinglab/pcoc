# Rust SDK

The Rust SDK for Satoshi Hike v2.0, the PCOC specification. It is one Cargo workspace:

| Crate | What it is |
|---|---|
| `hike` | The library: decode, measure and verify a hike object (the section 14 record); produce one (section 13); the section 15 refusals; the index and plant clients; the constants. |
| `hike-net` | Network code kept out of `hike`: `ChainRpc::new(url, user, password)` reads a Bitcoin Core node for the plant check. (The test-plane `judge_test_policy` moved out of the SDK in rc.2: it is policy, not measurement.) |
| `nip06` | DEMO keys only: BIP-39 English 12-word, empty passphrase, NIP-06 path, BIP-340 x-only. Not a hike `production_seed`. |
| `satoshi-hike-verify`, `satoshi-hike-produce` | Command-line programs built on `hike`. |

The admission gate and the desktop app use this SDK as their engine, and the Android app links it through JNI. It is also the program that generates the conformance vectors.

## Install / pin

- **Rust version.** Consumers need Rust **1.83** or newer, from `rust-version` in the crates. `sdk/rust/rust-toolchain.toml` pins 1.99.0, but that pin governs only builds inside `sdk/rust/`. Your project uses its own toolchain.
- **Where to get it.** Nothing is published to crates.io. Depend on a git tag, or on a path:

```toml
[dependencies]
# a tag (Rust and Java share the repository tag)
hike = { git = "https://github.com/groundinglab/pcoc", tag = "sdk-v2.0.0-rc.3" }
# or a checkout: the path is relative to your Cargo.toml, pointing at <clone>/sdk/rust/hike
hike = { path = "../pcoc/sdk/rust/hike" }
```

> `sdk-v2.0.0-rc.3` is the repository tag for this version. The path form is for a local checkout; the git form works once the tag exists.

Cargo finds `hike` inside the repository by its package name, so you don't need a `subdir` key. Add `nip06` or `hike-net` the same way if you need them.

**Exact dependency pins.** `hike` pins `serde_json = "=1.0.151"`, `ureq = "=2.12.1"`, `zeroize = "=1.8.2"` and `subtle = "=2.5.0"` so that the workspace builds reproducibly on Rust 1.83. Cargo allows only one semver-compatible version of a crate per build, so your project must accept those exact versions. If you also use `serde_json`, write `serde_json = "=1.0.151"` or a range that includes it. A later release may relax these pins.

## Quickstart (run by `sdk/test.sh`)

[`sdk/examples/rust`](../examples/rust/) verifies the `kat-small` conformance object against the index replies recorded in the vector, and reproduces the vector's stored record byte for byte. It needs `serde_json` and `sha2` besides `hike`:

<!-- from: sdk/examples/rust/Cargo.toml -->
```toml
[dependencies]
# In your own project, pin the tag instead:
# hike = { git = "https://github.com/groundinglab/pcoc", tag = "sdk-v2.0.0-rc.3" }
hike = { path = "../../rust/hike" }
serde_json = "=1.0.151"
sha2 = "0.10"
```

<!-- from: sdk/examples/rust/src/main.rs -->
```rust
//! Verify the kat-small conformance object and reproduce the vector's stored
//! section 14 record byte for byte.
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn main() {
    let pack = std::env::args()
        .nth(1)
        .expect("usage: pcoc-quickstart <spec/conformance/v2.0>");
    let kat: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{pack}/kat-small.json")).unwrap())
            .unwrap();
    let object: Vec<u8> = unhex(kat["object_hex"].as_str().unwrap());

    // The index: height -> section 9 document body. A real verifier fetches
    // GET <index>/index/<height>; here the vector carries the replies.
    let docs: HashMap<u64, String> = kat["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let height = d["height"].as_u64().unwrap();
            (height, d["body"].as_str().unwrap().to_owned())
        })
        .collect();

    // The six-leg draw. kat-small's draw_bytes_hex is the pinned stream
    // SHA-256(seed || le64(0)) || SHA-256(seed || le64(1)) || ... for this seed,
    // so PinnedSeed(seed) reproduces the vector. Production uses AuditDraw::OsCsprng.
    let seed = [0x5A; 32];
    let stream: Vec<u8> = (0u64..8)
        .flat_map(|i| {
            Sha256::new()
                .chain_update(seed)
                .chain_update(i.to_le_bytes())
                .finalize()
        })
        .collect();
    println!(
        "draw = draw_bytes_hex: {}",
        stream == unhex(kat["draw_bytes_hex"].as_str().unwrap())
    );

    let measurements: hike::Measurements = hike::verify_sized(
        &object,
        hike::AuditDraw::PinnedSeed(seed),
        // beacon_at: the section 9 beacon at a height, Err(()) when there is no document.
        |height: u64| -> Result<hike::IndexBeacon, ()> {
            hike::parse_index_beacon(docs.get(&height).ok_or(())?).map_err(|_| ())
        },
        // path_chain: the plant check through the index (what the program does without --chain-rpc).
        |loc: &hike::PathLocators| match docs.get(&u64::from(loc.block_height)) {
            Some(doc) => hike::proxy_matches(doc, loc).into(),
            None => hike::ChainRead::NotRead,
        },
        hike::Sizes::conformance_small(), // Sizes::production() for real objects
    );
    // The record bytes: compact JSON in section 14 key order plus one newline.
    let record: String = hike::verify_record_json("conformance-small", &measurements);

    println!(
        "shape: {}",
        if measurements.shape {
            "match"
        } else {
            "mismatch"
        }
    );
    println!(
        "matches kat-small.json record: {}",
        record == kat["record"].as_str().unwrap()
    );
    println!(
        "record bytes sha256: {:x}",
        Sha256::digest(record.as_bytes())
    );
    let jcs: [u8; 32] = hike::verify_record_sha256(&record).unwrap();
    println!(
        "verify_record_sha256: {}",
        jcs.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
}
```

```sh
cd sdk/examples/rust
cargo run --release --bin pcoc-quickstart -- ../../../spec/conformance/v2.0
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
- **`record bytes sha256`** is the SHA-256 of the record bytes, including the trailing newline. It equals SHA-256 of `kat-small.json`'s `record`.
- **`verify_record_sha256`** is a different hash: SHA-256 over the RFC 8785 (JCS) form of the record, excluding the newline. It is the one to use as a record identifier. Don't confuse either of them with the vector's `sha256` field, which is the hash of the object bytes.
- **Seed.** The draw seed is `[0x5A; 32]` because `kat-small.json`'s `draw_bytes_hex` was made from it. `AuditDraw` has no variant that takes raw draw bytes, so to reproduce a vector in Rust you pass its seed with `PinnedSeed`. With any other seed, the six-leg indices, and so the record, differ.

The Go and Java quickstarts print the same lines.

## Verify a real object, and gate it

[`sdk/examples/rust/src/bin/live.rs`](../examples/rust/src/bin/live.rs) uses production sizes, reads the public index over HTTP, draws from the OS CSPRNG, then applies the section 15 refusals the way a gate does. `sdk/test.sh` compiles it but doesn't run it, because it needs the network.

<!-- from: sdk/examples/rust/src/bin/live.rs -->
```rust
let agent = hike::http_agent();
let index = "https://index.groundinglab.io"; // or your own pcoc-index
let m: hike::Measurements = hike::verify(
    &object,
    hike::AuditDraw::OsCsprng,
    |h| hike::read_index_beacon(&agent, index, h).map_err(|_| ()),
    |loc| match hike::index_document(&agent, index, u64::from(loc.block_height)) {
        Ok(doc) => hike::proxy_matches(&doc, loc).into(),
        Err(_) => hike::ChainRead::NotRead,
    },
);
print!("{}", hike::verify_record_json("production", &m));
// ...
if hike::admit(&m) {
    eprintln!("clear: no section 15 refusal and no held read");
} else {
    let facts: Vec<&'static str> = hike::failure_facts(&m);
    if facts.is_empty() {
        eprintln!("held: a read was not taken (for example the six-leg draw); retry later");
    } else {
        eprintln!("refused: {facts:?}");
    }
}
```

Notes:
- **`admit` and `failure_facts`.** They apply the section 15 invariant refusals only: earlier object kind, shape, checkpoint mismatch, seed value, plant mismatch, successor, and the six legs. They apply **no threshold**. A gate adds its own policy on top, such as a leg-rate floor or a minimum number of anchored paths. groundinglab's argued default is [POLICY.md](../../services/admission-gate/POLICY.md), a recommendation and not part of the spec. The [admission gate](../../services/admission-gate/) is a complete service built this way.
- **The index.** `https://index.groundinglab.io` is the public [pcoc-index](../../services/index/). It serves `GET /index/<height>` for the mainnet heights it retains, and answers **404** for any other height. The heights inside the `conformance-small` vectors (for example 1004) are not mainnet heights. Run on `kat-small`, this example therefore records every header read as `not read`, and the shape as `mismatch` (`constant`), because a small-profile object fails production sizes. Use a real production object.

## Produce

[`sdk/examples/rust/src/bin/produce.rs`](../examples/rust/src/bin/produce.rs) produces an object offline. It uses the `stage0-start.json` tip script at `conformance-small` sizes, with no plant service, and then verifies the result:

<!-- from: sdk/examples/rust/src/bin/produce.rs -->
```rust
let object: Vec<u8> = hike::produce_with_settlement_sized(
    public_key,
    hike::Sizes::conformance_small(), // Sizes::production() for a real PCOC
    // observe: the current tip. Called after every checkpoint.
    || {
        next.next()
            .ok_or_else(|| hike::ProduceFailure::new("tip script ended"))
    },
    // open_stage: post the waypoint (the plant) and return its path body.
    // Offline there is no plant service, so every path is empty bytes.
    |_waypoint: &[u8; 32], _beacon_height: u64| Ok(Vec::new()),
    // settle: may replace stage paths after the last leg. Nothing to do here.
    |_stages: &mut [hike::Stage], _cut_off: u64| Ok(()),
)
.expect("produce");
```

```sh
cargo run --release --bin produce -- ../../../spec/conformance/v2.0 2>/dev/null
```

<!-- output: produce -->
```text
object bytes: 3670
object sha256: da3acd20d063b7492e192b557fdf711fbe5761a49e446a6b2f664af17f72f765
shape: match
```

The Go and Java produce examples write the same object bytes from the same tips.

**A real produce run** uses `Sizes::production()`, reads tips from Bitcoin header gossip (`HeaderClock`, `GossipNetwork`), and posts each waypoint to the plant service through `open_stage` / `LivePathBook`. The plant service, Bari, is the service that plants waypoints in Bitcoin and serves their paths (spec section 13.1). For the full wiring, read `satoshi-hike-produce/src/main.rs` (the program) or the desktop app's `apps/desktop/core/src/produce/adapter.rs`. For what it costs (18 tip waits × the mean header interval, with 256 MiB held), see [What producing costs](../README.md#what-producing-costs).

## API overview

Everything below is re-exported at the crate root as `hike::…`. Generated docs: `cargo doc -p hike --open` in `sdk/rust`.

**Verify (section 14).**

```rust
pub fn verify<F, C, R>(bytes: &[u8], draw: AuditDraw, beacon_at: F, path_chain: C) -> Measurements
pub fn verify_sized<F, C, R>(bytes: &[u8], draw: AuditDraw, beacon_at: F, path_chain: C, sizes: Sizes) -> Measurements
where F: FnMut(u64) -> Result<IndexBeacon, ()>,
      C: FnMut(&PathLocators) -> R,
      R: Into<ChainRead>,          // Result<(), ()> converts: Ok = Match, Err = Mismatch
pub fn verify_record_json(profile: &str, measurements: &Measurements) -> String   // record bytes, trailing newline
pub fn verify_record_sha256(record: &str) -> Result<[u8; 32], String>             // JCS hash, newline excluded
pub enum AuditDraw { OsCsprng, PinnedSeed([u8; 32]) }
pub enum ChainRead { Match, Mismatch, NotRead }
```

- **`Measurements`.** Its fields are `version: ObjectLineage`, `shape: bool`, `checkpoints: Recorded<CheckpointMeasurement>`, `waypoints: Recorded<WaypointMeasurement>`, `leg_rate: LegRate`, `paths_included: Recorded<u64>`, `six_legs: SixLegs`, `stages: StageReport` and `record_body: String` (the record JSON, without the newline). `Recorded<T>` is `Obtained(T)` or `NotObtained`. A value that could not be measured is a variant such as `NotObtained` or `NotRead`; it is not an error.
- **Other verify entry points.** `verify_with_bind_threads[_sized]` sets the Rule A4.3 pool size, which does not change the record. `verify_fast[_sized]` and `screen_fast[_sized]` are the cheap phases a gate runs first.
- **Sizes.** `Sizes::production()` (section 6) and `Sizes::conformance_small()` (section 16.1).

**Objects.**

```rust
pub fn decoded_hike(bytes: &[u8]) -> Result<Hike, ()>
pub fn top_level_map(bytes: &[u8]) -> bool   // the first CBOR item decodes and is a map (trailing bytes allowed)
pub fn canonical_object(bytes: &[u8]) -> Result<bool, ()>
pub fn object_public_key(bytes: &[u8]) -> Result<[u8; 32], ()>
pub fn yielded_public_key(bytes: &[u8]) -> YieldedKey     // Undecodable | NoPublicKey | PublicKey([u8; 32])
pub fn earlier_object_kind(bytes: &[u8]) -> Option<String>
```

The other object functions are `encode_hike[_sized]` and `structural_object`. `Hike` holds `public_key`, `seed_beacon`, `stages: Vec<Stage>`, `last_interrupt` and `last_interrupt_height`. Each `Stage` holds `beacon`, `beacon_height`, `leg_count`, `waypoint`, `path`, `checkpoints` and `leg_results`.

**Index and plant.**

```rust
pub fn parse_index_beacon(body: &str) -> Result<IndexBeacon, ClockError>   // IndexBeacon { natural, height, header_time_s, timestamp_unix_ms }
pub fn parse_index_document(body: &str) -> Result<([u8; 32], u64), ClockError>
pub fn read_index_beacon(agent: &ureq::Agent, index_url: &str, height: u64) -> Result<IndexBeacon, String>
pub fn index_document(agent: &ureq::Agent, index_url: &str, height: u64) -> Result<String, String>
pub fn http_agent() -> ureq::Agent
pub fn path_locators(bytes: &[u8]) -> Result<PathLocators, PathError>        // PathLocators { root, txid, vout, block_hash, block_height }
pub fn proxy_matches(document: &str, loc: &PathLocators) -> Result<(), ()>    // plant check via an index document
pub fn direct_matches(facts: &DirectFacts, loc: &PathLocators) -> Result<(), ()>  // via a node; see hike_net::ChainRpc
```

**Produce (section 13).**

```rust
pub fn produce(public_key: [u8; 32], observe: O, open_stage: P) -> Result<Vec<u8>, ProduceFailure>
pub fn produce_with_settlement_sized(public_key: [u8; 32], sizes: Sizes, observe: O, open_stage: P, settle: S)
    -> Result<Vec<u8>, ProduceFailure>
where O: FnMut() -> Result<([u8; 32], u64), ProduceFailure>,        // the tip: (natural beacon, height)
      P: FnMut(&[u8; 32], u64) -> Result<Vec<u8>, ProduceFailure>, // (waypoint, beacon height) -> path body
      S: FnMut(&mut [Stage], u64) -> Result<(), ProduceFailure>,   // (stages, cut-off height)
```

Related produce APIs:
- **More entry points.** `produce_with_settlement` (production sizes) and `produce_stage`.
- **The header feed.** `HeaderClock`, `GossipNetwork`, `note_produce_headers` and `serve_synth`.
- **The plant (Bari) side.** `plant_stage`, `open_stage`, `LivePathBook` and `PathFetch`.

**Building blocks.**
- Functions: `fill_terrain[_sized]`, `production_seed`, `leg_result[_sized]`, `checkpoint_sized`, `successor_waypoint[_sized]`, `terrain_sample_sized`, `draw_distinct` and `effective_bits`.
- The section 6 constants: `KIND`, `STAGES`, `STEPS_PER_LEG`, `SAMPLE_BYTES`, …, `ANCHOR_WINDOW_OFFSETS`.

**Admission (section 15).** `admit(&Measurements) -> bool` returns true when there is no refusal and no held read. `failure_facts(&Measurements) -> Vec<&'static str>` names the refusals, and is empty when the record is clear or held. The informative §15.5 example floors are policy values, not SDK constants (rc.2): they live in `spec/conformance/v2.0/constants.json`.

**Events and reports.**
- `verify_events` turns one observation into `VerifyEvent`s: the object, then the six checks in section 14 order.
- `publish_verify` hands a record to a `VerifySubscriber`.
- `Report` and `emit_report` write the programs' `--report` JSON (`satoshi-hike-cli-report-v0.2`).

**nip06.**
- `from_mnemonic` and `create_demo_key` return a `DemoKey`.
- `DemoKey` has public fields `public_key` and `npub`, and the accessors `secret_key()`, `nsec()` and `mnemonic()`. Its `Debug` output shows public parts only, and it has no `Display`.
- `sign_bip340_message` and `verify_bip340_message` sign and check messages.

## Errors

| Type | Returned by | Cause |
|---|---|---|
| `PathError::Truncated` | `path_locators` | The body is shorter than its fixed 146 bytes, or a field runs past the end |
| `PathError::UnsupportedVersion(v)` | same | The first byte is not `0x01` |
| `PathError::TooManySiblings(n)` | same | More than 32 siblings |
| `PathError::WrongLength { actual, expected }` | same | The length is not 146 + 32 × the sibling count |
| `PathError::EmptyTree` | same, and path validation | `tree_size` is 0 |
| `PathError::LeafIndexOutOfRange { index, tree_size }` | path validation | `leaf_index` ≥ `tree_size` |
| `PathError::NonCanonicalSiblingCount { actual, expected }` | path validation | The sibling count is not the one `tree_size` implies |
| `PathError::WrongSelfSibling` | path validation | A lone last node is not paired with itself |
| `PathError::WrongLeaf` | path validation | The leaf is not the waypoint being checked |
| `PathError::WrongRoot` | path validation | The recomputed Merkle root differs from the stored root |
| opaque `ClockError` (not nameable as `hike::ClockError`; match it as `Err(_)`) | `parse_index_beacon`, `parse_index_document` | The body is not one JSON value with unique members; `header` is not an object; `header.hash` is not 64 hex digits; or `header.height` is not an unsigned integer |
| `ProduceFailure { message, http_status }` | `produce*`, `read_clock`, plant calls | A tip or plant call failed. `http_status` is set when there was an HTTP answer |
| `UsageError` | `parse_verify`, `parse_produce` | Bad command-line arguments (the programs exit 2) |
| `Err(())` | `decoded_hike` | The first CBOR item does not decode, is not a map, or has bytes after it (rc.2: trailing bytes are an error here too). A field that is absent or of the wrong type is not an error: that slot holds zero (an empty array for `checkpoints` or `leg_results`, an empty `path`), and `verify` records the condition. It is not a validity check: a hike with a `mismatch` shape is still returned. Use `verify` for the record |
| `Err(())` | `canonical_object`, `object_public_key` | The bytes are not exactly one well-formed CBOR item (trailing bytes included), or are not a hike map |
| `Err(())` | `proxy_matches`, `direct_matches` | The plant document or the node facts do not match the path |
| `None`, `YieldedKey::Undecodable`, `YieldedKey::NoPublicKey` | `earlier_object_kind`, `yielded_public_key` | Not a CBOR map or not an earlier kind; no 32-byte `public_key` field |
| `Err(String)` | `verify_record_sha256` | The record is not JSON |
| `Err(String)` | `read_index_beacon`, `index_document` | No HTTP answer, a non-200 answer, or (for a beacon) a body that `parse_index_beacon` rejects |
| `Err(String)` | `nip06::from_mnemonic`, `create_demo_key`, `sign_bip340_message`, `nip06::decode_hex32`, `hike_net::ChainRpc::facts`, `emit_report` | Not 12 English BIP-39 words, or a bad checksum; OS CSPRNG failure; the secret is rejected by secp256k1; bad hex; node RPC failure; the report file cannot be written |

`verify` itself never fails. Malformed input gives a record that says `mismatch` or `not defined`. Untrusted bytes never cause a panic: `tests/malformed_input.rs` feeds thousands of deterministic mutants of the pack objects, path bodies and index replies through every public decoder.

The error types are Rust's own. The Go and Java SDKs report the same conditions with different texts or values, and the diagnostic texts differ between SDKs; only the record bytes are conformance-tested. [The comparison table](#error-names-across-the-sdks) is below.

The programs exit 1 only when no top-level CBOR map decodes (`hike::top_level_map(bytes)` is false): the file cannot be read, the first item does not decode under section 7.3, or it is well-formed but not a map (for example the single byte `01`). In every other case, trailing bytes after a map included, they write the record and exit 0. Usage errors exit 2. `verify` itself gives the `["malformed cbor"]` record for every input the program refuses. Go and Java behave the same (rc.2).

**Nesting limit (rc.3, implementation resource limit).** The decoder accepts at most `hike::MAX_CBOR_NESTING` = 64 nested arrays and maps (an item counts the containers around it, itself included). A deeper item is malformed CBOR: the record is `["malformed cbor"]` and the programs exit 1. A valid section 8 hike object nests 4 deep (top-level map, `stages` array, stage map, `checkpoints`/`leg_results` array), so no valid object is refused. The value is the same in the Rust, Go and Java SDKs; see [KNOWN-DIVERGENCES.md](../KNOWN-DIVERGENCES.md#implementation-resource-limits).

### Error names across the SDKs

| Condition | Rust | Go | Java |
|---|---|---|---|
| Path body too short | `PathError::Truncated` | `truncated` | `PathBody.locatorsFor` returns `null` |
| Path version byte | `UnsupportedVersion` | `version` | `null` |
| Too many siblings | `TooManySiblings` | `siblings` | `null` |
| Wrong body length | `WrongLength` | `length` | `null` |
| Empty tree / leaf index / sibling count / self sibling / leaf / root | `EmptyTree` / `LeafIndexOutOfRange` / `NonCanonicalSiblingCount` / `WrongSelfSibling` / `WrongLeaf` / `WrongRoot` | `empty tree` / `leaf index` / `sibling count` / `self sibling` / `leaf` / `root` | `null` |
| Bytes after the first CBOR item | record `malformed cbor` | `trailing` | `Cbor.Problem`, `.issue` `"trailing"` |
| Reserved CBOR length code (28–30) | record `malformed cbor` | `float-or-simple` | `.issue` `"truncated"` |
| Text that is not UTF-8 | record `malformed cbor` | `malformed cbor` | `.issue` `"truncated"` |
| Top item not a map | record `malformed cbor` | `malformed cbor` | `.issue` `"not a map"` |
| More than 64 nested arrays and maps (rc.3) | record `malformed cbor`; `yielded_public_key` is `Undecodable` | `nesting depth` | `.issue` `"nesting depth"` |

## Conformance vectors

`spec/conformance/v2.0/` is the source of truth ([what each file is](../../spec/conformance/v2.0/README.md)). `hike/tests/conformance_v2.rs` reproduces every vector byte for byte. With `PCOC_REGEN_V2=1`, and only in a spec change, it rewrites them instead:

```sh
# from the repository root
PCOC_REGEN_V2=1 cargo test --manifest-path sdk/rust/hike/Cargo.toml --test conformance_v2 -- --exact conformance_v2_pack
git diff --exit-code spec/conformance/v2.0
```

The pack's own README still names the pre-move path `reference/rust/hike/Cargo.toml` in its regenerate commands, and its `stage0-start.json` paragraph repeats the `sign-in-origins.json` sentence. That file is covered by the `SPEC_PIN` hash, so both fixes wait for the next spec release; the command above is the current one. [../SPEC_PIN](../SPEC_PIN) holds the spec tag and the vector hash.

## Test

```sh
bash sdk/test.sh                                # every SDK, the cross-check, examples and docs (from the repo root)
cd sdk/rust && cargo test --locked --release    # this workspace only; ends with "test result: ok." lines and exit 0
```

## Programs

```sh
cd sdk/rust && cargo build --release --locked
./target/release/satoshi-hike-verify --input OBJECT --profile live --index-url https://index.groundinglab.io
./target/release/satoshi-hike-produce --help
```

`satoshi-hike-verify` takes the same flags as the Go and Java programs (see [the verify programs](../README.md#the-verify-programs)):
- The default `--profile` is `test`. It does not dial `https://index.groundinglab.io`; it reads `PCOC_TEST_INDEX_URL` when `--index-url` is omitted.
- `--profile conformance-small` requires `--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N`. The record states those integers. Any of the four with another profile is a usage error.
- Without `--chain-rpc`, the plant check reads the index document (`proxy_matches`). With `--chain-rpc` (live profile only), it reads the node (`hike-net`).

`satoshi-hike-produce` has flags of its own: `--demo-mnemonic`, `--report`, `--clock-source`, `--gossip-network`, and the profiles `test|live|slow-lan`. See its `--help`.
