# Admission gate service

The admission gate is a standalone service built on the Rust reference (`ref-rust-v2.0.5`). It implements the specification's admission service model (Part B, §15) and applies a configured **policy** to the verifier's measurements. It adds no Part A rule. Accept, refuse, and revoke are the operating relying party's decisions. They are not pass or fail.

Three layers stay separate:

- **Specification** defines measurements and the admission model. It sets no threshold values.
- **Policy** chooses values. [POLICY.md](POLICY.md) is the companion. groundinglab's recommended policy is `gl-admit-v4`, a recommendation, not authority. This build's code default is `gl-admit-v4`. Choose your own.
- **Operations** is a deployed instance's own config. groundinglab's instance config is private and is not in this repository.

groundinglab runs one configured instance ("the groundinglab Admission Gate"). While it is the only public gate, its public page states its live policy.

Service status: [STATUS.md](STATUS.md). Evidence: [EVIDENCE.md](EVIDENCE.md).

The admission gate (`admission-service`, crate `pcoc-admission`) checks a Satoshi Hike v2.0 object. It reads one config file. It does not run v1.35 verify and it does not load a v1.35 profile.

Install is [deploy/DEPLOY.md](deploy/DEPLOY.md) (systemd unit, `env.example`, `gate.toml`). Request and response bodies are [WIRE.md](WIRE.md).

The Sign-in door is `POST /v1/sign-in`. The body is the locked §15.1 six-field JSON object. The website polls `GET /v1/sign-in/result/<nonce>`. The client may disconnect after the POST. The result is read at `GET /v1/sign-in/result/<nonce>`. A signature miss is HTTP 400 `bad_signature` with no message. A grammar miss is HTTP 400 `malformed` with no message. Any other response is delivery of that POST. This process does not mint a session.

An earlier-kind object is a measurement (`version` `earlier version`). The Gate's reason `earlier_version` is policy (Rule B9). A v1.35 object on sign-in is website outcome `earlier_version` after a valid signature. On verify, the job error message is `This object is an earlier version. This gate checks Satoshi Hike v2.0 only.` It is not "could not be read".

| Piece | Name |
|---|---|
| Service | admission gate (groundinglab runs one instance) |
| Spec | Satoshi Hike v2.0 (`spec_kind` `v2.0`) |
| Cargo package | `pcoc-admission` |
| Directory | `services/admission-gate/` |
| Binary | `admission-service` |
| Config | one TOML file, `--config` or `GATE_CONFIG` |
| Crate version | `0.4.1` |

```text
POST /v1/sign-in           -> locked §15.1 body
GET  /v1/sign-in/result/{nonce}
GET  /latest.json          -> tip after a durable line, plus policy
GET  /log/{YYYY-MM-DD}.jsonl
GET  /log/{YYYY-MM-DD}/{seq}
GET  /v1/policy            -> the full policy table, plus source
POST /v1/verify            -> 202 { "job_id" }
GET  /v1/verify/{job_id}   -> status, and the section 14 record when the object is v2.0
GET  /v1/health            -> version, commit, spec, spec_kind, config
```

`POST /v1/admit` and the other legacy names are HTTP 404.

The reference verifier is `linked_measure` in `src/verifier.rs`. It calls `hike::verify_with_bind_threads` from the v2.0 reference crate. Index reads are one shot. An earlier kind is that crate's Rule A5. Policy then judges the measurements.

Policy values come from the policy file, not from the specification. This build reads `policy/gl-admit-v4.toml` or an inline `policy_table`. A change of any value needs a new id. The code default is `gl-admit-v4` (one leg per 5 seconds, 6 anchored waypoints among `W_1` … `W_16`). See [POLICY.md](POLICY.md). The verifier writes `measures`. The gate writes `policy_id` and `policy_checks`. A refusal carries one `reason_code` and a class. `bitcoin_tip_height` is null when unknown. Public health does not show `log_dir` or `config_path`. Health echoes the policy id.

The public log starts empty. The process creates `log_dir`. A chain that is not Satoshi Hike v2.0 is not opened and not served.

This directory is a standalone crate with its own `Cargo.lock`. It depends on the v2.0 reference crate `hike` from this repository at tag `ref-rust-v2.0.5`. Build and test from this directory:

```bash
cargo build --locked --release
cargo test --locked
```

Test vectors are pinned copies under `tests/vectors/`; `tests/vectors.rs` checks them against `tests/vectors/SHA256SUMS`.

`GET /v1/health` on the bind (default `127.0.0.1:8787`) reports what is running. It does not fetch the index.
