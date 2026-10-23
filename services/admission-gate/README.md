# Admission gate service

> This checkout is pinned to `spec-2.3.0` (Satoshi Hike 2.3). Live gate 0.5.8 uses that pin. The process refuses a live plane unless `allow_2_2_live = true`. That opt-in defaults to off and is not set anywhere in this repo.

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
| Spec | Health `spec` / `spec_kind` follow `sdk/SPEC_PIN`: Satoshi Hike 2.2 / `2.2` on a spec-2.2.x pin, Satoshi Hike 2.3 / `2.3` on a spec-2.3.x pin. This checkout's pin is `spec-2.3.0`, so health reports Satoshi Hike 2.3 / `2.3`. Live gate 0.5.8 is that pin. Object kind stays `v2.2-pcoc-object`. Policy `spec` `2.2` loads on either pin; `2.3` loads on a spec-2.3.x pin. |
| Cargo package | `pcoc-admission` |
| Directory | `services/admission-gate/` |
| Binary | `admission-service` |
| Config | one TOML file, `--config` or `GATE_CONFIG` |
| Crate version | `0.5.8` |

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

Policy values come from the policy file, not from the specification. The gate does not fill a built-in number for a missing lever. A lever whose SPEC default is unset (`verify_cap_per_object`, `verify_cap_per_door_session`, `fake_leg_ignore_seconds`) stays unset when the key is absent. Every other §15.4 lever must be present; a missing key refuses the file and the error names it. The file must also set `policy_id`, `draw_commitment`, and `born_height`. A missing key refuses the file and the error names it. There is no fallback id, commitment sentence, or born height of 0. `expected_network` may be absent, which means no index-network constraint. The groundinglab policy is [gl-admit-v10.toml](../../policy/groundinglab-admission/gl-admit-v10.toml). `gl-admit-v9.toml` is `superseded` and does not set `born_height`, so the gate refuses that file. The file was not edited. A change of any value needs a new id. See [POLICY.md](../../policy/groundinglab-admission/POLICY.md). `born_height` is policy, not a SPEC lever. The current value is 972685. A stored or claimed height below it finishes the verify job as terminal `before_born_height`, with `born_height` and `lowest_height` on the output. That job is not `held`. The index still serves the median-time lookback of `MTP_LOOKBACK` headers below `born_height` (10; `MTP_WINDOW` is 11). Those headers are not production heights. Health and verify output report `born_height` next to `policy_id`. The verifier writes `measures`. The gate writes `policy_id` and `policy_checks`. A refusal carries one `reason_code` and a class. `bitcoin_tip_height` is null when unknown. Public health does not show `log_dir` or `config_path`. Health echoes the policy id.

The public log starts empty. The process creates `log_dir`. A chain that is not Satoshi Hike 2.2 is not opened and not served.

This directory is a standalone crate with its own `Cargo.lock`. It depends on the Rust SDK crate `pcoc` by path (`../../sdk/rust/pcoc`), so a checkout needs `services/admission-gate`, `sdk/rust`, `sdk/VERSION`, `sdk/SPEC_PIN`, and `policy/groundinglab-admission/gl-admit-v10.toml`.

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

`GET /v1/health` on the bind (default `127.0.0.1:8787`) reports what is running, including `sdk`, the Rust SDK version the binary was built against. `spec` and `spec_kind` follow the SDK pin (`Satoshi Hike 2.3` / `2.3` while `sdk/SPEC_PIN` is `spec-2.3.0`). The JSON keys are unchanged. Decision-log lines stay `spec` `Satoshi Hike 2.2`. `admission-service --version` prints the same version, commit and SDK version. The SDK version ends in `(path, dirty)` when the SDK files had uncommitted changes at build time. It does not fetch the index.
