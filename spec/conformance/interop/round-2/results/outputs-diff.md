# Fixture outputs compared byte for byte

Each cell is the git blob id prefix of the candidate file the build generated on that machine. `=` means identical to build A on the same machine.

## Reference system 1

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

## Reference system 2

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

## Reference system 3

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

Only `hashcash.txt` from F-glm differs: it hashes the fee header once where CASES.md asks for SHA-256(SHA-256(fee header)). That is a test-vector slip in that build; it touches no object and no verify record.
