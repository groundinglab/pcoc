# Deploy admission-claims

One binary, one config file, one SQLite file, one issuer key. Stage 1 answers `POST /badge` and `GET /health`. A failed smoke stays on the new binary. The fix is a new build and a new restart.

This page does not deploy the Worker, does not create Cloudflare resources, and does not change the gate. The gate's loopback `GET /log/{day}/{seq}` is a separate change.

## Build

Build from a tag checkout so `commit` is the SHA (`COMMIT` overrides it). Build on a glibc no newer than the target host:

```bash
cd services/admission-claims
COMMIT="$(git rev-parse HEAD)" cargo build --locked --release
```

`/health` then reports that commit. The binary is `services/admission-claims/target/release/admission-claims`. Record its sha256. Install that one file on the host as `/usr/local/bin/admission-claims-<ver>-<sha>`, mode `0755`, owner root, and point the unit at it or link it to `/usr/local/bin/admission-claims`.

## Layout

```bash
sudo useradd --system --home-dir /var/lib/admission-claims --create-home --shell /usr/sbin/nologin claims
sudo mkdir -p /etc/admission-claims /var/lib/admission-claims
sudo chown claims:claims /var/lib/admission-claims
sudo chmod 0700 /var/lib/admission-claims
```

Config `/etc/admission-claims/claims.toml`, mode `0640`, `root:claims`. Copy [claims.toml.example](claims.toml.example) and keep the pinned `issuer_pubkey_hex`. The process reads that file only.

The issuer key is already on the host at `/var/lib/admission-claims/issuer.key`: 64 hex characters, mode `0600`, owner `claims`. Do not print it, do not copy it into the unit, and do not commit it. The process refuses to start when group or other can read the file, or when the derived public key is not the pin. Startup may log the public hex once.

Copy [blocked-names.txt](blocked-names.txt) to `/etc/admission-claims/blocked-names.txt` (readable by the claims user). Edit it in place; the next name request uses it. Blocked attempts: `journalctl -u admission-claims | grep "blocked name"`.

R2 token files, when they exist, are mode `0600`, owner `claims`. `r2.token` is two lines: access key id, then secret, for bucket `groundinglab-claims` only. `backup.token` is the same shape for `groundinglab-claims-backup` (object write on that bucket only). `backup.age.pub` is one `age1…` recipient. The age identity is not on the box.

An empty `r2_account_id` is directory mode. The process writes artifacts under `artifact_dir` and does not PUT. Fill `r2_account_id` and the token file when `groundinglab-claims` exists, then restart.

## Unit

[admission-claims.service](admission-claims.service) runs as `claims`, loopback bind, `ProtectSystem=strict`, `ProtectHome=true`, `NoNewPrivileges=true`, `PrivateTmp=true`. `ReadWritePaths` is `/var/lib/admission-claims`. There is no `EnvironmentFile`. The journal is capped at 200 lines per 30 seconds.

```bash
sudo cp services/admission-claims/deploy/admission-claims.service /etc/systemd/system/admission-claims.service
sudo systemctl daemon-reload
sudo systemctl enable --now admission-claims
```

The process holds an exclusive flock on the SQLite file. A second writer exits at start.

Before the first start that will issue an award, run the backup command below. An integrity failure stops the install. Fix the database, then install again.

Smoke `http://127.0.0.1:8793/health`, then `https://claims.groundinglab.io/health` after the tunnel ingress exists. `db` is `ok` and `key` is `loaded`. A failed smoke stays on this binary.

cloudflared ingress for `claims.groundinglab.io` to `http://127.0.0.1:8793` is a separate step. No Access application. The claims user reads the gate only at `127.0.0.1:8787`.

## strfry

The relay write policy (accept kinds 30009, 8 and 1742 only from the issuer pubkey) is part of the relay deployment, not this service.

## Backup

[admission-claims-backup.timer](admission-claims-backup.timer) runs [admission-claims-backup.service](admission-claims-backup.service) as `claims` every hour:

```bash
admission-claims backup --config /etc/admission-claims/claims.toml
```

The command copies the database, runs `PRAGMA integrity_check`, and age-encrypts the snapshot and `issuer.key` to `backup_age_recipient_file`. Plaintext snapshots are removed after encryption. The box keeps `backup_keep` ciphertext pairs (default 48). The ciphertext is PUT to the private bucket `groundinglab-claims-backup`. The age identity is not on the host. Keep an offline copy of the issuer key.

Every deploy runs this backup first and does not restart when the check fails.

## Worker

`services/cf-claims-worker` is the GET-only Worker for `/claims/*`, `/badge/*`, and `/.well-known/nostr.json`. CORS is `*`. It does not redirect. `wrangler.jsonc` has placeholders. Do not deploy it from this change. Deploy is its own GO, with a short-lived token that is not the Pages token and is not stored on the host.

## What a badge publishes

The issuer signs one kind 30009 definition (`d` = `groundinglab-admitted`) and one kind 8 award per npub. The award tags are `a`, `p`, `policy`, and `r`. The `r` tag is `https://admission.groundinglab.io/log/<accept day>.jsonl#<accept seq>`. Artifacts are `claims/<event-id>.json`, `claims/index.json`, `badge/<npub>.html`, and `.well-known/nostr.json` (`{"names":{}}` until stage 2).
