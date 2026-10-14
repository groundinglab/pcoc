# bari

One binary, `bari`. Crate `pcoc-bari`, library `pcoc_bari`, version 0.6.5, tag `bari-v0.6.5`.

It accepts 32-byte waypoints and later serves a Merkle path. It writes the index cache. It does not serve `GET /index`. The index HTTP reader is [services/index](../index/).

Bari timestamps waypoints by sealing each tip's pool into one Bitcoin transaction. The plant fee buys position, not work. Bitcoin's proof-of-work does not secure a PCOC and does not pay for the hike.

## Routes

- `POST /waypoint`
- `GET /path/{leaf}`
- `GET /ops`

## Tip modes

| Mode | What it is |
|---|---|
| Live | `BARI_TIP_SOURCE=bitcoin-gossip`, plus a Bitcoin Core plant |
| Test | `BARI_TIP_SOURCE=synth`, plus a simulated commit |
| Unset | `BARI_TIP_SOURCE` remains the RPC poll and is not the live tip |

## Pins

`hike` and `hike-net` stay at `ref-rust-v2.0.3` until a later pin bump. This page does not move them.

## More

- [OPERATING.md](OPERATING.md) — the HTTP table, the operator page, and the fixture command
- [pool-handoff/REQUIREMENTS.md](pool-handoff/REQUIREMENTS.md)
- [deploy/bari.service](deploy/bari.service)
- [env.example](env.example)
