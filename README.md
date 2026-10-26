# Production-constrained observable commitment (PCOC)

**Early public release.** Specification pin [`spec-2.3.0`](sdk/SPEC_PIN) (Satoshi Hike 2.3). The SDKs are 2.3.1. Released component tags are in [DEPLOYABLES](DEPLOYABLES.md). Website: https://groundinglab.io

## What it is

Open systems give identifiers away. A new key costs nothing, so a crowd of them costs the same as one. Filters and scores then try to tell the crowd apart.

A production-constrained observable commitment (PCOC) is an identifier you issue yourself, on your own device, with no registrar to ask. What limits it is the work of making it: memory you hold across a hike, paced by Bitcoin blocks. The bytes you keep are the PCOC object (the specification's hike object). Anyone can measure that object. Copying an object is cheap. The design aims to make each additional object require its own hike, without giving bulk production a discount.

An identifier is not an identity. A PCOC says nothing about who made it.

## Why detection loses

A detection rule is solved once. Every account an attacker runs can reuse that solution. The attacker's cost can be amortised across accounts. Services and legitimate users continue to bear the costs of screening. Volume is not aggression, so a tighter rule also catches people who simply show up a lot.

Most of that starts at issuance. Identifiers are either free, which floods, or handed out by an office, which installs a gatekeeper. A PCOC is a third path: you issue your own, and the cost sits in the making. The longer version is [Where does the trouble start?](docs/briefs/where-does-the-trouble-start.md).

## Three steps

Create, check, and admit are separate. A policy can change while the object stays the same.

1. **Create (the hike).** Your machine fills a terrain of 256 MiB, fixed by your public key and a Bitcoin block, then walks it in legs of two million steps. Satoshi Hike 2.3 records eighteen stages. Bitcoin blocks pace those stages. They are a public clock. Bari timestamps each tip's pooled waypoints in one Bitcoin transaction. Bitcoin's proof-of-work does not secure a PCOC and does not pay for it. A full hike is about three hours, and it varies with the blocks.

2. **Check (verify).** A verifier recomputes from the object and writes a measurement record. The words are measurements. The closed set defined in §4 is `match`, `mismatch`, `present`, `skip`, `not defined`, `not read`, `not fetched by design`, `n/a`, `anchored`, `not anchored`, and `unknown kind`. The record does not say pass or fail. The public index stands in for Bitcoin. Anyone can skip the index and read Bitcoin. Which programs in this tree actually fetch an index is under [Measure an object](#4-measure-an-object).

3. **Admit (policy).** Accept, refuse, and revoke belong to each relying party. groundinglab's Admission Gate is one configured instance. Its current example is policy id `gl-admit-v12`, written in [policy/groundinglab-admission/POLICY.md](policy/groundinglab-admission/POLICY.md). The specification sets no lever value.

The Bitcoin tip (the newest block's hash and height) is the shared tick that paces the staged work. That is its role: a clock, not a fee, not a confirmation count, and not an archive of the chain. The hike is the quantity of work.

## Versions that move separately

Three names are versioned on their own. Moving one does not move the others.

| Name | What it pins | In this tree |
|---|---|---|
| SPEC | Satoshi Hike, the normative pack | tag `spec-2.3.0`, named by [`sdk/SPEC_PIN`](sdk/SPEC_PIN) |
| KIND | The wire label inside the object | `v2.2-pcoc-object` |
| Policy | One relying party's lever values | `gl-admit-v12`, groundinglab's current example |

Satoshi Hike 2.3 / `spec-2.3.0` is the specification pin. The object kind on that pin is `v2.2-pcoc-object`. There is no `v2.3-pcoc-object` kind. The digits in `pcoc-walk-v1`, `bari-hc-v2`, `pcoc-gate-signin-v1`, and the `/v1` routes are wire domains, not specification versions. The gate's own note is [services/admission-gate/WIRE.md](services/admission-gate/WIRE.md).

Health follows the SDK pin. A build on `spec-2.3.0` reports `spec` Satoshi Hike 2.3. A new decision-log line and `/latest.json` carry that same string. A log line stamped `Satoshi Hike 2.2` still opens on this pin, and that line is not rewritten.

The conformance pack id is `pack-2.2.0`. That id is historical. The bytes are valid for the 2.3 pin, and the id was left unchanged so the manifest hash beside `spec-2.3.0` stays put. SDK 2.3.1 ([`sdk/VERSION`](sdk/VERSION)) implements that pin. An SDK version is not a specification version.

## What this does not claim

- Bitcoin's proof-of-work does not secure a PCOC or pay for one. Bitcoin is the public clock. Bari is the timestamp channel.
- It does not make fakes impossible. The design aims to make each additional object require its own hike. How that holds at scale is unmeasured ([STATUS](STATUS.md)).
- It is not a token, coin, currency, or payment.
- There is no issuer, account, or identity provider, and it says nothing about personhood.
- Verify measures. Admission is each relying party's own decision.
- groundinglab's index is a convenience. Only hash-bound values are evidence.
- The public key and its Bitcoin-timed record are public. The specification states the privacy claim, and there is nothing beyond it.
- Not affiliated with Bitcoin, its developers, or its creator. "Satoshi Hike" is named for the block clock it keeps pace with.
- Economic security, catch rates, and deployed cost are open research.

## How the repository is laid out

The specification and the conformance pack are what you build from. Anyone can write a producer, a verifier, a service, or an app from those two alone, with no groundinglab code, service, or account. Everything else pins a version of the layer under it. Where an SDK and the specification disagree, the specification wins.

| Layer | Path | Role |
|---|---|---|
| Specification | [spec/](spec/) | Satoshi Hike 2.3: six documents plus [GUIDE](spec/GUIDE.md). Normative |
| Conformance | [spec/conformance/pack/](spec/conformance/pack/) | The one pack, id `pack-2.2.0`, status complete. The id stays 2.2 on the `spec-2.3.0` pin. Hash in `sdk/SPEC_PIN` |
| SDKs | [sdk/](sdk/) | Rust, C, Go, and Java at 2.3.1, tested on the shared vectors |
| Policy | [policy/groundinglab-admission/](policy/groundinglab-admission/) | groundinglab's example levers. Current id `gl-admit-v12`. Not the specification |
| Services | [services/](services/) | Bari (the plant), the index, the Admission Gate, the claims service, and a Cloudflare Worker. Each builds alone |
| Apps | [apps/](apps/) | Linux desktop demonstrator. Android notes are in [apps/android](apps/android/); the app is built from [groundinglab/primal-pcoc](https://github.com/groundinglab/primal-pcoc) |
| Docs | [docs/](docs/README.md) | Briefs and rationale. Explanatory, not normative |
| Your own deployment | [docs/run-under-your-own-name.md](docs/run-under-your-own-name.md) | Where a builder changes hosts to run the pieces under their own name |

At the top level: [STATUS](STATUS.md) (specified, measured, open), [DEPLOYABLES](DEPLOYABLES.md) (released components and their tags), [PRINCIPLES](PRINCIPLES.md), [CONTRIBUTING](CONTRIBUTING.md), [SECURITY](SECURITY.md), and the licences. CI is in `.github/`. Instructions for a coding agent are in [ai/README.md](ai/README.md).

## Get started

From a clone to a measurement on your own machine. The paths below match this tree. The sequence was written for Debian 13 (x86-64). The desktop app's library notes also name Ubuntu 24.04 and Fedora 40. Other systems are untested with these exact steps.

For the ideas behind the steps, see [How it works](https://groundinglab.io/how-it-works/) and [Technical](https://groundinglab.io/technical/).

### 1. Get the code

The code is public at https://github.com/groundinglab/pcoc. Clone the full history:

```sh
git clone https://github.com/groundinglab/pcoc.git
cd pcoc
```

Use a full clone. `sdk/SPEC_PIN` names `spec-2.3.0`, and the pin check looks that tag up. A shallow clone (`--depth 1`) has no tags. If you already have one, run `git fetch --tags`.

### 2. Install the tools

| Tool | Version | Needed for |
|---|---|---|
| Python | 3 | the specification checks |
| Rust, from [rustup](https://rustup.rs) | 1.99.0 | the Rust SDK, the services, and the desktop app |
| Go | 1.24 or newer | the Go SDK (`sdk/go/go.mod`) |
| JDK | 21 or newer | the Java SDK |
| GTK and libadwaita | GTK 4.14+, libadwaita 1.5+ | the desktop app only ([apps/desktop](apps/desktop/)) |

`sdk/rust/`, `services/admission-gate/`, and `apps/desktop/` each pin Rust 1.99.0 in a `rust-toolchain.toml`. rustup installs that version on the first `cargo` command in those folders. `services/bari/`, `services/index/`, and `services/admission-claims/` have no pin, so set a default as well:

```sh
rustup default 1.99.0
```

Without a default, those three folders stop with "rustup could not choose a version of cargo to run".

### 3. Run the tests

An SDK is the library you add to your own program to make or measure a PCOC. The Rust workspace is `sdk/rust/`:

```sh
cd sdk/rust
cargo test -p pcoc --test ac_a3 --locked
cargo test --workspace --locked
bash check/negatives.sh
```

Go, Java, and C:

```sh
(cd sdk/go && go test ./...)
(cd sdk/java && ./gradlew test)
(cd sdk/c && make check)
```

The specification's own checks (section 6 constants, the sign-in wire vectors, links, and the vector manifest hash):

```sh
python3 spec/conformance/tools/ring1_checks.py
```

Run that from the repository root.

Each Rust service builds and tests on its own. From `services/admission-gate`, `services/bari`, `services/index`, or `services/admission-claims`:

```sh
cargo test --locked --release
```

The claims Worker, from `services/cf-claims-worker`:

```sh
node --test test/
```

The desktop app's core tests do not need GTK:

```sh
cd apps/desktop
cargo test --locked -p pcoc-hike-core
```

### 4. Measure an object

`pcoc verify` writes one measurement record to stdout and nothing else. Exit 0 means a record was written, including a record full of `not read` or an unknown-kind record. It does not mean the object was admitted.

The commands in this section are the Rust binary `sdk/rust/target/release/pcoc`. The C, Go, and Java programs are separate, and each prints its own usage.

```sh
(cd sdk/rust && cargo build --locked --release -p pcoc-cli)
```

This binary does not take `--index-url`, and it does not open a network connection. It measures with an empty index, so header and plant reads are `not read`. `--profile production` is the default. `--profile live` and `--profile test` are accepted. The record's `profile` field is still `production`, the sizes are still section 6, and the index is still empty. The specification says `--profile live` selects the live index. This binary does not do that.

A fast check of a current-kind object uses the pack fixture `sdk/rust/objects/O1-S1.cbor` (kind `v2.2-pcoc-object`) on the small profile. `--draw-label` is the pack's rig input, so the six-leg rows repeat. Stderr adds one line, `draw: injected label (rig input)`. Omit the flag and that draw reads the operating-system random source.

```sh
sdk/rust/target/release/pcoc verify \
  --profile conformance-small \
  --sample-bytes 32 --slot-bytes 64 --terrain-bytes 65536 --steps-per-leg 100 \
  --draw-label pcoc-pack-2.2/draw \
  --input sdk/rust/objects/O1-S1.cbor
```

`pcoc quickstart`, run from the repository root, checks the pin, measures that same object, and prints six lines:

```sh
sdk/rust/target/release/pcoc quickstart
```

`pcoc --version` prints the SDK version, the spec tag, and the manifest hash.

The file at https://groundinglab.io/verify/groundinglab.pcoc.cbor is still served. Its kind text is `satoshi-hike-v2.0-object`. This binary records an unknown kind (Rule A5) and does not measure the hike:

```json
{"kind":"unknown kind","kind_text":"satoshi-hike-v2.0-object"}
```

That record is not a measurement of a current-kind object. The page at https://groundinglab.io/verify/ is a separate deploy.

A live read of `GET {index-url}/index/{height}` is in two programs here. The desktop app's Verify uses it, and the default URL is `https://index.groundinglab.io`. The Admission Gate uses the `index_url` in its config. Build and run steps for the app are in [apps/desktop/README.md](apps/desktop/README.md).

Java's `pcoc verify` can read a directory of `{height}.json` files already on disk (`--index DIR`). That flag is not a URL, and those names are not the index service's `height-XXXXXXXXXX.json` files. The conformance drivers under `sdk/conformance/` take `--index-url` for a local test server (the Rust driver accepts only `http://127.0.0.1:PORT`). They are the harness, not the `pcoc` command.

`sdk/rust/objects/O3-PROD.cbor` is a current-kind object at the section 6 sizes: a 256 MiB terrain and two million steps a leg. Measuring it is the production-sized measure:

```sh
sdk/rust/target/release/pcoc verify --profile production --input sdk/rust/objects/O3-PROD.cbor
```

### 5. Make a PCOC

A live hike is the eighteen stages above, about three hours. The desktop app runs one on your machine, with your key and Bitcoin's live blocks. See [apps/desktop/README.md](apps/desktop/README.md) and [Make a PCOC](https://groundinglab.io/make/). The Android app is built from [groundinglab/primal-pcoc](https://github.com/groundinglab/primal-pcoc).

The Rust CLI can also produce. The scripted rig is offline:

```sh
sdk/rust/target/release/pcoc produce --rig --case s1 --public-key HEX --out object.cbor
```

`--case` is `s1`, `s2`, or `prod`. `HEX` is 64 hex digits, with an optional `0x` prefix. Progress is JSON on stderr. The object is the `--out` file.

A live produce dials Bitcoin and posts waypoints:

```sh
sdk/rust/target/release/pcoc produce --live --public-key HEX --out object.cbor
```

`--gossip-peer HOST:PORT` and `--bari URL` are optional. With no peer, the client uses the Bitcoin Core DNS seeds. With no `--bari`, it posts to `https://bari.groundinglab.io`. Resume is not supported. A hike that stops before it closes writes no object.

## Run it yourself

Hosted names are opt-in. `https://index.groundinglab.io` and `https://bari.groundinglab.io` are a convenience. The Rust `pcoc verify` command does not call either of them.

Live produce uses the hosted Bari when you omit `--bari`, as in the command above. Pass your own URL and it posts there.

The index is its own program ([services/index](services/index/)). It serves one JSON file per block height. Three ways to have one, each opt-in:

1. Use the hosted index. The desktop app does this with its default URL. A gate uses the `index_url` its config names.
2. Rebuild the files from a public block API. No Bitcoin node:

```sh
cd services/index
cargo build --locked --release
./target/release/pcoc-index rebuild --source esplora --from-height N --to-height M --out DIR
```

Esplora is the default source. The base URL defaults to `https://mempool.space/api`. Pacing, the header floor, and `--source rpc` are in [services/index/README.md](services/index/README.md).

3. Rebuild from a pruned Bitcoin Core (`--source rpc`), or run Bari and let it write the same files. A node is what planting needs. Serving an index, and rebuilding one, do not.

A snapshot or Nostr cache of those files is not built.

Serve a directory you rebuilt with `pcoc-index` (default bind `127.0.0.1:38081`, data directory `PCOC_INDEX_DATA_DIR`). Point a desktop app or a gate at that origin. The `pcoc verify` command will not fetch it.

## Where next

- **Public checker:** https://groundinglab.io/verify
- **Make one:** [desktop app](apps/desktop/)
- **Build one:** [spec](spec/GUIDE.md) → [conformance pack](spec/conformance/pack/) → [SDKs](sdk/)
- **Read why:** [briefs](docs/briefs/README.md) and [rationale](docs/README.md)
- **What is measured, and what is open:** [STATUS](STATUS.md)
- **Contribute, or report a vulnerability:** [CONTRIBUTING](CONTRIBUTING.md) · [SECURITY](SECURITY.md)

## Built on

Satoshi Nakamoto's timestamp server and attack economics (Bitcoin white paper §3, §4, §6, §11), Haber and Stornetta's linked timestamps, Dwork and Naor's pricing via computation and Adam Back's Hashcash, and memory-hard work (scrypt). The composition is ours.

## Licence

Code: Apache-2.0 ([LICENSE](LICENSE)). Specification and docs prose: CC BY 4.0 ([LICENSE-DOCS.md](LICENSE-DOCS.md)). Conformance vector bytes: Apache-2.0 ([LICENSE](LICENSE)).
