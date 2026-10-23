# Admission gate service

> **Do not deploy this tree over the live v2.0 service.** This branch is Satoshi Hike 2.2. The process refuses a live plane unless `allow_2_2_live = true`. That opt-in defaults to off and is not set anywhere in this repo.

The admission gate is a standalone service built on the Rust SDK (`sdk/rust`, version in `sdk/VERSION`). It implements the specification's admission service model (Part B, §15) and applies a configured **policy** to the verifier's measurements. It adds no Part A rule. Accept, refuse, and revoke are the operating relying party's decisions. They are not pass or fail.

Three layers stay separate:

- **Specification** defines measurements and the admission model. It sets no threshold values.
- **Policy** chooses values. The groundinglab policy is [policy/groundinglab-admission/](../../policy/groundinglab-admission/). It does not change a SPEC rule. This build's code still has its own default policy id. That id is listed with the code references and is not changed here. Choose your own.
- **Operations** is a deployed instance's own config. groundinglab's instance config is private and is not in this repository.

groundinglab runs one configured instance ("the groundinglab Admission Gate"). While it is the only public gate, its public page states its live policy.

Service status: [STATUS.md](STATUS.md). Evidence: [EVIDENCE.md](EVIDENCE.md).

The admission gate (`admission-service`, crate `pcoc-admission`) checks a Satoshi Hike object. It reads one config file. It does not run v1.35 verify and it does not load a v1.35 profile.

Install is [deploy/DEPLOY.md](deploy/DEPLOY.md) (systemd unit, `env.example`, `gate.toml`). Request and response bodies are [WIRE.md](WIRE.md).

The Sign-in door is `POST /v1/sign-in`. The body is the locked §15.1 six-field JSON object. The website polls `GET /v1/sign-in/result/<nonce>`. The client may disconnect after the POST. The result is read at `GET /v1/sign-in/result/<nonce>`. A signature miss is HTTP 400 `bad_signature` with no message. A grammar miss is HTTP 400 `malformed` with no message. Any other response is delivery of that POST. This process does not mint a session.

Any kind other than `v2.2-pcoc-object` is Rule A5. The fast phase's reason is `unknown_kind`. On sign-in the website token is `rejected`. On verify the job error code is `unknown_kind`. It is not "could not be read".

| Piece | Name |
|---|---|
| Service | admission gate (groundinglab runs one instance) |
| Spec | Satoshi Hike 2.2 (`spec_kind` `2.2`, kind `v2.2-pcoc-object`) |
| Cargo package | `pcoc-admission` |
| Directory | `services/admission-gate/` |
| Binary | `admission-service` |
| Config | one TOML file, `--config` or `GATE_CONFIG` |
| Crate version | `0.5.6` |

```text
POST /v1/sign-in           -> locked §15.1 body
GET  /v1/sign-in/result/{nonce}
GET  /latest.json          -> tip after a durable line, plus policy
GET  /log/{YYYY-MM-DD}.jsonl
GET  /log/{YYYY-MM-DD}/{seq}
GET  /v1/policy            -> the full policy table, plus source
POST /v1/verify            -> 202 { "job_id" }
GET  /v1/verify/{job_id}   -> status, and the section 14 record
GET  /v1/health            -> version, commit, sdk, spec, spec_kind, config
```

`POST /v1/admit` and the other legacy names are HTTP 404.

`POST /v1/verify` runs section 14 (`pcoc::verify`) against `index_url`. Any other kind is Rule A5 (`unknown_kind`) and does not read the index. A shape match whose `hike_span` is `not read` finishes `held`. One whose pair meets the floor finishes `done`. The sign-in fast phase is `pcoc::precheck` plus the header and plant reads in `reads`. The thorough phase calls `pcoc::thorough_draw`.

Policy values come from the policy file, not from the specification. The groundinglab policy is [gl-admit-v9.toml](../../policy/groundinglab-admission/gl-admit-v9.toml). A change of any value needs a new id. See [POLICY.md](../../policy/groundinglab-admission/POLICY.md). `born_height` is policy, not a SPEC lever. The current value is 972685. A stored or claimed height below it finishes the verify job as terminal `before_born_height`, with `born_height` and `lowest_height` on the output. That job is not `held`. Health and verify output report `born_height` next to `policy_id`. The verifier writes `measures`. The gate writes `policy_id` and `policy_checks`. A refusal carries one `reason_code` and a class. `bitcoin_tip_height` is null when unknown. Public health does not show `log_dir` or `config_path`. Health echoes the policy id.

The public log starts empty. The process creates `log_dir`. A chain that is not Satoshi Hike 2.2 is not opened and not served.

This directory is a standalone crate with its own `Cargo.lock`. It depends on the Rust SDK crate `pcoc` by path (`../../sdk/rust/pcoc`), so a checkout needs `services/admission-gate`, `sdk/rust`, `sdk/VERSION`, `sdk/SPEC_PIN`, and `policy/groundinglab-admission/gl-admit-v9.toml`.

`rust-toolchain.toml` in this directory pins Rust 1.99.0, the same pin as `sdk/rust`. On Debian or Ubuntu, install the build tools and rustup; rustup installs the pinned toolchain on the first `cargo` command:

```bash
sudo apt install -y build-essential git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
. "$HOME/.cargo/env"
```

Build and test from this directory:

```bash
cargo --version   # prints cargo 1.99.0, the pinned version
cargo build --locked --release
cargo test --locked
```

Test vectors are pinned copies under `tests/vectors/`; `tests/vectors.rs` checks them against `tests/vectors/SHA256SUMS`.

`GET /v1/health` on the bind (default `127.0.0.1:8787`) reports what is running, including `sdk`, the Rust SDK version the binary was built against. `admission-service --version` prints the same version, commit and SDK version. The SDK version ends in `(path, dirty)` when the SDK files had uncommitted changes at build time. It does not fetch the index.
