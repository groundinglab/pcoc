# Satoshi Hike SPEC 2.2 (erratum 1): TypeScript / Node.js implementation

An independent implementation of the Satoshi Hike construction, object encoding, §14 verifier and §16.1 conformance-small profile, built only from the pinned public SPEC, POLICY and CASES files. It is one entry in a cross-language interoperability test.

- `impl/` holds the code, the generated `candidates/` (every CASES case), the `objects/` (O1 on S1, O2 on S2, O3 on the production profile) and the tests.
- `impl/RESULTS.md` holds the build report: possible SPEC flaw (none found), timings, optimisations, SPEC questions and unfinished items.
- `CASES.md` is the pinned fixtures file (commit 8c40cf8).

## Run

Requires Node.js 22 or newer.

```sh
cd impl
npm install
npm run build
node dist/gen-candidates.js candidates        # deterministic; rewrites candidates/
node dist/cli.js produce --all objects        # writes O1, O2, O3 into objects/
node dist/cli.js verify objects/O3-PROD.cbor --index-fixture   # prints the §14 record (about 11 s)
npm test                                      # 38 tests, including a re-run of the candidate generator
node dist/bench.js timings                    # timing table used in RESULTS.md
```

`verify` for the conformance-small objects needs the profile flags; see `impl/RESULTS.md` for the exact commands.
