# Producer Process

**Responsibility.** Reference live sequence for a producer (tip observation, stage boundaries, posting, path asking, restart). Used by a producer implementer who wants that sequence.

**Source.** Inherited without change of wording from the following sections of Satoshi Hike 2.3 (`spec/satoshi-hike-v2.3/SPEC.md`):

- §13 Produce, Rule A1 (Event order at stage boundaries)
- §13.1 Stage path
- §13.2 Procedure
- §13.3 Path skip and not-fetched-by-design
- §13.4 Bari restart
- §13.5 Live event log

---

## 13. Produce

**Normative.**

Inputs are the 32-byte public key and one plane profile: the tip sequence and `{bari-url}`. The default profile is the test plane. The live profile is explicit. A missing parameter is not filled from the other plane.

`{bari-url}` is an absolute URL with scheme `http` or `https` (ASCII case-insensitive) and no trailing slash, the same rule section 9 states for a URL. Produce does not read `{index-url}`.

A public-key input may be those 32 bytes, or a hexadecimal spelling of them under section 7.2 with an optional leading `0x` and exactly 64 digits. The object stores the 32 bytes.

Output is one hike object in the encoding of section 7.3. Every run that reaches the final interrupt `I_17` emits it. A clock-read failure, a header-source stall, any other transitory network condition, a waypoint POST that does not succeed, and an ask that does not complete never withhold the object. Rule A3.4 stores empty path bytes and produce continues. A run that a producer failure ends, or that is cancelled, emits no object. A producer failure is a bug or a resource error in the producer itself; it is not a network, Bari, peer, or clock outcome. On the live profile, produce also appends the event log in section 13.5, asynchronously, while the drive runs. That log is not the hike object. Only the live profile writes it.

<a id="rule-a1"></a>

### Rule A1. Event order at every stage boundary

Produce is a continuous drive from the seed beacon through `STAGES`. A Bitcoin header interrupts that drive. The interrupt is a world event, unknown in time and value. Each stage boundary is one event, in this order. There is no rebinding. A later header is the next interrupt, read after the next stage’s next checkpoint. Never say a header arrives during the waypoint post.

<a id="rule-a1-1"></a>

#### Rule A1.1 Legs, then the tip

Stage `g`’s legs run back to back. After each checkpoint the producer reads the tip: the latest header its header source already holds. That read returns at once and never waits. A leg already in progress always finishes.

<a id="rule-a1-2"></a>

#### Rule A1.2 The interrupt ends the stage

When that read shows a new interrupt `I_g`, leg production in stage `g` stops.

<a id="rule-a1-3"></a>

#### Rule A1.3 Waypoint byte order

The producer builds `W_{g+1}` by section 12.2:

```text
W_{g+1} = SHA-256(I_g ‖ slot_sample(I_g) ‖ stages[g].waypoint ‖ fold_g)
```

The third limb is the seed on `g = 0` and `W_g` on `g ≥ 1`. The seed is not called `W_g`.

<a id="rule-a1-4"></a>

#### Rule A1.4 Post (asynchronous)

When `W_{g+1}` is known, the producer starts the post of `W_{g+1}` to Bari with the Hashcash fee (section 13.1) as an asynchronous event. The post does not delay stage `g+1`’s leg 0 or any later leg. Its outcome is not an input to any leg, checkpoint, leg result, or waypoint.

<a id="rule-a1-5"></a>

#### Rule A1.5 Pivot

`W_{g+1}` is the pivot. Stage `g+1`’s leg 0 starts from it at once. It does not wait for the post.

Stage 0 has no boundary before it. After the seed Observe and the terrain fill, produce enters stage 0’s legs from the seed. There is no Bari post. The first Bari post is `W_1`, started after `I_0` alongside stage 1’s legs. The interrupt that ends stage 17 is `last_interrupt`. Produce stores it and builds no waypoint from it.

Between interrupts the drive runs legs of `STEPS_PER_LEG` steps and records a checkpoint after each leg. The checkpoint binds the leg result, the seed or the stage waypoint, the previous checkpoint, and the terrain sample. That record is what the drive did. `leg_count` is how many of those legs were recorded before the interrupt. Several legs in one stage are the consequence of an uninterrupted stretch. This specification does not bound the wall-clock time of a leg. It does not set a steps-per-second quota. It does not set a leg quota. Principle 9: there is no minimum or maximum leg count. `leg_count ≥ 1` follows from Rule A1.1, because the repeat in section 13.2 runs at least once.

`Observe` performs the read in section 9 and returns `(natural, height)`.

<a id="rule-a6"></a>

#### Rule A6. Nothing pauses a leg

**Normative.** The walk is the back-to-back run of legs from the seed through `I_17`. Every waypoint post, Hashcash solve, path ask, peer or header-source read, Bari exchange, event-log write, and stderr write is an asynchronous event. No such event pauses, delays, orders, or interrupts a leg, and no leg waits for one to finish. The only read on the walk path is the post-checkpoint tip read (Rule A1.1), which returns at once the latest header the header source already holds. A failed read, a stalled header source, a Bari outage, a log that cannot be written, a slow peer, or any other transitory network condition is logged (section 13.5), and the walk continues. A run ends only at `I_17`, on a producer failure, or on explicit cancellation. Bugs are producer failures; they are not covered by this rule. Asynchronous work may share the host’s CPU with legs. This rule does not change what a leg computes, and it does not permit any change to the terrain, the step, the fold, or the slot read.

<a id="sec-13-1"></a>

### 13.1 Stage path

For a stage `g ≥ 1`, when its waypoint `W_g` is known, produce starts the post of that waypoint as an asynchronous event and starts the stage’s leg 0 at once (Rule A1.4, Rule A6). It is the pivot built after `I_{g−1}`. The seed is never posted. Stage 0 has no plant and no path ask.

**Anchor window.** This is the only list of offsets.

```text
ANCHOR_WINDOW_OFFSETS = 2, 3, 4
```

A waypoint posted at height H is asked when the producer’s header source learns each header whose height is H plus one offset in that list, in list order. An ask is an asynchronous event (Rule A6); its answer may arrive while later legs run. If the path is served, it is stored and later headers in the window are not asked. If it is not served, the producer asks at the next offset, and then at the last offset. After the last offset a miss stores an empty path and the stage does not keep waiting. Nothing is asked before the first offset or after the last. One header after the last offset, the path is gone. An ask is made only for a waypoint whose post returned 204. Test and live use this one rule. An operator setting does not change the list.

For `W_g` with `g` from 1 through 16, H is `stages[g].beacon_height`. That is the height of the interrupt header the object already stores for that stage: `I_{g−1}` (`stages[g].beacon`). The producer asks at each H plus one offset. The fee header on the POST is not H. The object does not store a second post height.


**Fee header.** When the post of `W_g` starts, the producer takes as fee header the 80-byte block header its header source holds as best tip. Its `header_hash` is `SHA-256(SHA-256(header))`, 32 bytes in natural order. Taking it is not a tip read of the walk, does not end a stage, and is not `I_g`. If the header source holds no header, the POST does not succeed: produce logs the miss and continues.

After the post is started, the drive continues. Produce never pauses a leg for the post, its answer, the solve, a further header, or a wall clock (Rule A6). A header newer than `I_{g−1}` is read at the tip read after this stage’s first checkpoint. If it meets the interrupt rule of section 9, it ends this stage at `leg_count = 1`. A stage that ends at one leg is the interrupt the producer recorded.

**Hashcash.** This section defines Hashcash completely. Produce and Bari use exactly this puzzle and no other. No implementation file defines it.

`bari_origin` is the ASCII serialization of the web origin of `{bari-url}` under RFC 6454 section 6.2: the lowercase scheme (`http` or `https`), then `://`, then the host in lowercase (an internationalized host in its A-label form), then `:` and the port in shortest decimal only when the port is not the scheme’s default (80 for `http`, 443 for `https`). It has no userinfo, no path, no query, no fragment, and no trailing slash. The challenge is `challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)`, where `"bari-hc-v2"` is those ten ASCII bytes and `header_hash` is the fee header’s 32-byte natural-order hash. A nonce is a `u64`. Its digest is `SHA-256(challenge ‖ waypoint ‖ le64(nonce))`. Leading zero bits are counted from the first digest byte. Inside a byte the high bit is first. A zero byte contributes eight bits and the count continues. The first non-zero byte contributes that byte’s leading zeros and the count stops. The challenge binds the fee to one Bari origin and one header of the Bitcoin best chain (section 13.6); a fee paid at one Bari operator is not paid at another.

The difficulty is 21 leading zero bits. That is the one Hashcash difficulty value in this specification. Produce and Bari both use it. Two environment variables that can disagree are not allowed.

The nonce is the first `u64` in the order 0, 1, 2, … whose digest meets the difficulty. If that search exhausts every `u64`, the POST does not succeed; produce logs the miss and continues. The request carries two headers: `X-Bari-Hashcash-Nonce`, the nonce in decimal, and `X-Bari-Hashcash-Header`, the 80 fee-header bytes in wire order as 160 lowercase hexadecimal digits, byte 0 first. The solve is asynchronous work (Rule A6).

Produce sends one `POST {bari-url}/waypoint` as an asynchronous event and does not follow a redirect. The body is the 32 waypoint bytes. `Content-Type` is `application/octet-stream`. The request carries both Hashcash headers. Status 204 means Bari holds the waypoint (section 13.6). Any other answer is final for that waypoint: produce logs it and continues. A transitory send failure is the only case that triggers the resend. The miss is kept until the stderr step at the close (Rule A3.4). There is no fee bump, RBF, replacement, or repair of a missed plant. A missed POST does not stop the hike, does not pause a leg, and does not withhold the object.

**One resend.** A transitory send failure is a POST that received no HTTP status line (connection refused, name resolution failure, or a connection reset or timeout before a status line), or that received status 502, 503, or 504. That is a transport or proxy condition, not a Bari answer: Bari’s only answers to a waypoint POST are 204 and 402 (Rule BR1). After a transitory send failure the producer resends the identical request exactly once, in the background, 30 seconds after that transport failure, and then drops the waypoint if the resend fails. The resend carries the same body, the same nonce, and the same fee header; it does not solve again. If the resend fails too, in any way, the waypoint is dropped and produce sends no further POST for that stage. Both attempts are logged (section 13.5). No other outcome is retried, including status 402, 408, 429, and every other status. This is the only waypoint-POST retry this file allows. Bari’s dedupe (Rule BR2) makes the resend safe. The nonce header value is the shortest decimal digits of the `u64`, with no sign, no leading zeros, and no surrounding space. Zero is `0`.

The POST that returned 204 is the plant. H stays the stage beacon height defined above. The fee header on that POST is not H.

The serve window is the anchor window above. A plant posted at height H can be included at a height in that window. The path is served through the last offset. It is gone one header after that offset.

On the live profile, Bari's plant is a Bitcoin transaction. The anchor is the block that includes that transaction. Bari serves the waypoint path for the anchor window. The producer posts the waypoint to Bari and asks Bari for the path. This specification does not define a path that anchors by talking to Bitcoin and skipping Bari.

The leaf is the waypoint’s 64 lowercase hexadecimal digits, byte 0 first. An ask sends `GET {bari-url}/path/{leaf}` and does not follow a redirect. An ask is due when the producer’s header source learns a header at a height in the anchor window, before the close. Produce sends it then, as an asynchronous event, with no wait. Produce must make the ask at every due header among `W_1` … `W_16`. Watching only the newest tip is not enough. `W_17` is not asked.

- Status 200: a checked path is a 200 response whose bytes are a path for that waypoint. The bytes are a path for that waypoint when they decode as a canonical path body as defined in section 14.3 and its `validate_for` succeeds for that waypoint, including the Merkle checks in section 14.3. Produce checks that before it stores the bytes. A 200 that does not check is a miss. Produce does not require a `Content-Type`. The path’s block must be one height in the anchor window for the stage that posted at H. It does not have to be the same header as `I_{g−1}`.
- Status 404 names which of these it is: still in the pool, sealed but not in a block yet, not included, already archived after it was served, or unknown. Any of those is a miss. Produce makes the ask at every header where it is due.
- Any other status, or an ask that does not complete, is a miss. Produce continues.

An ask that has not completed at the close, or that returns anything other than a checked 200, is a miss. At the close, a waypoint `W_1` … `W_16` with no checked path stores empty bytes. The `path_skip <g>` line is written in the stderr step at the close (Rule A3.4), not when the ask returns. Produce must make the ask at every due header. A miss does not stop the hike and does not withhold the object.

There is no silent skip. A silent skip stores path bytes that were not fetched and leaves no record. That is forbidden. “No silent skip” is not a reason to pause the drive for further headers. That pause does not satisfy it. Section 13.3 separates that rule from `path_skip`.

The `path` bytes are not an input to section 11.4 or section 12.2.

**Close.** The cut-off is `last_interrupt_height`, the height of `I_17`. Production ends when the tip read shows `I_17`, and the object closes at once: produce encodes it with every checked path that has already arrived. There is no collection window. An ask still in flight is abandoned and its waypoint is a skip. An answer that arrives after the close is not used, and no ask is made after the close. An ask made at the cut-off header counts only when its checked answer arrived before the tip read that closes the object. `W_17` is not asked. A jump that puts `H+2`, `H+3`, or `H+4` for `W_17` at or below the cut-off does not create an ask, and produce does not send `GET {bari-url}/path/{leaf}` for `W_17`. Produce stores empty path bytes for stage 17. Produce does not wait for an ask, a post, or a header at the close.

**Informative.** The object closes on the final block, so the last waypoints are normally not fetched; they are skips. That is expected. Sixteen waypoints, `W_1` … `W_16`, are fetchable (Rule A3.3). `W_17` is not.

<a id="sec-13-2"></a>

### 13.2 Procedure

```text
(seed_beacon, height_0) ← Observe()
seed_preimage ← public_key ‖ seed_beacon        # 64 bytes, section 10; no label
seed ← SHA-256(seed_preimage)                   # stages[0].waypoint; equals the seed by design
terrain ← section 10.4                          # NL4, once per terrain; not per leg

waypoint ← seed
(beacon, beacon_height) ← (seed_beacon, height_0)

for g in 0 … 17:
    if g ≥ 1:
        start post of waypoint       # W_g; section 13.1; asynchronous; never awaited; does not end a stage
    previous ← waypoint              # leg 0: seed on stage 0, W_g on stage g ≥ 1
    checkpoints ← []
    leg_results ← []

    repeat:
        leg_result ← LegResult(terrain, previous)     # section 11.2, STEPS_PER_LEG steps
        sample ← terrain sample at previous           # section 11.3, SAMPLE_BYTES
        checkpoint ← SHA-256(
            previous ‖ waypoint ‖ sample ‖ le64(leg_result))
        append checkpoint to checkpoints
        append le64(leg_result) to leg_results
        previous ← checkpoint
        (tip, tip_height) ← TipRead()                 # Rule A1.1; after the checkpoint; returns at once
        if tip ≠ beacon and tip_height > beacon_height:
            (interrupt, interrupt_height) ← (tip, tip_height)    # I_g; Rule A1.2
            stop the repeat

    record stages[g] with
        beacon, beacon_height, waypoint,
        leg_count = len(checkpoints),
        checkpoints, leg_results
        # stage 0: no path key

    if g < 17:
        waypoint ← SHA-256(interrupt ‖ slot_sample(interrupt) ‖ stages[g].waypoint ‖ fold(checkpoints))
                                                      # Rule A1.3; the pivot W_{g+1}
        (beacon, beacon_height) ← (interrupt, interrupt_height)  # I_g stored as stages[g+1].beacon
    else:
        (last_interrupt, last_interrupt_height) ← (interrupt, interrupt_height)
                                                      # I_17; no waypoint is built

# close: at I_17, at once; no collection window
# asynchronously, throughout the run: on each header the header source learns,
#   start every path ask for W_1 … W_16 due at that height (section 13.1); none after the close
cut_off ← last_interrupt_height                       # W_17 is not asked; in-flight asks are abandoned
for g in 1 … 16:
    stages[g].path ← stage g's checked path body if it has arrived, or empty bytes
stages[17].path ← empty bytes                         # not asked, including when a jump would put an offset at or below the cut-off

encode the object                                     # section 7.3; stage 0 has no path key
write the stderr step: path_skip <g> ascending g, then not fetched by design 17   # does not delay the object
```

The repeat body always runs at least once, so `leg_count ≥ 1`. Each iteration is one leg of the drive, exactly `STEPS_PER_LEG` steps, then one checkpoint record. The checkpoint preimage is the previous checkpoint, the seed or the stage waypoint, the terrain sample at that previous checkpoint (`SAMPLE_BYTES`), and `le64(leg_result)`, as section 11.4 defines. The tip read runs after the checkpoint is recorded, not in the middle of a leg, and returns at once. That read is how the drive notices the interrupt. The section 13.1 fee-header read is not an interrupt read.

Stage 0 starts with no post. Its legs begin after the terrain fill, from the seed. The interrupt comes first. The waypoint of the next stage is built from it, and its post starts after it, alongside the next stage’s legs. That waypoint is the pivot. `I_g` is stored as the next stage’s `beacon`. It is not replaced.

Stage 0 has no plant and no ask. For `W_1` … `W_16`, the ask window is section 13.1. Those asks are asynchronous events that run while the drive is in whichever stage it has reached. No ask is made after the close. `W_17` is not asked. An ask does not end a stage. A miss does not stop the hike.

When stage 17’s repeat stops at `I_17`, produce records stage 17, stores `last_interrupt` and its height, builds no waypoint, and closes the object at once (section 13.1 **Close**). It does not wait for an ask, a post, or a header.

<a id="sec-13-3"></a>

### 13.3 Drive, path_skip, and not fetched by design

**Normative.**

`leg_count` is how many legs the section 13.2 drive recorded before the interrupt. The count is that stretch. It is a convenience copy of `len(checkpoints)`. The repeat runs at least once, so `leg_count ≥ 1`. Principle 9: there is no minimum or maximum leg count beyond that. Produce does not enforce a rate.

A stage-loop test rig checks one production stage of this drive. A result there checks that one stage. It does not check two headers.

Produce does not invent path bytes. A checked 200 is stored. Anything else is a miss, and the miss is recorded. That is “no silent skip”. A miss does not stop the hike.

If the POST for `W_g` does not return 204, after the one resend section 13.1 requires, no path ask is due for that waypoint, because there is no plant height. Produce continues. The miss is kept for the final stderr step. If the POST returns 204, produce makes every ask that falls due before the close. At the close, produce writes every stderr line in one step: `path_skip <g>` for each empty path among `W_1` … `W_16`, in ascending `g`, then `not fetched by design 17`. No stderr line for those paths is written before that step. That step does not delay the object.

<a id="rule-a3-4"></a>

#### Rule A3.4 Produce stderr

The stderr line for an empty path on `W_g`, with `g` in `1 … 16`, is exactly one line:

```text
path_skip <g>
```

`<g>` is the decimal waypoint index with no leading zeros. It is the same `g` as in `W_g` and `stages[g]`. The line is the ASCII bytes of `path_skip`, one ASCII space (U+0020), the decimal digits of `g`, and one LF (U+000A). No carriage return and no second space. This includes a POST that does not succeed, a miss on an ask, an ask still in flight at the close, and a waypoint with no ask due before the close. Produce stores empty path bytes and continues. Empty bytes are not a canonical path body. `path_skip` is not a silent skip. It does not authorize pausing the drive to wait for further headers. Test and live use this one line. No other spelling is this record.

That final step writes the lines in ascending `g`. Within that step, a line for a smaller `g` is not written after a line for a larger `g`. `not fetched by design 17` is last.

The stderr line for `W_17` is exactly the ASCII bytes of `not fetched by design`, one ASCII space (U+0020), `17`, and one LF (U+000A):

```text
not fetched by design 17
```

That line is the last line of the final stderr step. It is not `path_skip`. `W_17` is never asked, including when a jump would put `H+2`, `H+3`, or `H+4` at or below the cut-off. Produce does not send `GET {bari-url}/path/{leaf}` for `W_17`. Produce stores empty path bytes for stage 17 and continues to encode the object. It does not authorize waiting for a header above the cut-off. Produce does not write `not fetched by design` for `W_15`, `W_16`, or any other waypoint. Produce writes nothing on stderr for stage 0. Stage 0 has no path.

<a id="sec-13-4"></a>

### 13.4 Bari restart

**Informative.**

Replacing a live Bari during a running produce is normal. A waypoint already in the pool is retained across a quick restart. A skip is the worst case, not a fee bump.

<a id="sec-13-5"></a>

### 13.5 Live event log

**Normative.**

The live profile writes one append-only event log while produce runs, so a reader can see progress before the hike object exists. Only the live profile writes this log. The log is not the hike object, it is not a field of that object, and it is not an input to a leg, a checkpoint, a leg result, or a waypoint. The producer does not write the log from inside a leg. It does not write one line per checkpoint. It does not write one line per leg result. It does not write the log on the path that places the hike object. Every append is an asynchronous event (Rule A6).

The file path is the hike object path with the suffix `.events`. At the start of a live run the producer creates that file, replacing a file already at that path, and then only appends. Each record is one compact JSON object with no insignificant whitespace, keys in the order listed below for that record, then one newline (U+000A). The producer appends each record after the work that record names, in the order the events happened. Appending is asynchronous: produce does not wait for a record to be written, flushed, or synced, and this file sets no durability requirement. The hike object is written at the close. If the log is not created or a record is not appended, the hike, the path ask, and `path_skip` are unchanged.

A `tip` in this log is the shared public tick: the natural hash and the height. It is not a count of confirmations, not a fee, and not an archive. This section does not add a wait for a tip.

The records are:

1. **Block.** After the live producer reads a new header from the tip sequence. A header it ignores is not a record. `event` is `hike.produce.block`. `height` is that header’s height. `tip` is 64 lowercase hexadecimal digits, byte 0 first, the natural hash.

2. **Waypoint.** When the live producer has waypoint `W_g` for stage `g ≥ 1` and starts its post. There is no waypoint record for stage 0. The seed is not posted. `W_g` is the pivot from section 12.2. The record is not written inside that calculation. `event` is `hike.produce.waypoint`. `stage` is `g`, from 1 through 17. `waypoint` is 64 lowercase hexadecimal digits, byte 0 first.

3. **Posted.** After each POST attempt for that stage’s waypoint has settled: the first POST and, when section 13.1 requires it, the one resend. Each attempt is its own record. `event` is `hike.produce.posted`. `stage` and `waypoint` are the same as the waypoint record. `attempt` is 1 for the first POST and 2 for the resend. `posted` is true when Bari answered the POST, and false when it did not. `succeeded` is true only when that attempt returned status 204. Status 204 means Bari holds the waypoint. `dropped` is true when that attempt did not succeed and no further attempt will be made, and false otherwise. `status` is that HTTP status when Bari answered, and is omitted when it did not. A missed plant is not fee-bumped, replaced, or repaired.

4. **Checked.** After a status 200 body is checked under section 13.1, and only then. A miss is not this check. `event` is `hike.produce.checked`. `waypoint` is the asked waypoint, 64 lowercase hexadecimal digits, byte 0 first. `checked` is true only when the body is a path for that waypoint and the path’s block is one height in the anchor window. Otherwise `checked` is false.

5. **Fetch.** After `GET {bari-url}/path/{leaf}` has returned and the body has been read. The record is not written when the request is sent. `event` is `hike.produce.fetch`. `waypoint` is the asked waypoint, 64 lowercase hexadecimal digits, byte 0 first. `worked` is true only when this fetch is the successful fetch section 13.1 stores. `status` is the HTTP status when the producer has one, and is omitted when it does not. `block` is the path body’s block hash, 64 lowercase hexadecimal digits, byte 0 first, natural order, not Bitcoin display order, and `block_height` is that block’s height. Those two keys are present only when the body names that block. `miss` is present only when the body is one of these phrases: `still in the pool`, `sealed but not in a block yet`, `not included`, `already archived after it was served`, `unknown`. A miss stays a miss. `not included` is not a successful fetch. Stderr `path_skip` remains the record that an empty path was stored. This fetch record does not replace it.

6. **Alert.** When the run meets a transitory condition that does not stop it, for example a tip read that did not succeed, a header-source stall, a fee header that is not available, or a log append that did not succeed (written when appending works again, if it does). `event` is `hike.produce.alert`. `alert` is a short lowercase name of the condition, for example `clock_read_failed`, `header_source_stalled`, `fee_header_missing`, or `log_append_failed`. `stage` is the stage the walk was in. `height` is the height concerned, and is omitted when there is none. An alert is not an input to the hike and does not change the object.

Key order, omitting a key this section says is omitted:

1. Block: `event`, `height`, `tip`.
2. Waypoint: `event`, `stage`, `waypoint`.
3. Posted: `event`, `stage`, `waypoint`, `attempt`, `posted`, `succeeded`, `dropped`, then `status` only when Bari answered.
4. Checked: `event`, `waypoint`, `checked`.
5. Fetch: `event`, `waypoint`, `worked`, then `status` only when the producer has one, then `block` and `block_height` only when the body names that block, then `miss` only when the body is exactly one of the five phrases and no other bytes.
6. Alert: `event`, `alert`, `stage`, then `height` only when there is one.

The log does not choose a Bari URL. `{bari-url}` is the URL section 13 uses. A producer may name any Bari operator’s https origin. An SDK default, if an SDK has one, is not a requirement. This section does not define a path that skips Bari.

<a id="sec-13-6"></a>

### 13.6 Bari waypoint service

**Normative for a Bari operator.** Bari is the service at `{bari-url}`. This section states what a conforming Bari does with a waypoint POST and a path ask. It is the whole Bari rule set for this file. An implementation constant does not add another rule.

<a id="rule-br1"></a>

#### Rule BR1. Paid waypoint

Bari only accepts or denies a submitted waypoint. It accepts a POST if and only if the POST carries a valid Hashcash payment solved within ±1 block of Bari’s current block. That is, all of these hold:

1. The body is exactly 32 bytes. `X-Bari-Hashcash-Nonce` is the shortest decimal spelling of a `u64`. `X-Bari-Hashcash-Header` is 160 hexadecimal digits, the 80 fee-header bytes.
2. The fee header is one of these three blocks, judged against Bari’s own view of the Bitcoin best chain when the POST arrives:
   - **current:** its `header_hash` equals the hash of Bari’s tip;
   - **before:** its `header_hash` equals the previous-block hash of Bari’s tip;
   - **after:** its previous-block field equals the hash of Bari’s tip, its `nBits` equals the required target for the plane (live plane: the Bitcoin mainnet consensus target for the height of Bari’s tip plus one, including a retarget at a 2016-block boundary; test plane: the fixed synth `nBits` of section 9), and `SHA-256(SHA-256(header))`, read as a little-endian 256-bit integer, is at most the target that `nBits` encodes.
3. The section 13.1 digest over `SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)`, the 32 body bytes, and the nonce has at least 21 leading zero bits. Here `bari_origin` is Bari’s own public origin, serialized as section 13.1 states.

An accepted waypoint is answered 204 and Bari holds it. Nothing else about timing applies: the gap between Bari’s tip and the producer’s tip, the arrival time, and which of the three blocks the fee was solved on do not change the answer. Any other POST, unpaid or invalid in any way, is answered with a plain 402 and nothing more. Bari’s only answers to a waypoint POST are 204 and 402.

<a id="rule-br2"></a>

#### Rule BR2. Dedupe

**Normative.** Dedupe applies only to paid posts. The dedupe key is the 32 waypoint bytes within one pool. A POST that meets Rule BR1 and whose waypoint Bari already holds in the current pool is answered 204 and makes no second entry. Any POST without a valid fee is answered with a plain 402, duplicate or not. Bari checks the fee before dedupe.

<a id="rule-br3"></a>

#### Rule BR3. Pool and tree

**Normative.** Bari appends each accepted waypoint to its pool. When a beacon arrives, that is, a new block on Bari’s best chain, Bari builds the tree from the pool, and that clears the pool for the next stage. Pool capacity, and what happens when a pool is full, are out of scope of this file.

<a id="rule-br4"></a>

#### Rule BR4. Path serve

**Normative.** `GET {bari-url}/path/{leaf}` answers 200 with the canonical path body (section 14.3) once the waypoint’s plant is included in a block, through the last offset of the anchor window (section 13.1). Otherwise it answers 404 with exactly one of the phrases `still in the pool`, `sealed but not in a block yet`, `not included`, `already archived after it was served`, or `unknown`. A path ask is never refused for timing.

**Informative.** Bari’s cost per entry is one Hashcash fee. The fee header can only come from a three-block span of the best chain, so precomputed fees are bounded to that span, and the origin binding stops one fee from being reused at a second Bari operator. The 21-bit difficulty is a constant of this file (P13.5).

<a id="sec-14"></a>

