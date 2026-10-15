# Plant attribution

Status: PROPOSED, not normative.

This note does not change Satoshi Hike v2.0, the conformance vectors, or verifier behaviour. Part A still compares a plant by `root`, `txid`, `vout`, `block_hash`, and `block_height` (specification P14.5). `tree_size` is not compared. A verifier does not read who paid for the plant.

## Funding input

Each plant is funded by the operator's key. The input that pays the plant is signed by that key, so the signature attributes the plant to the operator's pubkey or descriptor.

The 39-byte tag stays `OP_RETURN` ‖ push-37 ‖ `BARI` ‖ `0x02` ‖ the 32-byte root. Attribution adds no OP_RETURN bytes.

## Chained plants

Each plant spends the change output of the previous plant. The operator publishes one genesis plant txid. A verifier that wants the operator's sequence starts at that txid and follows each change output to the next plant.

A plant that does not spend the previous plant's change is not in that chain. The note does not say such a plant is invalid. Existing plants stay valid under Part A.

## Several operators

A verifier chooses which operators to trust. Trusting none, one, or several is that verifier's choice. This note does not name a default operator and does not add an operator list to the hike object or to the portable index.

## What a check still ignores

`tree_size` and the time Bari observed the header are off-chain. A chain rebuild of the portable index omits both. The verifier ignores both today: `tree_size` is not a plant comparison (P14.5), and header measurement reads `header.hash`, `header.height`, and `header.time` (section 9). `header.timestamp_unix_ms` is the observation clock. It is not the header time.
