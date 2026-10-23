# SPEC 2.3 re-check of round 2

Round 2 is frozen. This folder is a new run, on RS3 only, of the short checks. It does not replace round 2.

- **SPEC:** Satoshi Hike 2.3, tag `spec-2.3.0`, commit `7189a6df9deed0b7dca47f06ba332091d8fa2425`. `spec/satoshi-hike-v2.3/SPEC.md` sha256 `d564561381a2c6c8f83c89c412f9a2e0c835f775146810f2c9828676c68433c6`.
- **Pack:** `sdk/SPEC_PIN` vector-sha256 `91ded57e5c05b7cb4cd74d313a51d83d2373fa86dc625df41faed9586e9032af`, the SHA-256 of `spec/conformance/pack/manifest.json`. The pack id inside that manifest stays `pack-2.2.0`.
- **SDKs:** `sdk-rust-v2.3.0`, `sdk-go-v2.3.0`, `sdk-java-v2.3.0`, and `sdk-c-v2.3.0`, all at `9c0c408469e3a0c359e15ebcb4ed7e19cbc38d5c`. The Go module tag `sdk/go/v2.3.0` names that same commit.
- **Date:** 23 Oct 2026.
- **Machine:** RS3.
- **Not run:** packed ladders, bench phases, and production of new hike objects.

The round-2 builds are the committed sources. Fixture files were regenerated with `kit/run-build.sh` phases `build`, `test`, and `cands`. For B-sol the candidate command was `generate-candidates`. The kit phase `cands` calls `generate-all`, which also writes O3, and this run does not produce a new hike.

Objects in the cross-verify grid are the committed round-2 O1–O3. Each file was checked against that build's `SHA256SUMS` before a verifier ran. Details are in [results/](results/).
