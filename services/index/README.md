# index

`pcoc-index` serves the portable index: one JSON file per block height. A verifier reads `GET /index/{height}` and treats that body as the stand-in for Bitcoin. Anyone can skip the service and read Bitcoin.

`pcoc-bari` is the in-repo crate at `services/bari` (path). That crate no longer pins an SDK tag. Crate version `0.1.4`, tag `index-v0.1.4`. Nothing here is deployed by this change.

Default bind `127.0.0.1:38081`. No arguments, or `pcoc-index serve`, serves the cache. `pcoc-index rebuild` writes the cache from public chain data.

```text
height-XXXXXXXXXX.json  -->  pcoc-index :38081  -->  GET /index, /index/{height}, /index/since
```

## What it does

The process is a reader. It does not call Bitcoin, and it does not plant. Bari is the writer on a deploy that runs one: it persists `height-XXXXXXXXXX.json` as tips arrive. This process serves those files.

A file is version 2: `{version, network, header, plant}`. `plants` is present when that block included more than the single `plant` slot. `header.hash` is the block hash in Bitcoin display hex. `header.height` is the height. `header.time` is Bitcoin header `nTime` in unix seconds. `header.timestamp_unix_ms` is Bari's wall-clock observation of that header. `plant.tree_size` is the leaf count of the Merkle tree Bari sealed.

`tree_size` and `timestamp_unix_ms` are off-chain. The 39-byte plant tag is `OP_RETURN` ‖ push-37 ‖ `BARI` ‖ `0x02` ‖ the 32-byte root. It does not carry the tree size, and the chain does not record when Bari saw the header. The verifier ignores both: plant comparison does not use `tree_size` (specification P14.5), and header measurement reads `header.hash`, `header.height`, and `header.time` (section 9). A file that omits either field still serves. A file Bari wrote, with both fields set, still serves.

## Inputs

Four ways to fill the directory. The first three work today. The fourth is not built.

| Input | What you run | What you trust |
|---|---|---|
| Hosted index | Nothing. Point the verifier at `https://index.groundinglab.io` | That host's files |
| Public API rebuild | `pcoc-index rebuild --source esplora` | The API's blocks, checked locally (header hash and previous-hash link) |
| Pruned node | `pcoc-index rebuild --source rpc` against Bitcoin Core | Your node. A pruned node can serve the blocks it still has |
| Snapshot or Nostr cache | Not implemented | A future copy of the same files, checked the same way |

groundinglab hosting the index is a convenience. It is not a dependency. Each of the three working inputs is opt-in. Using one does not require the others.

### A. Hosted index

`https://index.groundinglab.io` serves the same routes as this process. The live verify profile uses it when `--index-url` is omitted. Pass another URL and the verifier reads that instead. Stop using the hosted name by passing your own.

### B. Rebuild from a public API

No Bitcoin node. Esplora is the default source. The base URL defaults to `https://mempool.space/api` (`--esplora-url` or `PCOC_INDEX_ESPLORA_URL`).

```bash
cargo build --locked --release
./target/release/pcoc-index rebuild \
  --from-height 971497 --to-height 971516 \
  --out /var/lib/pcoc-index
```

`--from-txid TXID` starts at the block that contains that plant (a 39-byte `BARI` tag). The search goes back from `--to-height` by `PCOC_INDEX_REBUILD_TXID_LOOKBACK` blocks (default 2016). `--to-height` defaults to the source tip.

Each header must link: its previous hash is the previous header's hash, the double SHA-256 of the 80-byte header is the claimed hash, and `nTime` matches. A break stops the rebuild. After the range is written, the last `--reorg-depth` blocks (default 6) are fetched again. A file whose header changed is rewritten.

Calls are paced by `--min-interval-ms` (default 250). HTTP 429 waits for `Retry-After`, or doubles the interval when that header is absent, up to `--max-retries` (default 6).

The written file matches what Bari writes, minus the off-chain fields. `tree_size` and `timestamp_unix_ms` are omitted. `network` is `main`. `header.time` is the header `nTime`. Plants are the transactions in the block whose output script is the 39-byte tag, in transaction order. When a block has several plants, Bari's `plants` order is serving-batch order, which the chain does not record. The verifier matches any one plant, so the set is the check.

### C. Pruned node

A node is needed to plant (that is Bari). It is not needed to serve or rebuild an index.

`pcoc-index rebuild --source rpc` calls Bitcoin Core JSON-RPC. The calls are `getblockhash`, `getblockheader`, and `getblock`. Nothing else. No wallet calls, no broadcast.

| Variable | Meaning |
|---|---|
| `PCOC_INDEX_RPC_URL` | RPC URL. `BARI_BITCOIN_RPC_URL` is the fallback |
| `PCOC_INDEX_RPC_COOKIE` | Cookie file (`user:password`). `BARI_BITCOIN_COOKIE_FILE` is the fallback |
| `PCOC_INDEX_RPC_USER`, `PCOC_INDEX_RPC_PASSWORD` | User and password when there is no cookie file. `BARI_BITCOIN_RPC_USER` and `BARI_BITCOIN_RPC_PASSWORD` are the fallbacks |

A pruned node can rebuild the blocks it kept. Older blocks need an unpruned source or the public API.

## Outputs

`PCOC_INDEX_DATA_DIR` (serve) and `--out` (rebuild) are the directory of the files, not a parent. Each file is `height-` plus a ten-digit zero-padded height plus `.json`. Pretty JSON, one trailing newline, the same writer shape Bari uses. `GET /index` serves the highest filename. A missing height is 404.

Serve the directory you rebuilt:

```bash
export PCOC_INDEX_DATA_DIR=/var/lib/pcoc-index
export PCOC_INDEX_BIND_ADDR=127.0.0.1:38081
./target/release/pcoc-index
```

Point a verifier at it with `--index-url http://127.0.0.1:38081`.

## Who calls which URL

| Caller | Request |
|---|---|
| `pcoc verify`, checkpoint measurement | `GET {index-url}/index/{beacon_height}`. On the production profile, an omitted URL means `https://index.groundinglab.io` |
| `pcoc produce`, Observe | the plane gossip peer. Not an HTTP tip URL. Live produce is not in the Phase A SDK |
| [`admission-gate`](../admission-gate/) checkpoint measurement | `GET {index_url}/index/{beacon_height}`, where `index_url` is read from `gate.toml` |

The hike object does not gain a field for `{index-url}` or `{tip-url}`.

## Routes

| Method | Path | Result |
|---|---|---|
| `GET` | `/index` | **200** current tip: the `height-XXXXXXXXXX.json` file with the highest height in the name. **404** with an empty body when the cache has no such file |
| `GET` | `/index/{height}` | **200** the retained document for that height. **404** with an empty body when that height is absent |
| `GET` | `/index/since` | **200** `application/x-ndjson`. One line per retained mainnet height, ascending. See below |
| `OPTIONS` | `/index`, `/index/{height}`, and `/index/since` | **204** CORS preflight |

`{height}` is a decimal `u32`. A path segment that is not a `u32` is **400** (`Cannot parse … to a u32`). A `u32` missing from the cache is **404**. A path this router does not define is **404**.

`/index/since` is registered before `/index/{height}`. `GET /index/since` and `OPTIONS /index/since` are that fixed path. They are not a height parse, and they are not **400**.

A read or parse error on `GET /index` or `GET /index/{height}` is **500** with an empty body.

### `GET /index/since`

Catch-up over the retained cache. The body is newline-delimited JSON (`Content-Type: application/x-ndjson`). Each line is the same bytes as the body of `GET /index/{height}` for that height, then `\n`. The line uses the same JSON object that height GET serializes, including `plants` when that array is non-empty and omitting it when the array is empty.

Only documents whose `network` is `main` are included. A `synthetic` or `signet` file stays on disk and is still served by `GET /index/{height}`. It is not a line in this stream, and it does not count toward `limit`.

Heights are the filename heights (`height-XXXXXXXXXX.json`), ascending. The start is inclusive.

A missing filename at or below the tip is a gap. `GET /index/{height}` for that height is **404**. The since stream stores no line for it and continues at the next retained height. It does not stop at the first gap. A hole such as 969874, with 969873 and 969875 on disk, still yields those surrounding mainnet records in order.

| Query | Rule |
|---|---|
| `height` | Optional `u32`. When set, lines are filename heights `>= height`. When omitted, the stream starts at the lowest stored mainnet height (every matching mainnet record) |
| `limit` | Optional `u32`. When set, at most that many mainnet lines are sent. The maximum is **10000** (`SINCE_LIMIT_MAX`). `0` is an empty body. A `limit` above 10000 is **400** with an empty body. When omitted, there is no cap: every matching mainnet record from the start height is sent |

A `height` or `limit` that is not a `u32` is **400**. No matching mainnet record (empty cache, only non-mainnet files, or `height` past the last retained mainnet height) is **200** with an empty body and `Content-Type: application/x-ndjson`. A read or parse error while listing the cache is **500** with an empty body. A read or parse error on one file after the body has started ends the stream.

## Cache

`PCOC_INDEX_DATA_DIR` is the directory that holds the files, not a parent data root. Bari writes them at `$BARI_DATA_DIR/index`. A rebuild writes them at `--out`.

Each file is named `height-` plus a ten-digit zero-padded height, then `.json`. The file name selects the document. `header.height` is returned as stored.

Retention is an operator choice. A height that is not on disk is **404**. The response is the document this cache retained for that height. It is the copy in this directory.

A `PCOC_INDEX_DATA_DIR` that does not exist yet is created. `GET /index` and `GET /index/{height}` answer **404** until files appear. `GET /index/since` answers **200** with an empty body. A path that exists and is not a directory exits 1. Creation is silent, so a mistyped path starts an empty index on a new directory: check the `data_dir=` in the startup line.

## Body

Version 2: `{version, network, header, plant}`. `plants` is included when that array is non-empty and omitted when it is empty. `GET /index/since` uses that same body, one height per line.

`header.hash` is a hexadecimal string. `header.height` is an integer. `header.time` is unix seconds. `header.timestamp_unix_ms` is unix milliseconds. Either time field may be absent, and absent time fields are omitted. `plant` is a JSON object or `null`. `plant: null` is a retained document and is returned as stored. `plant.tree_size` may be absent. An absent `tree_size` is omitted. A present `tree_size` is returned as stored.

Bari's writer type is `pcoc_bari::storage::StoredPortableIndex`. This crate's serve type is `pcoc_index::document::IndexDocument`. The difference is `tree_size: Option<u32>` here, so a rebuild that omits it still decodes. The HTTP body is that document serialized as JSON.

## CORS

CORS is how a browser on `https://groundinglab.io` or `https://www.groundinglab.io` reads these responses. The allow-list is `origin_allowed` in this crate.

| Item | Rule |
|---|---|
| Methods | `GET` and `OPTIONS` on `/index`, `/index/{height}`, and `/index/since` |
| Origin | `https://groundinglab.io`, `https://www.groundinglab.io` |
| Also allowed | Loopback only: `http://localhost`, `http://127.0.0.1`, `http://[::1]`, with an optional port. No other origin is allowed |
| Response | The request `Origin` is reflected when it is allowed, with `Vary: Origin` |
| Allow-Methods | `GET, OPTIONS` |
| Allow-Headers | `Accept, Content-Type` |
| Max-Age | `86400` |

## Optional sequencer clock

`PCOC_INDEX_SEQUENCER_DIR` is optional. Unset means the files are served as stored.

When it is set, the value is a directory of `state.json` and `events.jsonl`.

The map is loaded once, at startup. The process exits if that path is not a directory, if it contains neither file, or if `state.json` is not JSON. A line in `events.jsonl` that is not JSON is skipped.

`state.json` `headers[].minted_unix_ms` supplies unix seconds (`milliseconds / 1000`). A value below one second is skipped. Heights still missing are filled from `events.jsonl`: event `initialized` is height 0, event `header_minted` uses `payload.height`, and the clock is that line's `observed_unix_ms`. When both sources name a height, the state file wins.

The fill runs only when the stored document has neither `header.time` nor `header.timestamp_unix_ms`. It sets `header.time`. It leaves `header.timestamp_unix_ms` absent. A document that already has either field is served unchanged. The cache file is left as stored.

Clocks for heights minted after startup stay out of the map until the process starts again. A rebuild writes `header.time` from the Bitcoin header, so this fill does not apply to those files.

## Other processes

| Process | Role beside this reader |
|---|---|
| [`bari`](../bari/) | Writes `height-XXXXXXXXXX.json` when you run your own plant. Serves `POST /waypoint` and `GET /path/{leaf}`. The plant is the only Bitcoin Core RPC this repository makes, and it is Bari's. |
| [`admission-gate`](../admission-gate/) | Verify door. Its checkpoint measurement reads a height URL |

Hashcash belongs to Bari `POST /waypoint`. These GETs leave that header unread.

## Build and run

This crate depends on `pcoc-bari`, pinned by git tag in `Cargo.toml`, and builds on its own from this directory:

```bash
cargo build --locked --release
export PCOC_INDEX_DATA_DIR=/var/lib/pcoc-index
./target/release/pcoc-index
```

[`deploy/`](deploy/) has a systemd unit and an `env.example`. Publishing a public name sits outside this directory: put a reverse proxy in front of the loopback bind.

## Environment

| Variable | Default | Meaning |
|---|---|---|
| `PCOC_INDEX_DATA_DIR` | required to serve | Index cache directory (the directory of `height-*.json` files) |
| `PCOC_INDEX_BIND_ADDR` | `127.0.0.1:38081` | Listen address |
| `PCOC_INDEX_SEQUENCER_DIR` | unset | Optional directory of `state.json` and `events.jsonl`. Fills `header.time` only when a document has neither `header.time` nor `header.timestamp_unix_ms`. Unset serves the files as stored |
| `PCOC_INDEX_ESPLORA_URL` | `https://mempool.space/api` | Esplora base URL for `rebuild --source esplora` |
| `PCOC_INDEX_REBUILD_MIN_INTERVAL_MS` | `250` | Pause between public-API calls. `--min-interval-ms` overrides it |
| `PCOC_INDEX_REBUILD_TXID_LOOKBACK` | `2016` | Blocks walked back from the tip when `--from-txid` is set |
| `PCOC_INDEX_RPC_URL` | unset | Bitcoin Core RPC URL for `--source rpc` |
| `PCOC_INDEX_RPC_COOKIE` | unset | Cookie file for `--source rpc` |
| `PCOC_INDEX_RPC_USER`, `PCOC_INDEX_RPC_PASSWORD` | unset | RPC user and password when there is no cookie file |

Startup logs the bind address, the cache directory, and how many sequencer clocks were loaded. An unset `PCOC_INDEX_DATA_DIR`, an unparseable bind address, or an unusable sequencer directory exits 1.

## Tests

```bash
cargo test --locked
```

`tests/live_rebuild.rs` is ignored. It rebuilds a real height range from mempool.space, diffs it against the hosted index, and runs the 2.2 Go SDK `pcoc` (`sdk/go` `./cmd/pcoc`) on both. `REBUILD-EVIDENCE.md` records an earlier run.

```bash
cargo test --locked --release --test live_rebuild -- --ignored --nocapture
```

## Contract

| | Holds |
|---|---|
| Tip | `GET /index` returns the highest retained height, version 2 |
| Height | `GET /index/{height}` returns the stored document for that height |
| Since | `GET /index/since` is not a height parse. It streams mainnet records as `application/x-ndjson`, ascending. Each line matches that height's GET body. `height` defaults to the lowest stored mainnet height. `limit` max is 10000. A missing height at or below the tip is a gap: no line, and the stream continues |
| Miss | An unknown height is **404** with an empty body. A since query with nothing to send is **200** with an empty body |
| CORS | Preflight and GET from `https://groundinglab.io` and `https://www.groundinglab.io` succeed on `/index`, `/index/{height}`, and `/index/since` |
| Writer | Bari may write the files. `pcoc-index rebuild` may write them from chain data. The GETs are this process either way |
| Optional fields | `header.time`, `header.timestamp_unix_ms`, and `plant.tree_size` may be absent. Absent fields are omitted. Serving a file that has them is unchanged |
