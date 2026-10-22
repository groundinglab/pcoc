# Gate wire

Satoshi Hike v2.0 only. The `/v1` prefix stays: the website and the phone already call these paths. This process does not mint a session and does not call the website.

## Routes

```text
POST /v1/sign-in                    admission. Locked six-field body (SPEC §15.1).
GET  /v1/sign-in/result/{nonce}     locked wire. One website outcome. 64 lowercase hex.
GET  /latest.json                   unsigned tip, after a durable line
GET  /log/{YYYY-MM-DD}.jsonl        lines whose time_unix_s falls on that UTC date
GET  /log/{YYYY-MM-DD}/{seq}        one decision-log line
POST /v1/verify                     202 { "job_id" }
GET  /v1/verify/{job_id}            status; the section 14 record when the object is v2.0
GET  /v1/health                     version, commit, sdk, spec kind, config summary
GET  /v1/policy                     the policy in force (policy_id and its parameters)
```

Every route also answers `OPTIONS` (CORS preflight). Any other path is HTTP 404 `not_found`. Source: `src/http.rs` router.

`POST /v1/admit`, `POST /v1/challenge`, `POST /v1/sign-in/challenge`, and any friendly-list path are not routes. The response is HTTP 404 `not_found`. There is no profile file, no friendly list, and no held state.

## POST /v1/sign-in

One JSON body is this route. It is the locked SPEC §15.1 body. `{object_b64, signature}` without the other four fields, a raw object, and unpadded base64 are not a sign-in. A body that is not the six-field object is `malformed`. A content type other than `application/json` is HTTP 415 and writes no website outcome.

### Locked body (SPEC §15.1)

`Content-Type: application/json`. The six keys appear once:

```json
{
  "origin": "https://groundinglab.io",
  "nonce": "<64 lowercase hex>",
  "exp": 0,
  "object": "<64 lowercase hex of SHA-256(object bytes)>",
  "object_b64": "<standard base64 of the object bytes, with = padding, no whitespace>",
  "signature": "<128 lowercase hex, 64 bytes>"
}
```

`exp` is a JSON integer in the `u64` range, not a string. The signature origin is the body field `origin`. It is the requesting site's own web origin: any site may run a gate, and each gate binds the origin of the site it serves. `https://groundinglab.io` is the origin of groundinglab's instance. The accepted origins are the required config key `served_origins` (no default; groundinglab's `deploy/gate.toml` sets `["https://groundinglab.io"]`). It is separate from the CORS `allowed_origins`. It is not the caller's `Origin` header.

The website link is `https://groundinglab.io/sign-in/?v=1&nonce=<64 lowercase hex>&exp=<decimal>`, in that order and with no other parameter. `v` stays `1` because the domain tag below is unchanged. The link does not carry `object`. This door does not parse the link. A link that contains `object=` is not a link, and this process never sees one.

`challenge_bytes` is rebuilt from the body. It is the domain ASCII `pcoc-gate-signin-v1`, one `0x00`, one length byte, the origin UTF-8, the 32 nonce bytes, the expiry as `uint64` big-endian, and the 32-byte `object` field. For `https://groundinglab.io` that length is 116. The BIP-340 message is `SHA-256(challenge_bytes)` once. The door does not hash that digest again.

The window accepts the challenge when `now < exp + 5` and `exp <= now + 120 + 5`. The sums use wide integers, so they do not wrap. Lifetime is 120 seconds. Skew is 5 seconds. The published example expiry `1893456000` is outside a clock near 2026-10-10.

The client sees one of three bodies. There is no `message` field and no field that says which check missed.

```json
{"delivery":"ok"}
{"error":{"code":"bad_signature"}}
{"error":{"code":"malformed"}}
```

Delivery OK is every completed response other than those two HTTP 400 bodies, including HTTP 413, 415, 429, and 503. The client shows `Sign-in submitted` for delivery OK. `unknown_challenge`, `expired`, and `reused` are not client codes.

`GET /v1/sign-in/result/<64 lowercase hex nonce>` returns HTTP 200.

An unknown nonce, a nonce whose row has expired, and any token other than a consumed `pending` or `accepted`, is one field:

```json
{"outcome":"<token>"}
```

The token is `pending`, `accepted`, `malformed`, `bad_signature`, `too_short`, `rejected`, `earlier_version`, or `too_few_waypoints_anchored`. `rejected` has no extra field. An unknown nonce, and a nonce whose row has expired, is `pending`. That read creates no row.

A consumed row that is still `pending` adds `submitted`:

```json
{"outcome":"pending","submitted":true}
```

`accepted` adds the measurements the door stored. `public_key` and `object_sha256` are 64 lowercase hex. `legs` is bookend `N` and `span_s` is bookend `S`. Each bookend is present only when that side of the record is a number. There is no floor, no threshold, and no verifier note. `log_day` and `log_seq` name the decision-log line that opened the claim window; each is present only once that line is written.

```json
{"outcome":"accepted","public_key":"<64 hex>","object_sha256":"<64 hex>","legs":1348,"span_s":5294,"log_day":"<YYYY-MM-DD>","log_seq":<n>}
```

The check order returns on the first step that decides:

1. The body is not one JSON object, or `nonce` is not 64 lowercase hex. Client `malformed`. No row. The nonce is not consumed.
2. `origin` is not one of this gate's `served_origins` (groundinglab's instance: `https://groundinglab.io`), a field breaks the Submit grammar, or the window misses. Client `malformed`. The nonce is not consumed. When the object bytes hash to `object` and the nonce has no consumed row, the website outcome is `malformed`. Otherwise the outcome stays.
3. SHA-256 of the object bytes is not `object`. Client `bad_signature`. The nonce is not consumed. The website outcome stays.
4. A consumed row exists. The same `object` and the same signature return the original client class and the original website outcome. Policy does not run again. Any other object or signature is client `malformed`, and the stored outcome stays.
5. The object is shorter than 32 bytes. Website `too_short`. Client delivery OK. The nonce is consumed. No signature check runs.
6. The bytes yield no 32-byte public key, or the signature does not verify over `bip340_message_32`. Client `bad_signature`. The nonce is not consumed. Website `bad_signature`. A later consuming submit replaces that recorded outcome.
7. Kind `satoshi-hike-v1.35-object` after a valid signature. Website `earlier_version`. Client delivery OK. The nonce is consumed.
8. The bytes are not a v2.0 hike object. Website `rejected`. Client delivery OK. The nonce is consumed.
9. The bytes are a v2.0 hike object. Client `{"delivery":"ok"}` and no other field. The nonce is consumed when this step starts. Website `pending` until policy settles, then `accepted`, or `too_few_waypoints_anchored` when this build enforces that refuse-at-once, or `rejected`. This build does not enforce fewer than two of `W_1` through `W_16`, so it does not emit `too_few_waypoints_anchored`. A later policy refusal does not free the nonce.

Before that order, oversize, the wrong content type, and the transport rate are completed responses. The client class is delivery OK. The wrong content type writes no website outcome. Oversize and the rate write website `rejected` and do not consume the nonce only when the body is the JSON object, `nonce` is 64 lowercase hex, the nonce has no consumed row, and the object bytes hash to `object`. Otherwise the outcome stays. An unavailable door writes no website outcome. The transport figures are unchanged and are not on this wire.

The single-use row is keyed by `nonce` and stores `origin`, the expiry, `object_sha256`, the signature, the client class, and the website outcome. A consuming submit may also store the holder key. After a verifier record, it may store numeric bookends `N` as `legs` and `S` as `span_s`. Those measurements are on the result read only for `accepted`. It is written when the nonce is consumed. Two racing first submits do not both consume the nonce. That compare-and-set does not admit a provisional grant. A result row stays readable for 600 seconds after the outcome leaves `pending`. A `pending` row stays until it settles or the process drops it. A restart that drops the row forgets the challenge. After the row is gone, the result read is `pending` and creates no row. A late replay whose window has closed is `malformed`.

On this wire, no public response and no public decision-log `reason` contains a leg-rate fraction, a threshold, a verifier note, or a retry instruction. Which thresholds apply is policy, not wire: see [POLICY.md](../../policy/groundinglab-admission/POLICY.md). A thorough line for this wire uses the website token as `reasons` and, on a refusal, `reason` `rejected`. `verify_summary` on that line is `{}`.

`submission_sha256` for this wire is SHA-256 of `object_sha256 ‖ public_key ‖ bip340_message_32 ‖ signature` (160 bytes).

### Earlier version

On this wire, kind `satoshi-hike-v1.35-object` after a valid signature is website outcome `earlier_version`. The client body is `{"delivery":"ok"}`. The nonce is consumed. The sign-in response does not carry `reason`.

The same object on `POST /v1/verify` finishes the job with `status` `error`, `error.code` `earlier_version`, and this sentence in `error.message`: `This object is an earlier version. This gate checks Satoshi Hike v2.0 only.` The verify page shows `error.message`. The sentence is not "could not be read".

## Poll

The sign-in page reads `GET /v1/sign-in/result/<nonce>`. It does not read `/latest.json` for that outcome.

The public log is still:

```text
GET  /latest.json
GET  /log/{today}.jsonl
```

`today` is the `today` field of `/latest.json` (UTC date of the last line's `time_unix_s`). The day route is `application/x-ndjson`.

A provisional line already in the log, with no thorough line, is settled on restart. A new sign-in does not write that provisional line. An accept moves `last_seq` by one. A refuse moves it by one.

Opening the process rewrites `/latest.json` from the chain. A crash after the line is durable and before the hint is renamed is repaired on the next start. A chain with no line serves HTTP 404, including when a leftover `latest.json` was removed at open.

`/latest.json` before any line is HTTP 404:

```json
{"error":{"code":"not_found","message":"the decision log has no line yet"}}
```

After a durable append:

```json
{
  "feed": "pcoc-gate-feed-v2",
  "spec": "Satoshi Hike v2.0",
  "last_seq": 0,
  "last_event_hash": "<64 hex>",
  "today": "YYYY-MM-DD",
  "published_unix_s": 0
}
```

There is no `policy_current`. Profiles are gone. `verify_summary` on a locked sign-in thorough line is `{}`. A provisional line settled on restart stores the section 14 record from `hike::verify_record_json` under `profile` `production`, including `profile` and that record's trailing newline. The newline is inside the decision object. A reader takes JSON values. Splitting the file on every newline splits that resumed line. A fast line's `verify_summary` is `{}` because thorough has not run. The gate does not trim the record and does not remove a key.

A decision line carries `event` `gate.decision`, `seq`, `prev_event_hash`, `time_unix_s`, `bitcoin_tip_height`, `spec` `Satoshi Hike v2.0`, `gate_version`, `gate_commit`, `phase`, `decision`, `submission_sha256`, `public_key`, `object_sha256`, `reasons`, and `verify_summary`. A resumed provisional's thorough line also carries `fast_seq`. `reason` is present when the decision is `refuse`.

The feed id is `pcoc-gate-feed-v2`. The hash domain on each line is the frozen string `pcoc-gate-log-v1` (a trailing newline in the preimage). That domain is not the feed id and is not renamed when the feed moves.

A later line may name a lower `bitcoin_tip_height` than the line before it. The hash chain still holds. A reorg is recorded. It does not stop later appends.

Sign-in posts JSON. The transport limit is large enough for the base64 of an object at `max_object_bytes`, plus the JSON envelope. An object over `max_object_bytes` is HTTP 413. `POST /v1/verify` names that `oversize`. A six-field sign-in over that cap is delivery OK with no message.

## Health

`GET /v1/health` returns `version` (crate), `commit` (git SHA baked in at build), `sdk` (the Rust SDK version from Cargo.lock, for example `2.0.0-rc.2 (path)`, or `2.0.0-rc.2 (path, dirty)` when the SDK files had uncommitted changes at build time), `spec` (`Satoshi Hike v2.0`), `spec_kind` (`v2.0`), and `config` (the file: bind, index, log directory, limits, rate, leg-rate floor). No secrets.

## Verify job

`POST /v1/verify` does not check a signature. `GET /v1/verify/{job_id}` carries `record`, the bytes of `hike::verify_record_json` under `profile` `production`, including the trailing newline. The job splices those bytes. It does not build a second record. An earlier version does not get that record. It finishes as `error.code` `earlier_version`. A refusal still carries `record` when the object was measured. `reasons`, `reason`, and `error` follow it.

Thorough verify calls `hike::verify_with_bind_threads`. The thread count is `bind_threads` in the config (default 6). The process does not read `PCOC_BIND_THREADS`. The draw is the operating-system CSPRNG. Two verifies of one object can differ in `six_leg` and `six_leg_runs`. The other keys of one draw are the reference record.

Each index read is one shot, bounded by `index_timeout_secs`. HTTP 5xx, a timeout, a refused connection, and a DNS failure are not retried. The reference records a failed beacon read as a checkpoint mismatch. The gate also puts `the index did not answer for a stored beacon` on the thorough line when the reference already recorded a fact. An unread closing header by itself is `not read` in the record. How policy treats that pair is in [POLICY.md](../../policy/groundinglab-admission/POLICY.md). The verify slot is released. A restart settles an unsettled provisional the same way. It does not enter an unbounded wait. A second POST is a new check.

A reference fact that names a leg result checked again is stored as `a recomputed leg result differs`.

A locked sign-in of a v2.0 object is `{"delivery":"ok"}`, then a website outcome. A verify job returns the section 14 record. It does not admit.
