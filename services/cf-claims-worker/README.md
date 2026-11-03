# cf-claims-worker

A Cloudflare Worker that reads published claims from an R2 bucket. It is GET-only (`GET`, `HEAD`, `OPTIONS`), sends no redirects, and allows any origin (CORS `*`).

| Path | R2 key |
|---|---|
| `/.well-known/nostr.json?name=<name>` | `.well-known/nostr.json`, answering only `<name>` (no-store) |
| `/claims/index.json` | `claims/index.json` |
| `/claims/<64 hex>.json` | `claims/<hex>.json` (immutable) |
| `/badge/<npub>` | `badge/<npub>.html` (immutable) |

It does not issue claims. [admission-claims](../admission-claims/) writes them. The Worker only serves what is in the bucket.

`wrangler.jsonc` is groundinglab's deploy file for its own zone, with placeholders (`ACCOUNT_ID`). To run it under your own name, copy the file and change the name, routes, zone and bucket. Tests: `node --test test/`.
