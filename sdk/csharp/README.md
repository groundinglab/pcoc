# PCOC C# SDK 2.3.2

Producer (rig and test plane; live plane in the follow-on Cut B ticket) and verifier. This tree produces a Satoshi Hike 2.3 object on the scripted rig and the test plane, verifies a hike object, and runs the public conformance pack including the Rule I4 producer tests. The C# SDK is 2.3.2 while `sdk/VERSION` (shared) stays 2.3.1.

It is not yet a reference SDK under Rule I1 while the sign-in vectors (`sign-in.json`, `sign-in-origins.json`) are excluded. A reference SDK passes the whole pack. The live header-source half of Rule I4 is Cut B.

`pcoc` is `verify`, `selftest`, `quickstart`, `candidates`, and `produce --rig`. `candidates` launches `pcoc-gen`, which is not a release artefact.

## Build

.NET 10.0.403, pinned in `global.json` with `rollForward` disable. No third-party NuGet packages.

```sh
export PATH="$HOME/.dotnet:$PATH"
export DOTNET_ROOT="$HOME/.dotnet"
export DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
export DOTNET_CLI_TELEMETRY_OPTOUT=1
dotnet run --project cli/Pcoc.Cli -c Release -- quickstart
dotnet run --project check/Pcoc.Check -c Release
sh check/negatives.sh
```

`dotnet publish cli/Pcoc.Cli -c Release` builds the verifier CLI. It does not build `gen/Pcoc.Gen`.

## Pack

From `sdk/csharp`, the check runner passes the 16 pack files named in the slice: `anchor.json`, `blockhash-display.json`, `br1.json`, `draw.json`, `fold.json`, `hashcash.json`, `kat-small.json`, `negatives.json`, `nl4.json`, `prod.json`, `rate.json`, `seed.json`, `terrain.json`, `test-3leg.json`, `unknown-kind.json`, and `wes.json`. It prints `sign-in.json` and `sign-in-origins.json` as excluded. `manifest.json` is the pin source.

The production-size leg in `prod.json` is `legs_le_hex` `88a04ac5beacb8a7`.

## SHA-256

`System.Security.Cryptography.SHA256` is the hash. On Linux that implementation is OpenSSL, which selects SHA-NI itself when the host has it. Quickstart reports the backend as `openssl`. It does not report a hard-coded `sha-ni`.

## Objects

`objects/O1-S1.cbor`, `O2-S2.cbor`, and `O3-PROD.cbor` are the pinned conformance objects. Their SHA-256 values are `800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246`, `6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0`, and `b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7`.
