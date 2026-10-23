# services

These processes are built on the Rust reference, plus one Cloudflare Worker. They add no Part A rules. Verify does not say pass or fail.

## Specification, policy, operations

- **Specification** (`spec/`) defines measurements and the admission service model. It may discuss thresholds but never sets a threshold value.
- **Policy** chooses values. The groundinglab policy is [policy/groundinglab-admission/](../policy/groundinglab-admission/). It does not change a SPEC rule. Anyone may choose their own, and a policy id encodes its values.
- **Operations** is a deployed instance's own config: hosts, origins, the active policy, secrets. groundinglab's is private and is not in this repository. Files here that name groundinglab hosts (`deploy/`, `wrangler.jsonc`) are examples of one deployment.

## What is here

| Folder | What it is | Generic or groundinglab-specific |
|---|---|---|
| [`bari/`](bari/) | Bari: takes waypoints, seals each tip's pool into one Bitcoin transaction, serves Merkle paths. The plant fee buys position, not the hike's work. | Generic service |
| [`index/`](index/) | Portable index: one JSON file per block height. A read model of Bitcoin headers and Bari plants. Anyone can rebuild it or bypass it. | Generic service; groundinglab's host is opt-in |
| [`admission-gate/`](admission-gate/) | Admission gate service (§15): sign-in door, two-phase admission, public decision log. Applies a configured policy. | Generic service; groundinglab runs one instance |
| [`admission-claims/`](admission-claims/) | Issues groundinglab's admission badge (Nostr award) for accepted sign-ins, citing the gate log. | groundinglab extension |
| [`cf-claims-worker/`](cf-claims-worker/) | Cloudflare Worker: GET-only reader of the claims bucket (`/claims`, `/badge`, `nostr.json`). | groundinglab deployment; copyable |

The gate monitor page has no code in this repository.

## Layout

Each Rust folder is its own crate with its own `Cargo.toml`, `Cargo.lock` and `deploy/` (a systemd unit and an `env.example`). `cf-claims-worker/` is a JavaScript Worker with `wrangler.jsonc`. There is no shared workspace. Bari builds from its own directory. The index also checks out `services/bari` and depends on it by path. The gate also checks out `sdk/rust` and `policy/groundinglab-admission`, and depends on the Rust SDK crate `pcoc` by path. Claims has no SDK dependency.

```bash
cd services/<service>
cargo build --locked --release
cargo test --locked
```

Every service binds to loopback by default. To serve one publicly, put a reverse proxy of your choice in front of it. Nothing here names a host.

| Folder | Process | Default bind | Depends on | Release tag |
|---|---|---|---|---|
| [`bari/`](bari/) | `bari` (crate `pcoc-bari`) | `127.0.0.1:38080` | in-repo header source; live mainnet gossip is in the default release build | `bari-v0.6.9` |
| [`index/`](index/) | `pcoc-index` | `127.0.0.1:38081` | `pcoc-bari` by path (`../bari`) | `index-v0.1.5` |
| [`admission-gate/`](admission-gate/) | `admission-service` (crate `pcoc-admission`) | `127.0.0.1:8787` | `pcoc` from the in-tree Rust SDK, by path (`sdk/rust/pcoc`; version in `sdk/VERSION`) | `gate-v0.5.7` |
| [`admission-claims/`](admission-claims/) | `admission-claims` (crate 0.1.0) | `127.0.0.1:8793` | none in this repository (reads the gate at `127.0.0.1:8787` by default) | none yet |

Release tags are `<service>-v<x.y.z>` and point at the commit that ships that release. The table's Release tag column is the release that matches the working tree.

## Bari

[bari/](bari/) takes 32-byte waypoints (`POST /waypoint`, Hashcash 21 leading zero bits over `bari-hc-v2`, the public origin, and `X-Bari-Hashcash-Header`) into the pool for the live tip and serves Merkle paths (`GET /path/{leaf}`). `GET /ops` is the operator page. It writes the index cache and does not serve `GET /index`. On the live plane the tip comes from Bitcoin gossip (`BARI_TIP_SOURCE=bitcoin-gossip`, peer `PCOC_LIVE_GOSSIP_PEER`, which this tree does not fill). A bad, dropped, or silent peer is replaced. The tip moves only when two valid peers agree. The plant is Bitcoin Core. On the test plane the tip comes from the synth sequencer (`BARI_TIP_SOURCE=synth`) and the commit is simulated. See [bari/README.md](bari/README.md).

## Index

[index/](index/) is its own service. It serves the portable-index files: `GET /index` (tip), `GET /index/{height}`, and `GET /index/since` (NDJSON catch-up). `GET /health` reports this build's version and commit. It is not an index document. Bari can write those files. `pcoc-index rebuild` can write them from a public block API, or from a pruned node, with no plant and no wallet RPC. A verifier may read `GET {index-url}/index/{beacon_height}`. `https://index.groundinglab.io` is opt-in. This tree's `pcoc verify` does not dial a live index. See [index/README.md](index/README.md).

## Admission Gate

[admission-gate/](admission-gate/) is the admission gate service. groundinglab runs one configured instance of it. It checks a Satoshi Hike object and reads one TOML config file. Thresholds come from the configured policy ([policy/groundinglab-admission/](../policy/groundinglab-admission/)).

- `POST /v1/sign-in` is the sign-in door. The body is the §15.1 six-field object. The website reads `GET /v1/sign-in/result/{nonce}`. Any kind other than `v2.2-pcoc-object` is `unknown_kind` from the fast phase. The website token is `rejected`.
- `POST /v1/verify` measures an object. It does not check a signature and does not admit.
- `GET /latest.json` and `GET /log/{YYYY-MM-DD}.jsonl` are the public decision log.
- `GET /v1/health` reports version, commit, the Rust SDK version it was built with (`sdk`), spec kind and the config.

The code default for `allowed_origins` is empty (loopback only). A live groundinglab deploy needs `https://groundinglab.io` and `https://www.groundinglab.io` there; the shipped `deploy/gate.toml` sets them. Install is [deploy/DEPLOY.md](admission-gate/deploy/DEPLOY.md). Request and response bodies are in [WIRE.md](admission-gate/WIRE.md).

## Admission claims

[admission-claims/](admission-claims/) issues groundinglab's admission badge (`POST /badge`). The award cites the gate's decision-log line. It is an extension built on the gate, not part of §15. See [admission-claims/README.md](admission-claims/README.md).

## Claims Worker

[cf-claims-worker/](cf-claims-worker/) serves the published claims and badges from R2. It writes nothing. See [cf-claims-worker/README.md](cf-claims-worker/README.md).
