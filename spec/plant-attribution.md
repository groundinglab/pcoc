# Plant attribution

Status: PROPOSED, not normative.

This note does not change Satoshi Hike v2.0, the conformance vectors, or verifier behaviour. Part A still compares a plant by `root`, `txid`, `vout`, `block_hash`, and `block_height` (specification P14.5). `tree_size` is not compared. A verifier does not read who paid for the plant.

## Funding input

Each plant is funded by the operator's key. The input that pays the plant is signed by that key, so the signature attributes the plant to the operator's pubkey or descriptor.

The 39-byte tag stays `OP_RETURN` ‖ push-37 ‖ `BARI` ‖ `0x02` ‖ the 32-byte root. Attribution adds no OP_RETURN bytes.

## Confirmed coin pool

Plants are not a single change chain. Bari spends from a pool of several confirmed operator coins (around six), replenished in batches of roughly 10,000 sats each, so a confirmed coin is always available to fund the next plant. Each plant's funding input is signed by the operator's key, and that signature is the attribution.

A rebuild identifies an operator's plants by the operator key on the funding inputs, not by following one change chain. A plant funded by a key the verifier does not attribute to the operator is not that operator's plant. The note does not say such a plant is invalid. Existing plants stay valid under Part A.

## Several operators

A verifier chooses which operators to trust. Trusting none, one, or several is that verifier's choice. This note does not name a default operator and does not add an operator list to the hike object or to the portable index.

## What a check still ignores

`tree_size` and the time Bari observed the header are off-chain. A chain rebuild of the portable index omits both. The verifier ignores both today: `tree_size` is not a plant comparison (P14.5), and header measurement reads `header.hash`, `header.height`, and `header.time` (section 9). `header.timestamp_unix_ms` is the observation clock. It is not the header time.
