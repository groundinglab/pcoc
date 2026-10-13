# Pool handoff and tip-bound Hashcash

Bari accepts waypoints and serves paths. Bari writes the index files and does not serve `GET /index` or `GET /index/{height}`. This tree does not ship a separate index HTTP reader. Pool handoff is internal to Bari. Hashcash on `POST /waypoint` is on by default.

## 1. Goal

1. While tip \(T_n\) is live, append paid, well-formed waypoints to an open pool keyed by \(T_n\) (height and hash). The append is durable before 204.
2. When tip \(T_{n+1}\) is observed, hand that pool to an asynchronous seal and open an empty pool for \(T_{n+1}\) at once.
3. Before the tree, the live path reads the mempool next-block minimum (`getrawmempool`, ancestor feerate, one block). `estimatesmartfee` 1 economical is logged and shown. It is not the gate. The bid is `max(min(1.5 × that minimum, 1.1 sat/vB), relay floor)`. At or above 1.1, when an acceptance floor is above the cap, or when no confirmed input can fund the bid, there is no Merkle tree and no PSBT. The cohort is appended to \(P_{n+1}\), and the old directory is sealed only after that append. The hold reason is recorded (`next-block-min`, `cap`, or `no-confirmed-utxo`). It is not a stage. An accepted waypoint is not dropped. A tree already in the mempool is not a hold. Several non-RBF trees may be pending at once, and each is judged on its own 5-block window. The test plane has no fee gate and broadcasts on the next tip.
4. Otherwise the handoff deduplicates and sorts, builds one Merkle tree of every waypoint then in the pool, persists the batch with `opened_tip` of that pool and `closed_tip` \(T_{n+1}\), broadcasts it at a bid at or under 1.1 sat/vB, and marks it pending. One broadcast covers the accumulated pool. It is not replaced. `replaceable` is false. There is no fee bump.
5. A later tip decides whether pending becomes serving or stays pending. The anchor window is 5 blocks from the broadcast height. `GET /path` carries the inclusion height with the txid and the Merkle path, and a confirmed leaf keeps returning that body. A broadcast that confirms after the window is still served and marked confirmed late, outside the window. No operator setting changes the window. A miss does not broadcast the same tree again. SPEC §13 remains the producer and verifier window.
6. `POST /waypoint` requires Hashcash bound to the live tip. An unpaid proof or the wrong challenge is rejected.

A restart must not attach waypoints from one tip to a different tip, except a deferral that had not finished its carry. That cohort is appended to the live pool and is not planted while the gate still says wait.

## 2. Non-goals

- A change to the Merkle, path-body, or plant-script bytes.
- Hashcash on `GET /path/{leaf}`, `GET /index`, or `GET /index/{height}`.
- A promise that a plant failure still becomes serving. A skip remains allowed.

## 3. Flow

```text
Tip T_n live
  open pool P_n on disk, keyed by (height_n, hash_n)
  POST /waypoint:
    32 raw bytes, Content-Type application/octet-stream
    Hashcash matches the live tip
    already held in P_n, pending, or serving → 204
    else append to P_n, fsync, → 204
    no Merkle on accept

Tip T_{n+1} observed
  1. handoff: cohort = P_n
  2. empty pool P_{n+1}; new accepts go only there
  3. read the next-block minimum before Merkle
       minimum at or above 1.1, floor above the cap, or no confirmed input:
         append the cohort to P_{n+1}; no tree; no PSBT; log deferred
       an accepted waypoint is not dropped for age
       otherwise seal the cohort asynchronously:
         Merkle of every pooled waypoint, persist, broadcast at ≤ 1.1 sat/vB, pending
  4. pending becomes serving when the confirming block is known, including after the anchor window.
     A confirmed path stays servable. It is not archived.
  5. write the index file for T_{n+1}
       this process does not serve GET /index or GET /index/{height}
```

Accept does not wait on Merkle or plant.

While a tree is confirmed, `GET /path/{leaf}` returns 200 whose bytes are a path for that leaf, including the inclusion block height. A repeat returns the same body. A missing path is 404, and the body says which case it is: still in the pool, in mempool, not in a block yet, not included, or unknown.

## 4. Persistence and restart

The open-accept record is an append-only pool log keyed by tip hash and height.

204 is returned only after the append and fsync.

A memory set may mirror the open pool. After restart, the disk record wins.

There is no protocol cap of 65536. `BARI_POOL_SOFT_MAX`, when set above zero, may return 503.

On start:

1. Observe the selected tip \(T^*\).
2. Each on-disk pool whose tip is not \(T^*\) and that is not yet sealed is taken in tip order. When the gate says plant, it is sealed. When the gate says wait, its leaves are appended to the live pool and it is marked sealed without a tree.
3. Reopen the pool for \(T^*\), or create an empty one.
4. Do not merge pools that have different tip hashes, except a deferral carry into the live pool.

A batch keeps `opened_tip` and `closed_tip`.

## 5. Hashcash

Required on `POST /waypoint` only. `GET /path/{leaf}` does not take `X-Bari-Hashcash-Nonce`. `GET /index` and `GET /index/{height}` are not routes of this process.

| Item | Rule |
|---|---|
| Message | `SHA256(challenge ‖ waypoint ‖ nonce_le_u64)` |
| Challenge | `SHA256(b"bari-hc-v1" ‖ tip_hash)` from the live tip |
| Difficulty | 21 leading zero bits. The one difficulty. `BARI_HASHCASH_BITS` is not read |
| Wire | header `X-Bari-Hashcash-Nonce: <u64 decimal>` |
| Failures | missing or bad proof, or the wrong tip, is 400; soft cap is 503 |

Verify Hashcash for the live tip and append under one lock, before 204. A waypoint already held is 204 and is not stored twice.

## 6. Checks

`cargo test --locked --manifest-path services/bari/Cargo.toml` is the acceptance run. There is no harness binary.

1. Drive tip changes.
2. Solve Hashcash for the live tip and `POST /waypoint`, expecting 204.
3. After the serving tip, `GET /path/{leaf}` is 200 for leaves of that tree.
4. Restart on the same tip continues the same pool.
5. Restart after one missed tip does not mix the old pool into the new one.
6. Restart after two missed tips seals each completed pool in order. The open pool is the current tip only.

Also: an unpaid or bad nonce is not stored; a proof for the previous tip is rejected; a duplicate waypoint is one leaf; an empty pool does not invent a plant; a path is 200 only for the serving tree.

`tests/public_conformance.rs` locks the Merkle root literal, the canonical path body, the commitment script, and the 21-bit difficulty.

## 7. Acceptance

| ID | Must hold |
|---|---|
| A1 | The open pool is keyed by the live tip and stored on disk |
| A2 | A new tip opens an empty pool immediately. When the gate says plant, it seals the prior pool with the correct opened and closed tips. When the gate says wait, it carries that pool forward and does not seal a tree |
| A3 | Accept does not wait on Merkle or plant |
| A4 | Hashcash is required on `POST /waypoint` only, at 21 bits, bound to the live tip. There is no second difficulty dial |
| A5 | There is no protocol cap of 65536 |
| A6 | Restart on the same tip continues that pool |
| A7 | Restart after one missed tip does not mix pools |
| A8 | Restart after two missed tips seals each completed pool in order |
| A9 | `GET /path/{leaf}` is 200 for a path of that leaf while it is serving. A 404 names the case |
| A10 | Bari does not serve `GET /index` or `GET /index/{height}` |
