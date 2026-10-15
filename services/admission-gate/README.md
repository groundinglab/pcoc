# groundinglab Admission Gate

groundinglab's Admission Gate is a standalone service built on the Rust reference (`ref-rust-v2.0.4`). It applies groundinglab's admission policy (specification Part B) to the verifier's measurements. It adds no Part A rule. Accept, refuse, and revoke are this relying party's decisions. They are not pass or fail.

The groundinglab Admission Gate (`admission-service`, crate `pcoc-admission`) checks a Satoshi Hike v2.0 object. It reads one config file. It does not run v1.35 verify and it does not load a v1.35 profile.

Install is [deploy/DEPLOY.md](deploy/DEPLOY.md) (systemd unit, `env.example`, `gate.toml`). Request and response bodies are [WIRE.md](WIRE.md).

The Sign-in door is `POST /v1/sign-in`. The body is the locked §15.1 six-field JSON object. The website polls `GET /v1/sign-in/result/<nonce>`. The client may disconnect after the POST. The result is read at `GET /v1/sign-in/result/<nonce>`. A signature miss is HTTP 400 `bad_signature` with no message. A grammar miss is HTTP 400 `malformed` with no message. Any other response is delivery of that POST. This process does not mint a session.

An earlier-kind object is a measurement (`version` `earlier version`). The Gate's reason `earlier_version` is policy (Rule B9). A v1.35 object on sign-in is website outcome `earlier_version` after a valid signature. On verify, the job error message is `This object is an earlier version. This gate checks Satoshi Hike v2.0 only.` It is not "could not be read".

| Piece | Name |
|---|---|
| Service | groundinglab Admission Gate |
| Spec | Satoshi Hike v2.0 (`spec_kind` `v2.0`) |
| Cargo package | `pcoc-admission` |
| Directory | `services/admission-gate/` |
| Binary | `admission-service` |
| Config | one TOML file, `--config` or `GATE_CONFIG` |
| Crate version | `0.3.3` |

```text
POST /v1/sign-in           -> locked §15.1 body
GET  /v1/sign-in/result/{nonce}
GET  /latest.json          -> tip after a durable line
GET  /log/{YYYY-MM-DD}.jsonl
POST /v1/verify            -> 202 { "job_id" }
GET  /v1/verify/{job_id}   -> status, and the section 14 record when the object is v2.0
GET  /v1/health            -> version, commit, spec, spec_kind, config
```

`POST /v1/admit` and the other legacy names are HTTP 404.

The reference verifier is `linked_measure` in `src/verifier.rs`. It calls `hike::verify_with_bind_threads` from the v2.0 reference crate. Index reads are one shot. An earlier kind is that crate's Rule A5. Policy then judges the measurements.

Policy is not a profile. `leg_rate_floor` in the config is `strict` (7/20), `default` (1/4), or `lenient` (1/8). The floor compares the bookend pair `(N, S)` in the section 14 record. There is no friendly list and no held state. `verify_summary` is that record, including `profile` and the trailing newline.

The public log starts empty. The process creates `log_dir`. A chain that is not Satoshi Hike v2.0 is not opened and not served.

This directory is a standalone crate with its own `Cargo.lock`. It depends on the v2.0 reference crate `hike` from this repository at tag `ref-rust-v2.0.4`. Build and test from this directory:

```bash
cargo build --locked --release
cargo test --locked
```

Test vectors are pinned copies under `tests/vectors/`; `tests/vectors.rs` checks them against `tests/vectors/SHA256SUMS`.

`GET /v1/health` on the bind (default `127.0.0.1:8787`) reports what is running. It does not fetch the index.
