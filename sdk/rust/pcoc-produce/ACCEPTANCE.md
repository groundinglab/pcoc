# Acceptance — live produce

Intent is what the check is for. The test is the proof. These tests do not dial Bitcoin, do not post to the public Bari, and do not time a hike.

| Criterion | Intent | Test |
|---|---|---|
| Tip read never blocks a leg | Rule I3. The producer reads the latest header already held. A peer read that is stuck does not stall the leg. | `tip_read_does_not_block_a_leg` in `sdk/rust/pcoc-produce/tests/live_ac.rs` |
| Fail-closed header source stops produce and writes no object | A source that cannot run is a producer failure. The hike does not invent an object. | `fail_closed_source_writes_no_object` in the same file, and `fail_closed_source_writes_no_object` in `apps/desktop/core/src/produce/live.rs` |
| Bari posts are async, with retry and backoff, against a local mock | Rule A6 and the one resend in §13.1. The caller returns before the response. A 503 is resent once with the same nonce and fee header. A 402 is not resent. | `bari_post_is_async_and_retries_once_after_backoff` and `a_402_is_not_retried` |
| Object is `v2.2-pcoc-object` and passes the SDK verifier on conformance-small | The live producer, a mocked header source, and a mocked Bari emit an object section 14 can measure. | `conformance_small_object_verifies_against_mocks` |
| Desktop start and stop, against a mock | Start hike uses the same producer. Stop finishes the current leg, emits stopped, and leaves no object. | `stop_cancels_before_an_object_is_written` in `apps/desktop/core/src/produce/live.rs` |
| Peer swap and the fail-closed quorum bound stay the #91 gossip session | Produce does not grow a second header source. Two peers are required. A silent or invalid peer is replaced. One peer does not move the tip. | `pcoc-gossip` session tests, including `fewer_than_two_peers_fail_closed_then_recover` and `a_silent_peer_is_replaced_after_the_silence_window`. `live_smoke_agreed_tip_is_at_least_the_anchor` is `#[ignore]`. |

Resume is not supported. `{out}.checkpoint` records the last stage so a crash can be reported. A later run starts over. The file says `"resume": false`.
