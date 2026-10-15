# Rebuild evidence (#32)

Run on 2026-10-15 against `https://mempool.space/api` and `https://index.groundinglab.io`.

Heights **971497 through 971516** (20 blocks). That is the beacon range of `/home/christian/pcoc-build/pepe.cbor` (971497..=971514) plus `last_interrupt_height` 971515 and one further block. The range contains plants at 971500, 971501, 971503, 971504, 971506 (two), 971507, 971509 (two), 971510, 971512 (two), 971515 (three), and 971516.

## Command

From `services/index`, after `cargo test --locked` has built the binary:

```text
cargo test --locked --offline --test live_rebuild -- --ignored --nocapture
```

The ignored test (`tests/live_rebuild.rs`) does three things:

1. `pcoc-index rebuild --source esplora` for 971497..=971516 into a temp directory. Default base URL `https://mempool.space/api`. Reorg depth 6.
2. `GET https://index.groundinglab.io/index/{height}` for the same heights and compare.
3. `satoshi-hike-verify --input ~/pcoc-build/pepe.cbor --profile live --audit-seed 11…11` against `https://index.groundinglab.io` and against the rebuilt directory served on `127.0.0.1:38091`.

## Output

```text
rebuild wrote 20 unchanged 0 reorg 0
pcoc-index v0.1 bind=127.0.0.1:38091 data_dir=/tmp/.tmpRpG3j9 sequencer_clocks=0 (read-only height-*.json cache)
verifier records match (97273 bytes)
test rebuild_matches_hosted_index_and_the_verifier ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 57.60s
```

## Diff

No difference beyond the off-chain fields.

Compared and equal: `version`, `network`, `header.hash`, `header.height`, `header.time`, and the plant set (`root`, `txid`, `vout`, `block_hash`, `block_height`).

Ignored, because the chain does not carry them:

- `header.timestamp_unix_ms` (Bari's observation). Present on the hosted files, absent on the rebuild.
- `tree_size` on `plant` and on each `plants` entry. Present on the hosted files, absent on the rebuild.

`plants` order is not required to match. Bari writes serving-batch order. The rebuild writes transaction order. The verifier matches any one plant (specification P14.5). In this range the sets were equal, so order never had to be waived. The single `plant` member matched too, including the one-plant heights.

A one-height check of 971500, written with `pcoc-index rebuild --from-height 971500 --to-height 971500 --out /tmp/pcoc-evidence-index --reorg-depth 0`, produced the file below. The hosted body is the same object plus `timestamp_unix_ms` and `tree_size`.

```json
{
  "version": 2,
  "network": "main",
  "header": {
    "hash": "000000000000000000001867bc46cf1441310f8eb097da6d72b3b2f4a1bf0666",
    "height": 971500,
    "time": 1792034437
  },
  "plant": {
    "root": "97fe3f0cd4d24704e34666d4a581ed0092eb5a0a42c87f85d04a0949633aaf56",
    "txid": "05452d428c67c0f8d45304839de9738cc5f7df7ddbd7e12a4c76a690e4f7d4b8",
    "vout": 1,
    "block_hash": "000000000000000000001867bc46cf1441310f8eb097da6d72b3b2f4a1bf0666",
    "block_height": 971500
  },
  "plants": [
    {
      "root": "97fe3f0cd4d24704e34666d4a581ed0092eb5a0a42c87f85d04a0949633aaf56",
      "txid": "05452d428c67c0f8d45304839de9738cc5f7df7ddbd7e12a4c76a690e4f7d4b8",
      "vout": 1,
      "block_hash": "000000000000000000001867bc46cf1441310f8eb097da6d72b3b2f4a1bf0666",
      "block_height": 971500
    }
  ]
}
```

## Verifier

Both runs used the Go reference `satoshi-hike-verify` built from `reference/go`, profile `live`, audit seed 32 bytes of `0x11`.

- Hosted index: exit 0, stdout 97273 bytes.
- Rebuilt index, served locally: exit 0, stdout 97273 bytes, byte-identical to the hosted run.

The record starts `{"profile":"production","shape":"match",...}`. Header rows for 971497, 971498, and 971499 are `index_comparison` `match` with `T` 1792033416, 1792033634, and 1792033638, which are the header `nTime` values, not Bari's observation clock.
