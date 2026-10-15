# services

These processes are built on the Rust reference. They add no Part A rules. Bari timestamps waypoints by sealing each tip's pool into one Bitcoin transaction. The plant fee buys position in that transaction, not the hike's work. The index is a read model of what Bari wrote. The Admission Gate applies groundinglab's policy (specification section 15) to the verifier's measurements. Verify still does not say pass or fail.

## Layout

Each folder is its own crate with its own `Cargo.toml`, `Cargo.lock` and `deploy/` (a systemd unit and an `env.example`). There is no shared workspace: each service builds and tests alone from a checkout of only its own directory, and pins its dependencies by git tag in this repository.

```bash
cd services/<service>
cargo build --locked --release
cargo test --locked
```

Every service binds to loopback by default. To serve one publicly, put a reverse proxy of your choice in front of it. Nothing here names a host.

| Folder | Process | Default bind | Depends on (git tag) | Release tag |
|---|---|---|---|---|
| [`bari/`](bari/) | `bari` (crate `pcoc-bari`) | `127.0.0.1:38080` | `hike`, `hike-net` @ `ref-rust-v2.0.5` | `bari-v0.6.8` |
| [`index/`](index/) | `pcoc-index` | `127.0.0.1:38081` | `pcoc-bari` @ `bari-v0.6.7` (`hike` @ `ref-rust-v2.0.4`) | `index-v0.1.4` |
| [`admission-gate/`](admission-gate/) | `admission-service` (crate `pcoc-admission`) | `127.0.0.1:8787` | `hike` @ `ref-rust-v2.0.4` | `gate-v0.3.3` |

Release tags are `<service>-v<x.y.z>` and point at the commit that ships that release. The table's Release tag column is the release that matches the working tree.

## Bari

[bari/](bari/) takes 32-byte waypoints (`POST /waypoint`, Hashcash 21 leading zero bits in `X-Bari-Hashcash-Nonce`) into the pool for the live tip and serves Merkle paths (`GET /path/{leaf}`). `GET /ops` is the operator page. It writes the index cache and does not serve `GET /index`. On the live plane the tip comes from Bitcoin gossip (`BARI_TIP_SOURCE=bitcoin-gossip`, peer `PCOC_LIVE_GOSSIP_PEER`, which this tree does not fill) and the plant is Bitcoin Core. On the test plane the tip comes from the synth sequencer (`BARI_TIP_SOURCE=synth`) and the commit is simulated. See [bari/README.md](bari/README.md).

## Index

[index/](index/) is its own service. It serves the portable-index files: `GET /index` (tip), `GET /index/{height}`, and `GET /index/since` (NDJSON catch-up). Bari can write those files. `pcoc-index rebuild` can write them from a public block API, or from a pruned node, with no plant and no wallet RPC. The verifier's checkpoint measurement reads `GET {index-url}/index/{beacon_height}`. On the live profile an omitted URL means `https://index.groundinglab.io`. That host is opt-in. See [index/README.md](index/README.md).

## Admission Gate

[admission-gate/](admission-gate/) is the groundinglab Admission Gate. It checks a Satoshi Hike v2.0 object and reads one TOML config file.

- `POST /v1/sign-in` is the sign-in door. The body is the §15.1 six-field object. The website reads `GET /v1/sign-in/result/{nonce}`. A v1.35 object is refused as `earlier_version`.
- `POST /v1/verify` measures an object. It does not check a signature and does not admit.
- `GET /latest.json` and `GET /log/{YYYY-MM-DD}.jsonl` are the public decision log.
- `GET /v1/health` reports version, commit, spec kind and the config.

The code default for `allowed_origins` is empty (loopback only). A live groundinglab deploy needs `https://groundinglab.io` and `https://www.groundinglab.io` there; the shipped `deploy/gate.toml` sets them. Install is [deploy/DEPLOY.md](admission-gate/deploy/DEPLOY.md). Request and response bodies are in [WIRE.md](admission-gate/WIRE.md).
