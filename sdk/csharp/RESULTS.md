# C# SDK 2.3.1 check results

Verifier-only. Producer is slice 2. Sign-in vectors are excluded.

`dotnet run --project check/Pcoc.Check -c Release` from `sdk/csharp` passes:

- anchor.json
- blockhash-display.json
- br1.json
- draw.json
- fold.json
- hashcash.json
- kat-small.json
- negatives.json (13 permanent negatives, each a non-match)
- nl4.json
- prod.json (`legs_le_hex` `88a04ac5beacb8a7`)
- rate.json
- seed.json
- terrain.json
- test-3leg.json
- unknown-kind.json
- wes.json

Excluded, printed by the runner: `sign-in.json`, `sign-in-origins.json`.

`manifest.json` is read as the Rule I5 pin. A one-byte mutation of that manifest fails the pin check.

Slot mutation: flipping one byte in the first, middle, and last word of the selected slot changes the step result each time. The step loads every byte of the slot and mixes every word. No SIMD kernel is used. The kernel name is `scalar`.

SHA-256 backend reported by quickstart: `openssl`.
