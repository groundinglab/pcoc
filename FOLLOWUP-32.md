# Follow-up: Bari plant chaining (#32)

Not done in this change. Bari's plant funding is a confirmed-UTXO reserve, not a single change chain.

`services/bari/src/utxo.rs` selects from outputs with at least `BARI_PLANT_UTXO_MIN_CONF` confirmations (default 1). A 0-conf change is refused on purpose: the comment and the error text say a 0-conf change from the previous plant is not an input. The selector keeps a confirmed pool (default 2–3) and rotates through it (`select_plant_inputs`). `fund_and_broadcast` in `services/bari/src/bitcoin_rpc.rs` spends the selected input through `walletcreatefundedpsbt`.

The proposed rule (see `spec/plant-attribution.md`) is the other shape: each plant spends the change output of the previous plant, and the operator publishes a genesis txid. That needs a new selector, a genesis record, and a decision about the 0-conf refusal. It is larger than a contained edit, so this change does not touch Bari's plant path.

What the follow-up has to cover:

- Spend the previous plant's change output, including while it is still unconfirmed, or define the wait.
- Record the genesis plant txid the operator publishes.
- Keep the 39-byte `BARI` tag unchanged. Attribution stays the funding-input signature.
- Tests for the chain (second plant spends the first plant's change) and for a break (no previous change, plant does not broadcast that chain).
