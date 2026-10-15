# Plant attribution

Status: PROPOSED, not normative.

This note does not change Satoshi Hike v2.0, the conformance vectors, or verifier behaviour. Part A still compares a plant by `root`, `txid`, `vout`, `block_hash`, and `block_height` (specification P14.5). `tree_size` is not compared. A verifier does not read who paid for the plant.

## Initial release

For the initial release, plants are identified by the 39-byte tag only: `OP_RETURN` ‖ push-37 ‖ `BARI` ‖ `0x02` ‖ the 32-byte root. That tag is enough. Bari's current wallet funds each plant from a fresh HD-derived address and sends change to a new one, so the funding inputs do not share one visible operator key. Operator attribution by key is not yet possible on today's plants.

## Funding input (planned)

A later step may attribute a plant to an operator by the key that signed the funding input. When every coin in an operator's pool is locked to that operator's single published key, the funding-input signature attributes the plant whichever pool coin was rotated in. Coins on HD-derived distinct addresses would not be attributable by that key. This step is optional and planned; it is not required for the initial release. Attribution would add no OP_RETURN bytes.

## Confirmed coin pool

Plants are not a single change chain. Bari spends from a pool of several confirmed operator coins (around six), replenished in batches of roughly 10,000 sats each, so a confirmed coin is always available to fund the next plant.

A rebuild finds plants by the 39-byte `BARI` tag. Following one change chain is not required. Identifying an operator's plants by a shared published key on the funding inputs is the planned step above; until then, the tag alone marks a plant. The note does not say a plant without key attribution is invalid. Existing plants stay valid under Part A.

## Several operators

A verifier chooses which operators to trust. Trusting none, one, or several is that verifier's choice. This note does not name a default operator and does not add an operator list to the hike object or to the portable index.

## What a check still ignores

`tree_size` and the time Bari observed the header are off-chain. A chain rebuild of the portable index omits both. The verifier ignores both today: `tree_size` is not a plant comparison (P14.5), and header measurement reads `header.hash`, `header.height`, and `header.time` (section 9). `header.timestamp_unix_ms` is the observation clock. It is not the header time.
