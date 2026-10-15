# Deployable components

Everything in this repo that is deployed or published. Each one is released by a tag `<component>-v<semver>` cut on `main`. groundinglab ops pins these tags in dated deployment snapshots. Anything not listed here is library, spec or documentation, and is not deployed on its own.

| Component | Path | Tag prefix | Artifact | Runs as |
|---|---|---|---|---|
| Admission Gate (admission + verify) | `services/admission-gate` | `gate-v` | `admission-service` binary | systemd service |
| Claims service (badge and name issuer) | `services/admission-claims` | `claims-v` | `admission-claims` binary | systemd service |
| Claims Worker (`/claims/*`, `/badge/*`, `/.well-known/nostr.json*`) | `services/cf-claims-worker` | `claims-worker-v` | Cloudflare Worker `groundinglab-claims` | `wrangler deploy` |
| Bari (producer API) | `services/bari` | `bari-v` | `bari` binary | systemd service |
| Index (read-only cache) | `services/index` | `index-v` | `pcoc-index` binary | systemd service |
| Desktop app | `apps/desktop` | `desktop-v` | `pcoc.flatpak` (`apps/desktop/flatpak/build-bundle.sh`) | download |
| Android app | built from groundinglab/primal-pcoc (`apps/android` holds docs and verification only) | `android-v` (in primal-pcoc) | APK | download |

Reference implementations (`reference/`, tags `ref-rust-v`, `ref-java-v`, `reference/go/v`) and the spec (`spec-v`) are versioned libraries and documents, not deployables.

The website is deployed from groundinglab ops (`www/`), not from this repo.
