# Deploy the admission gate

> **Do not deploy this tree over the live v2.0 service.** This branch is Satoshi Hike 2.2. The process refuses a live plane unless `allow_2_2_live = true`. That opt-in defaults to off and is not set anywhere in this repo.

One binary, one config file, a log directory the process creates. The object kind is `v2.2-pcoc-object`. Policy `spec` `2.2` loads on a spec-2.2.x pin and on a spec-2.3.x pin. Policy `spec` `2.3` loads on a spec-2.3.x pin and is refused on a spec-2.2.x pin. This page is the whole install. Do not point `log_dir` at an earlier decision log. A decision-log line is still stamped `Satoshi Hike 2.2`.

## Build

The gate builds the Rust SDK from this repository by path (`sdk/rust/pcoc`), so the checkout must contain `sdk/rust`, `sdk/VERSION`, `sdk/SPEC_PIN`, and `policy/groundinglab-admission/gl-admit-v10.toml` as well as `services/admission-gate`. A sparse checkout needs at least:

```bash
git sparse-checkout set services/admission-gate sdk policy
```

Build from a clean checkout of a tag (`gate-v…` or `sdk-rust-v2.2.x`), from `services/admission-gate/`, with git present so `commit` is the SHA (`COMMIT` overrides it):

```bash
cargo build --locked --release
target/release/admission-service --version   # admission-service <version> commit=<sha> sdk=<sdk version> (path)
```

The binary is `target/release/admission-service`. Do not deploy a binary whose `--version` says `(path, dirty)`: the SDK files it was built from had uncommitted changes. Release binaries contain the absolute path of the checkout, so builds from different directories differ in bytes. To keep the host path out of the binary, build with `RUSTFLAGS="--remap-path-prefix=$(git rev-parse --show-toplevel)=/pcoc"` (not yet part of the release process).

## Layout

Create the system user, the config directory, and the working directory. Copy the binary to the path the unit runs. The paths below match [pcoc-admission.service](pcoc-admission.service). Replace them together if you use others.

```bash
sudo useradd --system --home-dir /var/lib/pcoc-admission --shell /usr/sbin/nologin pcoc
sudo mkdir -p /etc/pcoc-admission /var/lib/pcoc-admission
sudo cp target/release/admission-service /usr/local/bin/admission-service
sudo cp deploy/gate.toml /etc/pcoc-admission/gate.toml
sudo install -m 0640 deploy/env.example /etc/pcoc-admission/env
sudo chown pcoc:pcoc /var/lib/pcoc-admission
```

Edit `/etc/pcoc-admission/gate.toml`. The gate binds `127.0.0.1:8787`. **`served_origins` is required and has no default**: it lists the exact site origins whose signed sign-ins this gate serves (groundinglab's file sets `served_origins = ["https://groundinglab.io"]`). A file without it does not start. It is separate from the CORS lists. The code default for `allowed_origins` is empty (loopback only). **A live groundinglab deploy needs `allowed_origins = ["https://groundinglab.io", "https://www.groundinglab.io"]`** in this file (the shipped `gate.toml` sets it; see also [env.example](env.example)); without it the website's sign-in requests are refused by CORS. To serve it publicly, put a reverse proxy of your choice in front of the loopback bind. Every key is optional. That file lists each key, its default, and what it does. The process reads that file only. The path is `--config` or `GATE_CONFIG`. It does not read `PCOC_BIND_THREADS` or any `GL_VERIFY_*` variable.

`log_dir` is the public log. The process creates it. A relative `log_dir` is under `WorkingDirectory`. A chain that is not Satoshi Hike 2.2 is refused at startup and is not served. Leave an earlier log where it is.

## Unit

[pcoc-admission.service](pcoc-admission.service) sets `User`, `StateDirectory`, `WorkingDirectory`, `GATE_CONFIG`, an optional `EnvironmentFile` ([env.example](env.example)), and `ExecStart`. `StateDirectory=pcoc-admission` creates `/var/lib/pcoc-admission` on a system unit. Create that directory before enable if you drop `StateDirectory`.

```bash
sudo cp deploy/pcoc-admission.service /etc/systemd/system/pcoc-admission.service
sudo systemctl daemon-reload
sudo systemctl enable --now pcoc-admission
```

A user unit drops `User=` and `Group=`, sets `WantedBy=default.target`, and uses paths the user can write. Pick a unit name that is not a gate already running. Create `WorkingDirectory` yourself, copy the binary and [gate.toml](gate.toml) under a directory you own, set `bind` to a free localhost port, and set `log_dir` to an empty directory.

`PrivateTmp=true` on the system unit hides `/tmp` and `/var/tmp` from the process. A binary or `WorkingDirectory` under either of those mounts does not start (`status=203/EXEC`). The system paths above are outside those mounts, so the system unit keeps the line. A user unit whose binary and `WorkingDirectory` are also outside `/tmp` and `/var/tmp` can keep `PrivateTmp=true`. A proof unit that puts the binary under `/tmp` omits that line.

## Health

```bash
curl -sS http://127.0.0.1:8787/v1/health
```

`version` is the crate version, `commit` is the build SHA, and `config` repeats the file with no secrets. `spec` and `spec_kind` follow the SDK pin. This checkout's pin is `spec-2.2.0`, so they are `Satoshi Hike 2.2` and `2.2`. On a spec-2.3.x pin they are `Satoshi Hike 2.3` and `2.3`. The JSON keys do not change.

## Upgrade

Build the new binary, replace the file in `ExecStart`, then `sudo systemctl restart pcoc-admission`. Keep the same `log_dir`. A restart continues that decision-log chain and rebuilds `/latest.json` from it. It does not import a v1.35 chain.

## Routes

`POST /v1/sign-in`, `POST /v1/verify`, `GET /v1/verify/{job_id}`, `GET /v1/health`, `GET /v1/policy`, `GET /latest.json`, `GET /log/{YYYY-MM-DD}.jsonl`, `GET /log/{YYYY-MM-DD}/{seq}`. Anything else, including `POST /v1/admit`, is HTTP 404. Bodies are in [WIRE.md](../WIRE.md).

The groundinglab policy file is `policy/groundinglab-admission/gl-admit-v10.toml`. `gl-admit-v9.toml` is `superseded`. It does not set `born_height`, so this gate refuses that file. A file that omits `policy_id`, `draw_commitment`, or `born_height` does not start; the error names the key. The id is bound to a hash of the full table. Changing any value requires a new id. The process writes that binding to `{log_dir}/policy-bind.json` and exits before it listens when the same id is paired with a different table.
