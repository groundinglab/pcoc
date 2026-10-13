# index

Read-only portable-index HTTP for commitment reads, served as `index.groundinglab.io`. Crate and binary name `pcoc-index`.

`GET /index` is the current tip. `GET /index/{height}` is one retained height. `GET /index/since` streams retained mainnet heights as newline-delimited JSON. These GETs do not use Hashcash. Default bind `127.0.0.1:38081`.

```text
Bari  --persist-->  index/height-XXXXXXXXXX.json
                         |
                         v
                  pcoc-index :38081
                  tip and height
```

Live Bari's tip advance is Bitcoin gossip (`BARI_TIP_SOURCE=bitcoin-gossip`). Its plant is Bitcoin Core. Test Bari's tip is the synth sequencer. Its plant is a simulated commit, not a Core RPC.

With `PCOC_INDEX_SEQUENCER_DIR` unset, documents are served through the stored version-2 type. With that variable set, pcoc-index may fill `header.time`.

## Who calls which URL

| Caller | Request |
|---|---|
| `satoshi-hike-verify`, checkpoint measurement | `GET {index-url}/index/{beacon_height}`. On the live profile, an omitted URL means `https://index.groundinglab.io` |
| `satoshi-hike-produce`, Observe | the plane gossip peer. Not an HTTP tip URL |
| [`admission-gate`](../admission-gate/) checkpoint measurement | `GET {GL_VERIFY_INDEX_URL}/index/{beacon_height}` |

The hike object does not gain a field for `{index-url}` or `{tip-url}`.

Live Bari's plant points at Bitcoin Core with `BARI_BITCOIN_RPC_*` and `BARI_BITCOIN_NETWORK=main`. The tip peer is `PCOC_LIVE_GOSSIP_PEER`. Test Bari does not open Bitcoin Core. This process keeps the routes above either way.

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

`PCOC_INDEX_DATA_DIR` is the directory that holds the files, not a parent data root. Bari writes them at `$BARI_DATA_DIR/index`.

Each file is named `height-` plus a ten-digit zero-padded height, then `.json`. The file name selects the document. `header.height` is returned as stored.

Retention is an operator choice. A height that is not on disk is **404**. The response is the document this cache retained for that height. It is the copy in this directory.

A `PCOC_INDEX_DATA_DIR` that does not exist yet is created. `GET /index` and `GET /index/{height}` answer **404** until files appear. `GET /index/since` answers **200** with an empty body. A path that exists and is not a directory exits 1.

## Body

Version 2: `{version, network, header, plant}`. `plants` is included when that array is non-empty and omitted when it is empty. `GET /index/since` uses that same body, one height per line.

The Rust type is `pcoc_bari::storage::StoredPortableIndex` in [`bari`](../bari/). This directory does not define a second JSON grammar. The HTTP body is that type serialized as JSON.

`header.hash` is a hexadecimal string. `header.height` is an integer. `header.time` is unix seconds. `header.timestamp_unix_ms` is unix milliseconds. Either time field may be absent, and absent time fields are omitted. `plant` is a JSON object or `null`. `plant: null` is a retained document and is returned as stored.

## CORS

CORS is how a browser on `https://groundinglab.io` or `https://www.groundinglab.io` reads these responses. The allow-list is `origin_allowed` in this crate.

| Item | Rule |
|---|---|
| Methods | `GET` and `OPTIONS` on `/index`, `/index/{height}`, and `/index/since` |
| Origin | `https://groundinglab.io`, `https://www.groundinglab.io` |
| Also allowed | `http://localhost`, `http://127.0.0.1`, `http://[::1]`, and private LAN HTTP (`10.0.0.0/8`, `192.168.0.0/16`, `172.16.0.0/12`), with an optional port |
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

Clocks for heights minted after startup stay out of the map until the process starts again.

## Other processes

| Process | Role beside this reader |
|---|---|
| [`bari`](../bari/) | Writes `height-XXXXXXXXXX.json`. Serves `POST /waypoint` and `GET /path/{leaf}`. |
| [`admission-gate`](../admission-gate/) | Verify door. Its checkpoint measurement reads a height URL |

Hashcash belongs to Bari `POST /waypoint`. These GETs leave that header unread.

## Build and run

This crate depends on `pcoc-bari`, pinned by git tag in `Cargo.toml`, and builds on its own from this directory:

```bash
cargo build --locked --release
export PCOC_INDEX_DATA_DIR=/var/lib/bari/index
./target/release/pcoc-index
```

`PCOC_INDEX_DATA_DIR` must contain `height-XXXXXXXXXX.json` files (zero-padded height, ten digits). [`deploy/`](deploy/) has a systemd unit and an `env.example`. Publishing the public name sits outside this directory: put a reverse proxy in front of the loopback bind.

## Environment

| Variable | Default | Meaning |
|---|---|---|
| `PCOC_INDEX_DATA_DIR` | required | Index cache directory (the directory of `height-*.json` files) |
| `PCOC_INDEX_BIND_ADDR` | `127.0.0.1:38081` | Listen address |
| `PCOC_INDEX_SEQUENCER_DIR` | unset | Optional directory of `state.json` and `events.jsonl`. Fills `header.time` only when a document has neither `header.time` nor `header.timestamp_unix_ms`. Unset serves the files as stored |

Startup logs the bind address, the cache directory, and how many sequencer clocks were loaded. An unset `PCOC_INDEX_DATA_DIR`, an unparseable bind address, or an unusable sequencer directory exits 1.

## Tests

```bash
cargo test --locked
```

## Contract

| | Holds |
|---|---|
| Tip | `GET /index` returns the highest retained height, version 2 |
| Height | `GET /index/{height}` returns the stored document for that height |
| Since | `GET /index/since` is not a height parse. It streams mainnet records as `application/x-ndjson`, ascending. Each line matches that height's GET body. `height` defaults to the lowest stored mainnet height. `limit` max is 10000. A missing height at or below the tip is a gap: no line, and the stream continues |
| Miss | An unknown height is **404** with an empty body. A since query with nothing to send is **200** with an empty body |
| CORS | Preflight and GET from `https://groundinglab.io` and `https://www.groundinglab.io` succeed on `/index`, `/index/{height}`, and `/index/since` |
| Writer | Bari serves waypoint POST and path GET. The index GETs are this process |
