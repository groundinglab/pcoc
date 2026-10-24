# Apps AI handoff

Drop-in for the groundinglab website (`ops` `www/site/`). This repository does not deploy the site. Infra copies these files onto the www tree. Do not publish this note.

Snapshot base: live https://groundinglab.io/ on 24 Oct 2026. Re-diff `apps/index.html`, `integrate/index.html`, `sitemap.xml`, and `sitemap/index.html` against ops before copying, in case ops has moved on from what is live.

## Usability

`/apps/` is a platform tab list (Linux, Android, Windows) plus download chips. It is not a card grid. The page's job is still "pick your platform and install". Leading with a coding-agent handoff would bury the downloads and overclaim what Apps is for, so AI stays fourth.

It is not a fourth tab. A tab in that list means "the device I am installing on", and Windows is already a coming-soon chip in that pattern. An AI tab would read as another binary. The handoff is a text card under the three pills, outside the tab list, labelled "Not a download", linking to `/apps/ai/`. It stays visible on every platform tab. The same destination is linked from "Build it into your own app" and from For services → Going further.

`/apps/ai/` matches the Apps URL family. `/integrate/` is the service pitch, not a page you hand to an agent. The agent page stays under Apps, where the card is.

## URL map

| Role | URL or path |
|---|---|
| Handoff page | https://groundinglab.io/apps/ai/ |
| Specification, live single file (tag `spec-2.3.0`, commit `7189a6d`) | https://groundinglab.io/technical/spec/ |
| Six-document reading on main | `spec/GUIDE.md`, `spec/01`–`spec/06` |
| SDKs | https://groundinglab.io/technical/sdk/ |
| Producer, verifier, gate, sign-in wire | `/technical/integrate-a-producer/`, `/technical/verifier-and-policy/`, `/technical/integrate-an-admission-gate/`, `/technical/sign-in-wire/` |
| Conformance pack and pin | `spec/conformance/pack`, `sdk/SPEC_PIN` (`91ded57e…`) |
| Examples | `apps/desktop`, `services/admission-gate`, https://github.com/groundinglab/primal-pcoc |
| Policy (not the specification) | `policy/README.md`, `policy/groundinglab-admission/gl-admit-v12.toml` (in-repo successor, not necessarily the live host) |

SDK git tags checked in this clone: Rust `sdk-rust-v2.3.1`, Go `sdk/go/v2.3.1`, C `sdk-c-v2.3.1`, Java `sdk-java-v2.3.1`. The live SDK page's GitHub release list still shows C and Java at 2.3.0. The prompt prefers the 2.3.1 tag when the clone has it.

The live verifier-and-policy page still shows an older worked example (`gl-admit-v10`, including `submission_deadline_blocks`). The prompt tells the agent to follow `policy/README.md` when they disagree, and not to add that deadline or enable sticky refuse.
