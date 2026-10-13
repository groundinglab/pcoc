# spec

This folder is Satoshi Hike construction law. The conformance vectors are in `conformance/`.

## Not here

The reference programs are in `reference/`. Runnable services are in `services/`. The visitor story is the root README. Admission policy is a relying-party choice, outside this tree.

## Read order

Open [Satoshi Hike v2.0](satoshi-hike-v2.0/Satoshi-Hike-v2.0.md) first. This page is the map. The specification file is the construction. This page does not add a second construction.

## Pins

Vocabulary here is **leg** and **leg result**. The tip is the public sequencer: it paces stages and orders them. Payment, confirmation counts, and an archive of the chain sit outside that role.

Pins are [Satoshi Hike v2.0](satoshi-hike-v2.0/Satoshi-Hike-v2.0.md), released as tag `spec-v2.0.1`.

## Index

| Child | Role |
|---|---|
| [Satoshi Hike v2.0](satoshi-hike-v2.0/Satoshi-Hike-v2.0.md) | **Live construction.** Terrain, leg, leg result, checkpoint, waypoint, and the hike object. |
| `services/admission-gate/WIRE.md` | **Not a construction.** The admission service’s decision-log contract. Companion to Satoshi Hike v2.0 section 15. Not folded into the hike file. |

This tree holds one live hike cut, v2.0. The admission wire sits beside that cut. It is the admission service’s log contract. It is not a second construction.
