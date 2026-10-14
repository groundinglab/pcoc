# bari

The Bari process. Crate name `pcoc-bari`. Waypoints only.

It accepts a 32-byte waypoint, keeps a pool for the live tip, and later serves a Merkle path for a leaf in the serving tree. It writes the index cache. It does not serve `GET /index` or `GET /index/{height}`. This tree does not ship a separate index HTTP reader.

Live and test are separate deploys of this same process. The live plane's tip is Bitcoin selected-chain gossip and its plant is Bitcoin Core. The test unit's tip is the synth sequencer and its plant is a simulated commit. Accept and seal stay inside this folder: [pool handoff](pool-handoff/REQUIREMENTS.md). Hashcash on `POST /waypoint` is 21 leading zero bits.

## Pool handoff

| Item | Choice |
|---|---|
| Hashcash wire | `X-Bari-Hashcash-Nonce: <u64 decimal>` |
| Durable accept | append, then fsync, then 204 |
| Soft max | off unless `BARI_POOL_SOFT_MAX` is set |
| Index reads | not this process |
| Chain tip | Live plane: Bitcoin gossip (`BARI_TIP_SOURCE=bitcoin-gossip`, peer `PCOC_LIVE_GOSSIP_PEER`). Test plane: synth sequence (`BARI_TIP_SOURCE=synth`). Unset `BARI_TIP_SOURCE` remains the RPC poll and is not the live tip |

## HTTP

Hashcash is required on `POST /waypoint` only.

| Surface | Code | Meaning |
|---|---|---|
| `POST /waypoint` | 204 | Appended to the live tip pool after fsync, or already held |
| `POST /waypoint` | 400 | Body is not 32 raw bytes, or Hashcash is missing, invalid, or for the wrong tip |
| `POST /waypoint` | 415 | `Content-Type` is missing or not `application/octet-stream` |
| `POST /waypoint` | 503 | Open pool reached `BARI_POOL_SOFT_MAX` |
| `GET /path/{leaf}` | 200 | Canonical path body for a confirmed leaf. A repeat returns the same body |
| `GET /path/{leaf}` | 400 | Leaf is not 64 lowercase hex characters |
| `GET /path/{leaf}` | 404 | The body says which: `still in the pool`, `in mempool, not in a block yet`, `not included`, or `unknown` |
| `GET /ops` | 200 | Operator page. `BARI_OPS_FIXTURE=1` serves it from the built-in fixture, with no node |

`GET /index` and `GET /index/{height}` are not routes of this process. A request for them on this router is 404. This process writes the files and does not serve them.

Hashcash header: `X-Bari-Hashcash-Nonce: <u64 decimal>`.

| Item | Value |
|---|---|
| Message | `SHA256(challenge ‖ waypoint ‖ nonce_le_u64)` |
| Challenge | `SHA256(b"bari-hc-v1" ‖ tip_hash)` from the live tip |
| Difficulty | 21 leading zero bits. That is the one difficulty, shared with produce. `BARI_HASHCASH_BITS` is not read. |

`204` means the waypoint was appended or was already held. It is not a promise that a path will be served. A confirmed path stays 200. The proof is checked against the live tip under the same lock as the append.

## Cycle

1. `POST /waypoint` into the pool of the live tip. One post. Fire-and-forget.
2. A later tip broadcasts one tree while the mempool next-block minimum is below 1.1 sat/vB. The bid is `max(min(1.5 × that minimum, 1.1), relay floor)`. At or above 1.1, or when no confirmed input can fund the bid, Bari builds no tree and submits no PSBT. The waypoints stay collected. The hold reason is recorded. It is not a stage. An accepted waypoint is not dropped because blocks have passed. A tree already in the mempool does not hold the next cohort. Several non-RBF trees may sit in the mempool at once. Each is judged on its own window.
3. The anchor window is 5 blocks from the broadcast height. A tree broadcast while the tip is B can confirm in B+1 through B+5. More than 5 blocks is Missed. A hit moves the tree to serving. `GET /path/{leaf}` returns the inclusion block height, the txid, and the Merkle path. SPEC §13 is the producer and verifier window. Bari's own window is this 5-block span.
4. A confirmed path stays servable after that window. A broadcast that confirms later is still marked included and the path is served. Bari does not drop a leaf because the window moved. The producer decides not anchored when no path arrives inside the window. There is no second broadcast of the same tree, no fee bump, no RBF, and no repair.

No operator setting changes that window. `BARI_CONFIRMATION_WINDOW_TIPS` is not read.

## Operator events

stderr, one JSON object per line, same stream as the `tip height=` line. Always on. No env var and no feature flag. Grep `bari.ops.`.

`level` is `info` or `warn`. Leaf, waypoint, and root ids are natural-order hex. Tip hashes, txids, and block hashes are Bitcoin display hex. That split is the one `events.jsonl` already uses. These stderr lines do not replace `events.jsonl`.

`at` and `detail` on a plant skip name the existing code branch that emitted the line. They are not stored states.

| Event | Level | When | Fields |
|---|---|---|---|
| `bari.ops.waypoint.accept` | info | `POST /waypoint` returns 204 | `status` 204, `reason` `appended` or `already held`, `waypoint`, `tip_height`, `tip_hash` |
| `bari.ops.waypoint.reject` | warn | `POST /waypoint` returns 400, 415, 500, or 503 | `status`, `reason` (the response text, or the README meaning of 503), `waypoint` when the body was 32 bytes, `tip_height`, `tip_hash`. 503 also has `soft_max`. A body that is not 32 bytes also has `body_len` |
| `bari.ops.open_pool.admit` | info | A waypoint is appended to the open pool | `waypoint`, `leaf_count`, `leaf_ids` (the whole pool), `tip_height`, `tip_hash`. A duplicate 204 does not emit this |
| `bari.ops.seal` | info | A pool is sealed | Non-empty: `tree_size`, `leaves`, `root`, `batch_sequence`, opened and closed tip. Empty: `empty` true, `leaf_count` 0, `leaves` `[]`, `durable_event` `pool_empty_handoff` or `pool_empty_seal`. Empty has no `tree_size` |
| `bari.ops.seal.decision` | info, or warn when `decision` is `defer` | One line per seal decision, including an empty handoff | `decision_height` (block D), `decision_block` (Bitcoin display hex), `next_block_min_sat_vb` (mempool next-block minimum), `node_estimate_sat_vb` (`estimatesmartfee` 1 economical; null when that call fails), `fee_cap_sat_vb`, `bid_sat_vb`, `decision` (`plant`, `defer`, or `empty`), `leaf_count`, `leaves`, `dropped_leaf_count` (0), `dropped_leaves` (`[]`). On the test plane the fee rates are null. This is not `bari.ops.seal` |
| `bari.ops.plant.utxo` | info, or warn when `thin` is true | A confirmed input set was chosen for this plant, before broadcast | `mode` (`round_robin` or `consolidate`), `min_conf`, `reserve_min`, `reserve_max`, `confirmed`, `thin`, `consolidate`, `inputs` (display `txid`, `vout`, `amount_sat`, `confirmations`) |
| `bari.ops.plant.broadcast` | info | `plant` returned a receipt | `txid`, `vout`, `fee_rate_sat_vb`, `fee_source` (`mempool` or `floor` on the live plant), `fee_cap_sat_vb`, `batch_sequence`, `root`, `tree_size`, `leaves` |
| `bari.ops.plant.included` | info | Pending batch included in the decision tip | `txid`, `fee_rate_sat_vb`, `fee_source`, `block_hash`, `block_height`, `tree_size`, `leaves`, decision tip, `confirmation_window_tips` |
| `bari.ops.plant.deferred` | warn | The cohort stays in the pool. No tree, no PSBT, no broadcast | `deferred` true, `reason` (`next-block-min`, `cap`, `no-estimate`, `fallback`, `no-confirmed-utxo`, `fee-view`, or `utxo-view`), `message`, `leaves`, `leaf_count`, `decision_height`, `next_block_min_sat_vb` (mempool next-block minimum), `node_estimate_sat_vb` (`estimatesmartfee` 1 economical; null when that call fails or this deferral had no fee view), `needed_sat_vb`, `fee_cap_sat_vb`. The waypoints are appended to the live tip's pool |
| `bari.ops.fee.estimate` | warn | `estimatesmartfee` failed or returned a feerate that could not be read | `message`. The mempool minimum still gates. A successful estimate is not this line; it is `node_estimate_sat_vb` on the decision |
| `bari.ops.plant.skip` | warn | Plant failed for a reason other than a deferral, or the inclusion decision skipped for a reason other than `not-mined` | `skip_reason` (`fee` or `other`), `at`, `message`. On `at` `plant`, also `fee_rate_sat_vb` and `fee_source` when a fee decision exists. `txid` and fee fields when a receipt exists. `not-mined` is `bari.ops.confirmation_window.missed` and the broadcast stays pending so a later confirmation can still be served. A missing confirmed input and a next-block minimum above 1.0 are `bari.ops.plant.deferred`, not this line. A funded amount below `ceil(bid × vsize)` stays in the pool and is not broadcast |
| `bari.ops.path.serve` | info | `GET /path/{leaf}` returns 200 | `leaf`, `tree_size`, `root`, `txid`, `block_hash`, `block_height`, `batch_sequence`, current tip |
| `bari.ops.confirmation_window.missed` | warn | The anchor window ended and the plant is not in that tip | `skip_reason` `not-mined`, `txid`, fee fields, `leaves`, decision tip, `confirmation_window_tips`, `lookup_block_hash` (null when the lookup returned nothing), `kept_pending` true. The leaves stay pending. A later confirming block still serves the path |
| `bari.ops.page.check` | warn | A reconciliation rule on `GET /ops` does not hold | `rule`, `tip_height`, `detail`. One line per rule per tip |

A 404 or 400 on `GET /path/{leaf}` is not a serve, so it has no line here. The 404 body is the case: still in the pool, in mempool, not in a block yet, not included, or unknown. A confirmed leaf does not become `already archived after it was served`.

## Operator page

`GET /ops` is the page for ops.groundinglab.io. One function, `build_ops_view`, derives every figure from the tree records, the block records, the open pool, and one set of node reads. `render` takes only that view. The page does not look up a transaction on the chain and does not print a wallet address.

The header is one line: groundinglab, BARI, the Bitcoin Anchored Registry Index, operator v0.6.2, the instance, and the tip. At 390 px it drops the long name and the word operator. The pipeline is Collecting, In mempool, then Last anchored. Collecting shows the pool count, the oldest waypoint, the current next-block minimum, and whether the next block will fire. A tree in the mempool is not a hold. In mempool lists every in-flight tree, each with its own `+N of 5`. Last anchored shows the newest included tree.

The ledger is newest first, 48 hours. One row is one tree. Columns are Sent at, Waypoints, Included, and Result. Result is `In mempool +N of 5`, `On time +N`, `Late +N`, `Missed`, or `Collecting`. `+N` is included height minus sent height. `+1` through `+5` are on time, and `+5` is inside the window. `+6` and later are late. A tree with no inclusion once the tip is 5 or more past the broadcast is missed. Held and quiet blocks between trees are one collapsed row, labelled by the reason (`held: fee`, `held: no confirmed coin`, `held (tree in mempool, older build)`, quiet, `not seen`, or `not seen (Bari offline)`). Fees paid are the transaction fees. A tree with no recorded fee uses `ceil(bid × 159)` and the total is marked `≈`. Both heights of a tree link to the mempool.space transaction. No txid is printed. The fee chart is the next-block minimum read at each block, the 1.1 cap, and a dot at each bid. The 48 h, Wallet, and System tiles sit beside it on a desktop and stack at 390 px. Wallet confirmed and unconfirmed come from `listunspent` split at `BARI_PLANT_UTXO_MIN_CONF`. They do not overlap.

A tip that jumps by more than one block does not drop a pending tree. If a later block includes it, including a block the clock did not step through, the path is served and the result is Late. The words sealed, planted, deferred, and failed are not on the page.

```sh
BARI_OPS_FIXTURE=1 BARI_BIND_ADDR=127.0.0.1:38080 \
  cargo run --locked --manifest-path services/bari/Cargo.toml --bin bari
```

Open `http://127.0.0.1:38080/ops`. The fixture is the design seed: tip 970700, the same arrivals and fee runs as the mockup, and several trees in the mempool at once because an eligible block sends. The 970682 tree is Late +8. Held rows are the fee cap. The open pool is collecting and the next block will fire. No data directory and no Bitcoin node. `BARI_INSTANCE` names the instance in the header. Unset, it says `local`. `BARI_COMMIT` names the commit. Unset, it says `unknown`.

## Restart

Incomplete batch records still fail-closed (`restart_incomplete_failed_closed`). These shapes are restored instead:

- `Broadcast` with `plant` set and `inclusion` absent: every such batch is restored into pending, not only the newest. Durable event `restart_broadcast_restored_pending`. A later confirming block can still be served. A restart does not broadcast the tree again.
- `Included` with `plant` and `inclusion` set, and a `Flushed` batch that still has both: every such batch is restored into serving. A flushed confirmed batch is stored as `Included` again. Durable event `restart_included_restored_serving`.

A root or `tree_size` mismatch on either restore is a storage `InvalidData` error. The process does not open.

If that same startup also seals a newer non-empty pool into pending, the newer seal takes the slot and the restored batch is catch-up skipped (`restart_pool_catchup_skipped`). An empty pool does not.

## Index files

Published tips are written at `$BARI_DATA_DIR/index/height-XXXXXXXXXX.json` (ten-digit height). An index reader's data directory is that `index/` directory. This process does not prune those files and does not serve a missing height.

On open, once the tip is known, that tip’s height file is written when it is missing. The write is `persist_published_index` for the open tip. This open did not see the tip advance, so the new file has no `header.timestamp_unix_ms`. When the tip at startup carries Bitcoin `nTime`, that file does include `header.time` in unix seconds. A file already stored at that height is left unchanged, including one that already has an observation stamp and one that has no `header.time`. Other heights are not rewritten. There is no backfill of historical rows. A reorg policy for stale heights is not this write. See index reorg proposal (not in this release).

`header.timestamp_unix_ms` is Bari's wall-clock observation: unix milliseconds from `SystemTime::now` when that header hash was selected, written when the tip file is persisted. It is not the chain clock.

`header.time` is Bitcoin header `nTime` in **unix seconds** (not milliseconds). It is the same integer `getblockheader` returns as `time`, and the same integer in gossip header bytes 68..72. It is omitted when that header time was not read. A new tip stamps it from the header. An older height file is not rewritten to add it. A missing `header.time` is not read. Roll forward only. It is not `timestamp_unix_ms`, and it is not `nTime × 1000`.

## Run

`BARI_DATA_DIR` is required.

```sh
cargo test --locked --manifest-path services/bari/Cargo.toml
```

### Live plane

`BARI_TIP_SOURCE=bitcoin-gossip`. The tip advance is Bitcoin selected-chain gossip (`HeaderClock`, mainnet). `BariClock::on_tip` takes those tips. `BARI_POLL_INTERVAL_MS` is not read. The plant, the inclusion lookup, and the post-check are Bitcoin Core (`BitcoinRpc`). This mode does not call `getbestblockhash` to advance the tip.

The gossip peer is `PCOC_LIVE_GOSSIP_PEER` (`host:port`). The same variable is the producer live peer. This tree does not fill that host or port. Startup fails when it is unset, empty, or not `host:port`. `BARI_BITCOIN_NETWORK` must be `main`. `regtest`, `testnet4`, and `signet` are rejected on this tip source. `BARI_SYNTH_PEER` and `BARI_SYNTH_NETWORK` are refused. The test tip stays `synth`.

```sh
BARI_TIP_SOURCE=bitcoin-gossip \
PCOC_LIVE_GOSSIP_PEER=HOST:PORT \
BARI_DATA_DIR=/safe/durable/path \
BARI_BITCOIN_NETWORK=main \
BARI_BITCOIN_WALLET=dedicated-wallet \
BARI_BITCOIN_COOKIE_FILE=/path/to/.cookie \
BARI_FEE_CAP_SAT_VB=1.1 \
BARI_FEE_MULTIPLIER=1.5 \
cargo run --locked --manifest-path services/bari/Cargo.toml --release --bin bari
```

`HOST:PORT` is not a bind. The operator sets `PCOC_LIVE_GOSSIP_PEER`. Default HTTP bind is `127.0.0.1:38080`. Default Bitcoin Core RPC endpoint is `127.0.0.1:38332`. That URL is the plant, not the gossip peer. Unset, the cap is `1.1` sat/vB and the multiplier is `1.5`. Startup is `fee=bid max(min(multiplier×next-block, cap), relay) cap=1.1 multiplier=1.5 fire_below=1.1 target=mempool-next-block mode=economical`. [`deploy/env.example`](deploy/env.example) is the env shape and [`deploy/bari.service`](deploy/bari.service) is a systemd unit. `BARI_FEE_BID_SAT_VB` and `BARI_DEFER_ABOVE_SAT_VB` are the testing-phase keys. If either is set, startup refuses and names the key. Remove them from a live unit before any later restart. A deploy restarts the process only after the operator has checked this binary is the one that will start.

### Plant wallet

The plant spends confirmed wallet outputs and does not spend the change from the previous plant while that change has fewer than `BARI_PLANT_UTXO_MIN_CONF` confirmations. `walletcreatefundedpsbt` is called with the inputs this process already chose, `add_inputs` false, and `replaceable` false. A skipped plant is not fee-bumped, replaced, or repaired. No transaction is broadcast when no suitable confirmed output is available. That wait is `bari.ops.plant.deferred` with `reason` `no-confirmed-utxo`. The waypoints stay in the pool. The selector does not drop them.

Before a tree is built, Bari reads `getrawmempool true` and takes the lowest ancestor feerate that fits in one block (1,000,000 vB minus 4,000 vB for the coinbase and a reserve). Each package is charged once: an entry adds its own virtual size, and `fees.ancestor` / `ancestorsize` is the package rate. `ancestorsize` is not added again for every member. An empty mempool, or a mempool that fits in that budget, uses `max(relay floor, lowest feerate present)`. The relay floor comes from `getmempoolinfo` `minrelaytxfee` (`0.00000100` BTC/kvB is 0.1 sat/vB). `estimatesmartfee` for one block in economical mode is logged and shown as the node estimate. A failure of that call does not hold the plant.

The bid is `max(min(1.5 × that mempool minimum, 1.1 sat/vB), relay floor)`. `mempoolminfee` does not raise the bid. A tree is broadcast only while the mempool minimum is below 1.1. At 1.1 or above, Bari builds no tree, submits no PSBT, and keeps every pooled waypoint. The hold reason is `holding, min 2.2 is above the 1.1 cap` when the minimum is 2.2. The pool keeps accumulating across tips. An accepted waypoint is not evicted. A tree already in the mempool does not hold the next block. Several non-RBF trees may be in the mempool at once.

`GET /ops` reads that same mempool minimum for the Collecting tile. `getrawmempool true` is one JSON body of the whole mempool, and a live mempool takes longer than two seconds. The page waits 30 seconds for that read. A timeout still says the node was not read. A node that answers inside the budget shows the minimum.

An acceptance floor above the cap holds (`reason` `cap`) and does not broadcast. No next-block answer holds (`reason` `no-estimate`). A recent-block figure is not a fee (`reason` `fallback`). `walletcreatefundedpsbt` is given `fee_rate` of the accepted bid and `replaceable` false. If the funded amount is below `ceil(bid × vsize)`, the transaction is not broadcast and the cohort stays collected. There is no RBF and no fee bump. A missing confirmed input still holds (`reason` `no-confirmed-utxo`). A failed fee read holds (`reason` `fee-view`).

Every Bitcoin block height this process sees gets one block record: the decision (`sent`, `held`, or `quiet`), the hold reason, the next-block minimum read at that decision, the bid when one was named, the pool size, and the waypoints added while the previous tip was current. Inclusion lives on the tree record, not on the block record. Rows are kept for 48 hours. `GET /ops` reads the in-memory copies. A height with no record is `not seen`. A quiet height written long after its header, in one catch-up, reads `not seen (Bari offline)`. `added` is the count taken when the pool is handed off, before the seal runs. A waypoint posted during the seal is the next height's.

| Variable | Default | Meaning |
|---|---|---|
| `BARI_FEE_CAP_SAT_VB` | `1.1` | Highest sat/vB this process will broadcast. Must be 1.1, or unset. Any other value refuses startup |
| `BARI_FEE_MULTIPLIER` | `1.5` | `max(min(multiplier × mempool next-block minimum, cap), relay floor)`. Must be 1.5, or unset |
| `BARI_FEE_DYNAMIC` | on (`1`) | `0` selects the fixed rate below. The fire line is the cap on the multiplier path |
| `BARI_FEE_RATE_SAT_VB` | unset | Fixed sat/vB, only with `BARI_FEE_DYNAMIC=0`. Must be above 0 and below 1.1. Refused while the multiplier path is on |
| `BARI_FEE_BID_SAT_VB`, `BARI_DEFER_ABOVE_SAT_VB` | refused | Testing-phase keys. If set, startup refuses and names the key |
| `BARI_OPS_FIXTURE` | unset | `1` serves `GET /ops` from the built-in fixture. No node |
| `BARI_INSTANCE` | `local` (default), or the host name to show | Name in the operator header |

`fee_source` on a broadcast plant is `mempool` (the bid came from the mempool next-block minimum) or `floor` (the relay floor raised it). The same source is stored on the batch record and on the block record. `bari.ops.plant.broadcast` carries the bid and the cap. `bari.ops.seal.decision` carries block D, the mempool next-block minimum, the node estimate, the cap, the bid, and `plant`, `defer`, or `empty`. `bari.ops.plant.deferred` carries the same two rates. A skip carries `fee_source` and `skip_reason` on `bari.ops.plant.skip`. The portable index does not carry a fee.

Selection keeps a small confirmed pool so the next seal does not depend on the prior change:

| Variable | Default | Meaning |
|---|---|---|
| `BARI_PLANT_UTXO_MIN_CONF` | `1` | Minimum confirmations. `0` is rejected |
| `BARI_PLANT_UTXO_RESERVE_MIN` | `2` | Low end of the confirmed pool. A lone confirmed output is still spent. The utxo line sets `thin` true when the confirmed count is below this |
| `BARI_PLANT_UTXO_RESERVE_MAX` | `3` | High end of the pool |
| `BARI_PLANT_UTXO_CONSOLIDATE` | unset (`0`) | `1` allows an occasional consolidate. Unset and `0` do not |

Round-robin spends one confirmed output large enough for a conservative fee bound and leaves the other confirmed outputs unspent. The cursor is in-process. A restart starts it at zero of the txid-sorted set. List order from Core does not choose the input. An unconfirmed output of a tree still in the mempool is not spent while a confirmed coin can pay. When no confirmed coin can pay, the cohort stays in the pool and the block is `held: no confirmed coin`.

Several trees can be in the mempool, and each one spends a confirmed coin. Six confirmed coins cover that. Bari does not split the wallet itself. The operator splits it once, on the plant wallet, and leaves the change to confirm before the next campaign:

```sh
# One confirmed coin, COIN_TXID:VOUT, large enough to fund six outputs plus the fee.
# AMOUNT is the per-coin amount in BTC. Six equal outputs. No address is printed here.
bitcoin-cli -rpcwallet="$BARI_BITCOIN_WALLET" createrawtransaction \
  '[ {"txid":"COIN_TXID","vout":VOUT} ]' \
  '[ {"WALLET_ADDRESS_1":AMOUNT}, {"WALLET_ADDRESS_2":AMOUNT}, {"WALLET_ADDRESS_3":AMOUNT}, {"WALLET_ADDRESS_4":AMOUNT}, {"WALLET_ADDRESS_5":AMOUNT}, {"WALLET_ADDRESS_6":AMOUNT} ]'
# Fund the fee from that same coin, sign, and broadcast. replaceable stays false.
# Wait until the six outputs have BARI_PLANT_UTXO_MIN_CONF confirmations.
```

The six addresses come from `getnewaddress` on that wallet. This command is the operator's. A restart does not run it. `BARI_PLANT_UTXO_RESERVE_MIN` stays 2, which is the thin warning, not the split.

Consolidate runs only when `BARI_PLANT_UTXO_CONSOLIDATE=1`:

- Confirmed count above `BARI_PLANT_UTXO_RESERVE_MAX`: spend the smallest excess and leave `reserve_max - 1` confirmed outputs unspent. The change from that plant is what brings the pool back toward the reserve after it confirms.
- No single confirmed output can pay the fee: spend the fewest confirmed outputs that can. The unconfirmed change is not one of them. If they still cannot pay, the cohort stays in the pool (`reason` `no-confirmed-utxo`). It is not dropped.

Fund the wallet with 2 or 3 confirmed outputs before a campaign. These variables are read at process start. They do not retune a running process.

Startup still checks that the node reports `main` and is not in initial block download. A gossip tick carries hash, height, previous hash, and Bitcoin header `nTime` (unix seconds). New tips write that value as `header.time`. Older height files are not backfilled.

### RPC poll

**Lab path, not the live plane.** The live plane is `BARI_TIP_SOURCE=bitcoin-gossip` only. The signet example below is a lab example.

Unset `BARI_TIP_SOURCE`, or `rpc`, still polls `getbestblockhash` on `BARI_POLL_INTERVAL_MS` (default 5 seconds). The plant is the same Bitcoin Core node, including the [plant wallet](#plant-wallet) reserve. This path is not the live plane tip. This tree does not switch a running unit onto it or off it.

`BARI_BITCOIN_NETWORK` on the poll must match the network the node reports (`main`, `testnet4`, `signet`, or `regtest`). Startup fails if they differ. The process also accepts the name `synthetic` when the connected node reports that name.

```sh
BARI_DATA_DIR=/safe/durable/path \
BARI_BITCOIN_NETWORK=signet \
BARI_BITCOIN_WALLET=dedicated-signet-wallet \
BARI_BITCOIN_COOKIE_FILE=/path/to/signet/.cookie \
BARI_FEE_CAP_SAT_VB=1.1 \
BARI_FEE_MULTIPLIER=1.5 \
cargo run --locked --manifest-path services/bari/Cargo.toml --release --bin bari
```

### Test plane

`BARI_TIP_SOURCE=synth` advances `BariClock::on_tip` from `BARI_SYNTH_PEER` (the synth sequencer produce Observes). The cadence is that sequencer, 60 seconds. `BARI_POLL_INTERVAL_MS` is not read. Do not set synth mode on the live unit.

The plant, the inclusion lookup, and the post-check are a simulated commit (`SynthCommit`). This process does not open Bitcoin Core. A waypoint broadcasts on the next synth tip. A tree already in the mempool does not hold that broadcast. The anchor window is 5 blocks from the broadcast height. A confirmed path stays servable after that window. A confirmation after the window is still served and marked confirmed late. A skip is legal. The broadcast is not retried and is not replaced. The synthetic plant does not change the mainnet inclusion check or the mainnet fee gate.

The index `network` field on this plane is `synthetic`. Header wire magic on the synth peer is Bitcoin's regtest magic. `BARI_SYNTH_NETWORK` may be omitted or `regtest`. That name is the wire magic, not a Core network, and it does not select a plant.

Do not point this process at Bitcoin Core. Startup refuses `BARI_BITCOIN_NETWORK`, `BARI_BITCOIN_RPC_URL`, `BARI_BITCOIN_WALLET`, `BARI_BITCOIN_COOKIE_FILE`, `BARI_BITCOIN_RPC_USER`, `BARI_BITCOIN_RPC_PASSWORD`, `BARI_FEE_RATE_SAT_VB`, `BARI_FEE_DYNAMIC`, `BARI_FEE_CAP_SAT_VB`, `BARI_FEE_MULTIPLIER`, `BARI_FEE_BID_SAT_VB`, `BARI_DEFER_ABOVE_SAT_VB`, `BARI_PLANT_UTXO_MIN_CONF`, `BARI_PLANT_UTXO_RESERVE_MIN`, `BARI_PLANT_UTXO_RESERVE_MAX`, and `BARI_PLANT_UTXO_CONSOLIDATE` when `BARI_TIP_SOURCE=synth`. The test plane has no fee gate. `regtest` and `testnet4` are not test-plane notaries.

```sh
BARI_TIP_SOURCE=synth \
BARI_SYNTH_PEER=127.0.0.1:38532 \
BARI_DATA_DIR=/tmp/pcoc-test-bari \
BARI_BIND_ADDR=127.0.0.1:38080 \
cargo run --locked --manifest-path services/bari/Cargo.toml --release --bin bari
```

`BARI_POOL_SOFT_MAX` is optional. Unset or `0` means no soft maximum. A positive value returns 503 when the open pool reaches that many distinct waypoints. A waypoint already accepted stays in the pool. There is no protocol cap of 65536.

The anchor window is 5 blocks from the broadcast height. Produce does not wait on a wall clock before the first leg. This tree does not restart a running unit.

Clients call this process for waypoints and paths. They do not call this process for the published tip or for a height. They do not call Bitcoin RPC.
