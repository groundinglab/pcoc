# bari

> **Do not deploy this tree over the live v2.0 service.** This branch is Satoshi Hike 2.2 and must not replace the live v2.0 service.

One binary, `bari`. Crate `pcoc-bari`, library `pcoc_bari`, version 0.6.8, tag `bari-v0.6.8`.

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

## Header source

The synth plane is in this crate (`header_source`). A live Bitcoin header source is not in this build (SPEC §9, Phase C). The process fails closed if `BARI_TIP_SOURCE=bitcoin-gossip` is asked to follow live headers. Nothing here is deployed from this change.

Hashcash is `bari-hc-v2` (SPEC §13.1). `BARI_PUBLIC_ORIGIN` is this process's public origin, serialized as RFC 6454 §6.2 and mixed into the challenge. The fee header arrives in `X-Bari-Hashcash-Header`. A header for another origin is refused. The paid rule is the Rust SDK's Rule BR1 check. A live-plane "after" header uses the Bitcoin mainnet target for the next height, including a retarget at a 2016-block boundary. The synthetic plane uses the fixed `nBits` `0x207fffff`.

## More

- [OPERATING.md](OPERATING.md) — the HTTP table, the operator page, and the fixture command
- [deploy/bari.service](deploy/bari.service)
- [deploy/env.example](deploy/env.example)
