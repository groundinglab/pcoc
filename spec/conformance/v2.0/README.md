# Conformance pack, Satoshi Hike v2.0

Historical. This pack was regenerated from the Rust reference under the withdrawn fill pin and the withdrawn FatSlot fold. It is not the v2.1 conformance target. The v2.1 target is `spec/conformance/v2.1/`. The bytes below stay so the old release can still be read.

What conformed at v2.0 is Part A only. See [Conformance vectors](../README.md). The leg-rate figures in `constants.json` (`7/20`, `1/4`, `1/8`) are withdrawn. They are not the v2.1 floor. `sign-in.json` is the locked §15.1 wire. Matching it is not Part A conformance.

## What the files are

| File | Record |
| --- | --- |
| `constants.json` | Section 6 values the reference compiles in, and the anchor-window offsets. `DEFAULT_BIND_THREADS` is a non-normative reference default (section 14.6), not a conformance constant. `LEG_RATE_FLOOR` holds informative §15.5 examples. A section 13 producer and a section 14 verifier are not expected to carry it |
| `checkpoint-preimage.json` | `preimage_hex`, the bytes of one checkpoint preimage, plus that preimage’s length and SHA-256 |
| `anchor-window.json` | One verify record covering a path inside the window, a path outside it, a header that is not in the object, and no path. The hike object is `object_hex`. `header_reads` are the Rule A4.2 bodies (`null` when no document returns). `plant_reads` is the one issued plant GET. That body has `network` and `plant` and no `header` member, so the header token at that height is `not read` and the plant token is `match`. `index_reads` is the ordered list of heights for which a plant GET was issued. It does not include the nineteen header heights. A path outside the window, and a path whose header is not stored, are `plant` `not read` because the read was not issued |
| `anchor-w16-i17.json` | `W_16` included in `last_interrupt` (`I_17`). The hike object is `object_hex`. The verify record’s `anchors` token for that waypoint is `anchored`. `index_reads` is that one issued plant height |
| `earlier-run-b.json` | `earlier-kind.cbor` by path, `object_hex`, and sha256, and the Rule A5 record. Rule A5 does not read the index |
| `bind-threads.json` | The hike object and the verify record produced with 1 thread and with the default thread count. The two runs are the same bytes |
| `test-3leg.json` | TEST vector, mock anchor, not a real anchored PCOC, not a public example. Three legs at the SPEC §16.1 `conformance-small` sizes. The object, the header reads, and the plant read ship in the file. The anchor is the in-repo mock path body. The draw bytes and `legs_per_stage` are rig metadata. The key is a fixed test pattern, not a real key |
| `kat-small.json` | `conformance-small` known answers: terrain bytes, one real leg, that checkpoint, `W_1`, numeric bookends (`a` 1, `b` 2, `N` 1, `S` 3, `delta_h` 3), and a six-leg check of six `match` results. `documents` are the index bodies. `draw_bytes_hex` is the section 14.6 byte source, 256 bytes, consumed in order. Profile integers are sample 32, slot 64, terrain 256, steps 32. Not a production-size object |
| `manifest.json` | The file list the hike pack generator writes |
| `sign-in.json` | Locked section 15.1 sign-in wire: link rules, `challenge_bytes`, the digest, and one signature. Not a section 14 record |
| `sign-in-origins.json` | Section 15.1 **Origin**: one challenge signed for `https://example.com` and for near misses (another deployment's origin, trailing slash, `http`, another port, explicit default port, uppercase), with what a service that serves only `https://example.com` does at check-order step 2. Not a section 14 record |

`sign-in.json` is the locked section 15.1 sign-in wire. It is not a section 14 record. The hike pack generator does not write it and does not delete it. It is not listed in `manifest.json`. `sign-in-origins.json` is the same: written by `services/admission-gate` (`PCOC_REGEN_SIGNIN_ORIGINS=1 cargo test --test signin_origins`, signatures from the reference `nip06` signer), not by the hike pack generator, and not in `manifest.json`, which lists only the hike pack.

`stage0-start.json` is the P9.14 produce vector. The first tip is already known and is observed again after the first leg. Stage 0 stores two legs and its beacon height is that first tip. The next header is `I_0`. Rust, Go, and Java read this file. The hike pack generator does not write it and does not delete it. It is not listed in `manifest.json`. `sign-in-origins.json` is the same: written by `services/admission-gate` (`PCOC_REGEN_SIGNIN_ORIGINS=1 cargo test --test signin_origins`, signatures from the reference `nip06` signer), not by the hike pack generator, and not in `manifest.json`, which lists only the hike pack.

`test-3leg.json` is a TEST vector. It uses a mock anchor. It is not a real anchored PCOC. It is not a public example. It is not an 18-stage object. Test objects are kept short so the pack runs quickly. `legs_per_stage` has length 3, which is shorter than the section 16.1 length of 18. The section 14 record therefore has shape `mismatch` and `shape_item` includes `stages length` and `leg_count`. `field set` applies only to a map that is present with the wrong keys or value types, and the length of this short `stages` array is reported only by `stages length`. The present stages each have `leg_count` 1. The missing stages have no `leg_count`, and section 14.1 treats that absent field as a violation. Three legs is fewer than six, so the injected draw bytes are not consumed and `six_leg` is `not defined`.

`test-3leg`, `anchor-window`, `anchor-w16-i17`, and `bind-threads` are verifier inputs. A section 13.2 producer does not emit them. `test-3leg` stores three stages. `anchor-window`, `anchor-w16-i17`, and `bind-threads` store `leg_count` 0 on at least one stage. Section 13.2 runs the repeat at least once, so `leg_count` is at least 1 on every stage, and eighteen stages stay eighteen. `kat-small` is the producer byte-equality target.

`kat-small.json` is the 18-stage known-answer object at the same small sizes. Its `record` has shape `match`. Bookends are the integers above. `whole_hike_legs` is 17 and `legs_outside` is 17. `index_reads` is `[1004, 1004]`: two issued plant GETs of one height. The header reads are the nineteen `documents`, and the document at height 1004 carries both plants. A section 13.2 producer replay of that object equals `object_hex`.

The verify bytes in this pack are the section 14 record that `satoshi-hike-verify` writes. Keys are in the order SPEC section 14 lists, with no insignificant whitespace and one trailing newline. The `record` string is those bytes, including that newline. The newline that ends the JSON file closes the wrapper. It is not a substitute for the newline inside `record`. An earlier kind is one line, `version` `earlier version` and `kind`. It is not a shape `mismatch`. A current object’s record carries every section 14 key, including `shape_item`, `headers`, `paths`, `anchors`, bookends, `N`, and `S`. The compact line that named only `profile`, `shape`, `checkpoints`, `waypoints`, `leg_rate`, `six_leg`, and `anchors` is not a conformance output. SPEC section 16 names this pack as these reference bytes. Reason `earlier_version` is Gate policy in SPEC section 15. It is not in these records.

## Regenerate

From the repository root, with the reference toolchain:

```text
PCOC_REGEN_V2=1 cargo test --manifest-path reference/rust/hike/Cargo.toml --test conformance_v2 -- --exact conformance_v2_pack
```

That writes this directory from `reference/rust/hike`. Then:

```text
git diff --exit-code spec/conformance/v2.0
cargo test --manifest-path reference/rust/hike/Cargo.toml --test conformance_v2 -- --exact conformance_v2_pack
```

The second command reads the committed files and checks them against a fresh run. `PCOC_REGEN_V2` unset does not write.
