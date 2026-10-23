# Fixture outputs compared byte for byte

RS3, 23 Oct 2026. Each cell is the git blob id prefix of the candidate file regenerated on this run. `=` means identical to build A on this run.

The frozen round-2 ref3 copies are the comparison base. Every regenerated file matches that build's own round-2 file: 14 of 14 for each of the six builds.

| file | A-sonnet | B-sol | C-grok | D-gemini | E-kimi | F-glm |
|---|---|---|---|---|---|---|
| anchor.txt | 1474bdcddf9b | = | = | = | = | = |
| draw.txt | 287a1f1e0054 | = | = | = | = | = |
| fold.txt | a08413f82033 | = | = | = | = | = |
| hashcash.txt | c37b6f6cfeed | = | = | = | = | 69c647b8fd67 |
| kat-small.txt | a0d710326595 | = | = | = | = | = |
| negatives.txt | bd63be129750 | = | = | = | = | = |
| nl4.txt | 961d2699d218 | = | = | = | = | = |
| prod.txt | 0e3041da9cbc | = | = | = | = | = |
| rate.txt | cb4ac9bfc97d | = | = | = | = | = |
| seed.txt | ab0878b6739b | = | = | = | = | = |
| terrain.txt | e771bf10c53a | = | = | = | = | = |
| test-3leg.txt | 7e21042b5320 | = | = | = | = | = |
| unknown-kind.txt | 022d5d97a624 | = | = | = | = | = |
| wes.txt | 7bcc7a99fed8 | = | = | = | = | = |

`hashcash.txt` from F-glm is the same single-file exception round 2 recorded: blob `69c647b8fd67` here and in the frozen round-2 ref3 copy, against `c37b6f6cfeed` for the other five builds. It hashes the fee header once where CASES.md asks for SHA-256(SHA-256(fee header)). That slip is inside the test vector. It does not change an object or a verify record.
