# PCOC

**Early prototype.** Specification: Satoshi Hike 2.2 (`spec-2.2.0`, cut by the maintainers on the merge commit). Website: https://groundinglab.io

A PCOC (production-constrained observable commitment) is a self-issued identifier backed by a resource commitment, not a payment. You make it on your own device by holding memory through a hike, the timed making process, paced by Bitcoin blocks. The result is the PCOC object (the specification's hike object): the bytes you keep and present, holding your public key and the record of the hike. Anyone can check the PCOC object cheaply, without trusting you or us. The design aims to give bulk making no discount: copying the PCOC object is cheap, but a new public key means a new hike.

An identifier is not an identity: a PCOC says nothing about who made it.

## Why detection loses

Open systems usually defend against bots and fake accounts by detection: rules, filters, scores. A rule is solved once and the solution is reused by every account the attacker runs. The attacker pays once; the defence pays per user, forever. And volume is not aggression, so tightening the rule punishes heavy honest users along with the crowd.

Most of that trouble starts at issuance. Identifiers are either free and unlimited, which floods, or handed out by an office, which installs a gatekeeper. PCOC takes a third path: you issue your own, and the cost is in the making.

## What "self-issued" means

A PCOC is self-issued: you create it on your own device from your own key, with no issuer, registrar, or account to ask. What limits it is the resource commitment you make in producing it (memory held across a Bitcoin-paced hike), not a payment or anyone's permission. See [Where does the trouble start?](docs/briefs/where-does-the-trouble-start.md).

## Three separate steps

1. **Create (the hike).** Your computer fills a block of memory fixed by your public key and a Bitcoin block, then works through it in legs of two million steps, each ending in a leg result. Bitcoin blocks pace the stages: Satoshi Hike 2.2 records eighteen. The cost is your own device's work. Bitcoin only timestamps it: Bari records each tip's pooled waypoints in one Bitcoin transaction. Bitcoin's proof-of-work does not secure a PCOC or pay for it.
2. **Check (verify).** A verifier recomputes from the PCOC object and the public index and records measurements: `match`, `mismatch`, `present`, `skip`, `not defined`, `not read`, `not fetched by design`, `n/a`, `anchored`, `not anchored`, and `earlier version`. It never says pass or fail. The index stands in for Bitcoin; anyone can bypass it by checking Bitcoin directly.
3. **Admit (policy).** Accept, refuse, and revoke are each relying party's own decisions. groundinglab's Admission Gate is one configured instance. Its policy is [policy/groundinglab-admission/POLICY.md](policy/groundinglab-admission/POLICY.md); the specification sets no lever value. Policy can change without changing the PCOC object.

The Bitcoin tip (the newest block's hash and height) is the shared public tick that paces the staged work. That is the whole of its role: not a fee, not a confirmation count, and not an archive of the chain. The hike is the quantity of work.

## What this does not claim

- Bitcoin's proof-of-work does not secure or pay for a PCOC. Bitcoin is a public clock and timestamp, via Bari.
- It does not make fakes impossible. It raises the cost of each one above the near-free price of a new key. How that cost behaves at scale is still being measured ([STATUS](STATUS.md)).
- It is not a token, coin, currency, or payment.
- There is no issuer, account, or identity provider, and it says nothing about personhood.
- Verify does not judge; it measures. Admission belongs to each relying party.
- You do not need to trust groundinglab's index. Only hash-bound values are evidence.
- No privacy claim beyond the specification: the public key and its Bitcoin-timed record are public.
- Not affiliated with Bitcoin, its developers, or its creator. "Satoshi Hike" is named for the Bitcoin block clock it keeps pace with.
- Economic security, catch rates, and deployed cost are open research.

## How the repository fits together

The specification and the conformance vectors are the core deliverable. The point is interoperability. Everything else is built from them and pins a version of the layer below. Anyone can build a producer, a verifier, a service, or an app from the specification and the vectors alone, with no groundinglab code, service, or account.

| Layer | Folder | Role |
|---|---|---|
| Specification | [spec/](spec/) | The complete, normative definition of how a PCOC is made and checked |
| Conformance | [spec/conformance/pack/](spec/conformance/pack/) | The one pack. Incomplete until the SDK phase |
| SDKs | [sdk/](sdk/) | Rust, Go, and Java SDKs, tested against the shared vectors and cross-checked against each other. The specification wins if they disagree |
| Services | [services/](services/) | Bari (the plant), the index (its own reader), and the Admission Gate, built on the Rust SDK |
| Apps | [apps/](apps/) | Desktop demonstrator, and an Android demonstrator described in [apps/android](apps/android/) |
| Docs | [docs/](docs/README.md) | Plain-language briefs and rationale. Explanatory, not normative |
| Your own deployment | [docs/run-under-your-own-name.md](docs/run-under-your-own-name.md) | Every place a builder changes to run the pieces under their own name |

## Get started

This section takes you from no code to a checked PCOC on your own machine: get the code, install the tools, run the tests, and verify a real PCOC. Every command below was run on Debian 13 (x86-64). Other systems have not been tried with these steps.

For the ideas behind each step, see [How it works](https://groundinglab.io/how-it-works/) and [Technical](https://groundinglab.io/technical/) on groundinglab.io.

### 1. Get the code

The code is public at https://github.com/groundinglab/pcoc. Make a full clone:

```sh
git clone https://github.com/groundinglab/pcoc.git
cd pcoc
```

Don't use `--depth 1`. `sdk/SPEC_PIN` names `spec-2.2.0`. The maintainers cut that tag on the merge commit. The pin check looks that tag up in the clone, and a shallow clone has no tags. If you already have a shallow clone, run `git fetch --tags`.

### 2. Install the tools

| Tool | Version | Needed for |
|---|---|---|
| Python | 3 | the test harness and its checks |
| Rust, installed with [rustup](https://rustup.rs) | 1.99.0 | the Rust SDK, the services and the desktop app |
| Go | 1.22 or newer | the Go SDK |
| JDK | 21 or newer | the Java SDK |
| GTK and libadwaita | GTK 4.14+, libadwaita 1.5+ | the desktop app only ([apps/desktop](apps/desktop/)) |

`sdk/rust/`, `sdk/examples/rust/`, `services/admission-gate/` and `apps/desktop/` each pin Rust 1.99.0 in a `rust-toolchain.toml`, and rustup installs it on the first `cargo` command there. The other services (`services/bari/`, `services/index/` and `services/admission-claims/`) have no pin yet, so give rustup a default as well:

```sh
rustup default 1.99.0
```

Without a default, those folders stop with "rustup could not choose a version of cargo to run".

### 3. Run the tests

An SDK (software development kit) is the library you add to your own app or service to make or check a PCOC. The conformance pack is [spec/conformance/pack/](spec/conformance/pack/). It is incomplete until the SDK phase. One script tests all three SDKs against them, and against each other:

```sh
bash sdk/test.sh
```

It runs seven steps (version pins, Rust, Go, Java, a cross-check of the three verifiers, the examples, and the docs) and ends with `sdk/test.sh: PASS`. On an 8-core machine it took about four minutes, most of it compiling Rust and the cross-check. [sdk/README.md](sdk/README.md#one-harness) describes each step.

To run only the specification's own checks (section 6 constants, sign-in wire vectors, links and the vector manifest hash):

```sh
python3 spec/conformance/tools/ring1_checks.py
```

Each service builds and tests on its own:

```sh
cd services/admission-gate   # or bari, index, admission-claims
cargo test --locked --release
```

The desktop app's core logic tests without GTK:

```sh
cd apps/desktop
cargo test --locked -p pcoc-hike-core
```

### 4. Verify a real PCOC

To verify is to measure a PCOC against Bitcoin's public block record, read through an index (a read-only copy of the block data a verifier needs, one entry per block height). The verifier writes a measurement record: JSON, with values such as `match` and `anchored`. It never says pass or fail; deciding who gets in is a service's policy.

Download the PCOC object that groundinglab made, build the Rust verify program, and run it:

```sh
curl -fsSLO https://groundinglab.io/verify/groundinglab.pcoc.cbor
(cd sdk/rust && cargo build --locked --release -p satoshi-hike-verify)
sdk/rust/target/release/satoshi-hike-verify --input groundinglab.pcoc.cbor --profile live > record.json
```

`--profile live` reads the hosted index at `https://index.groundinglab.io`. To use your own index, add `--index-url URL` (see [Run it yourself](#run-it-yourself)). The check took under a minute here, and `record.json` started with `{"profile":"production","shape":"match",...`.

The same check as a short program, which also applies the specification's section 15 refusals the way a gate does:

```sh
cd sdk/examples/rust
cargo run --locked --release --bin live -- ../../../groundinglab.pcoc.cbor
```

It prints the record and then `clear: no section 15 refusal and no held read`. The [verify page](https://groundinglab.io/verify/) does the same check in a browser.

### 5. Make a PCOC

Making a PCOC takes 18 Bitcoin blocks, typically around three hours, but it varies. The desktop app makes one on your own machine with your own key, with Bitcoin's live blocks as the clock. Build and run steps are in [apps/desktop/README.md](apps/desktop/README.md), and the plain-language guide is [Make a PCOC](https://groundinglab.io/make/). The Android app is built from [groundinglab/primal-pcoc](https://github.com/groundinglab/primal-pcoc).

To produce one in code, the produce examples in [sdk/examples/](sdk/examples/) run offline at the small test sizes and finish in under a second.

## Repository map

| Folder | What is in it | Test it with |
|---|---|---|
| [spec/](spec/) | The Satoshi Hike 2.2 specification and its conformance pack | `python3 spec/conformance/tools/ring1_checks.py` |
| [sdk/](sdk/) | The Rust, Go and Java SDKs, runnable examples in each language, and the harness tools | `bash sdk/test.sh` |
| [services/](services/) | Bari (the anchoring service), the index, the Admission Gate, the claims service, and a Cloudflare Worker. Each builds alone | `cargo test --locked --release` in each Rust service; `node --test test/` in `cf-claims-worker` |
| [apps/](apps/) | The desktop app (Linux), and notes and checks for the Android app | `cargo test --locked -p pcoc-hike-core` in `apps/desktop` |
| [docs/](docs/README.md) | Plain-language briefs, rationale, architecture and integration notes. Explanatory, not normative | |
| [.github/](.github/) | CI (self-hosted runners), its scripts, and issue templates | |

At the top level: [STATUS](STATUS.md) (what is specified, measured and open), [DEPLOYABLES](DEPLOYABLES.md) (what is released and how it is tagged), [PRINCIPLES](PRINCIPLES.md), [CONTRIBUTING](CONTRIBUTING.md), [SECURITY](SECURITY.md) and the licences.

## Run it yourself

Hosted services are opt-in. `https://index.groundinglab.io` and `https://bari.groundinglab.io` are a convenience, not a dependency. A verifier takes `--index-url`. Omit it on the live profile and the SDK verify program uses the hosted index. Pass your own and it does not.

The index is its own component ([services/index](services/index/)). Three ways to have one, each opt-in:

1. Use the hosted index.
2. Rebuild the files from a public block API. No node: `pcoc-index rebuild --source esplora --from-height N --to-height M --out DIR`, then serve that directory.
3. Rebuild from a pruned Bitcoin Core (`--source rpc`), or run Bari and let it write the files. A node is what planting needs. Serving and rebuilding an index do not.

## Where next

- **Check one:** https://groundinglab.io/verify
- **Make one:** [desktop app](apps/desktop/)
- **Build one:** [spec](spec/) → [conformance](spec/conformance/pack/) → [SDKs](sdk/)
- **Read why:** [briefs](docs/briefs/README.md) and [rationale](docs/README.md)
- **What is measured:** [STATUS](STATUS.md)
- **Contribute / report a vulnerability:** [CONTRIBUTING](CONTRIBUTING.md) · [SECURITY](SECURITY.md)

## Built on

Satoshi Nakamoto's timestamp server and attack economics (Bitcoin white paper §3, §4, §6, §11), Haber and Stornetta's linked timestamps, Dwork and Naor's pricing via computation and Adam Back's Hashcash, and memory-hard work (scrypt). The composition is ours.

## Licence

Code: Apache-2.0 ([LICENSE](LICENSE)). Specification and docs prose: CC BY 4.0 ([LICENSE-DOCS.md](LICENSE-DOCS.md)). Conformance vector bytes: Apache-2.0 ([LICENSE](LICENSE)).
